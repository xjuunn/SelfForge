use super::agent::{
    AgentDefinition, AgentError, AgentPlan, AgentRegistry, AgentRunReference, AgentSession,
    AgentSessionError, AgentSessionStatus, AgentSessionStore, AgentSessionSummary, AgentStepStatus,
};
use super::ai_provider::{
    AiConfigError, AiConfigReport, AiExecutionError, AiExecutionReport, AiProviderRegistry,
    AiRequestError, AiRequestSpec,
};
use super::error_archive::{ArchivedErrorEntry, ErrorArchive, ErrorArchiveError, ErrorListQuery};
use crate::{
    CycleReport, CycleResult, EvolutionError, ExecutionError, ExecutionReport, ForgeError,
    ForgeState, StateError, Supervisor, next_version_after,
};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

const PREFLIGHT_OPEN_ERROR_LIMIT: usize = 10;

#[derive(Debug, Clone)]
pub struct SelfForgeApp {
    root: PathBuf,
    supervisor: Supervisor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinimalLoopOutcome {
    Prepared,
    PromotedAndPrepared,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalLoopReport {
    pub outcome: MinimalLoopOutcome,
    pub starting_version: String,
    pub stable_version: String,
    pub candidate_version: Option<String>,
    pub next_expected_version: Option<String>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub current_version: String,
    pub current_workspace: String,
    pub status: String,
    pub candidate_version: Option<String>,
    pub candidate_workspace: Option<String>,
    pub checked_paths: Vec<PathBuf>,
    pub candidate_checked_paths: Vec<PathBuf>,
    pub open_errors: Vec<ArchivedErrorEntry>,
    pub can_advance: bool,
}

#[derive(Debug, Clone)]
pub struct AgentEvolutionReport {
    pub session: AgentSession,
    pub preflight: PreflightReport,
    pub minimal_loop: MinimalLoopReport,
}

#[derive(Debug, Clone)]
pub struct AgentSingleEvolutionReport {
    pub session: AgentSession,
    pub preflight: PreflightReport,
    pub prepared_candidate_version: Option<String>,
    pub cycle: CycleReport,
}

#[derive(Debug, Clone)]
pub struct AgentRunReport {
    pub session: AgentSession,
    pub execution: ExecutionReport,
    pub run_id: String,
    pub step_order: usize,
}

#[derive(Debug)]
pub enum MinimalLoopError {
    State(StateError),
    Forge(ForgeError),
    Evolution(EvolutionError),
    ErrorArchive(ErrorArchiveError),
    OpenErrors { version: String, run_id: String },
}

#[derive(Debug)]
pub enum AgentEvolutionError {
    Session(AgentSessionError),
    Setup(MinimalLoopError),
    MinimalLoop {
        session: Box<AgentSession>,
        source: MinimalLoopError,
    },
    Blocked {
        session: Box<AgentSession>,
        open_errors: Vec<ArchivedErrorEntry>,
    },
}

#[derive(Debug)]
pub enum AgentRunError {
    Session(AgentSessionError),
    Execution {
        session: Box<AgentSession>,
        source: ExecutionError,
    },
    MissingRunId {
        session: Box<AgentSession>,
        run_dir: PathBuf,
    },
}

impl SelfForgeApp {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            supervisor: Supervisor::new(&root),
            root,
        }
    }

    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    pub fn preflight(&self) -> Result<PreflightReport, MinimalLoopError> {
        let state = ForgeState::load(&self.root)?;
        let current_validation = self.supervisor.verify_version(&state.current_version)?;
        let candidate_checked_paths = match &state.candidate_version {
            Some(candidate_version) => {
                self.supervisor
                    .verify_version(candidate_version)?
                    .checked_paths
            }
            None => Vec::new(),
        };
        let open_errors = ErrorArchive::new(&self.root).list_run_errors(
            &state.current_version,
            ErrorListQuery::open(PREFLIGHT_OPEN_ERROR_LIMIT),
        )?;
        let can_advance = open_errors.is_empty();

        Ok(PreflightReport {
            current_version: state.current_version,
            current_workspace: state.workspace,
            status: state.status,
            candidate_version: state.candidate_version,
            candidate_workspace: state.candidate_workspace,
            checked_paths: current_validation.checked_paths,
            candidate_checked_paths,
            open_errors,
            can_advance,
        })
    }

