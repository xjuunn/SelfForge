use super::agent::{
    AgentDefinition, AgentError, AgentPlan, AgentRegistry, AgentRunReference, AgentSession,
    AgentSessionError, AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus,
    AgentSessionStep, AgentSessionStore, AgentSessionSummary, AgentStepExecutionReport,
    AgentStepExecutionRequest, AgentStepStatus, AgentToolConfigInitReport, AgentToolError,
    AgentToolInvocation, AgentToolInvocationInput, AgentToolInvocationReport, AgentToolReport,
    apply_tools_to_plan, initialize_agent_tool_config, load_agent_tool_report,
};
use super::ai_provider::{
    AiConfigError, AiConfigReport, AiExecutionError, AiExecutionReport, AiProviderRegistry,
    AiRequestError, AiRequestSpec,
};
use super::error_archive::{ArchivedErrorEntry, ErrorArchive, ErrorArchiveError, ErrorListQuery};
use super::memory::{
    MemoryCompactionError, MemoryCompactionReport, MemoryContextError, MemoryContextReport,
    MemoryInsight, MemoryInsightReport, compact_memory_archive, extract_memory_insights,
    read_recent_memory_context,
};
use crate::{
    CycleReport, CycleResult, EvolutionError, ExecutionError, ExecutionReport, ForgeError,
    ForgeState, StateError, Supervisor, VersionError, next_version_after, version_major_file_name,
};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

const PREFLIGHT_OPEN_ERROR_LIMIT: usize = 10;
const DEFAULT_MEMORY_COMPACTION_KEEP: usize = 5;

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
pub struct AgentPlanReport {
    pub plan: AgentPlan,
    pub insights: MemoryInsightReport,
    pub tools: AgentToolReport,
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
    pub memory_compaction: Option<MemoryCompactionReport>,
}

#[derive(Debug, Clone)]
pub struct AgentRunReport {
    pub session: AgentSession,
    pub execution: ExecutionReport,
    pub run_id: String,
    pub step_order: usize,
}

#[derive(Debug, Clone)]
pub struct AgentVerificationReport {
    pub session: AgentSession,
    pub execution: ExecutionReport,
    pub run_id: String,
}

#[derive(Debug)]
pub enum MinimalLoopError {
    State(StateError),
    Forge(ForgeError),
    Evolution(EvolutionError),
    ErrorArchive(ErrorArchiveError),
    Memory(MemoryContextError),
    OpenErrors { version: String, run_id: String },
}

#[derive(Debug)]
pub enum AgentPlanReportError {
    Agent(AgentError),
    Memory(MemoryContextError),
    Tools(AgentToolError),
}

#[derive(Debug)]
pub enum AgentEvolutionError {
    Session(AgentSessionError),
    Setup(MinimalLoopError),
    MinimalLoop {
        session: Box<AgentSession>,
        source: MinimalLoopError,
    },
    MemoryCompaction {
        session: Box<AgentSession>,
        source: MemoryCompactionError,
    },
    Blocked {
        session: Box<AgentSession>,
        open_errors: Vec<ArchivedErrorEntry>,
    },
}

#[derive(Debug)]
pub enum AgentRunError {
    Session(AgentSessionError),
    Setup(MinimalLoopError),
    Execution {
        session: Box<AgentSession>,
        source: ExecutionError,
    },
    MissingRunId {
        session: Box<AgentSession>,
        run_dir: PathBuf,
    },
}

#[derive(Debug)]
pub enum AgentToolInvocationError {
    Tools(AgentToolError),
    Memory(MemoryContextError),
    Session(AgentSessionError),
    Run(AgentRunError),
    AiRequest(AiRequestError),
    Setup(MinimalLoopError),
    Version(VersionError),
    ToolNotAssigned { agent_id: String, tool_id: String },
    UnsupportedInput { tool_id: String, expected: String },
    ToolRunnerMissing { tool_id: String },
}

