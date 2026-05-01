use super::agent::{
    AgentDefinition, AgentError, AgentPlan, AgentRegistry, AgentRunReference, AgentSession,
    AgentSessionError, AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus,
    AgentSessionStep, AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext,
    AgentStepExecutionReport, AgentStepExecutionRequest, AgentStepStatus,
    AgentToolConfigInitReport, AgentToolError, AgentToolInvocation, AgentToolInvocationInput,
    AgentToolInvocationReport, AgentToolReport, AgentWorkClaimReport, AgentWorkCoordinator,
    AgentWorkError, AgentWorkQueueReport, AgentWorkReapReport, AiSelfUpgradeAuditError,
    AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditStatus, AiSelfUpgradeAuditStore,
    AiSelfUpgradeAuditSummary, apply_tools_to_plan, initialize_agent_tool_config,
    load_agent_tool_report,
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
pub struct AiSelfUpgradePreview {
    pub current_version: String,
    pub hint: Option<String>,
    pub prompt: String,
    pub request: AiRequestSpec,
    pub preflight: PreflightReport,
    pub insights: MemoryInsightReport,
}

#[derive(Debug, Clone)]
pub struct AiSelfUpgradeReport {
    pub preview: AiSelfUpgradePreview,
    pub ai: AiExecutionReport,
    pub proposed_goal: String,
    pub evolution: AgentSingleEvolutionReport,
    pub audit: AiSelfUpgradeAuditRecord,
}

#[derive(Debug, Clone)]
pub struct AgentRunReport {
    pub session: AgentSession,
    pub execution: ExecutionReport,
    pub run_id: String,
    pub step_order: usize,
}

#[derive(Debug, Clone)]
struct AgentStepWorkClaim {
    task_id: String,
    worker_id: String,
    newly_claimed: bool,
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
pub enum AiSelfUpgradeError {
    Preflight(MinimalLoopError),
    Memory(MemoryContextError),
    Ai(AiExecutionError),
    Audit(AiSelfUpgradeAuditError),
    Blocked {
        version: String,
        open_errors: Vec<ArchivedErrorEntry>,
    },
    EmptyGoal {
        response_preview: String,
    },
    Evolution(AgentEvolutionError),
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
    Work(AgentWorkError),
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

    pub fn ai_self_upgrade_preview(
        &self,
        hint: &str,
    ) -> Result<AiSelfUpgradePreview, AiSelfUpgradeError> {
        self.ai_self_upgrade_preview_with_lookup(hint, |key| std::env::var(key).ok())
    }

    pub(crate) fn ai_self_upgrade_preview_with_lookup<F>(
        &self,
        hint: &str,
        process_lookup: F,
    ) -> Result<AiSelfUpgradePreview, AiSelfUpgradeError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let preflight = self.preflight().map_err(AiSelfUpgradeError::Preflight)?;
        if !preflight.can_advance {
            return Err(AiSelfUpgradeError::Blocked {
                version: preflight.current_version.clone(),
                open_errors: preflight.open_errors.clone(),
            });
        }

        let insights = self
            .memory_insights(&preflight.current_version, 5)
            .map_err(AiSelfUpgradeError::Memory)?;
        let normalized_hint = normalize_optional_text(hint);
        let prompt =
            build_ai_self_upgrade_prompt(&preflight, &insights, normalized_hint.as_deref());
        let request = AiProviderRegistry::build_text_request_project_with(
            &self.root,
            &prompt,
            process_lookup,
        )
        .map_err(AiExecutionError::from)
        .map_err(AiSelfUpgradeError::Ai)?;

        Ok(AiSelfUpgradePreview {
            current_version: preflight.current_version.clone(),
            hint: normalized_hint,
            prompt,
            request,
            preflight,
            insights,
        })
    }

    pub fn ai_self_upgrade(
        &self,
        hint: &str,
        timeout_ms: u64,
    ) -> Result<AiSelfUpgradeReport, AiSelfUpgradeError> {
        let preview = self.ai_self_upgrade_preview(hint)?;
        let ai = match self.ai_request(&preview.prompt, timeout_ms) {
            Ok(report) => report,
            Err(error) => {
                self.record_ai_self_upgrade_failure(&preview, None, None, &error.to_string())?;
                return Err(AiSelfUpgradeError::Ai(error));
            }
        };

        self.finish_ai_self_upgrade(preview, ai)
    }

    pub(crate) fn finish_ai_self_upgrade(
        &self,
        preview: AiSelfUpgradePreview,
        ai: AiExecutionReport,
    ) -> Result<AiSelfUpgradeReport, AiSelfUpgradeError> {
        let proposed_goal = match normalize_ai_self_upgrade_goal(&ai.response.text) {
            Ok(goal) => goal,
            Err(error) => {
                self.record_ai_self_upgrade_failure(&preview, Some(&ai), None, &error.to_string())?;
                return Err(error);
            }
        };
        let evolution = match self.agent_evolve(&proposed_goal) {
            Ok(report) => report,
            Err(error) => {
                self.record_ai_self_upgrade_failure(
                    &preview,
                    Some(&ai),
                    Some(&proposed_goal),
                    &error.to_string(),
                )?;
                return Err(AiSelfUpgradeError::Evolution(error));
            }
        };
        let audit =
            self.record_ai_self_upgrade_success(&preview, &ai, &proposed_goal, &evolution)?;

        Ok(AiSelfUpgradeReport {
            preview,
            ai,
            proposed_goal,
            evolution,
            audit,
        })
    }

    pub fn ai_self_upgrade_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiSelfUpgradeAuditSummary>, AiSelfUpgradeAuditError> {
        AiSelfUpgradeAuditStore::new(&self.root).list(version, limit)
    }

    pub fn ai_self_upgrade_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditError> {
        AiSelfUpgradeAuditStore::new(&self.root).load(version, id)
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

    pub fn init_agent_work_queue(
        &self,
        version: &str,
        goal: &str,
        thread_count: usize,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).initialize(version, goal, thread_count)
    }

    pub fn agent_work_status(&self, version: &str) -> Result<AgentWorkQueueReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).status(version)
    }

    pub fn claim_agent_work(
        &self,
        version: &str,
        worker_id: &str,
        preferred_agent_id: Option<&str>,
    ) -> Result<AgentWorkClaimReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).claim_next(version, worker_id, preferred_agent_id)
    }

    pub fn claim_agent_work_with_lease(
        &self,
        version: &str,
        worker_id: &str,
        preferred_agent_id: Option<&str>,
        lease_seconds: Option<u64>,
    ) -> Result<AgentWorkClaimReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).claim_next_with_lease(
            version,
            worker_id,
            preferred_agent_id,
            lease_seconds,
        )
    }

    pub fn reap_expired_agent_work(
        &self,
        version: &str,
        reason: &str,
    ) -> Result<AgentWorkReapReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).reap_expired(version, reason)
    }

    pub fn complete_agent_work(
        &self,
        version: &str,
        task_id: &str,
        worker_id: &str,
        summary: &str,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).complete(version, task_id, worker_id, summary)
    }

    pub fn release_agent_work(
        &self,
        version: &str,
        task_id: &str,
        worker_id: &str,
        reason: &str,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        AgentWorkCoordinator::new(&self.root).release(version, task_id, worker_id, reason)
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
        let work_claim = self.claim_work_for_step(&session, &step)?;
        if let Some(claim) = &work_claim {
            if claim.newly_claimed {
                session.attach_work_claim(
                    step.order,
                    claim.task_id.clone(),
                    claim.worker_id.clone(),
                )?;
                store.save(&session)?;
            }
        }
        let tool = match self.invoke_agent_tool(invocation) {
            Ok(report) => report,
            Err(error) => {
                let release_result = work_claim.as_ref().map(|claim| {
                    self.release_agent_work(
                        &request.session_version,
                        &claim.task_id,
                        &claim.worker_id,
                        &format!("步骤 {} 工具调用失败：{error}", step.order),
                    )
                });
                let release_message = match release_result {
                    Some(Ok(_)) => work_claim
                        .as_ref()
                        .map(|claim| format!("，协作任务 {} 已释放", claim.task_id))
                        .unwrap_or_default(),
                    Some(Err(release_error)) => work_claim
                        .as_ref()
                        .map(|claim| {
                            format!("，协作任务 {} 释放失败：{release_error}", claim.task_id)
                        })
                        .unwrap_or_default(),
                    None => String::new(),
                };
                let mut failed_session =
                    store.load(&request.session_version, &request.session_id)?;
                failed_session.update_step(
                    step.order,
                    AgentStepStatus::Failed,
                    format!("工具调用失败：{error}{release_message}。"),
                )?;
                failed_session.mark_failed(format!(
                    "步骤 {} 工具调用失败：{error}{release_message}。",
                    step.order
                ));
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

        if let Some(claim) = &work_claim {
            let step_status = session
                .steps
                .iter()
                .find(|candidate| candidate.order == step.order)
                .map(|candidate| candidate.status)
                .unwrap_or(AgentStepStatus::Failed);
            match step_status {
                AgentStepStatus::Completed => {
                    self.complete_agent_work(
                        &request.session_version,
                        &claim.task_id,
                        &claim.worker_id,
                        &format!(
                            "步骤 {} 工具 {} 已完成：{}",
                            step.order, tool.tool_id, tool.summary
                        ),
                    )?;
                }
                AgentStepStatus::Failed => {
                    self.release_agent_work(
                        &request.session_version,
                        &claim.task_id,
                        &claim.worker_id,
                        &format!(
                            "步骤 {} 工具 {} 未通过验证：{}",
                            step.order, tool.tool_id, tool.summary
                        ),
                    )?;
                }
                AgentStepStatus::Pending | AgentStepStatus::Running => {}
            }
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
            work_task_id: work_claim.as_ref().map(|claim| claim.task_id.clone()),
            work_worker_id: work_claim.as_ref().map(|claim| claim.worker_id.clone()),
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
                if let Err(error) = self.attach_work_queue_context(&mut session, version) {
                    let message = error.to_string();
                    session.mark_failed(message.clone());
                    store.save(&session)?;
                    return Err(AgentSessionError::PlanContext { message });
                }
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

    fn claim_work_for_step(
        &self,
        session: &AgentSession,
        step: &AgentSessionStep,
    ) -> Result<Option<AgentStepWorkClaim>, AgentWorkError> {
        if let (Some(task_id), Some(worker_id)) = (&step.work_task_id, &step.work_worker_id) {
            return Ok(Some(AgentStepWorkClaim {
                task_id: task_id.clone(),
                worker_id: worker_id.clone(),
                newly_claimed: false,
            }));
        }

        let Some(context) = session.plan_context.as_ref() else {
            return Ok(None);
        };
        if context.work_queue.is_none() {
            return Ok(None);
        }

        let worker_id = format!("{}-step-{}", session.id, step.order);
        let claim = AgentWorkCoordinator::new(&self.root).claim_next(
            &session.version,
            &worker_id,
            Some(&step.agent_id),
        )?;
        Ok(Some(AgentStepWorkClaim {
            task_id: claim.task.id,
            worker_id: claim.worker_id,
            newly_claimed: true,
        }))
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

    fn attach_work_queue_context(
        &self,
        session: &mut AgentSession,
        version: &str,
    ) -> Result<AgentWorkQueueReport, AgentWorkError> {
        let thread_count = session.plan.agents.len().max(1);
        let report = AgentWorkCoordinator::new(&self.root).initialize(
            version,
            &session.goal,
            thread_count,
        )?;
        let queue_file = report
            .queue_path
            .strip_prefix(&self.root)
            .unwrap_or(&report.queue_path)
            .to_string_lossy()
            .into_owned();

        if let Some(context) = session.plan_context.as_mut() {
            context.work_queue = Some(AgentSessionWorkQueueContext {
                version: report.version.clone(),
                queue_file: queue_file.clone(),
                task_count: report.queue.tasks.len(),
                thread_count: report.queue.thread_count,
                lease_duration_seconds: report.queue.lease_duration_seconds,
                created: report.created,
            });
        }
        session.record_work_queue_prepared(&queue_file, report.created);
        Ok(report)
    }

    fn record_ai_self_upgrade_success(
        &self,
        preview: &AiSelfUpgradePreview,
        ai: &AiExecutionReport,
        proposed_goal: &str,
        evolution: &AgentSingleEvolutionReport,
    ) -> Result<AiSelfUpgradeAuditRecord, AiSelfUpgradeError> {
        let mut record =
            self.ai_self_upgrade_audit_base(preview, AiSelfUpgradeAuditStatus::Succeeded);
        record.provider_id = ai.response.provider_id.clone();
        record.model = ai.response.model.clone();
        record.protocol = ai.response.protocol.clone();
        record.ai_response_preview = Some(truncate_chars(&ai.response.text, 240));
        record.proposed_goal = Some(proposed_goal.to_string());
        record.session_id = Some(evolution.session.id.clone());
        record.prepared_candidate_version = evolution.prepared_candidate_version.clone();
        record.candidate_version = Some(evolution.cycle.candidate_version.clone());
        record.cycle_result = Some(format!("{:?}", evolution.cycle.result));
        record.stable_version_after = Some(evolution.cycle.state.current_version.clone());

        AiSelfUpgradeAuditStore::new(&self.root)
            .create(record)
            .map_err(AiSelfUpgradeError::Audit)
    }

    fn record_ai_self_upgrade_failure(
        &self,
        preview: &AiSelfUpgradePreview,
        ai: Option<&AiExecutionReport>,
        proposed_goal: Option<&str>,
        error: &str,
    ) -> Result<AiSelfUpgradeAuditRecord, AiSelfUpgradeError> {
        let mut record = self.ai_self_upgrade_audit_base(preview, AiSelfUpgradeAuditStatus::Failed);
        if let Some(ai) = ai {
            record.provider_id = ai.response.provider_id.clone();
            record.model = ai.response.model.clone();
            record.protocol = ai.response.protocol.clone();
            record.ai_response_preview = Some(truncate_chars(&ai.response.text, 240));
        }
        record.proposed_goal = proposed_goal.map(ToString::to_string);
        record.error = Some(truncate_chars(error, 400));

        AiSelfUpgradeAuditStore::new(&self.root)
            .create(record)
            .map_err(AiSelfUpgradeError::Audit)
    }

    fn ai_self_upgrade_audit_base(
        &self,
        preview: &AiSelfUpgradePreview,
        status: AiSelfUpgradeAuditStatus,
    ) -> AiSelfUpgradeAuditRecord {
        AiSelfUpgradeAuditRecord {
            id: String::new(),
            version: preview.current_version.clone(),
            created_at_unix_seconds: 0,
            status,
            hint: preview.hint.clone(),
            provider_id: preview.request.provider_id.clone(),
            model: preview.request.model.clone(),
            protocol: preview.request.protocol.clone(),
            prompt_bytes: preview.prompt.len(),
            memory_source_versions: preview.insights.source_versions.clone(),
            success_experience_count: preview.insights.success_experiences.len(),
            failure_experience_count: preview.insights.failure_experiences.len(),
            optimization_suggestion_count: preview.insights.optimization_suggestions.len(),
            reusable_experience_count: preview.insights.reusable_experiences.len(),
            open_error_count: preview.preflight.open_errors.len(),
            ai_response_preview: None,
            proposed_goal: None,
            session_id: None,
            prepared_candidate_version: None,
            candidate_version: None,
            cycle_result: None,
            stable_version_after: None,
            error: None,
            file: PathBuf::new(),
        }
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
            work_queue: None,
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

fn build_ai_self_upgrade_prompt(
    preflight: &PreflightReport,
    insights: &MemoryInsightReport,
    hint: Option<&str>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("你是 SelfForge 的自我升级目标决策 Agent。\n");
    prompt.push_str("请基于当前状态和近期记忆，选择下一轮最小、可验证、可回滚的小版本升级目标。\n");
    prompt.push_str("必须遵守：只返回一个中文目标句子；不要 Markdown；不要编号；不要解释；不要输出代码；不要要求写入密钥；默认只做 patch 级升级；禁止修改 runtime 和 supervisor 受保护边界。\n");
    prompt.push_str(
        "目标必须能交给 SelfForge 的 agent-evolve 流程执行，并优先推进自动自我升级闭环。\n\n",
    );
    prompt.push_str("# 当前状态\n");
    prompt.push_str(&format!(
        "- 当前稳定版本：{}\n- 状态：{}\n- 候选版本：{}\n- 未解决错误：{}\n",
        preflight.current_version,
        preflight.status,
        preflight.candidate_version.as_deref().unwrap_or("无"),
        preflight.open_errors.len()
    ));
    if let Some(hint) = hint {
        prompt.push_str(&format!("- 用户补充目标：{hint}\n"));
    }
    prompt.push('\n');

    prompt.push_str("# 近期成功经验\n");
    prompt.push_str(&format_memory_insight_lines(
        &insights.success_experiences,
        5,
    ));
    prompt.push_str("\n# 近期失败风险\n");
    prompt.push_str(&format_memory_insight_lines(
        &insights.failure_experiences,
        5,
    ));
    prompt.push_str("\n# 近期优化建议\n");
    prompt.push_str(&format_memory_insight_lines(
        &insights.optimization_suggestions,
        5,
    ));
    prompt.push_str("\n# 可复用经验\n");
    prompt.push_str(&format_memory_insight_lines(
        &insights.reusable_experiences,
        5,
    ));
    prompt.push_str("\n# 输出格式\n");
    prompt.push_str("只返回一个中文目标句子，例如：继续完善 AI 自我升级流程的受控执行记录。\n");
    prompt
}

fn format_memory_insight_lines(insights: &[MemoryInsight], limit: usize) -> String {
    if insights.is_empty() || limit == 0 {
        return "- 暂无记录。\n".to_string();
    }

    insights
        .iter()
        .take(limit)
        .map(|insight| format!("- {}：{}\n", insight.version, insight.text.trim()))
        .collect::<Vec<_>>()
        .join("")
}

fn normalize_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn normalize_ai_self_upgrade_goal(text: &str) -> Result<String, AiSelfUpgradeError> {
    let response_preview = truncate_chars(text.trim(), 160);
    let Some(line) = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("```"))
    else {
        return Err(AiSelfUpgradeError::EmptyGoal { response_preview });
    };

    let cleaned = strip_goal_prefix(line)
        .chars()
        .filter(|character| !is_disallowed_symbol(*character))
        .collect::<String>();
    let goal = truncate_chars(
        cleaned.trim_matches(|character| {
            matches!(character, '"' | '\'' | '“' | '”' | '‘' | '’' | '`')
        }),
        160,
    );

    if goal.trim().is_empty() {
        Err(AiSelfUpgradeError::EmptyGoal { response_preview })
    } else {
        Ok(goal.trim().to_string())
    }
}

fn strip_goal_prefix(line: &str) -> &str {
    let mut value = line.trim();
    loop {
        let next = value
            .strip_prefix("- ")
            .or_else(|| value.strip_prefix("* "))
            .or_else(|| value.strip_prefix("目标："))
            .or_else(|| value.strip_prefix("目标:"))
            .or_else(|| value.strip_prefix("升级目标："))
            .or_else(|| value.strip_prefix("升级目标:"));
        if let Some(next) = next {
            value = next.trim();
            continue;
        }

        let Some((prefix, rest)) = value.split_once('.') else {
            break;
        };
        if prefix.chars().all(|character| character.is_ascii_digit()) {
            value = rest.trim();
            continue;
        }
        break;
    }

    value
}

fn truncate_chars(value: &str, max: usize) -> String {
    let mut output = value.chars().take(max).collect::<String>();
    if value.chars().count() > max {
        output.push_str("...");
    }
    output
}

fn is_disallowed_symbol(character: char) -> bool {
    matches!(
        character,
        '\u{1F000}'..='\u{1FAFF}' | '\u{2600}'..='\u{27BF}' | '\u{FE0F}'
    )
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

impl fmt::Display for AiSelfUpgradeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiSelfUpgradeError::Preflight(error) => {
                write!(formatter, "AI 自我升级预检失败：{error}")
            }
            AiSelfUpgradeError::Memory(error) => {
                write!(formatter, "AI 自我升级读取记忆失败：{error}")
            }
            AiSelfUpgradeError::Ai(error) => write!(formatter, "AI 自我升级请求失败：{error}"),
            AiSelfUpgradeError::Audit(error) => {
                write!(formatter, "AI 自我升级审计记录失败：{error}")
            }
            AiSelfUpgradeError::Blocked {
                version,
                open_errors,
            } => write!(
                formatter,
                "版本 {version} 存在 {} 条未解决错误，AI 自我升级已停止",
                open_errors.len()
            ),
            AiSelfUpgradeError::EmptyGoal { response_preview } => write!(
                formatter,
                "AI 自我升级响应未包含可执行目标，响应摘要：{response_preview}"
            ),
            AiSelfUpgradeError::Evolution(error) => {
                write!(formatter, "AI 自我升级执行受控进化失败：{error}")
            }
        }
    }
}

impl Error for AiSelfUpgradeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiSelfUpgradeError::Preflight(error) => Some(error),
            AiSelfUpgradeError::Memory(error) => Some(error),
            AiSelfUpgradeError::Ai(error) => Some(error),
            AiSelfUpgradeError::Audit(error) => Some(error),
            AiSelfUpgradeError::Evolution(error) => Some(error),
            AiSelfUpgradeError::Blocked { .. } | AiSelfUpgradeError::EmptyGoal { .. } => None,
        }
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
            AgentStepExecutionError::Work(error) => {
                write!(formatter, "协作任务板同步失败：{error}")
            }
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
            AgentStepExecutionError::Work(error) => Some(error),
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

impl From<AgentWorkError> for AgentStepExecutionError {
    fn from(error: AgentWorkError) -> Self {
        AgentStepExecutionError::Work(error)
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