    pub fn ai_config(&self) -> Result<AiConfigReport, AiConfigError> {
        AiProviderRegistry::inspect_project(&self.root)
    }

    pub fn ai_request_preview(&self, prompt: &str) -> Result<AiRequestSpec, AiRequestError> {
        AiProviderRegistry::build_text_request_project(&self.root, prompt)
    }

    pub fn ai_request(
        &self,
        prompt: &str,
        timeout_ms: u64,
    ) -> Result<AiExecutionReport, AiExecutionError> {
        AiProviderRegistry::execute_text_request_project(&self.root, prompt, timeout_ms)
    }

    pub fn agents(&self) -> Vec<AgentDefinition> {
        AgentRegistry::standard().agents().to_vec()
    }

    pub fn agent_plan(&self, goal: &str) -> Result<AgentPlan, AgentError> {
        AgentRegistry::standard().plan_for_goal(goal)
    }

    pub fn start_agent_session(
        &self,
        version: &str,
        goal: &str,
    ) -> Result<AgentSession, AgentSessionError> {
        AgentSessionStore::new(&self.root).start(version, goal)
    }

    pub fn agent_sessions(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AgentSessionSummary>, AgentSessionError> {
        AgentSessionStore::new(&self.root).list(version, limit)
    }

    pub fn agent_sessions_all(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AgentSessionSummary>, AgentSessionError> {
        AgentSessionStore::new(&self.root).list_all_major(version, limit)
    }

    pub fn agent_session(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AgentSession, AgentSessionError> {
        AgentSessionStore::new(&self.root).load(version, id)
    }

    pub fn agent_run(
        &self,
        session_version: &str,
        session_id: &str,
        target_version: &str,
        step_order: usize,
        program: &str,
        args: &[String],
        timeout_ms: u64,
    ) -> Result<AgentRunReport, AgentRunError> {
        let store = AgentSessionStore::new(&self.root);
        let mut session = store.load(session_version, session_id)?;
        if session.status == AgentSessionStatus::Planned {
            session.mark_running();
        }

        let execution =
            match self
                .supervisor
                .execute_in_workspace(target_version, program, args, timeout_ms)
            {
                Ok(report) => report,
                Err(error) => {
                    let message = error.to_string();
                    session.update_step(
                        step_order,
                        AgentStepStatus::Failed,
                        format!("Runtime 受控执行失败：{message}"),
                    )?;
                    session.mark_failed(message);
                    store.save(&session)?;
                    return Err(AgentRunError::Execution {
                        session: Box::new(session),
                        source: error,
                    });
                }
            };

        let Some(run_id) = execution
            .run_dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
        else {
            session.update_step(
                step_order,
                AgentStepStatus::Failed,
                "Runtime 受控执行未返回运行编号。",
            )?;
            session.mark_failed("Runtime 受控执行未返回运行编号。");
            store.save(&session)?;
            return Err(AgentRunError::MissingRunId {
                session: Box::new(session),
                run_dir: execution.run_dir,
            });
        };

        let report_path = execution.run_dir.join("report.json");
        let report_file = report_path
            .strip_prefix(&self.root)
            .unwrap_or(&report_path)
            .to_string_lossy()
            .into_owned();
        let reference = AgentRunReference {
            run_id: run_id.clone(),
            version: execution.version.clone(),
            report_file,
            exit_code: execution.exit_code,
            timed_out: execution.timed_out,
        };
        let failed = execution.timed_out || execution.exit_code != Some(0);
        let step_status = if failed {
            AgentStepStatus::Failed
        } else {
            AgentStepStatus::Completed
        };
        let result = format!(
            "运行 {run_id} 完成，退出码 {:?}，超时 {}，标准输出 {} 字节，标准错误 {} 字节。",
            execution.exit_code,
            execution.timed_out,
            execution.stdout.len(),
            execution.stderr.len()
        );
        session.update_step_with_run(step_order, step_status, result, reference)?;
        if failed {
            session.mark_failed(format!("运行记录 {run_id} 未通过验证。"));
        }
        store.save(&session)?;

        Ok(AgentRunReport {
            session,
            execution,
            run_id,
            step_order,
        })
    }

    pub fn agent_advance(&self, goal: &str) -> Result<AgentEvolutionReport, AgentEvolutionError> {
        let state = ForgeState::load(&self.root)
            .map_err(MinimalLoopError::from)
            .map_err(AgentEvolutionError::Setup)?;
        let store = AgentSessionStore::new(&self.root);
        let mut session = store.start(&state.current_version, goal)?;
        session.mark_running();
        session.update_step(
            1,
            AgentStepStatus::Completed,
            "已创建 Agent 会话并生成协作计划。",
        )?;
        store.save(&session)?;

        let preflight = match self.preflight() {
            Ok(report) => report,
            Err(error) => {
                session.update_step(2, AgentStepStatus::Failed, error.to_string())?;
                session.mark_failed(error.to_string());
                store.save(&session)?;
                return Err(AgentEvolutionError::MinimalLoop {
                    session: Box::new(session),
                    source: error,
                });
            }
        };
        session.update_step(
            2,
            AgentStepStatus::Completed,
            "前置检查通过，当前版本布局和未解决错误状态可用于进化。",
        )?;
        store.save(&session)?;

        if !preflight.can_advance {
            session.update_step(
                3,
                AgentStepStatus::Failed,
                "前置检查发现未解决错误，停止 Agent 自动进化。",
            )?;
            session.mark_failed("前置检查发现未解决错误。");
            store.save(&session)?;
            return Err(AgentEvolutionError::Blocked {
                session: Box::new(session),
                open_errors: preflight.open_errors,
            });
        }

        let minimal_loop = match self.advance(goal) {
            Ok(report) => report,
            Err(error) => {
                session.update_step(3, AgentStepStatus::Failed, error.to_string())?;
                session.mark_failed(error.to_string());
                store.save(&session)?;
                return Err(AgentEvolutionError::MinimalLoop {
                    session: Box::new(session),
                    source: error,
                });
            }
        };

        session.update_step(
            3,
            AgentStepStatus::Completed,
            "已调用 advance 执行受控进化状态机。",
        )?;
        session.update_step(
            4,
            AgentStepStatus::Completed,
            "advance 已完成候选验证、提升或回滚相关处理。",
        )?;
        session.update_step(
            5,
            AgentStepStatus::Completed,
            "未发现阻断继续推进的未解决错误。",
        )?;
        session.update_step(
            6,
            AgentStepStatus::Completed,
            "Agent 会话、计划步骤和进化结果已持久化。",
        )?;
        let outcome = format!(
            "结果 {:?}，稳定版本 {}，候选版本 {}",
            minimal_loop.outcome,
            minimal_loop.stable_version,
            minimal_loop.candidate_version.as_deref().unwrap_or("无")
        );
        session.mark_completed(outcome);
        store.save(&session)?;

        Ok(AgentEvolutionReport {
            session,
            preflight,
            minimal_loop,
        })
    }

    pub fn agent_evolve(
        &self,
        goal: &str,
    ) -> Result<AgentSingleEvolutionReport, AgentEvolutionError> {
        let state = ForgeState::load(&self.root)
            .map_err(MinimalLoopError::from)
            .map_err(AgentEvolutionError::Setup)?;
        let store = AgentSessionStore::new(&self.root);
        let mut session = store.start(&state.current_version, goal)?;
        session.mark_running();
        session.update_step(
            1,
            AgentStepStatus::Completed,
            "已创建 Agent 会话并生成单轮完整进化计划。",
        )?;
        store.save(&session)?;

        let preflight = match self.preflight() {
            Ok(report) => report,
            Err(error) => {
                session.update_step(2, AgentStepStatus::Failed, error.to_string())?;
                session.mark_failed(error.to_string());
                store.save(&session)?;
                return Err(AgentEvolutionError::MinimalLoop {
                    session: Box::new(session),
                    source: error,
                });
            }
        };
        session.update_step(
            2,
            AgentStepStatus::Completed,
            "前置检查通过，当前稳定版本可以进入单轮进化。",
        )?;
        store.save(&session)?;

        if !preflight.can_advance {
            session.update_step(
                3,
                AgentStepStatus::Failed,
                "前置检查发现未解决错误，停止单轮 Agent 进化。",
            )?;
            session.mark_failed("前置检查发现未解决错误。");
            store.save(&session)?;
            return Err(AgentEvolutionError::Blocked {
                session: Box::new(session),
                open_errors: preflight.open_errors,
            });
        }

        let prepared_candidate_version = if preflight.candidate_version.is_some() {
            let candidate = preflight
                .candidate_version
                .as_deref()
                .unwrap_or("未知候选版本");
            session.update_step(
                3,
                AgentStepStatus::Completed,
                format!("检测到已有候选版本 {candidate}，本轮直接进入候选验证。"),
            )?;
            None
        } else {
            match self.supervisor.prepare_next_version(goal) {
                Ok(report) => {
                    session.update_step(
                        3,
                        AgentStepStatus::Completed,
                        format!("已准备候选版本 {}。", report.next_version),
                    )?;
                    Some(report.next_version)
                }
                Err(error) => {
                    let source = MinimalLoopError::Evolution(error);
                    session.update_step(3, AgentStepStatus::Failed, source.to_string())?;
                    session.mark_failed(source.to_string());
                    store.save(&session)?;
                    return Err(AgentEvolutionError::MinimalLoop {
                        session: Box::new(session),
                        source,
                    });
                }
            }
        };
        store.save(&session)?;

        let cycle = match self.supervisor.run_candidate_cycle() {
            Ok(report) => report,
            Err(error) => {
                let source = MinimalLoopError::Evolution(error);
                session.update_step(4, AgentStepStatus::Failed, source.to_string())?;
                session.mark_failed(source.to_string());
                store.save(&session)?;
                return Err(AgentEvolutionError::MinimalLoop {
                    session: Box::new(session),
                    source,
                });
            }
        };

        let cycle_result = match cycle.result {
            CycleResult::Promoted => format!(
                "候选版本 {} 已通过验证并提升，当前稳定版本为 {}。",
                cycle.candidate_version, cycle.state.current_version
            ),
            CycleResult::RolledBack => format!(
                "候选版本 {} 验证失败并回滚，原因：{}",
                cycle.candidate_version,
                cycle.failure.as_deref().unwrap_or("未记录原因")
            ),
        };
        session.update_step(4, AgentStepStatus::Completed, cycle_result)?;
        session.update_step(
            5,
            AgentStepStatus::Completed,
            "已完成单轮候选验证、提升或回滚结果审查。",
        )?;
        session.update_step(
            6,
            AgentStepStatus::Completed,
            "Agent 单轮完整进化会话结果已持久化。",
        )?;
        let outcome = format!(
            "候选版本 {}，结果 {:?}，当前稳定版本 {}",
            cycle.candidate_version, cycle.result, cycle.state.current_version
        );
        session.mark_completed(outcome);
        store.save(&session)?;

        Ok(AgentSingleEvolutionReport {
            session,
            preflight,
            prepared_candidate_version,
            cycle,
        })
    }

    pub fn advance(&self, goal: &str) -> Result<MinimalLoopReport, MinimalLoopError> {
        let state = ForgeState::load(&self.root)?;
        let starting_version = state.current_version.clone();
        self.ensure_no_open_errors(&starting_version)?;

        if state.candidate_version.is_none() {
            let prepared = self.supervisor.prepare_next_version(goal)?;
            return Ok(MinimalLoopReport {
                outcome: MinimalLoopOutcome::Prepared,
                starting_version,
                stable_version: prepared.current_version,
                next_expected_version: next_version_after(&prepared.next_version).ok(),
                candidate_version: Some(prepared.next_version),
                failure: None,
            });
        }

        let cycle = self.supervisor.run_candidate_cycle()?;
        match cycle.result {
            CycleResult::Promoted => {
                let prepared = self.supervisor.prepare_next_version(goal)?;
                Ok(MinimalLoopReport {
                    outcome: MinimalLoopOutcome::PromotedAndPrepared,
                    starting_version,
                    stable_version: prepared.current_version,
                    next_expected_version: next_version_after(&prepared.next_version).ok(),
                    candidate_version: Some(prepared.next_version),
                    failure: None,
                })
            }
            CycleResult::RolledBack => Ok(MinimalLoopReport {
                outcome: MinimalLoopOutcome::RolledBack,
                starting_version,
                stable_version: cycle.previous_version,
                candidate_version: Some(cycle.candidate_version),
                next_expected_version: None,
                failure: cycle.failure,
            }),
        }
    }

    fn ensure_no_open_errors(&self, version: &str) -> Result<(), MinimalLoopError> {
        let errors =
            ErrorArchive::new(&self.root).list_run_errors(version, ErrorListQuery::open(1))?;
        if let Some(error) = errors.into_iter().next() {
            return Err(MinimalLoopError::OpenErrors {
                version: version.to_string(),
                run_id: error.run_id,
            });
        }

        Ok(())
    }
}

impl fmt::Display for MinimalLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MinimalLoopError::State(error) => write!(formatter, "{error}"),
            MinimalLoopError::Forge(error) => write!(formatter, "{error}"),
            MinimalLoopError::Evolution(error) => write!(formatter, "{error}"),
            MinimalLoopError::ErrorArchive(error) => write!(formatter, "{error}"),
            MinimalLoopError::OpenErrors { version, run_id } => write!(
                formatter,
                "版本 {version} 存在未解决错误 {run_id}，请先解决后再继续进化"
            ),
        }
    }
}