#[derive(Debug)]
pub enum AgentStepExecutionError {
    Session(AgentSessionError),
    Tool(AgentToolInvocationError),
    NoPendingStep {
        session_id: String,
    },
    ToolNotInStep {
        step_order: usize,
        tool_id: String,
    },
    NoRunnableTool {
        step_order: usize,
    },
    InputRequired {
        step_order: usize,
        tool_id: String,
        input: String,
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

    pub fn agent_tools(&self, version: &str) -> Result<AgentToolReport, AgentToolError> {
        let registry = AgentRegistry::standard();
        load_agent_tool_report(&self.root, version, registry.agents())
    }

    pub fn init_agent_tool_config(
        &self,
        version: &str,
    ) -> Result<AgentToolConfigInitReport, AgentToolError> {
        initialize_agent_tool_config(&self.root, version)
    }

    pub fn agent_plan(&self, goal: &str) -> Result<AgentPlan, AgentError> {
        AgentRegistry::standard().plan_for_goal(goal)
    }

    pub fn agent_plan_with_memory(
        &self,
        goal: &str,
        version: &str,
        limit: usize,
    ) -> Result<AgentPlanReport, AgentPlanReportError> {
        let mut plan = self.agent_plan(goal)?;
        let insights = self.memory_insights(version, limit)?;
        let tools = self.agent_tools(version)?;
        apply_tools_to_plan(&mut plan, &tools);

        Ok(AgentPlanReport {
            plan,
            insights,
            tools,
        })
    }

    pub fn invoke_agent_tool(
        &self,
        invocation: AgentToolInvocation,
    ) -> Result<AgentToolInvocationReport, AgentToolInvocationError> {
        let tools = self.agent_tools(&invocation.version)?;
        let assigned_tools = tools.tool_ids_for_agent(&invocation.agent_id);
        if !assigned_tools
            .iter()
            .any(|tool_id| tool_id == &invocation.tool_id)
        {
            return Err(AgentToolInvocationError::ToolNotAssigned {
                agent_id: invocation.agent_id,
                tool_id: invocation.tool_id,
            });
        }

        let AgentToolInvocation {
            agent_id,
            tool_id,
            version,
            input,
        } = invocation;

        match tool_id.as_str() {
            "memory.context" => match input {
                AgentToolInvocationInput::MemoryContext { limit } => {
                    let report = self.memory_context(&version, limit)?;
                    Ok(AgentToolInvocationReport {
                        agent_id,
                        tool_id,
                        version,
                        summary: format!("已读取最近记忆 {} 条。", report.entries.len()),
                        details: report
                            .entries
                            .iter()
                            .map(|entry| {
                                format!(
                                    "{} 标题 {} 字符 {}",
                                    entry.version,
                                    entry.title,
                                    entry.body.chars().count()
                                )
                            })
                            .collect(),
                        run: None,
                    })
                }
                _ => Err(AgentToolInvocationError::UnsupportedInput {
                    tool_id,
                    expected: "MemoryContext".to_string(),
                }),
            },
            "memory.insights" => match input {
                AgentToolInvocationInput::MemoryInsights { limit } => {
                    let report = self.memory_insights(&version, limit)?;
                    Ok(AgentToolInvocationReport {
                        agent_id,
                        tool_id,
                        version,
                        summary: format!(
                            "已提取记忆经验，来源 {}，成功 {}，风险 {}，建议 {}，经验 {}。",
                            report.source_versions.len(),
                            report.success_experiences.len(),
                            report.failure_experiences.len(),
                            report.optimization_suggestions.len(),
                            report.reusable_experiences.len()
                        ),
                        details: report
                            .source_versions
                            .iter()
                            .map(|source| format!("来源版本 {source}"))
                            .collect(),
                        run: None,
                    })
                }
                _ => Err(AgentToolInvocationError::UnsupportedInput {
                    tool_id,
                    expected: "MemoryInsights".to_string(),
                }),
            },
            "agent.session" => match input {
                AgentToolInvocationInput::AgentSessions { limit, all_major } => {
                    let sessions = if all_major {
                        self.agent_sessions_all(&version, limit)?
                    } else {
                        self.agent_sessions(&version, limit)?
                    };
                    Ok(AgentToolInvocationReport {
                        agent_id,
                        tool_id,
                        version,
                        summary: format!("已查询 Agent 会话 {} 条。", sessions.len()),
                        details: sessions
                            .iter()
                            .map(|session| {
                                format!(
                                    "{} 版本 {} 状态 {} 步骤 {}",
                                    session.id, session.version, session.status, session.step_count
                                )
                            })
                            .collect(),
                        run: None,
                    })
                }
                _ => Err(AgentToolInvocationError::UnsupportedInput {
                    tool_id,
                    expected: "AgentSessions".to_string(),
                }),
            },
            "runtime.run" => match input {
                AgentToolInvocationInput::RuntimeRun {
                    session_version,
                    session_id,
                    target_version,
                    step_order,
                    program,
                    args,
                    timeout_ms,
                } => {
                    let report = self.agent_run(
                        &session_version,
                        &session_id,
                        &target_version,
                        step_order,
                        &program,
                        &args,
                        timeout_ms,
                    )?;
                    let report_path = report.execution.run_dir.join("report.json");
                    let report_file = report_path
                        .strip_prefix(&self.root)
                        .unwrap_or(&report_path)
                        .to_string_lossy()
                        .into_owned();
                    let run = AgentRunReference {
                        run_id: report.run_id.clone(),
                        version: report.execution.version.clone(),
                        report_file,
                        exit_code: report.execution.exit_code,
                        timed_out: report.execution.timed_out,
                    };
                    Ok(AgentToolInvocationReport {
                        agent_id,
                        tool_id,
                        version,
                        summary: format!(
                            "运行 {} 完成，退出码 {:?}，超时 {}。",
                            report.run_id, report.execution.exit_code, report.execution.timed_out
                        ),
                        details: vec![format!("会话 {} 步骤 {}", session_id, step_order)],
                        run: Some(run),
                    })
                }
                _ => Err(AgentToolInvocationError::UnsupportedInput {
                    tool_id,
                    expected: "RuntimeRun".to_string(),
                }),
            },
            "ai.request" => match input {
                AgentToolInvocationInput::AiRequestPreview { prompt } => {
                    let spec = self.ai_request_preview(&prompt)?;
                    Ok(AgentToolInvocationReport {
                        agent_id,
                        tool_id,
                        version,
                        summary: format!(
                            "已生成 AI 请求预览，提供商 {}，模型 {}。",
                            spec.provider_id, spec.model
                        ),
                        details: vec![
                            format!("协议 {}", spec.protocol),
                            format!("方法 {}", spec.method),
                            format!("地址 {}", spec.url),
                            format!("密钥变量 {}", spec.api_key_env_var),
                        ],
                        run: None,
                    })
                }
                _ => Err(AgentToolInvocationError::UnsupportedInput {
                    tool_id,
                    expected: "AiRequestPreview".to_string(),
                }),
            },
            "forge.archive" => match input {
                AgentToolInvocationInput::ForgeArchiveStatus | AgentToolInvocationInput::Empty => {
                    let archive_file = version_major_file_name(&version)?;
                    Ok(AgentToolInvocationReport {
                        agent_id,
                        tool_id,
                        version,
                        summary: "已解析当前 major 聚合归档路径。".to_string(),
                        details: ["memory", "tasks", "errors", "versions"]
                            .iter()
                            .map(|directory| format!("forge/{directory}/{archive_file}"))
                            .collect(),
                        run: None,
                    })
                }
                _ => Err(AgentToolInvocationError::UnsupportedInput {
                    tool_id,
                    expected: "ForgeArchiveStatus".to_string(),
                }),
            },
            _ => Err(AgentToolInvocationError::ToolRunnerMissing { tool_id }),
        }
    }

    pub fn execute_next_agent_step(
        &self,
        request: AgentStepExecutionRequest,
    ) -> Result<AgentStepExecutionReport, AgentStepExecutionError> {
        let store = AgentSessionStore::new(&self.root);
        let mut session = store.load(&request.session_version, &request.session_id)?;
        let step = session
            .steps
            .iter()
            .find(|step| step.status == AgentStepStatus::Pending)
            .cloned()
            .ok_or_else(|| AgentStepExecutionError::NoPendingStep {
                session_id: session.id.clone(),
            })?;

        if session.status == AgentSessionStatus::Planned {
            session.mark_running();
            store.save(&session)?;
        }

        let invocation = self.step_invocation(&request, &step)?;
        let tool = match self.invoke_agent_tool(invocation) {
            Ok(report) => report,
            Err(error) => {
                let mut failed_session =
                    store.load(&request.session_version, &request.session_id)?;
                failed_session.update_step(
                    step.order,
                    AgentStepStatus::Failed,
                    format!("工具调用失败：{error}"),
                )?;
                failed_session.mark_failed(format!("步骤 {} 工具调用失败：{error}", step.order));
                store.save(&failed_session)?;
                return Err(AgentStepExecutionError::Tool(error));
            }
        };

        let mut session = store.load(&request.session_version, &request.session_id)?;
        if tool.run.is_none() {
            session.update_step(
                step.order,
                AgentStepStatus::Completed,
                format!("工具 {} 调用完成：{}", tool.tool_id, tool.summary),
            )?;
        }

        let session_completed = session
            .steps
            .iter()
            .all(|step| step.status == AgentStepStatus::Completed);
        if session_completed && session.status != AgentSessionStatus::Completed {
            session.mark_completed("所有计划步骤已完成。");
        }
        store.save(&session)?;

        Ok(AgentStepExecutionReport {
            session_id: session.id.clone(),
            session_version: session.version.clone(),
            step_order: step.order,
            agent_id: step.agent_id,
            tool,
            session_completed,
        })
    }

    pub fn memory_context(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<MemoryContextReport, MemoryContextError> {
        read_recent_memory_context(&self.root, version, limit)
    }

    pub fn memory_insights(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<MemoryInsightReport, MemoryContextError> {
        extract_memory_insights(&self.root, version, limit)
    }

    pub fn compact_memory(
        &self,
        version: &str,
        keep_recent: usize,
    ) -> Result<MemoryCompactionReport, MemoryCompactionError> {
        compact_memory_archive(&self.root, version, keep_recent)
    }

    pub fn start_agent_session(
        &self,
        version: &str,
        goal: &str,
    ) -> Result<AgentSession, AgentSessionError> {
        let store = AgentSessionStore::new(&self.root);
        let mut session = self.start_session_with_tools(&store, version, goal)?;
        match self.attach_plan_context(&mut session, version, 5) {
            Ok(_) => {
                store.save(&session)?;
                Ok(session)
            }
            Err(error) => {
                let message = error.to_string();
                session.mark_failed(message.clone());
                store.save(&session)?;
                Err(AgentSessionError::PlanContext { message })
            }
        }
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

    pub fn agent_verify(
        &self,
        goal: &str,
        target_version: &str,
        program: &str,
        args: &[String],
        timeout_ms: u64,
    ) -> Result<AgentVerificationReport, AgentRunError> {
        let state = ForgeState::load(&self.root)
            .map_err(MinimalLoopError::from)
            .map_err(AgentRunError::Setup)?;
        let store = AgentSessionStore::new(&self.root);
        let mut session = self.start_session_with_tools(&store, &state.current_version, goal)?;
        session.mark_running();
        let memory = match self.attach_plan_context(&mut session, &state.current_version, 5) {
            Ok(report) => report,
            Err(error) => {
                let source = MinimalLoopError::Memory(error);
                session.update_step(1, AgentStepStatus::Failed, source.to_string())?;
                session.mark_failed(source.to_string());
                store.save(&session)?;
                return Err(AgentRunError::Setup(source));
            }
        };
        session.update_step(
            1,
            AgentStepStatus::Completed,
            format!(
                "已创建 Agent 验证会话并生成计划，已读取最近 {} 条历史记忆，提取 {} 条可复用经验和 {} 条优化建议。",
                memory.source_versions.len(),
                memory.reusable_experiences.len(),
                memory.optimization_suggestions.len()
            ),
        )?;
        session.update_step(
            2,
            AgentStepStatus::Completed,
            format!("验证目标版本为 {target_version}。"),
        )?;
        session.update_step(
            3,
            AgentStepStatus::Completed,
            "准备通过 Runtime 受控执行验证命令。",
        )?;
        store.save(&session)?;

        let run = self.agent_run(
            &state.current_version,
            &session.id,
            target_version,
            4,
            program,
            args,
            timeout_ms,
        )?;

        let mut session = run.session;
        if session.status != AgentSessionStatus::Failed {
            session.update_step(
                5,
                AgentStepStatus::Completed,
                "Runtime 运行记录已关联到 Agent 会话。",
            )?;
            session.update_step(6, AgentStepStatus::Completed, "Agent 验证会话已归档。")?;
            session.mark_completed(format!(
                "验证运行 {} 完成，退出码 {:?}，超时 {}。",
                run.run_id, run.execution.exit_code, run.execution.timed_out
            ));
            store.save(&session)?;
        }

        Ok(AgentVerificationReport {
            session,
            execution: run.execution,
            run_id: run.run_id,
        })
    }

    pub fn agent_advance(&self, goal: &str) -> Result<AgentEvolutionReport, AgentEvolutionError> {
        let state = ForgeState::load(&self.root)
            .map_err(MinimalLoopError::from)
            .map_err(AgentEvolutionError::Setup)?;
        let store = AgentSessionStore::new(&self.root);
        let mut session = self.start_session_with_tools(&store, &state.current_version, goal)?;
        session.mark_running();
        let memory = match self.attach_plan_context(&mut session, &state.current_version, 5) {
            Ok(report) => report,
            Err(error) => {
                let source = MinimalLoopError::Memory(error);
                session.update_step(1, AgentStepStatus::Failed, source.to_string())?;
                session.mark_failed(source.to_string());
                store.save(&session)?;
                return Err(AgentEvolutionError::MinimalLoop {
                    session: Box::new(session),
                    source,
                });
            }
        };
        session.update_step(
            1,
            AgentStepStatus::Completed,
            format!(
                "已创建 Agent 会话并生成协作计划，已读取最近 {} 条历史记忆，提取 {} 条可复用经验和 {} 条优化建议。",
                memory.source_versions.len(),
                memory.reusable_experiences.len(),
                memory.optimization_suggestions.len()
            ),
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
        let mut session = self.start_session_with_tools(&store, &state.current_version, goal)?;
        session.mark_running();
        let memory = match self.attach_plan_context(&mut session, &state.current_version, 5) {
            Ok(report) => report,
            Err(error) => {
                let source = MinimalLoopError::Memory(error);
                session.update_step(1, AgentStepStatus::Failed, source.to_string())?;
                session.mark_failed(source.to_string());
                store.save(&session)?;
                return Err(AgentEvolutionError::MinimalLoop {
                    session: Box::new(session),
                    source,
                });
            }
        };
        session.update_step(
            1,
            AgentStepStatus::Completed,
            format!(
                "已创建 Agent 会话并生成单轮完整进化计划，已读取最近 {} 条历史记忆，提取 {} 条可复用经验和 {} 条优化建议。",
                memory.source_versions.len(),
                memory.reusable_experiences.len(),
                memory.optimization_suggestions.len()
            ),
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
        let memory_compaction = if cycle.result == CycleResult::Promoted {
            match self.compact_memory(&cycle.state.current_version, DEFAULT_MEMORY_COMPACTION_KEEP)
            {
                Ok(report) => Some(report),
                Err(error) => {
                    session.update_step(
                        5,
                        AgentStepStatus::Failed,
                        format!("记忆自动压缩失败：{error}"),
                    )?;
                    session.mark_failed(format!("记忆自动压缩失败：{error}"));
                    store.save(&session)?;
                    return Err(AgentEvolutionError::MemoryCompaction {
                        session: Box::new(session),
                        source: error,
                    });
                }
            }
        } else {
            None
        };
        let review_message = match &memory_compaction {
            Some(report) => format!(
                "已完成单轮候选验证、提升结果审查，并自动压缩热记忆：保留 {} 条，本次归档 {} 条。",
                report.kept_sections, report.archived_sections
            ),
            None => "已完成单轮候选验证、回滚结果审查，未执行记忆压缩。".to_string(),
        };
        session.update_step(5, AgentStepStatus::Completed, review_message)?;
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
            memory_compaction,
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

    fn start_session_with_tools(
        &self,
        store: &AgentSessionStore,
        version: &str,
        goal: &str,
    ) -> Result<AgentSession, AgentSessionError> {
        let mut plan = self.agent_plan(goal)?;
        let tools = self
            .agent_tools(version)
            .map_err(|error| AgentSessionError::PlanContext {
                message: error.to_string(),
            })?;
        apply_tools_to_plan(&mut plan, &tools);
        store.start_with_plan_context(version, plan, None)
    }

    fn step_invocation(
        &self,
        request: &AgentStepExecutionRequest,
        step: &AgentSessionStep,
    ) -> Result<AgentToolInvocation, AgentStepExecutionError> {
        if let Some(tool_id) = &request.tool_id {
            if !step.tool_ids.iter().any(|candidate| candidate == tool_id) {
                return Err(AgentStepExecutionError::ToolNotInStep {
                    step_order: step.order,
                    tool_id: tool_id.clone(),
                });
            }
            return self.step_invocation_for_tool(request, step, tool_id);
        }

        let mut input_required = None;
        for tool_id in &step.tool_ids {
            match self.step_invocation_for_tool(request, step, tool_id) {
                Ok(invocation) => return Ok(invocation),
                Err(AgentStepExecutionError::InputRequired {
                    step_order,
                    tool_id,
                    input,
                }) => {
                    if input_required.is_none() {
                        input_required = Some(AgentStepExecutionError::InputRequired {
                            step_order,
                            tool_id,
                            input,
                        });
                    }
                }
                Err(AgentStepExecutionError::NoRunnableTool { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        Err(
            input_required.unwrap_or(AgentStepExecutionError::NoRunnableTool {
                step_order: step.order,
            }),
        )
    }

    fn step_invocation_for_tool(
        &self,
        request: &AgentStepExecutionRequest,
        step: &AgentSessionStep,
        tool_id: &str,
    ) -> Result<AgentToolInvocation, AgentStepExecutionError> {
        let input = match tool_id {
            "memory.context" => AgentToolInvocationInput::MemoryContext {
                limit: request.limit,
            },
            "memory.insights" => AgentToolInvocationInput::MemoryInsights {
                limit: request.limit,
            },
            "agent.session" => AgentToolInvocationInput::AgentSessions {
                limit: request.limit,
                all_major: true,
            },
            "forge.archive" => AgentToolInvocationInput::ForgeArchiveStatus,
            "runtime.run" => {
                let Some(program) = &request.program else {
                    return Err(AgentStepExecutionError::InputRequired {
                        step_order: step.order,
                        tool_id: tool_id.to_string(),
                        input: "PROGRAM".to_string(),
                    });
                };
                AgentToolInvocationInput::RuntimeRun {
                    session_version: request.session_version.clone(),
                    session_id: request.session_id.clone(),
                    target_version: request.target_version.clone(),
                    step_order: step.order,
                    program: program.clone(),
                    args: request.args.clone(),
                    timeout_ms: request.timeout_ms,
                }
            }
            "ai.request" => {
                let Some(prompt) = &request.prompt else {
                    return Err(AgentStepExecutionError::InputRequired {
                        step_order: step.order,
                        tool_id: tool_id.to_string(),
                        input: "prompt".to_string(),
                    });
                };
                AgentToolInvocationInput::AiRequestPreview {
                    prompt: prompt.clone(),
                }
            }
            _ => {
                return Err(AgentStepExecutionError::NoRunnableTool {
                    step_order: step.order,
                });
            }
        };

        Ok(AgentToolInvocation {
            agent_id: step.agent_id.clone(),
            tool_id: tool_id.to_string(),
            version: request.session_version.clone(),
            input,
        })
    }

    fn attach_plan_context(
        &self,
        session: &mut AgentSession,
        version: &str,
        limit: usize,
    ) -> Result<MemoryInsightReport, MemoryContextError> {
        let insights = self.memory_insights(version, limit)?;
        session.plan_context = Some(self.session_plan_context(&insights));
        Ok(insights)
    }

    fn session_plan_context(&self, insights: &MemoryInsightReport) -> AgentSessionPlanContext {
        let archive_file = insights
            .archive_path
            .strip_prefix(&self.root)
            .unwrap_or(&insights.archive_path)
            .to_string_lossy()
            .into_owned();

        AgentSessionPlanContext {
            memory_version: insights.version.clone(),
            memory_archive_file: archive_file,
            source_versions: insights.source_versions.clone(),
            success_experiences: session_memory_insights(&insights.success_experiences),
            failure_experiences: session_memory_insights(&insights.failure_experiences),
            optimization_suggestions: session_memory_insights(&insights.optimization_suggestions),
            reusable_experiences: session_memory_insights(&insights.reusable_experiences),
        }
    }
}

fn session_memory_insights(insights: &[MemoryInsight]) -> Vec<AgentSessionMemoryInsight> {
    insights
        .iter()
        .map(|insight| AgentSessionMemoryInsight {
            version: insight.version.clone(),
            text: insight.text.clone(),
        })
        .collect()
}

impl fmt::Display for MinimalLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MinimalLoopError::State(error) => write!(formatter, "{error}"),
            MinimalLoopError::Forge(error) => write!(formatter, "{error}"),
            MinimalLoopError::Evolution(error) => write!(formatter, "{error}"),
            MinimalLoopError::ErrorArchive(error) => write!(formatter, "{error}"),
            MinimalLoopError::Memory(error) => write!(formatter, "{error}"),
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
            MinimalLoopError::Memory(error) => Some(error),
            MinimalLoopError::OpenErrors { .. } => None,
        }
    }
}

impl fmt::Display for AgentPlanReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentPlanReportError::Agent(error) => {
                write!(formatter, "Agent 计划生成失败：{error}")
            }
            AgentPlanReportError::Memory(error) => {
                write!(formatter, "Agent 计划记忆读取失败：{error}")
            }
            AgentPlanReportError::Tools(error) => {
                write!(formatter, "Agent 计划工具配置失败：{error}")
            }
        }
    }
}

impl Error for AgentPlanReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentPlanReportError::Agent(error) => Some(error),
            AgentPlanReportError::Memory(error) => Some(error),
            AgentPlanReportError::Tools(error) => Some(error),
        }
    }
}

impl From<AgentError> for AgentPlanReportError {
    fn from(error: AgentError) -> Self {
        AgentPlanReportError::Agent(error)
    }
}

impl From<MemoryContextError> for AgentPlanReportError {
    fn from(error: MemoryContextError) -> Self {
        AgentPlanReportError::Memory(error)
    }
}

impl From<AgentToolError> for AgentPlanReportError {
    fn from(error: AgentToolError) -> Self {
        AgentPlanReportError::Tools(error)
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
            AgentEvolutionError::MemoryCompaction { session, source } => write!(
                formatter,
                "Agent 会话 {} 记忆自动压缩失败：{}",
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
            AgentEvolutionError::MemoryCompaction { source, .. } => Some(source),
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
            AgentRunError::Setup(error) => {
                write!(formatter, "Agent 验证初始化失败：{error}")
            }
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
            AgentRunError::Setup(error) => Some(error),
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

impl fmt::Display for AgentToolInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentToolInvocationError::Tools(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::Memory(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::Session(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::Run(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::AiRequest(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::Setup(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::Version(error) => write!(formatter, "{error}"),
            AgentToolInvocationError::ToolNotAssigned { agent_id, tool_id } => {
                write!(formatter, "Agent {agent_id} 未绑定工具 {tool_id}，禁止调用")
            }
            AgentToolInvocationError::UnsupportedInput { tool_id, expected } => write!(
                formatter,
                "工具 {tool_id} 的调用输入不匹配，期望 {expected}"
            ),
            AgentToolInvocationError::ToolRunnerMissing { tool_id } => {
                write!(formatter, "工具 {tool_id} 尚未实现执行器")
            }
        }
    }
}

impl Error for AgentToolInvocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentToolInvocationError::Tools(error) => Some(error),
            AgentToolInvocationError::Memory(error) => Some(error),
            AgentToolInvocationError::Session(error) => Some(error),
            AgentToolInvocationError::Run(error) => Some(error),
            AgentToolInvocationError::AiRequest(error) => Some(error),
            AgentToolInvocationError::Setup(error) => Some(error),
            AgentToolInvocationError::Version(error) => Some(error),
            AgentToolInvocationError::ToolNotAssigned { .. }
            | AgentToolInvocationError::UnsupportedInput { .. }
            | AgentToolInvocationError::ToolRunnerMissing { .. } => None,
        }
    }
}

impl From<AgentToolError> for AgentToolInvocationError {
    fn from(error: AgentToolError) -> Self {
        AgentToolInvocationError::Tools(error)
    }
}

impl From<MemoryContextError> for AgentToolInvocationError {
    fn from(error: MemoryContextError) -> Self {
        AgentToolInvocationError::Memory(error)
    }
}

impl From<AgentSessionError> for AgentToolInvocationError {
    fn from(error: AgentSessionError) -> Self {
        AgentToolInvocationError::Session(error)
    }
}

impl From<AgentRunError> for AgentToolInvocationError {
    fn from(error: AgentRunError) -> Self {
        AgentToolInvocationError::Run(error)
    }
}

impl From<AiRequestError> for AgentToolInvocationError {
    fn from(error: AiRequestError) -> Self {
        AgentToolInvocationError::AiRequest(error)
    }
}

impl From<MinimalLoopError> for AgentToolInvocationError {
    fn from(error: MinimalLoopError) -> Self {
        AgentToolInvocationError::Setup(error)
    }
}

impl From<VersionError> for AgentToolInvocationError {
    fn from(error: VersionError) -> Self {
        AgentToolInvocationError::Version(error)
    }
}

impl fmt::Display for AgentStepExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStepExecutionError::Session(error) => write!(formatter, "{error}"),
            AgentStepExecutionError::Tool(error) => write!(formatter, "{error}"),
            AgentStepExecutionError::NoPendingStep { session_id } => {
                write!(formatter, "Agent 会话 {session_id} 没有待执行步骤")
            }
            AgentStepExecutionError::ToolNotInStep {
                step_order,
                tool_id,
            } => write!(formatter, "步骤 {step_order} 未绑定工具 {tool_id}"),
            AgentStepExecutionError::NoRunnableTool { step_order } => {
                write!(formatter, "步骤 {step_order} 没有可自动执行的工具")
            }
            AgentStepExecutionError::InputRequired {
                step_order,
                tool_id,
                input,
            } => write!(
                formatter,
                "步骤 {step_order} 的工具 {tool_id} 需要输入 {input}"
            ),
        }
    }
}

impl Error for AgentStepExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentStepExecutionError::Session(error) => Some(error),
            AgentStepExecutionError::Tool(error) => Some(error),
            AgentStepExecutionError::NoPendingStep { .. }
            | AgentStepExecutionError::ToolNotInStep { .. }
            | AgentStepExecutionError::NoRunnableTool { .. }
            | AgentStepExecutionError::InputRequired { .. } => None,
        }
    }
}

impl From<AgentSessionError> for AgentStepExecutionError {
    fn from(error: AgentSessionError) -> Self {
        AgentStepExecutionError::Session(error)
    }
}

impl From<AgentToolInvocationError> for AgentStepExecutionError {
    fn from(error: AgentToolInvocationError) -> Self {
        AgentStepExecutionError::Tool(error)
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