impl Error for MinimalLoopError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MinimalLoopError::State(error) => Some(error),
            MinimalLoopError::Forge(error) => Some(error),
            MinimalLoopError::Evolution(error) => Some(error),
            MinimalLoopError::ErrorArchive(error) => Some(error),
            MinimalLoopError::OpenErrors { .. } => None,
        }
    }
}

impl fmt::Display for AgentEvolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentEvolutionError::Session(error) => write!(formatter, "{error}"),
            AgentEvolutionError::Setup(error) => {
                write!(formatter, "Agent 自动进化初始化失败：{error}")
            }
            AgentEvolutionError::MinimalLoop { session, source } => write!(
                formatter,
                "Agent 会话 {} 执行进化失败：{}",
                session.id, source
            ),
            AgentEvolutionError::Blocked {
                session,
                open_errors,
            } => write!(
                formatter,
                "Agent 会话 {} 因 {} 个未解决错误停止进化",
                session.id,
                open_errors.len()
            ),
        }
    }
}

impl Error for AgentEvolutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentEvolutionError::Session(error) => Some(error),
            AgentEvolutionError::Setup(error) => Some(error),
            AgentEvolutionError::MinimalLoop { source, .. } => Some(source),
            AgentEvolutionError::Blocked { .. } => None,
        }
    }
}

impl From<AgentSessionError> for AgentEvolutionError {
    fn from(error: AgentSessionError) -> Self {
        AgentEvolutionError::Session(error)
    }
}

impl fmt::Display for AgentRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentRunError::Session(error) => write!(formatter, "{error}"),
            AgentRunError::Execution { session, source } => write!(
                formatter,
                "Agent 会话 {} 执行 Runtime 运行失败：{}",
                session.id, source
            ),
            AgentRunError::MissingRunId { session, run_dir } => write!(
                formatter,
                "Agent 会话 {} 的运行目录 {} 缺少运行编号",
                session.id,
                run_dir.display()
            ),
        }
    }
}

impl Error for AgentRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentRunError::Session(error) => Some(error),
            AgentRunError::Execution { source, .. } => Some(source),
            AgentRunError::MissingRunId { .. } => None,
        }
    }
}

impl From<AgentSessionError> for AgentRunError {
    fn from(error: AgentSessionError) -> Self {
        AgentRunError::Session(error)
    }
}

impl From<StateError> for MinimalLoopError {
    fn from(error: StateError) -> Self {
        MinimalLoopError::State(error)
    }
}

impl From<ForgeError> for MinimalLoopError {
    fn from(error: ForgeError) -> Self {
        MinimalLoopError::Forge(error)
    }
}

impl From<EvolutionError> for MinimalLoopError {
    fn from(error: EvolutionError) -> Self {
        MinimalLoopError::Evolution(error)
    }
}

impl From<ErrorArchiveError> for MinimalLoopError {
    fn from(error: ErrorArchiveError) -> Self {
        MinimalLoopError::ErrorArchive(error)
    }
}
