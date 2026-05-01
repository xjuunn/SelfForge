use super::agent::{
    AgentDefinition, AgentError, AgentPlan, AgentRegistry, AgentRunReference, AgentSession,
    AgentSessionError, AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus,
    AgentSessionStep, AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext,
    AgentStepExecutionReport, AgentStepExecutionRequest, AgentStepStatus,
    AgentToolConfigInitReport, AgentToolError, AgentToolInvocation, AgentToolInvocationInput,
    AgentToolInvocationReport, AgentToolReport, AgentWorkClaimReport, AgentWorkCoordinator,
    AgentWorkError, AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTaskStatus,
    AiPatchApplicationFile, AiPatchApplicationRecord, AiPatchApplicationStatus,
    AiPatchApplicationStore, AiPatchApplicationStoreError, AiPatchApplicationSummary,
    AiPatchAuditFinding, AiPatchAuditFindingKind, AiPatchAuditRecord, AiPatchAuditSeverity,
    AiPatchAuditStatus, AiPatchAuditStore, AiPatchAuditStoreError, AiPatchAuditSummary,
    AiPatchDraftRecord, AiPatchDraftStatus, AiPatchDraftStore, AiPatchDraftStoreError,
    AiPatchDraftSummary, AiPatchPreviewChange, AiPatchPreviewRecord, AiPatchPreviewStatus,
    AiPatchPreviewStore, AiPatchPreviewStoreError, AiPatchPreviewSummary,
    AiPatchSourceCandidateRecord, AiPatchSourceCandidateStatus, AiPatchSourceCandidateStore,
    AiPatchSourceCandidateStoreError, AiPatchSourceCandidateSummary,
    AiPatchSourceCycleFollowUpRecord, AiPatchSourceCycleFollowUpStatus,
    AiPatchSourceCycleFollowUpStore, AiPatchSourceCycleFollowUpStoreError,
    AiPatchSourceCycleFollowUpSummary, AiPatchSourceCycleRecord, AiPatchSourceCycleResult,
    AiPatchSourceCycleStatus, AiPatchSourceCycleStore, AiPatchSourceCycleStoreError,
    AiPatchSourceCycleSummary, AiPatchSourceExecutionFile, AiPatchSourceExecutionRecord,
    AiPatchSourceExecutionStatus, AiPatchSourceExecutionStore, AiPatchSourceExecutionStoreError,
    AiPatchSourceExecutionSummary, AiPatchSourcePlanFile, AiPatchSourcePlanRecord,
    AiPatchSourcePlanStatus, AiPatchSourcePlanStore, AiPatchSourcePlanStoreError,
    AiPatchSourcePlanSummary, AiPatchSourcePromotionRecord, AiPatchSourcePromotionStatus,
    AiPatchSourcePromotionStore, AiPatchSourcePromotionStoreError, AiPatchSourcePromotionSummary,
    AiPatchSourceTaskAuditFinding, AiPatchSourceTaskAuditRecord, AiPatchSourceTaskAuditStatus,
    AiPatchSourceTaskAuditStore, AiPatchSourceTaskAuditStoreError, AiPatchSourceTaskAuditSummary,
    AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftStatus, AiPatchSourceTaskDraftStore,
    AiPatchSourceTaskDraftStoreError, AiPatchSourceTaskDraftSummary,
    AiPatchVerificationCommandRecord, AiPatchVerificationStatus, AiSelfUpgradeAuditError,
    AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditStatus, AiSelfUpgradeAuditStore,
    AiSelfUpgradeAuditSummary, AiSelfUpgradeSummaryIndexEntry, AiSelfUpgradeSummaryRecord,
    AiSelfUpgradeSummaryStatus, AiSelfUpgradeSummaryStore, AiSelfUpgradeSummaryStoreError,
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
    version_major_key,
};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    pub summary: AiSelfUpgradeSummaryRecord,
}

#[derive(Debug, Clone)]
pub struct AiSelfUpgradeSummaryReport {
    pub audit: AiSelfUpgradeAuditRecord,
    pub session: Option<AgentSession>,
    pub record: AiSelfUpgradeSummaryRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchDraftPreview {
    pub current_version: String,
    pub target_version: String,
    pub goal: String,
    pub prompt: String,
    pub request: AiRequestSpec,
    pub preflight: PreflightReport,
    pub insights: MemoryInsightReport,
    pub allowed_write_roots: Vec<String>,
    pub required_sections: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AiPatchDraftReport {
    pub preview: AiPatchDraftPreview,
    pub ai: AiExecutionReport,
    pub record: AiPatchDraftRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchAuditReport {
    pub draft: AiPatchDraftRecord,
    pub queue: Option<AgentWorkQueueReport>,
    pub record: AiPatchAuditRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchPreviewReport {
    pub audit: AiPatchAuditRecord,
    pub draft: AiPatchDraftRecord,
    pub record: AiPatchPreviewRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchApplicationReport {
    pub preview: AiPatchPreviewRecord,
    pub draft: AiPatchDraftRecord,
    pub prepared_candidate_version: Option<String>,
    pub record: AiPatchApplicationRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchVerificationReport {
    pub record: AiPatchApplicationRecord,
    pub executed_count: usize,
    pub status: AiPatchVerificationStatus,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourcePlanReport {
    pub application: AiPatchApplicationRecord,
    pub record: AiPatchSourcePlanRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourceExecutionReport {
    pub source_plan: AiPatchSourcePlanRecord,
    pub record: AiPatchSourceExecutionRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourcePromotionReport {
    pub source_execution: AiPatchSourceExecutionRecord,
    pub record: AiPatchSourcePromotionRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourceCandidateReport {
    pub promotion: AiPatchSourcePromotionRecord,
    pub record: AiPatchSourceCandidateRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourceCycleReport {
    pub candidate: AiPatchSourceCandidateRecord,
    pub record: AiPatchSourceCycleRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourceCycleSummaryReport {
    pub cycle: AiPatchSourceCycleRecord,
    pub record: AiPatchSourceCycleFollowUpRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourceTaskDraftReport {
    pub summary: AiPatchSourceCycleFollowUpRecord,
    pub record: AiPatchSourceTaskDraftRecord,
}

#[derive(Debug, Clone)]
pub struct AiPatchSourceTaskAuditReport {
    pub task_draft: AiPatchSourceTaskDraftRecord,
    pub record: AiPatchSourceTaskAuditRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPatchVerificationCommandSpec {
    pub command: String,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AgentRunReport {
    pub session: AgentSession,
    pub execution: ExecutionReport,
    pub run_id: String,
    pub step_order: usize,
}

#[derive(Debug, Clone)]
pub struct AgentStepRunReport {
    pub session_id: String,
    pub session_version: String,
    pub target_version: String,
    pub max_steps: usize,
    pub executed_steps: Vec<AgentStepExecutionReport>,
    pub stop: AgentStepRunStop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStepRunStop {
    SessionCompleted,
    StepLimitReached,
    NoPendingStep {
        session_id: String,
    },
    InputRequired {
        step_order: usize,
        tool_id: String,
        input: String,
    },
    NoRunnableTool {
        step_order: usize,
    },
    Failed {
        message: String,
    },
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
    Summary(AiSelfUpgradeSummaryError),
}

#[derive(Debug)]
pub enum AiSelfUpgradeSummaryError {
    Audit(AiSelfUpgradeAuditError),
    Session(AgentSessionError),
    Store(AiSelfUpgradeSummaryStoreError),
}

#[derive(Debug)]
pub enum AiPatchDraftError {
    Preflight(MinimalLoopError),
    Memory(MemoryContextError),
    Ai(AiExecutionError),
    Store(AiPatchDraftStoreError),
    TaskAudit(AiPatchSourceTaskAuditStoreError),
    Version(VersionError),
    Blocked {
        version: String,
        open_errors: Vec<ArchivedErrorEntry>,
    },
    TaskAuditNotApproved {
        id: String,
        status: AiPatchSourceTaskAuditStatus,
        blocked_reason: Option<String>,
    },
    EmptyTaskAuditGoal {
        id: String,
    },
    InvalidDraft {
        reason: String,
        response_preview: String,
    },
}

#[derive(Debug)]
pub enum AiPatchAuditError {
    Draft(AiPatchDraftStoreError),
    Store(AiPatchAuditStoreError),
    WorkQueue(AgentWorkError),
    Version(VersionError),
    Io { path: PathBuf, source: io::Error },
}

#[derive(Debug)]
pub enum AiPatchPreviewError {
    Audit(AiPatchAuditStoreError),
    Draft(AiPatchDraftStoreError),
    Store(AiPatchPreviewStoreError),
    Io { path: PathBuf, source: io::Error },
}

#[derive(Debug)]
pub enum AiPatchApplicationError {
    Preflight(MinimalLoopError),
    Evolution(EvolutionError),
    Preview(AiPatchPreviewStoreError),
    Draft(AiPatchDraftStoreError),
    Store(AiPatchApplicationStoreError),
    Version(VersionError),
    Forge(ForgeError),
    Io { path: PathBuf, source: io::Error },
    InvalidPath { path: String, reason: String },
}

#[derive(Debug)]
pub enum AiPatchVerificationError {
    Store(AiPatchApplicationStoreError),
    Preview(AiPatchPreviewStoreError),
    UnsupportedCommand(String),
    Io { path: PathBuf, source: io::Error },
}

#[derive(Debug)]
pub enum AiPatchSourcePlanError {
    Application(AiPatchApplicationStoreError),
    Store(AiPatchSourcePlanStoreError),
    Io { path: PathBuf, source: io::Error },
    InvalidPath { path: String, reason: String },
}

#[derive(Debug)]
pub enum AiPatchSourceExecutionError {
    SourcePlan(AiPatchSourcePlanStoreError),
    Store(AiPatchSourceExecutionStoreError),
    Verification(AiPatchVerificationError),
    Io { path: PathBuf, source: io::Error },
    InvalidPath { path: String, reason: String },
}

#[derive(Debug)]
pub enum AiPatchSourcePromotionError {
    SourceExecution(AiPatchSourceExecutionStoreError),
    Store(AiPatchSourcePromotionStoreError),
    Version(VersionError),
}

#[derive(Debug)]
pub enum AiPatchSourceCandidateError {
    Promotion(AiPatchSourcePromotionStoreError),
    Store(AiPatchSourceCandidateStoreError),
    State(StateError),
    ErrorArchive(ErrorArchiveError),
    Version(VersionError),
}

#[derive(Debug)]
pub enum AiPatchSourceCycleError {
    Candidate(AiPatchSourceCandidateStoreError),
    Store(AiPatchSourceCycleStoreError),
    State(StateError),
}

#[derive(Debug)]
pub enum AiPatchSourceCycleSummaryError {
    Cycle(AiPatchSourceCycleStoreError),
    Store(AiPatchSourceCycleFollowUpStoreError),
}

#[derive(Debug)]
pub enum AiPatchSourceTaskDraftError {
    Summary(AiPatchSourceCycleFollowUpStoreError),
    Store(AiPatchSourceTaskDraftStoreError),
    Version(VersionError),
}

#[derive(Debug)]
pub enum AiPatchSourceTaskAuditError {
    TaskDraft(AiPatchSourceTaskDraftStoreError),
    Store(AiPatchSourceTaskAuditStoreError),
    Version(VersionError),
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

#[derive(Debug)]
pub enum AgentStepRunError {
    InvalidStepLimit,
    Step(AgentStepExecutionError),
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

    pub fn ai_patch_draft_preview(
        &self,
        goal: &str,
    ) -> Result<AiPatchDraftPreview, AiPatchDraftError> {
        self.ai_patch_draft_preview_with_lookup(goal, |key| std::env::var(key).ok())
    }

    pub(crate) fn ai_patch_draft_preview_with_lookup<F>(
        &self,
        goal: &str,
        process_lookup: F,
    ) -> Result<AiPatchDraftPreview, AiPatchDraftError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let preflight = self.preflight().map_err(AiPatchDraftError::Preflight)?;
        if !preflight.open_errors.is_empty() {
            return Err(AiPatchDraftError::Blocked {
                version: preflight.current_version.clone(),
                open_errors: preflight.open_errors.clone(),
            });
        }

        let target_version = preflight
            .candidate_version
            .clone()
            .map(Ok)
            .unwrap_or_else(|| {
                next_version_after(&preflight.current_version).map_err(AiPatchDraftError::Version)
            })?;
        let insights = self
            .memory_insights(&preflight.current_version, 5)
            .map_err(AiPatchDraftError::Memory)?;
        let goal =
            normalize_optional_text(goal).unwrap_or_else(|| "生成下一轮 AI 补丁草案".to_string());
        let allowed_write_roots = patch_draft_allowed_write_roots(&preflight.current_version)?;
        let required_sections = patch_draft_required_sections();
        let prompt = build_ai_patch_draft_prompt(
            &preflight,
            &target_version,
            &insights,
            &goal,
            &allowed_write_roots,
            &required_sections,
        );
        let request = AiProviderRegistry::build_text_request_project_with(
            &self.root,
            &prompt,
            process_lookup,
        )
        .map_err(AiExecutionError::from)
        .map_err(AiPatchDraftError::Ai)?;

        Ok(AiPatchDraftPreview {
            current_version: preflight.current_version.clone(),
            target_version,
            goal,
            prompt,
            request,
            preflight,
            insights,
            allowed_write_roots,
            required_sections,
        })
    }

    pub fn ai_patch_draft(
        &self,
        goal: &str,
        timeout_ms: u64,
    ) -> Result<AiPatchDraftReport, AiPatchDraftError> {
        let preview = self.ai_patch_draft_preview(goal)?;
        let ai = match self.ai_request(&preview.prompt, timeout_ms) {
            Ok(report) => report,
            Err(error) => {
                self.record_ai_patch_draft_failure(&preview, None, &error.to_string())?;
                return Err(AiPatchDraftError::Ai(error));
            }
        };

        self.finish_ai_patch_draft(preview, ai)
    }

    pub fn ai_patch_draft_preview_from_task_audit(
        &self,
        task_audit_id: &str,
    ) -> Result<AiPatchDraftPreview, AiPatchDraftError> {
        self.ai_patch_draft_preview_from_task_audit_with_lookup(task_audit_id, |key| {
            std::env::var(key).ok()
        })
    }

    pub(crate) fn ai_patch_draft_preview_from_task_audit_with_lookup<F>(
        &self,
        task_audit_id: &str,
        process_lookup: F,
    ) -> Result<AiPatchDraftPreview, AiPatchDraftError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let goal = self.approved_patch_draft_goal_from_task_audit(task_audit_id)?;
        self.ai_patch_draft_preview_with_lookup(&goal, process_lookup)
    }

    pub fn ai_patch_draft_from_task_audit(
        &self,
        task_audit_id: &str,
        timeout_ms: u64,
    ) -> Result<AiPatchDraftReport, AiPatchDraftError> {
        let preview = self.ai_patch_draft_preview_from_task_audit(task_audit_id)?;
        let ai = match self.ai_request(&preview.prompt, timeout_ms) {
            Ok(report) => report,
            Err(error) => {
                self.record_ai_patch_draft_failure(&preview, None, &error.to_string())?;
                return Err(AiPatchDraftError::Ai(error));
            }
        };

        self.finish_ai_patch_draft(preview, ai)
    }

    pub(crate) fn finish_ai_patch_draft(
        &self,
        preview: AiPatchDraftPreview,
        ai: AiExecutionReport,
    ) -> Result<AiPatchDraftReport, AiPatchDraftError> {
        let draft_markdown = match validate_ai_patch_draft_text(&ai.response.text) {
            Ok(draft_markdown) => draft_markdown,
            Err(error) => {
                self.record_ai_patch_draft_failure(&preview, Some(&ai), &error.to_string())?;
                return Err(error);
            }
        };
        let mut record = self.ai_patch_draft_base(&preview, AiPatchDraftStatus::Succeeded);
        record.provider_id = ai.response.provider_id.clone();
        record.model = ai.response.model.clone();
        record.protocol = ai.response.protocol.clone();
        record.ai_response_preview = Some(truncate_chars(&ai.response.text, 240));
        let record = AiPatchDraftStore::new(&self.root)
            .create(record, Some(&draft_markdown))
            .map_err(AiPatchDraftError::Store)?;

        Ok(AiPatchDraftReport {
            preview,
            ai,
            record,
        })
    }

    pub fn ai_patch_draft_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchDraftSummary>, AiPatchDraftStoreError> {
        AiPatchDraftStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_draft_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchDraftRecord, AiPatchDraftStoreError> {
        AiPatchDraftStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_audit(
        &self,
        version: &str,
        draft_id: &str,
    ) -> Result<AiPatchAuditReport, AiPatchAuditError> {
        let draft = AiPatchDraftStore::new(&self.root)
            .load(version, draft_id)
            .map_err(AiPatchAuditError::Draft)?;
        let mut requested_write_scope = Vec::new();
        let mut normalized_write_scope = Vec::new();
        let mut findings = Vec::new();

        if draft.status != AiPatchDraftStatus::Succeeded {
            findings.push(AiPatchAuditFinding {
                severity: AiPatchAuditSeverity::Error,
                kind: AiPatchAuditFindingKind::DraftNotSuccessful,
                message: "补丁草案状态不是成功，禁止作为候选补丁继续推进。".to_string(),
                path: None,
                task_id: None,
                task_title: None,
                worker_id: None,
            });
        }

        match draft.draft_file.as_ref() {
            Some(draft_file) => {
                let path = self.root.join(draft_file);
                match fs::read_to_string(&path) {
                    Ok(markdown) => {
                        requested_write_scope = extract_patch_audit_write_scope(&markdown);
                        let scope_report =
                            audit_patch_write_scope(&requested_write_scope, version)?;
                        normalized_write_scope = scope_report.normalized_write_scope;
                        findings.extend(scope_report.findings);
                    }
                    Err(source) => {
                        return Err(AiPatchAuditError::Io { path, source });
                    }
                }
            }
            None => findings.push(AiPatchAuditFinding {
                severity: AiPatchAuditSeverity::Error,
                kind: AiPatchAuditFindingKind::MissingDraftFile,
                message: "补丁草案记录缺少 Markdown 草案文件。".to_string(),
                path: None,
                task_id: None,
                task_title: None,
                worker_id: None,
            }),
        }

        let queue = match AgentWorkCoordinator::new(&self.root).status(version) {
            Ok(report) => Some(report),
            Err(AgentWorkError::MissingQueue { .. }) => {
                findings.push(AiPatchAuditFinding {
                    severity: AiPatchAuditSeverity::Warning,
                    kind: AiPatchAuditFindingKind::QueueUnavailable,
                    message: "当前版本没有协作任务板，本次只完成路径边界审计。".to_string(),
                    path: None,
                    task_id: None,
                    task_title: None,
                    worker_id: None,
                });
                None
            }
            Err(error) => return Err(AiPatchAuditError::WorkQueue(error)),
        };

        if let Some(queue_report) = queue.as_ref() {
            findings.extend(audit_patch_scope_conflicts(
                &normalized_write_scope,
                queue_report,
            ));
        }

        let active_conflict_count = findings
            .iter()
            .filter(|finding| finding.kind == AiPatchAuditFindingKind::ActiveConflict)
            .count();
        let status = if findings
            .iter()
            .any(|finding| finding.severity == AiPatchAuditSeverity::Error)
        {
            AiPatchAuditStatus::Failed
        } else {
            AiPatchAuditStatus::Passed
        };
        let finding_count = findings.len();
        let record = AiPatchAuditRecord {
            id: String::new(),
            version: draft.version.clone(),
            target_version: draft.target_version.clone(),
            draft_id: draft.id.clone(),
            created_at_unix_seconds: 0,
            status,
            requested_write_scope,
            normalized_write_scope,
            protected_roots: patch_audit_protected_roots(version)?,
            active_conflict_count,
            finding_count,
            findings,
            file: PathBuf::new(),
        };
        let record = AiPatchAuditStore::new(&self.root)
            .create(record)
            .map_err(AiPatchAuditError::Store)?;

        Ok(AiPatchAuditReport {
            draft,
            queue,
            record,
        })
    }

    pub fn ai_patch_audit_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchAuditSummary>, AiPatchAuditStoreError> {
        AiPatchAuditStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_audit_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchAuditRecord, AiPatchAuditStoreError> {
        AiPatchAuditStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_preview(
        &self,
        version: &str,
        audit_id: &str,
    ) -> Result<AiPatchPreviewReport, AiPatchPreviewError> {
        let audit = AiPatchAuditStore::new(&self.root)
            .load(version, audit_id)
            .map_err(AiPatchPreviewError::Audit)?;
        let draft = AiPatchDraftStore::new(&self.root)
            .load(version, &audit.draft_id)
            .map_err(AiPatchPreviewError::Draft)?;

        let mut status = AiPatchPreviewStatus::Previewed;
        let mut error = None;
        let mut code_block_count = 0;
        let mut changes = Vec::new();
        let mut draft_markdown = None;

        if audit.status != AiPatchAuditStatus::Passed {
            status = AiPatchPreviewStatus::Blocked;
            error = Some("补丁审计未通过，禁止生成可应用预演。".to_string());
        } else if draft.status != AiPatchDraftStatus::Succeeded {
            status = AiPatchPreviewStatus::Blocked;
            error = Some("补丁草案不是成功状态，禁止生成可应用预演。".to_string());
        } else if audit.normalized_write_scope.is_empty() {
            status = AiPatchPreviewStatus::Blocked;
            error = Some("补丁审计缺少规范化写入范围。".to_string());
        } else if let Some(draft_file) = draft.draft_file.as_ref() {
            let path = self.root.join(draft_file);
            let markdown = fs::read_to_string(&path).map_err(|source| AiPatchPreviewError::Io {
                path: path.clone(),
                source,
            })?;
            let code_blocks = extract_patch_preview_code_blocks(&markdown);
            code_block_count = code_blocks.len();
            if code_blocks.is_empty() {
                status = AiPatchPreviewStatus::Blocked;
                error = Some("补丁草案的代码草案章节缺少可预演代码块。".to_string());
            } else if code_blocks.len() < audit.normalized_write_scope.len() {
                status = AiPatchPreviewStatus::Blocked;
                error = Some("补丁草案代码块数量少于审计通过的写入范围数量。".to_string());
            } else {
                for (index, scope) in audit.normalized_write_scope.iter().enumerate() {
                    let block = &code_blocks[index];
                    changes.push(AiPatchPreviewChange {
                        path: scope.clone(),
                        code_block_index: index + 1,
                        language: block.language.clone(),
                        content_bytes: block.content.len(),
                        content_preview: truncate_chars(&block.content, 240),
                    });
                }
            }
            draft_markdown = Some(markdown);
        } else {
            status = AiPatchPreviewStatus::Blocked;
            error = Some("补丁草案记录缺少 Markdown 草案文件。".to_string());
        }

        let preview_markdown = build_ai_patch_preview_markdown(
            &audit,
            &draft,
            status,
            code_block_count,
            &changes,
            error.as_deref(),
            draft_markdown.as_deref(),
        );
        let record = AiPatchPreviewRecord {
            id: String::new(),
            version: audit.version.clone(),
            target_version: audit.target_version.clone(),
            audit_id: audit.id.clone(),
            draft_id: draft.id.clone(),
            created_at_unix_seconds: 0,
            status,
            normalized_write_scope: audit.normalized_write_scope.clone(),
            code_block_count,
            change_count: changes.len(),
            changes,
            preview_file: None,
            error,
            file: PathBuf::new(),
        };
        let record = AiPatchPreviewStore::new(&self.root)
            .create(record, Some(&preview_markdown))
            .map_err(AiPatchPreviewError::Store)?;

        Ok(AiPatchPreviewReport {
            audit,
            draft,
            record,
        })
    }

    pub fn ai_patch_preview_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchPreviewSummary>, AiPatchPreviewStoreError> {
        AiPatchPreviewStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_preview_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchPreviewRecord, AiPatchPreviewStoreError> {
        AiPatchPreviewStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_apply(
        &self,
        version: &str,
        preview_id: &str,
    ) -> Result<AiPatchApplicationReport, AiPatchApplicationError> {
        let preflight = self
            .preflight()
            .map_err(AiPatchApplicationError::Preflight)?;
        let preview = AiPatchPreviewStore::new(&self.root)
            .load(version, preview_id)
            .map_err(AiPatchApplicationError::Preview)?;
        let draft = AiPatchDraftStore::new(&self.root)
            .load(version, &preview.draft_id)
            .map_err(AiPatchApplicationError::Draft)?;

        let mut status = AiPatchApplicationStatus::Applied;
        let mut error = None;
        let mut files = Vec::new();
        let mut application_dir = None;
        let mut validation_checked_paths = Vec::new();
        let mut prepared_candidate_version = None;
        let mut candidate_version = preflight
            .candidate_version
            .clone()
            .unwrap_or_else(|| preview.target_version.clone());
        let application_id = make_patch_application_id(&self.root, version)?;

        if !preflight.open_errors.is_empty() {
            status = AiPatchApplicationStatus::Blocked;
            error = Some(format!(
                "当前稳定版本存在 {} 个未解决错误，禁止应用补丁预演。",
                preflight.open_errors.len()
            ));
        } else if preview.status != AiPatchPreviewStatus::Previewed {
            status = AiPatchApplicationStatus::Blocked;
            error = Some("补丁预演不是已预演状态，禁止应用到候选工作区。".to_string());
        } else if preview.changes.is_empty() {
            status = AiPatchApplicationStatus::Blocked;
            error = Some("补丁预演没有可应用变更。".to_string());
        } else {
            let draft_markdown = read_patch_draft_markdown(&self.root, &draft)
                .map_err(|(path, source)| AiPatchApplicationError::Io { path, source })?;
            let code_blocks = extract_patch_preview_code_blocks(&draft_markdown);
            let mut prepared_files = Vec::new();

            for change in &preview.changes {
                let block_index = change.code_block_index.saturating_sub(1);
                let Some(block) = code_blocks.get(block_index) else {
                    status = AiPatchApplicationStatus::Blocked;
                    error = Some(format!("预演变更 {} 缺少对应代码块。", change.path));
                    break;
                };
                let safe_path = match patch_application_safe_relative_path(&change.path) {
                    Ok(path) => path,
                    Err(path_error) => {
                        status = AiPatchApplicationStatus::Blocked;
                        error = Some(path_error.to_string());
                        break;
                    }
                };
                prepared_files.push((change.path.clone(), safe_path, block.content.clone()));
            }

            if status == AiPatchApplicationStatus::Applied {
                if preflight.candidate_version.is_none() {
                    let prepared = self
                        .supervisor
                        .prepare_next_version(&format!("应用补丁预演 {preview_id}"))
                        .map_err(AiPatchApplicationError::Evolution)?;
                    candidate_version = prepared.next_version.clone();
                    prepared_candidate_version = Some(prepared.next_version);
                }

                let major = version_major_key(&candidate_version)
                    .map_err(AiPatchApplicationError::Version)?;
                let relative_application_dir = PathBuf::from("workspaces")
                    .join(&major)
                    .join("source")
                    .join("patch-applications")
                    .join(&application_id);
                let absolute_application_dir = self.root.join(&relative_application_dir);
                fs::create_dir_all(&absolute_application_dir).map_err(|source| {
                    AiPatchApplicationError::Io {
                        path: absolute_application_dir.clone(),
                        source,
                    }
                })?;

                for (source_path, safe_path, contents) in prepared_files {
                    let relative_mirror_file = relative_application_dir.join(&safe_path);
                    let absolute_mirror_file = self.root.join(&relative_mirror_file);
                    if let Some(parent) = absolute_mirror_file.parent() {
                        fs::create_dir_all(parent).map_err(|source| {
                            AiPatchApplicationError::Io {
                                path: parent.to_path_buf(),
                                source,
                            }
                        })?;
                    }
                    fs::write(&absolute_mirror_file, &contents).map_err(|source| {
                        AiPatchApplicationError::Io {
                            path: absolute_mirror_file.clone(),
                            source,
                        }
                    })?;
                    files.push(AiPatchApplicationFile {
                        source_path,
                        mirror_file: relative_mirror_file,
                        content_bytes: contents.len(),
                    });
                }

                application_dir = Some(relative_application_dir);
                let validation = self
                    .supervisor
                    .verify_version(&candidate_version)
                    .map_err(AiPatchApplicationError::Forge)?;
                validation_checked_paths = validation.checked_paths;
            }
        }

        if status == AiPatchApplicationStatus::Blocked {
            files.clear();
        }

        let verification_commands = patch_application_verification_commands();
        let rollback_hint = format!(
            "如后续验证失败，执行 rollback \"补丁预演 {preview_id} 应用失败\" 并保留本记录。"
        );
        let report_markdown = build_ai_patch_application_markdown(
            &preview,
            &candidate_version,
            status,
            application_dir.as_ref(),
            &files,
            &verification_commands,
            &rollback_hint,
            error.as_deref(),
        );
        let record = AiPatchApplicationRecord {
            id: application_id,
            version: version.to_string(),
            candidate_version,
            preview_id: preview.id.clone(),
            audit_id: preview.audit_id.clone(),
            draft_id: preview.draft_id.clone(),
            created_at_unix_seconds: 0,
            status,
            application_dir,
            applied_file_count: files.len(),
            files,
            validation_checked_paths,
            verification_commands,
            verification_runs: Vec::new(),
            verification_status: AiPatchVerificationStatus::Pending,
            verified_at_unix_seconds: None,
            rollback_hint,
            report_file: None,
            error,
            file: PathBuf::new(),
        };
        let record = AiPatchApplicationStore::new(&self.root)
            .create(record, Some(&report_markdown))
            .map_err(AiPatchApplicationError::Store)?;

        Ok(AiPatchApplicationReport {
            preview,
            draft,
            prepared_candidate_version,
            record,
        })
    }

    pub fn ai_patch_application_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchApplicationSummary>, AiPatchApplicationStoreError> {
        AiPatchApplicationStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_application_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchApplicationRecord, AiPatchApplicationStoreError> {
        AiPatchApplicationStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_verify(
        &self,
        version: &str,
        id: &str,
        timeout_ms: u64,
    ) -> Result<AiPatchVerificationReport, AiPatchVerificationError> {
        let root = self.root.clone();
        self.ai_patch_verify_with_runner(version, id, timeout_ms, |spec, timeout_ms| {
            run_patch_verification_command(&root, spec, timeout_ms)
        })
    }

    pub(crate) fn ai_patch_verify_with_runner<F>(
        &self,
        version: &str,
        id: &str,
        timeout_ms: u64,
        mut runner: F,
    ) -> Result<AiPatchVerificationReport, AiPatchVerificationError>
    where
        F: FnMut(
            &AiPatchVerificationCommandSpec,
            u64,
        ) -> Result<AiPatchVerificationCommandRecord, AiPatchVerificationError>,
    {
        let store = AiPatchApplicationStore::new(&self.root);
        let mut record = store
            .load(version, id)
            .map_err(AiPatchVerificationError::Store)?;
        let mut executed_count = 0;

        if record.status != AiPatchApplicationStatus::Applied {
            record.verification_status = AiPatchVerificationStatus::Skipped;
            record.verified_at_unix_seconds = Some(current_unix_seconds());
        } else {
            let commands = if record.verification_commands.is_empty() {
                patch_application_verification_commands()
            } else {
                record.verification_commands.clone()
            };
            let specs = match patch_application_verification_specs(&commands) {
                Ok(specs) => specs,
                Err(AiPatchVerificationError::UnsupportedCommand(command)) => {
                    record
                        .verification_runs
                        .push(unsupported_patch_verification_run(&command, timeout_ms));
                    record.verification_status = AiPatchVerificationStatus::Failed;
                    record.verified_at_unix_seconds = Some(current_unix_seconds());
                    let markdown = build_ai_patch_application_record_markdown(&record);
                    store
                        .update(record, Some(&markdown))
                        .map_err(AiPatchVerificationError::Store)?;
                    return Err(AiPatchVerificationError::UnsupportedCommand(command));
                }
                Err(error) => return Err(error),
            };
            let mut runs = Vec::new();
            for spec in specs {
                runs.push(runner(&spec, timeout_ms)?);
            }
            executed_count = runs.len();
            let passed = runs.iter().all(patch_verification_run_passed);
            record.verification_runs.extend(runs);
            record.verification_status = if passed {
                AiPatchVerificationStatus::Passed
            } else {
                AiPatchVerificationStatus::Failed
            };
            record.verified_at_unix_seconds = Some(current_unix_seconds());
        }

        let markdown = build_ai_patch_application_record_markdown(&record);
        let status = record.verification_status;
        let record = store
            .update(record, Some(&markdown))
            .map_err(AiPatchVerificationError::Store)?;

        Ok(AiPatchVerificationReport {
            record,
            executed_count,
            status,
        })
    }

    pub fn ai_patch_source_plan(
        &self,
        version: &str,
        application_id: &str,
    ) -> Result<AiPatchSourcePlanReport, AiPatchSourcePlanError> {
        let application = AiPatchApplicationStore::new(&self.root)
            .load(version, application_id)
            .map_err(AiPatchSourcePlanError::Application)?;
        let plan_id = make_patch_source_plan_id(&self.root, version)?;
        let mut status = AiPatchSourcePlanStatus::Prepared;
        let mut error = None;
        let mut files = Vec::new();
        let mut backups = Vec::new();
        let major = version_major_key(version).map_err(|error| {
            AiPatchSourcePlanError::Store(AiPatchSourcePlanStoreError::Version(error))
        })?;
        let relative_plan_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-source-plans")
            .join(&plan_id);

        if application.status != AiPatchApplicationStatus::Applied {
            status = AiPatchSourcePlanStatus::Blocked;
            error = Some("候选应用不是已应用状态，禁止准备源码覆盖。".to_string());
        } else if application.verification_status != AiPatchVerificationStatus::Passed {
            status = AiPatchSourcePlanStatus::Blocked;
            error = Some("候选应用验证未通过，禁止准备源码覆盖。".to_string());
        } else if application.files.is_empty() {
            status = AiPatchSourcePlanStatus::Blocked;
            error = Some("候选应用没有可覆盖文件。".to_string());
        } else {
            for file in &application.files {
                let safe_path = match patch_application_safe_relative_path(&file.source_path) {
                    Ok(path) => path,
                    Err(path_error) => {
                        status = AiPatchSourcePlanStatus::Blocked;
                        error = Some(path_error.to_string());
                        break;
                    }
                };
                let mirror_path = self.root.join(&file.mirror_file);
                let new_contents = match fs::read(&mirror_path) {
                    Ok(contents) => contents,
                    Err(source) => {
                        status = AiPatchSourcePlanStatus::Blocked;
                        error = Some(format!(
                            "候选镜像文件不可读 {}：{}",
                            file.mirror_file.display(),
                            source
                        ));
                        break;
                    }
                };
                let target_file = safe_path.clone();
                let target_path = self.root.join(&target_file);
                let target_exists = target_path.exists();
                if target_exists && !target_path.is_file() {
                    status = AiPatchSourcePlanStatus::Blocked;
                    error = Some(format!(
                        "目标路径不是文件，禁止覆盖：{}",
                        target_file.display()
                    ));
                    break;
                }
                let original_contents = if target_exists {
                    match fs::read(&target_path) {
                        Ok(contents) => contents,
                        Err(source) => {
                            status = AiPatchSourcePlanStatus::Blocked;
                            error = Some(format!(
                                "目标文件不可读 {}：{}",
                                target_file.display(),
                                source
                            ));
                            break;
                        }
                    }
                } else {
                    Vec::new()
                };
                let rollback_backup_file = if target_exists {
                    Some(relative_plan_dir.join("rollback").join(&safe_path))
                } else {
                    None
                };
                let diff_summary = if target_exists && original_contents == new_contents {
                    format!(
                        "目标文件 {} 内容一致，覆盖不会改变字节。",
                        target_file.display()
                    )
                } else if target_exists {
                    format!(
                        "目标文件 {} 将被覆盖，原始字节 {}，新字节 {}。",
                        target_file.display(),
                        original_contents.len(),
                        new_contents.len()
                    )
                } else {
                    format!(
                        "目标文件 {} 将被创建，新字节 {}。",
                        target_file.display(),
                        new_contents.len()
                    )
                };
                let rollback_action = if let Some(backup_file) = rollback_backup_file.as_ref() {
                    format!(
                        "如覆盖失败，从 {} 恢复到 {}。",
                        backup_file.display(),
                        target_file.display()
                    )
                } else {
                    format!("如覆盖失败，删除新建文件 {}。", target_file.display())
                };
                if let Some(backup_file) = rollback_backup_file.as_ref() {
                    backups.push((backup_file.clone(), original_contents.clone()));
                }
                files.push(AiPatchSourcePlanFile {
                    source_path: file.source_path.clone(),
                    mirror_file: file.mirror_file.clone(),
                    target_file,
                    target_exists,
                    original_bytes: original_contents.len(),
                    new_bytes: new_contents.len(),
                    rollback_backup_file,
                    diff_summary,
                    rollback_action,
                });
            }
        }

        let rollback_steps = if status == AiPatchSourcePlanStatus::Prepared {
            let plan_path = self.root.join(&relative_plan_dir);
            fs::create_dir_all(&plan_path).map_err(|source| AiPatchSourcePlanError::Io {
                path: plan_path,
                source,
            })?;
            for (backup_file, contents) in &backups {
                let backup_path = self.root.join(backup_file);
                if let Some(parent) = backup_path.parent() {
                    fs::create_dir_all(parent).map_err(|source| AiPatchSourcePlanError::Io {
                        path: parent.to_path_buf(),
                        source,
                    })?;
                }
                fs::write(&backup_path, contents).map_err(|source| AiPatchSourcePlanError::Io {
                    path: backup_path,
                    source,
                })?;
            }
            files
                .iter()
                .map(|file| file.rollback_action.clone())
                .collect()
        } else {
            files.clear();
            Vec::new()
        };

        let record = AiPatchSourcePlanRecord {
            id: plan_id,
            version: version.to_string(),
            application_id: application.id.clone(),
            candidate_version: application.candidate_version.clone(),
            preview_id: application.preview_id.clone(),
            audit_id: application.audit_id.clone(),
            draft_id: application.draft_id.clone(),
            created_at_unix_seconds: 0,
            status,
            prerequisites: patch_source_plan_prerequisites(),
            files,
            rollback_steps,
            plan_dir: if status == AiPatchSourcePlanStatus::Prepared {
                Some(relative_plan_dir)
            } else {
                None
            },
            report_file: None,
            error,
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_plan_markdown(&record);
        let record = AiPatchSourcePlanStore::new(&self.root)
            .create(record, Some(&markdown))
            .map_err(AiPatchSourcePlanError::Store)?;

        Ok(AiPatchSourcePlanReport {
            application,
            record,
        })
    }

    pub fn ai_patch_source_plan_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourcePlanSummary>, AiPatchSourcePlanStoreError> {
        AiPatchSourcePlanStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_plan_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourcePlanRecord, AiPatchSourcePlanStoreError> {
        AiPatchSourcePlanStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_execute(
        &self,
        version: &str,
        source_plan_id: &str,
        timeout_ms: u64,
    ) -> Result<AiPatchSourceExecutionReport, AiPatchSourceExecutionError> {
        let root = self.root.clone();
        self.ai_patch_source_execute_with_runner(
            version,
            source_plan_id,
            timeout_ms,
            |spec, timeout_ms| run_patch_verification_command(&root, spec, timeout_ms),
        )
    }

    pub(crate) fn ai_patch_source_execute_with_runner<F>(
        &self,
        version: &str,
        source_plan_id: &str,
        timeout_ms: u64,
        mut runner: F,
    ) -> Result<AiPatchSourceExecutionReport, AiPatchSourceExecutionError>
    where
        F: FnMut(
            &AiPatchVerificationCommandSpec,
            u64,
        ) -> Result<AiPatchVerificationCommandRecord, AiPatchVerificationError>,
    {
        let source_plan = AiPatchSourcePlanStore::new(&self.root)
            .load(version, source_plan_id)
            .map_err(AiPatchSourceExecutionError::SourcePlan)?;
        let execution_id = make_patch_source_execution_id(&self.root, version)?;
        let major = version_major_key(version).map_err(|error| {
            AiPatchSourceExecutionError::Store(AiPatchSourceExecutionStoreError::Version(error))
        })?;
        let relative_execution_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-source-executions")
            .join(&execution_id);

        let mut status = AiPatchSourceExecutionStatus::Applied;
        let mut error = None;
        let mut files = Vec::new();
        let mut prepared_files = Vec::new();
        let mut verification_runs = Vec::new();
        let mut verification_status = AiPatchVerificationStatus::Skipped;
        let mut rollback_performed = false;
        let mut rollback_steps = Vec::new();

        if source_plan.status != AiPatchSourcePlanStatus::Prepared {
            status = AiPatchSourceExecutionStatus::Blocked;
            error = Some("源码覆盖准备记录不是已准备状态，禁止执行覆盖。".to_string());
        } else if source_plan.files.is_empty() {
            status = AiPatchSourceExecutionStatus::Blocked;
            error = Some("源码覆盖准备记录没有可执行文件。".to_string());
        } else {
            for file in &source_plan.files {
                let prepared =
                    match self.prepare_patch_source_execution_file(file, &relative_execution_dir) {
                        Ok(prepared) => prepared,
                        Err(reason) => {
                            status = AiPatchSourceExecutionStatus::Blocked;
                            error = Some(reason);
                            break;
                        }
                    };
                files.push(prepared.record_file.clone());
                prepared_files.push(prepared);
            }
        }

        if status == AiPatchSourceExecutionStatus::Applied {
            let execution_dir = self.root.join(&relative_execution_dir);
            fs::create_dir_all(&execution_dir).map_err(|source| {
                AiPatchSourceExecutionError::Io {
                    path: execution_dir.clone(),
                    source,
                }
            })?;

            for prepared in &prepared_files {
                if let Some(backup_file) = prepared.record_file.execution_backup_file.as_ref() {
                    let backup_path = self.root.join(backup_file);
                    if let Some(parent) = backup_path.parent() {
                        fs::create_dir_all(parent).map_err(|source| {
                            AiPatchSourceExecutionError::Io {
                                path: parent.to_path_buf(),
                                source,
                            }
                        })?;
                    }
                    fs::write(&backup_path, &prepared.before_contents).map_err(|source| {
                        AiPatchSourceExecutionError::Io {
                            path: backup_path,
                            source,
                        }
                    })?;
                }
            }

            let mut written_count = 0;
            for prepared in &prepared_files {
                if let Some(parent) = prepared.target_path.parent() {
                    if let Err(source) = fs::create_dir_all(parent) {
                        status = AiPatchSourceExecutionStatus::RolledBack;
                        verification_status = AiPatchVerificationStatus::Failed;
                        error = Some(format!(
                            "源码覆盖创建父目录失败 {}：{}",
                            parent.display(),
                            source
                        ));
                        break;
                    }
                }
                if let Err(source) = fs::write(&prepared.target_path, &prepared.new_contents) {
                    status = AiPatchSourceExecutionStatus::RolledBack;
                    verification_status = AiPatchVerificationStatus::Failed;
                    error = Some(format!(
                        "源码覆盖写入失败 {}：{}",
                        prepared.record_file.target_file.display(),
                        source
                    ));
                    break;
                }
                written_count += 1;
            }

            if status == AiPatchSourceExecutionStatus::RolledBack {
                rollback_performed = true;
                rollback_steps = rollback_patch_source_execution(&prepared_files[..written_count]);
            } else {
                let commands = patch_application_verification_commands();
                let specs = patch_application_verification_specs(&commands)
                    .map_err(AiPatchSourceExecutionError::Verification)?;
                for spec in specs {
                    let run = runner(&spec, timeout_ms)
                        .map_err(AiPatchSourceExecutionError::Verification)?;
                    let failed = run.status != AiPatchVerificationStatus::Passed;
                    verification_runs.push(run);
                    if failed {
                        break;
                    }
                }
                verification_status = if verification_runs
                    .iter()
                    .all(|run| run.status == AiPatchVerificationStatus::Passed)
                {
                    AiPatchVerificationStatus::Passed
                } else {
                    AiPatchVerificationStatus::Failed
                };
                if verification_status != AiPatchVerificationStatus::Passed {
                    status = AiPatchSourceExecutionStatus::RolledBack;
                    rollback_performed = true;
                    rollback_steps = rollback_patch_source_execution(&prepared_files);
                    error = Some("源码覆盖后验证未通过，已按执行级备份回滚。".to_string());
                }
            }
        } else {
            files.clear();
        }

        let record = AiPatchSourceExecutionRecord {
            id: execution_id,
            version: version.to_string(),
            source_plan_id: source_plan.id.clone(),
            application_id: source_plan.application_id.clone(),
            candidate_version: source_plan.candidate_version.clone(),
            preview_id: source_plan.preview_id.clone(),
            audit_id: source_plan.audit_id.clone(),
            draft_id: source_plan.draft_id.clone(),
            created_at_unix_seconds: 0,
            status,
            execution_dir: if status == AiPatchSourceExecutionStatus::Blocked {
                None
            } else {
                Some(relative_execution_dir)
            },
            files,
            verification_commands: if status == AiPatchSourceExecutionStatus::Blocked {
                Vec::new()
            } else {
                patch_application_verification_commands()
            },
            verification_runs,
            verification_status,
            rollback_performed,
            rollback_steps,
            report_file: None,
            error,
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_execution_markdown(&record);
        let record = AiPatchSourceExecutionStore::new(&self.root)
            .create(record, Some(&markdown))
            .map_err(AiPatchSourceExecutionError::Store)?;

        Ok(AiPatchSourceExecutionReport {
            source_plan,
            record,
        })
    }

    pub fn ai_patch_source_execution_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceExecutionSummary>, AiPatchSourceExecutionStoreError> {
        AiPatchSourceExecutionStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_execution_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourceExecutionRecord, AiPatchSourceExecutionStoreError> {
        AiPatchSourceExecutionStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_promotion(
        &self,
        version: &str,
        source_execution_id: &str,
    ) -> Result<AiPatchSourcePromotionReport, AiPatchSourcePromotionError> {
        let source_execution = AiPatchSourceExecutionStore::new(&self.root)
            .load(version, source_execution_id)
            .map_err(AiPatchSourcePromotionError::SourceExecution)?;
        let next_candidate_version =
            next_version_after(version).map_err(AiPatchSourcePromotionError::Version)?;
        let next_candidate_goal = format!(
            "基于源码覆盖执行 {} 生成 {} 候选版本，并保留验证结果、提交信息和候选生成线索。",
            source_execution.id, next_candidate_version
        );

        let mut status = AiPatchSourcePromotionStatus::Ready;
        let mut readiness_checks = Vec::new();
        let mut error = None;

        if source_execution.status == AiPatchSourceExecutionStatus::Applied {
            readiness_checks.push("源码覆盖执行状态为已覆盖。".to_string());
        } else {
            status = AiPatchSourcePromotionStatus::Blocked;
            error = Some(format!(
                "源码覆盖执行未成功，当前状态为 {}，禁止进入版本提升衔接。",
                source_execution.status
            ));
        }

        if status == AiPatchSourcePromotionStatus::Ready {
            if source_execution.verification_status == AiPatchVerificationStatus::Passed {
                readiness_checks.push("源码覆盖执行验证状态为通过。".to_string());
            } else {
                status = AiPatchSourcePromotionStatus::Blocked;
                error = Some(format!(
                    "源码覆盖执行验证未通过，当前验证状态为 {}，禁止进入版本提升衔接。",
                    source_execution.verification_status
                ));
            }
        }

        if status == AiPatchSourcePromotionStatus::Ready {
            if source_execution.rollback_performed {
                status = AiPatchSourcePromotionStatus::Blocked;
                error = Some("源码覆盖执行发生过回滚，禁止进入版本提升衔接。".to_string());
            } else {
                readiness_checks.push("源码覆盖执行未发生回滚。".to_string());
            }
        }

        if status == AiPatchSourcePromotionStatus::Ready {
            if source_execution.files.is_empty() {
                status = AiPatchSourcePromotionStatus::Blocked;
                error = Some("源码覆盖执行没有覆盖文件，禁止进入版本提升衔接。".to_string());
            } else {
                readiness_checks.push(format!(
                    "源码覆盖执行包含 {} 个覆盖文件。",
                    source_execution.files.len()
                ));
            }
        }

        if status == AiPatchSourcePromotionStatus::Ready {
            if source_execution.verification_runs.is_empty() {
                status = AiPatchSourcePromotionStatus::Blocked;
                error = Some("源码覆盖执行缺少验证运行记录，禁止进入版本提升衔接。".to_string());
            } else if source_execution
                .verification_runs
                .iter()
                .any(|run| run.status != AiPatchVerificationStatus::Passed)
            {
                status = AiPatchSourcePromotionStatus::Blocked;
                error =
                    Some("源码覆盖执行存在未通过的验证运行，禁止进入版本提升衔接。".to_string());
            } else {
                readiness_checks.push(format!(
                    "源码覆盖执行包含 {} 条已通过验证运行。",
                    source_execution.verification_runs.len()
                ));
            }
        }

        let suggested_commit_title = if status == AiPatchSourcePromotionStatus::Ready {
            Some(format!(
                "feat(agent): {} 衔接源码覆盖执行",
                next_candidate_version
            ))
        } else {
            None
        };
        let suggested_commit_body = suggested_commit_title.as_ref().map(|_| {
            format!(
                "关联源码覆盖执行 {}。\n\n覆盖文件数量：{}。\n验证运行数量：{}。\n下一候选目标：{}",
                source_execution.id,
                source_execution.files.len(),
                source_execution.verification_runs.len(),
                next_candidate_goal
            )
        });
        let record = AiPatchSourcePromotionRecord {
            id: String::new(),
            version: version.to_string(),
            source_execution_id: source_execution.id.clone(),
            source_plan_id: source_execution.source_plan_id.clone(),
            application_id: source_execution.application_id.clone(),
            candidate_version: source_execution.candidate_version.clone(),
            preview_id: source_execution.preview_id.clone(),
            audit_id: source_execution.audit_id.clone(),
            draft_id: source_execution.draft_id.clone(),
            created_at_unix_seconds: 0,
            status,
            next_candidate_version,
            next_candidate_goal,
            suggested_commit_title,
            suggested_commit_body,
            verification_status: source_execution.verification_status,
            verification_run_count: source_execution.verification_runs.len(),
            verification_commands: source_execution
                .verification_runs
                .iter()
                .map(|run| run.command.clone())
                .collect(),
            file_count: source_execution.files.len(),
            changed_files: source_execution
                .files
                .iter()
                .map(|file| file.source_path.clone())
                .collect(),
            rollback_performed: source_execution.rollback_performed,
            readiness_checks,
            report_file: None,
            error,
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_promotion_markdown(&record);
        let record = AiPatchSourcePromotionStore::new(&self.root)
            .create(record, Some(&markdown))
            .map_err(AiPatchSourcePromotionError::Store)?;

        Ok(AiPatchSourcePromotionReport {
            source_execution,
            record,
        })
    }

    pub fn ai_patch_source_promotion_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourcePromotionSummary>, AiPatchSourcePromotionStoreError> {
        AiPatchSourcePromotionStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_promotion_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourcePromotionRecord, AiPatchSourcePromotionStoreError> {
        AiPatchSourcePromotionStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_candidate(
        &self,
        version: &str,
        promotion_id: &str,
    ) -> Result<AiPatchSourceCandidateReport, AiPatchSourceCandidateError> {
        let promotion = AiPatchSourcePromotionStore::new(&self.root)
            .load(version, promotion_id)
            .map_err(AiPatchSourceCandidateError::Promotion)?;
        let state_before =
            ForgeState::load(&self.root).map_err(AiPatchSourceCandidateError::State)?;
        let expected_next =
            next_version_after(version).map_err(AiPatchSourceCandidateError::Version)?;

        let mut status = AiPatchSourceCandidateStatus::Prepared;
        let mut readiness_checks = Vec::new();
        let mut error = None;
        let mut candidate_checked_path_count = 0;
        let mut created_path_count = 0;
        let mut existing_path_count = 0;
        let mut candidate_workspace = None;
        let mut state_after = state_before.clone();

        if promotion.status != AiPatchSourcePromotionStatus::Ready {
            status = AiPatchSourceCandidateStatus::Blocked;
            error = Some(format!(
                "源码覆盖提升衔接记录未就绪，当前状态为 {}。",
                promotion.status
            ));
        } else {
            readiness_checks.push("源码覆盖提升衔接记录状态为已就绪。".to_string());
        }

        if status != AiPatchSourceCandidateStatus::Blocked {
            if state_before.current_version != version {
                status = AiPatchSourceCandidateStatus::Blocked;
                error = Some(format!(
                    "当前稳定版本为 {}，与请求版本 {} 不一致，禁止准备候选。",
                    state_before.current_version, version
                ));
            } else {
                readiness_checks.push("当前稳定版本与请求版本一致。".to_string());
            }
        }

        if status != AiPatchSourceCandidateStatus::Blocked {
            if promotion.next_candidate_version != expected_next {
                status = AiPatchSourceCandidateStatus::Blocked;
                error = Some(format!(
                    "提升衔接记录的下一候选版本为 {}，但当前版本规则要求 {}。",
                    promotion.next_candidate_version, expected_next
                ));
            } else {
                readiness_checks.push("下一候选版本符合 patch 递增规则。".to_string());
            }
        }

        if status != AiPatchSourceCandidateStatus::Blocked {
            let open_errors = ErrorArchive::new(&self.root)
                .list_run_errors(version, ErrorListQuery::open(1))
                .map_err(AiPatchSourceCandidateError::ErrorArchive)?;
            if let Some(open_error) = open_errors.first() {
                status = AiPatchSourceCandidateStatus::Blocked;
                error = Some(format!(
                    "版本 {version} 存在未解决错误 {}，禁止准备候选。",
                    open_error.run_id
                ));
            } else {
                readiness_checks.push("当前版本没有开放错误。".to_string());
            }
        }

        if status != AiPatchSourceCandidateStatus::Blocked {
            if let Some(existing_candidate) = state_before.candidate_version.as_deref() {
                if state_before.status == "candidate_prepared"
                    && existing_candidate == promotion.next_candidate_version
                {
                    match self.supervisor.verify_version(existing_candidate) {
                        Ok(validation) => {
                            status = AiPatchSourceCandidateStatus::Reused;
                            candidate_checked_path_count = validation.checked_paths.len();
                            candidate_workspace = state_before.candidate_workspace.clone();
                            readiness_checks
                                .push("目标候选版本已存在，已复用并完成布局验证。".to_string());
                        }
                        Err(source) => {
                            status = AiPatchSourceCandidateStatus::Blocked;
                            error = Some(format!("已有候选版本布局验证失败：{source}"));
                        }
                    }
                } else {
                    status = AiPatchSourceCandidateStatus::Blocked;
                    error = Some(format!(
                        "当前已有候选版本 {}，状态为 {}，与提升衔接目标不一致。",
                        existing_candidate, state_before.status
                    ));
                }
            } else {
                match self
                    .supervisor
                    .prepare_next_version(&promotion.next_candidate_goal)
                {
                    Ok(evolution) => {
                        if evolution.next_version == promotion.next_candidate_version {
                            status = AiPatchSourceCandidateStatus::Prepared;
                            candidate_checked_path_count =
                                evolution.candidate_validation.checked_paths.len();
                            created_path_count = evolution.created_paths.len();
                            existing_path_count = evolution.existing_paths.len();
                            candidate_workspace = evolution
                                .state
                                .candidate_workspace
                                .clone()
                                .or_else(|| Some(evolution.workspace.display().to_string()));
                            state_after = evolution.state;
                            readiness_checks.push("已调用版本状态机生成下一候选版本。".to_string());
                        } else {
                            status = AiPatchSourceCandidateStatus::Blocked;
                            error = Some(format!(
                                "版本状态机生成的候选版本为 {}，与提升衔接目标 {} 不一致。",
                                evolution.next_version, promotion.next_candidate_version
                            ));
                        }
                    }
                    Err(source) => {
                        status = AiPatchSourceCandidateStatus::Blocked;
                        error = Some(format!("候选版本生成失败：{source}"));
                        state_after =
                            ForgeState::load(&self.root).unwrap_or_else(|_| state_before.clone());
                    }
                }
            }
        }

        if status == AiPatchSourceCandidateStatus::Blocked {
            state_after = ForgeState::load(&self.root).unwrap_or_else(|_| state_before.clone());
        }

        let follow_up_commands = if status == AiPatchSourceCandidateStatus::Blocked {
            vec![
                "cargo run -- errors --current --open --limit 5".to_string(),
                "cargo run -- agent-patch-source-candidate PROMOTION_ID".to_string(),
            ]
        } else {
            vec![
                "cargo run -- validate".to_string(),
                "cargo run -- preflight".to_string(),
                "cargo run -- cycle".to_string(),
            ]
        };
        let record = AiPatchSourceCandidateRecord {
            id: String::new(),
            version: version.to_string(),
            promotion_id: promotion.id.clone(),
            source_execution_id: promotion.source_execution_id.clone(),
            source_plan_id: promotion.source_plan_id.clone(),
            application_id: promotion.application_id.clone(),
            candidate_version: promotion.next_candidate_version.clone(),
            candidate_goal: promotion.next_candidate_goal.clone(),
            created_at_unix_seconds: 0,
            status,
            stable_version_before: state_before.current_version.clone(),
            state_status_before: state_before.status.clone(),
            candidate_version_before: state_before.candidate_version.clone(),
            stable_version_after: state_after.current_version.clone(),
            state_status_after: state_after.status.clone(),
            candidate_version_after: state_after.candidate_version.clone(),
            candidate_workspace,
            candidate_checked_path_count,
            created_path_count,
            existing_path_count,
            readiness_checks,
            follow_up_commands,
            report_file: None,
            error,
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_candidate_markdown(&record);
        let record = AiPatchSourceCandidateStore::new(&self.root)
            .create(record, Some(&markdown))
            .map_err(AiPatchSourceCandidateError::Store)?;

        Ok(AiPatchSourceCandidateReport { promotion, record })
    }

    pub fn ai_patch_source_candidate_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceCandidateSummary>, AiPatchSourceCandidateStoreError> {
        AiPatchSourceCandidateStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_candidate_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourceCandidateRecord, AiPatchSourceCandidateStoreError> {
        AiPatchSourceCandidateStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_cycle(
        &self,
        version: &str,
        candidate_record_id: &str,
    ) -> Result<AiPatchSourceCycleReport, AiPatchSourceCycleError> {
        let candidate = AiPatchSourceCandidateStore::new(&self.root)
            .load(version, candidate_record_id)
            .map_err(AiPatchSourceCycleError::Candidate)?;
        let state_before = ForgeState::load(&self.root).map_err(AiPatchSourceCycleError::State)?;

        let mut status = AiPatchSourceCycleStatus::Blocked;
        let mut cycle_result = None;
        let mut failure = None;
        let mut readiness_checks = Vec::new();
        let mut error = None;
        let mut preflight_current_checked_path_count = 0;
        let mut preflight_candidate_checked_path_count = 0;
        let mut preflight_can_advance = false;
        let mut open_error_count = 0;
        let mut cycle_candidate_checked_path_count = 0;
        let mut state_after = state_before.clone();

        match candidate.status {
            AiPatchSourceCandidateStatus::Prepared | AiPatchSourceCandidateStatus::Reused => {
                readiness_checks.push("候选准备记录状态允许进入 cycle。".to_string());
            }
            AiPatchSourceCandidateStatus::Blocked => {
                error = Some("候选准备记录未准备完成，禁止执行 cycle。".to_string());
            }
        }

        if error.is_none() {
            if state_before.current_version != version {
                error = Some(format!(
                    "当前稳定版本为 {}，与请求版本 {} 不一致，禁止执行候选 cycle。",
                    state_before.current_version, version
                ));
            } else {
                readiness_checks.push("当前稳定版本与候选准备记录版本一致。".to_string());
            }
        }

        if error.is_none() {
            if state_before.status != "candidate_prepared" {
                error = Some(format!(
                    "状态文件当前状态为 {}，不是 candidate_prepared，禁止执行候选 cycle。",
                    state_before.status
                ));
            } else {
                readiness_checks.push("状态文件处于候选准备完成状态。".to_string());
            }
        }

        if error.is_none() {
            if state_before.candidate_version.as_deref()
                != Some(candidate.candidate_version.as_str())
            {
                error = Some(format!(
                    "状态文件候选版本为 {}，与候选准备记录 {} 不一致。",
                    state_before.candidate_version.as_deref().unwrap_or("无"),
                    candidate.candidate_version
                ));
            } else {
                readiness_checks.push("状态文件候选版本与候选准备记录一致。".to_string());
            }
        }

        if error.is_none() {
            match self.preflight() {
                Ok(preflight) => {
                    preflight_current_checked_path_count = preflight.checked_paths.len();
                    preflight_candidate_checked_path_count =
                        preflight.candidate_checked_paths.len();
                    preflight_can_advance = preflight.can_advance;
                    open_error_count = preflight.open_errors.len();
                    if preflight.can_advance {
                        readiness_checks
                            .push("cycle 前置检查通过，当前版本没有开放错误。".to_string());
                    } else if let Some(open_error) = preflight.open_errors.first() {
                        error = Some(format!(
                            "前置检查发现未解决错误 {}，禁止执行候选 cycle。",
                            open_error.run_id
                        ));
                    } else {
                        error = Some("前置检查未允许继续进化，禁止执行候选 cycle。".to_string());
                    }
                }
                Err(source) => {
                    error = Some(format!("cycle 前置检查失败：{source}"));
                }
            }
        }

        if error.is_none() {
            match self.supervisor.run_candidate_cycle() {
                Ok(cycle) => {
                    cycle_candidate_checked_path_count = cycle
                        .candidate_validation
                        .as_ref()
                        .map(|validation| validation.checked_paths.len())
                        .unwrap_or(0);
                    state_after = cycle.state.clone();
                    match cycle.result {
                        CycleResult::Promoted => {
                            status = AiPatchSourceCycleStatus::Promoted;
                            cycle_result = Some(AiPatchSourceCycleResult::Promoted);
                            readiness_checks
                                .push("候选版本验证通过并已提升为稳定版本。".to_string());
                        }
                        CycleResult::RolledBack => {
                            status = AiPatchSourceCycleStatus::RolledBack;
                            cycle_result = Some(AiPatchSourceCycleResult::RolledBack);
                            failure = cycle.failure.clone();
                            error = cycle.failure.clone();
                            readiness_checks
                                .push("候选版本验证失败，状态机已执行回滚。".to_string());
                        }
                    }
                }
                Err(source) => {
                    error = Some(format!("候选 cycle 执行失败：{source}"));
                    state_after =
                        ForgeState::load(&self.root).unwrap_or_else(|_| state_before.clone());
                }
            }
        }

        if error.is_some() && status == AiPatchSourceCycleStatus::Blocked {
            state_after = ForgeState::load(&self.root).unwrap_or_else(|_| state_before.clone());
        }

        let follow_up_commands = match status {
            AiPatchSourceCycleStatus::Promoted => vec![
                "cargo run -- preflight".to_string(),
                "cargo run -- memory-compact --current --keep 5".to_string(),
            ],
            AiPatchSourceCycleStatus::RolledBack => vec![
                "cargo run -- preflight".to_string(),
                "cargo run -- errors --current --open --limit 5".to_string(),
                format!(
                    "cargo run -- agent-patch-source-candidate --version {} {}",
                    candidate.version, candidate.promotion_id
                ),
            ],
            AiPatchSourceCycleStatus::Blocked => vec![
                "cargo run -- preflight".to_string(),
                format!(
                    "cargo run -- agent-patch-source-cycle --version {version} {candidate_record_id}"
                ),
            ],
        };

        let record = AiPatchSourceCycleRecord {
            id: String::new(),
            version: version.to_string(),
            candidate_record_id: candidate.id.clone(),
            promotion_id: candidate.promotion_id.clone(),
            source_execution_id: candidate.source_execution_id.clone(),
            candidate_version: candidate.candidate_version.clone(),
            created_at_unix_seconds: 0,
            status,
            cycle_result,
            stable_version_before: state_before.current_version.clone(),
            state_status_before: state_before.status.clone(),
            candidate_version_before: state_before.candidate_version.clone(),
            stable_version_after: state_after.current_version.clone(),
            state_status_after: state_after.status.clone(),
            candidate_version_after: state_after.candidate_version.clone(),
            preflight_current_checked_path_count,
            preflight_candidate_checked_path_count,
            preflight_can_advance,
            open_error_count,
            cycle_candidate_checked_path_count,
            failure,
            readiness_checks,
            follow_up_commands,
            report_file: None,
            error,
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_cycle_markdown(&record);
        let record = AiPatchSourceCycleStore::new(&self.root)
            .create(record, Some(&markdown))
            .map_err(AiPatchSourceCycleError::Store)?;

        Ok(AiPatchSourceCycleReport { candidate, record })
    }

    pub fn ai_patch_source_cycle_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceCycleSummary>, AiPatchSourceCycleStoreError> {
        AiPatchSourceCycleStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_cycle_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourceCycleRecord, AiPatchSourceCycleStoreError> {
        AiPatchSourceCycleStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_cycle_summary(
        &self,
        version: &str,
        cycle_id: &str,
    ) -> Result<AiPatchSourceCycleSummaryReport, AiPatchSourceCycleSummaryError> {
        let cycle = AiPatchSourceCycleStore::new(&self.root)
            .load(version, cycle_id)
            .map_err(AiPatchSourceCycleSummaryError::Cycle)?;
        let status = match cycle.status {
            AiPatchSourceCycleStatus::Promoted => AiPatchSourceCycleFollowUpStatus::Promoted,
            AiPatchSourceCycleStatus::RolledBack => AiPatchSourceCycleFollowUpStatus::RolledBack,
            AiPatchSourceCycleStatus::Blocked => AiPatchSourceCycleFollowUpStatus::Blocked,
        };
        let memory_compaction_recommended = status == AiPatchSourceCycleFollowUpStatus::Promoted;
        let next_goal = patch_source_cycle_follow_up_goal(&cycle, status);
        let next_task = patch_source_cycle_follow_up_task(&cycle, status);
        let follow_up_commands = patch_source_cycle_follow_up_commands(&cycle, status);
        let record = AiPatchSourceCycleFollowUpRecord {
            id: String::new(),
            version: version.to_string(),
            cycle_id: cycle.id.clone(),
            candidate_record_id: cycle.candidate_record_id.clone(),
            promotion_id: cycle.promotion_id.clone(),
            candidate_version: cycle.candidate_version.clone(),
            created_at_unix_seconds: 0,
            status,
            cycle_result: cycle.cycle_result.clone(),
            stable_version_after: cycle.stable_version_after.clone(),
            state_status_after: cycle.state_status_after.clone(),
            candidate_version_after: cycle.candidate_version_after.clone(),
            preflight_can_advance: cycle.preflight_can_advance,
            open_error_count: cycle.open_error_count,
            memory_compaction_recommended,
            next_goal,
            next_task,
            failure: cycle.failure.clone().or_else(|| cycle.error.clone()),
            follow_up_commands,
            markdown_file: PathBuf::new(),
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_cycle_summary_markdown(&record, &cycle);
        let record = AiPatchSourceCycleFollowUpStore::new(&self.root)
            .create(record, &markdown)
            .map_err(AiPatchSourceCycleSummaryError::Store)?;

        Ok(AiPatchSourceCycleSummaryReport { cycle, record })
    }

    pub fn ai_patch_source_cycle_summary_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceCycleFollowUpSummary>, AiPatchSourceCycleFollowUpStoreError> {
        AiPatchSourceCycleFollowUpStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_cycle_summary_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourceCycleFollowUpRecord, AiPatchSourceCycleFollowUpStoreError> {
        AiPatchSourceCycleFollowUpStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_task_draft(
        &self,
        version: &str,
        summary_id: &str,
    ) -> Result<AiPatchSourceTaskDraftReport, AiPatchSourceTaskDraftError> {
        let summary = AiPatchSourceCycleFollowUpStore::new(&self.root)
            .load(version, summary_id)
            .map_err(AiPatchSourceTaskDraftError::Summary)?;
        let suggested_target_version = next_version_after(&summary.stable_version_after)
            .map_err(AiPatchSourceTaskDraftError::Version)?;
        let normalized_goal = normalize_optional_text(&summary.next_goal);
        let normalized_task = normalize_optional_text(&summary.next_task);
        let error = match (&normalized_goal, &normalized_task) {
            (None, None) => {
                Some("后续总结缺少下一目标和下一任务，禁止生成可执行草案。".to_string())
            }
            (None, Some(_)) => Some("后续总结缺少下一目标，禁止生成可执行草案。".to_string()),
            (Some(_), None) => Some("后续总结缺少下一任务，禁止生成可执行草案。".to_string()),
            (Some(_), Some(_)) => None,
        };
        let status = if error.is_some() {
            AiPatchSourceTaskDraftStatus::Blocked
        } else {
            AiPatchSourceTaskDraftStatus::Drafted
        };
        let source_next_goal = normalized_goal.unwrap_or_else(|| summary.next_goal.clone());
        let source_next_task = normalized_task.unwrap_or_else(|| summary.next_task.clone());
        let proposed_task_title =
            patch_source_task_draft_title(summary.status, status, &suggested_target_version);
        let proposed_task_description = patch_source_task_draft_description(
            summary.status,
            &source_next_goal,
            &source_next_task,
        );
        let acceptance_checks = patch_source_task_draft_acceptance_checks();
        let follow_up_commands = patch_source_task_draft_follow_up_commands(
            &suggested_target_version,
            &proposed_task_title,
        );
        let record = AiPatchSourceTaskDraftRecord {
            id: String::new(),
            version: version.to_string(),
            summary_id: summary.id.clone(),
            cycle_id: summary.cycle_id.clone(),
            created_at_unix_seconds: 0,
            status,
            source_status: summary.status,
            stable_version_after: summary.stable_version_after.clone(),
            source_next_goal,
            source_next_task,
            proposed_task_title,
            proposed_task_description,
            suggested_target_version,
            required_audit: true,
            acceptance_checks,
            follow_up_commands,
            error,
            markdown_file: PathBuf::new(),
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_task_draft_markdown(&record, &summary);
        let record = AiPatchSourceTaskDraftStore::new(&self.root)
            .create(record, &markdown)
            .map_err(AiPatchSourceTaskDraftError::Store)?;

        Ok(AiPatchSourceTaskDraftReport { summary, record })
    }

    pub fn ai_patch_source_task_draft_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceTaskDraftSummary>, AiPatchSourceTaskDraftStoreError> {
        AiPatchSourceTaskDraftStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_task_draft_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftStoreError> {
        AiPatchSourceTaskDraftStore::new(&self.root).load(version, id)
    }

    pub fn ai_patch_source_task_audit(
        &self,
        version: &str,
        task_draft_id: &str,
    ) -> Result<AiPatchSourceTaskAuditReport, AiPatchSourceTaskAuditError> {
        let task_draft = AiPatchSourceTaskDraftStore::new(&self.root)
            .load(version, task_draft_id)
            .map_err(AiPatchSourceTaskAuditError::TaskDraft)?;
        let expected_target_version = next_version_after(&task_draft.stable_version_after)
            .map_err(AiPatchSourceTaskAuditError::Version)?;
        let findings = patch_source_task_audit_findings(&task_draft, &expected_target_version);
        let failed_messages = findings
            .iter()
            .filter(|finding| !finding.passed)
            .map(|finding| finding.message.clone())
            .collect::<Vec<_>>();
        let status = if failed_messages.is_empty() {
            AiPatchSourceTaskAuditStatus::Approved
        } else {
            AiPatchSourceTaskAuditStatus::Blocked
        };
        let blocked_reason = if failed_messages.is_empty() {
            None
        } else {
            Some(failed_messages.join("；"))
        };
        let approved_goal = patch_source_task_audit_goal(&task_draft);
        let follow_up_commands =
            patch_source_task_audit_follow_up_commands(status, &task_draft, &approved_goal);
        let record = AiPatchSourceTaskAuditRecord {
            id: String::new(),
            version: version.to_string(),
            task_draft_id: task_draft.id.clone(),
            summary_id: task_draft.summary_id.clone(),
            cycle_id: task_draft.cycle_id.clone(),
            created_at_unix_seconds: 0,
            status,
            source_task_status: task_draft.status,
            proposed_task_title: task_draft.proposed_task_title.clone(),
            proposed_task_description: task_draft.proposed_task_description.clone(),
            suggested_target_version: task_draft.suggested_target_version.clone(),
            approved_goal,
            findings,
            follow_up_commands,
            blocked_reason,
            markdown_file: PathBuf::new(),
            file: PathBuf::new(),
        };
        let markdown = build_ai_patch_source_task_audit_markdown(&record, &task_draft);
        let record = AiPatchSourceTaskAuditStore::new(&self.root)
            .create(record, &markdown)
            .map_err(AiPatchSourceTaskAuditError::Store)?;

        Ok(AiPatchSourceTaskAuditReport { task_draft, record })
    }

    pub fn ai_patch_source_task_audit_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceTaskAuditSummary>, AiPatchSourceTaskAuditStoreError> {
        AiPatchSourceTaskAuditStore::new(&self.root).list(version, limit)
    }

    pub fn ai_patch_source_task_audit_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiPatchSourceTaskAuditRecord, AiPatchSourceTaskAuditStoreError> {
        AiPatchSourceTaskAuditStore::new(&self.root).load(version, id)
    }

    fn prepare_patch_source_execution_file(
        &self,
        file: &AiPatchSourcePlanFile,
        relative_execution_dir: &Path,
    ) -> Result<PreparedPatchSourceExecutionFile, String> {
        let safe_path = patch_application_safe_relative_path(&file.source_path)
            .map_err(|error| format!("源码覆盖执行路径不合法 {}：{}", file.source_path, error))?;
        if safe_path != file.target_file {
            return Err(format!(
                "源码覆盖准备记录路径不一致：来源 {}，计划目标 {}。",
                file.source_path,
                file.target_file.display()
            ));
        }

        let mirror_path = self.root.join(&file.mirror_file);
        let new_contents = fs::read(&mirror_path).map_err(|source| {
            format!(
                "源码覆盖候选镜像文件不可读 {}：{}",
                file.mirror_file.display(),
                source
            )
        })?;
        if new_contents.len() != file.new_bytes {
            return Err(format!(
                "源码覆盖候选镜像字节数已变化 {}：计划 {}，当前 {}。",
                file.mirror_file.display(),
                file.new_bytes,
                new_contents.len()
            ));
        }

        let target_path = self.root.join(&file.target_file);
        let target_exists_now = target_path.exists();
        if target_exists_now != file.target_exists {
            return Err(format!(
                "源码覆盖目标存在状态已变化 {}：计划 {}，当前 {}。",
                file.target_file.display(),
                if file.target_exists {
                    "存在"
                } else {
                    "不存在"
                },
                if target_exists_now {
                    "存在"
                } else {
                    "不存在"
                }
            ));
        }
        if target_exists_now && !target_path.is_file() {
            return Err(format!(
                "源码覆盖目标路径不是文件，禁止覆盖：{}",
                file.target_file.display()
            ));
        }

        let before_contents = if target_exists_now {
            fs::read(&target_path).map_err(|source| {
                format!(
                    "源码覆盖目标文件不可读 {}：{}",
                    file.target_file.display(),
                    source
                )
            })?
        } else {
            Vec::new()
        };
        if before_contents.len() != file.original_bytes {
            return Err(format!(
                "源码覆盖目标文件字节数已变化 {}：计划 {}，当前 {}。",
                file.target_file.display(),
                file.original_bytes,
                before_contents.len()
            ));
        }
        if target_exists_now {
            let Some(plan_backup_file) = file.rollback_backup_file.as_ref() else {
                return Err(format!(
                    "源码覆盖准备记录缺少目标文件回滚备份：{}",
                    file.target_file.display()
                ));
            };
            let plan_backup_contents =
                fs::read(self.root.join(plan_backup_file)).map_err(|source| {
                    format!(
                        "源码覆盖准备回滚备份不可读 {}：{}",
                        plan_backup_file.display(),
                        source
                    )
                })?;
            if plan_backup_contents != before_contents {
                return Err(format!(
                    "源码覆盖目标文件已在准备后变化，禁止使用过期计划：{}",
                    file.target_file.display()
                ));
            }
        }

        let execution_backup_file = if target_exists_now {
            Some(relative_execution_dir.join("rollback").join(&safe_path))
        } else {
            None
        };
        let action = if target_exists_now {
            "覆盖已有文件".to_string()
        } else {
            "创建新文件".to_string()
        };
        let rollback_action = if let Some(backup_file) = execution_backup_file.as_ref() {
            format!(
                "从执行级备份 {} 恢复到 {}。",
                backup_file.display(),
                file.target_file.display()
            )
        } else {
            format!("删除执行中新建的文件 {}。", file.target_file.display())
        };
        let before_bytes = before_contents.len();
        let after_bytes = new_contents.len();

        Ok(PreparedPatchSourceExecutionFile {
            target_path,
            before_contents,
            new_contents: new_contents.clone(),
            record_file: AiPatchSourceExecutionFile {
                source_path: file.source_path.clone(),
                mirror_file: file.mirror_file.clone(),
                target_file: file.target_file.clone(),
                target_existed_before: target_exists_now,
                before_bytes,
                after_bytes,
                execution_backup_file,
                action,
                rollback_action,
            },
        })
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
        let summary = self
            .create_ai_self_upgrade_summary_from_audit(audit.clone())
            .map_err(AiSelfUpgradeError::Summary)?
            .record;

        Ok(AiSelfUpgradeReport {
            preview,
            ai,
            proposed_goal,
            evolution,
            audit,
            summary,
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

    pub fn ai_self_upgrade_record_for_session(
        &self,
        version: &str,
        session_id: &str,
    ) -> Result<Option<AiSelfUpgradeAuditSummary>, AiSelfUpgradeAuditError> {
        AiSelfUpgradeAuditStore::new(&self.root).find_by_session(version, session_id)
    }

    pub fn ai_self_upgrade_summary(
        &self,
        version: &str,
        audit_id: &str,
    ) -> Result<AiSelfUpgradeSummaryReport, AiSelfUpgradeSummaryError> {
        let audit = AiSelfUpgradeAuditStore::new(&self.root)
            .load(version, audit_id)
            .map_err(AiSelfUpgradeSummaryError::Audit)?;
        self.create_ai_self_upgrade_summary_from_audit(audit)
    }

    pub fn ai_self_upgrade_summary_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<AiSelfUpgradeSummaryIndexEntry>, AiSelfUpgradeSummaryStoreError> {
        AiSelfUpgradeSummaryStore::new(&self.root).list(version, limit)
    }

    pub fn ai_self_upgrade_summary_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<AiSelfUpgradeSummaryRecord, AiSelfUpgradeSummaryStoreError> {
        AiSelfUpgradeSummaryStore::new(&self.root).load(version, id)
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

    pub fn execute_agent_steps(
        &self,
        request: AgentStepExecutionRequest,
        max_steps: usize,
    ) -> Result<AgentStepRunReport, AgentStepRunError> {
        if max_steps == 0 {
            return Err(AgentStepRunError::InvalidStepLimit);
        }

        let session_id = request.session_id.clone();
        let session_version = request.session_version.clone();
        let target_version = request.target_version.clone();
        let mut executed_steps = Vec::new();

        for _ in 0..max_steps {
            match self.execute_next_agent_step(request.clone()) {
                Ok(report) => {
                    let session_completed = report.session_completed;
                    executed_steps.push(report);
                    if session_completed {
                        return Ok(AgentStepRunReport {
                            session_id,
                            session_version,
                            target_version,
                            max_steps,
                            executed_steps,
                            stop: AgentStepRunStop::SessionCompleted,
                        });
                    }
                }
                Err(AgentStepExecutionError::NoPendingStep { session_id }) => {
                    return Ok(AgentStepRunReport {
                        session_id: session_id.clone(),
                        session_version,
                        target_version,
                        max_steps,
                        executed_steps,
                        stop: AgentStepRunStop::NoPendingStep { session_id },
                    });
                }
                Err(AgentStepExecutionError::InputRequired {
                    step_order,
                    tool_id,
                    input,
                }) => {
                    return Ok(AgentStepRunReport {
                        session_id,
                        session_version,
                        target_version,
                        max_steps,
                        executed_steps,
                        stop: AgentStepRunStop::InputRequired {
                            step_order,
                            tool_id,
                            input,
                        },
                    });
                }
                Err(AgentStepExecutionError::NoRunnableTool { step_order }) => {
                    return Ok(AgentStepRunReport {
                        session_id,
                        session_version,
                        target_version,
                        max_steps,
                        executed_steps,
                        stop: AgentStepRunStop::NoRunnableTool { step_order },
                    });
                }
                Err(AgentStepExecutionError::Tool(error)) => {
                    return Ok(AgentStepRunReport {
                        session_id,
                        session_version,
                        target_version,
                        max_steps,
                        executed_steps,
                        stop: AgentStepRunStop::Failed {
                            message: error.to_string(),
                        },
                    });
                }
                Err(error) => return Err(AgentStepRunError::Step(error)),
            }
        }

        Ok(AgentStepRunReport {
            session_id,
            session_version,
            target_version,
            max_steps,
            executed_steps,
            stop: AgentStepRunStop::StepLimitReached,
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

    fn create_ai_self_upgrade_summary_from_audit(
        &self,
        audit: AiSelfUpgradeAuditRecord,
    ) -> Result<AiSelfUpgradeSummaryReport, AiSelfUpgradeSummaryError> {
        let session = if let Some(session_id) = audit.session_id.as_deref() {
            Some(
                AgentSessionStore::new(&self.root)
                    .load(&audit.version, session_id)
                    .map_err(AiSelfUpgradeSummaryError::Session)?,
            )
        } else {
            None
        };
        let markdown = build_ai_self_upgrade_summary_markdown(&audit, session.as_ref());
        let status = match audit.status {
            AiSelfUpgradeAuditStatus::Succeeded => AiSelfUpgradeSummaryStatus::Succeeded,
            AiSelfUpgradeAuditStatus::Failed => AiSelfUpgradeSummaryStatus::Failed,
        };
        let record = AiSelfUpgradeSummaryRecord {
            id: String::new(),
            version: audit.version.clone(),
            audit_id: audit.id.clone(),
            created_at_unix_seconds: 0,
            status,
            proposed_goal: audit.proposed_goal.clone(),
            session_id: audit.session_id.clone(),
            candidate_version: audit.candidate_version.clone(),
            stable_version_after: audit.stable_version_after.clone(),
            cycle_result: audit.cycle_result.clone(),
            markdown_file: PathBuf::new(),
            file: PathBuf::new(),
        };
        let record = AiSelfUpgradeSummaryStore::new(&self.root)
            .create(record, &markdown)
            .map_err(AiSelfUpgradeSummaryError::Store)?;

        Ok(AiSelfUpgradeSummaryReport {
            audit,
            session,
            record,
        })
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

    fn record_ai_patch_draft_failure(
        &self,
        preview: &AiPatchDraftPreview,
        ai: Option<&AiExecutionReport>,
        error: &str,
    ) -> Result<AiPatchDraftRecord, AiPatchDraftError> {
        let mut record = self.ai_patch_draft_base(preview, AiPatchDraftStatus::Failed);
        if let Some(ai) = ai {
            record.provider_id = ai.response.provider_id.clone();
            record.model = ai.response.model.clone();
            record.protocol = ai.response.protocol.clone();
            record.ai_response_preview = Some(truncate_chars(&ai.response.text, 240));
        }
        record.error = Some(truncate_chars(error, 400));

        AiPatchDraftStore::new(&self.root)
            .create(record, None)
            .map_err(AiPatchDraftError::Store)
    }

    fn ai_patch_draft_base(
        &self,
        preview: &AiPatchDraftPreview,
        status: AiPatchDraftStatus,
    ) -> AiPatchDraftRecord {
        AiPatchDraftRecord {
            id: String::new(),
            version: preview.current_version.clone(),
            target_version: preview.target_version.clone(),
            created_at_unix_seconds: 0,
            status,
            goal: preview.goal.clone(),
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
            allowed_write_roots: preview.allowed_write_roots.clone(),
            required_sections: preview.required_sections.clone(),
            ai_response_preview: None,
            draft_file: None,
            error: None,
            file: PathBuf::new(),
        }
    }

    fn approved_patch_draft_goal_from_task_audit(
        &self,
        task_audit_id: &str,
    ) -> Result<String, AiPatchDraftError> {
        let preflight = self.preflight().map_err(AiPatchDraftError::Preflight)?;
        if !preflight.open_errors.is_empty() {
            return Err(AiPatchDraftError::Blocked {
                version: preflight.current_version.clone(),
                open_errors: preflight.open_errors.clone(),
            });
        }

        let task_audit = AiPatchSourceTaskAuditStore::new(&self.root)
            .load(&preflight.current_version, task_audit_id)
            .map_err(AiPatchDraftError::TaskAudit)?;
        if task_audit.status != AiPatchSourceTaskAuditStatus::Approved {
            return Err(AiPatchDraftError::TaskAuditNotApproved {
                id: task_audit.id,
                status: task_audit.status,
                blocked_reason: task_audit.blocked_reason,
            });
        }

        normalize_optional_text(&task_audit.approved_goal)
            .ok_or_else(|| AiPatchDraftError::EmptyTaskAuditGoal { id: task_audit.id })
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

fn build_ai_self_upgrade_summary_markdown(
    audit: &AiSelfUpgradeAuditRecord,
    session: Option<&AgentSession>,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 自我升级总结报告\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 审计记录：{}\n- 审计状态：{}\n- 生成时间：unix:{}\n- 提供商：{}\n- 模型：{}\n- 协议：{}\n",
        audit.version,
        audit.id,
        audit.status,
        audit.created_at_unix_seconds,
        audit.provider_id,
        audit.model,
        audit.protocol
    ));
    markdown.push_str(&format!(
        "- Agent 会话：{}\n- 候选版本：{}\n- 当前稳定版本：{}\n- 循环结果：{}\n\n",
        audit.session_id.as_deref().unwrap_or("无"),
        audit.candidate_version.as_deref().unwrap_or("无"),
        audit.stable_version_after.as_deref().unwrap_or("无"),
        audit.cycle_result.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 目标\n\n");
    markdown.push_str(&format!(
        "- 用户提示：{}\n- AI 归一化目标：{}\n- AI 响应摘要：{}\n\n",
        audit.hint.as_deref().unwrap_or("无"),
        audit.proposed_goal.as_deref().unwrap_or("无"),
        audit.ai_response_preview.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 计划（Plan）\n\n");
    if let Some(session) = session {
        markdown.push_str(&format!(
            "- 会话目标：{}\n- 会话状态：{}\n- 步骤数量：{}\n",
            session.goal,
            session.status,
            session.steps.len()
        ));
        for step in &session.steps {
            markdown.push_str(&format!(
                "- 步骤 {}：{}；Agent：{}；能力：{}；状态：{}；验证：{}；结果：{}\n",
                step.order,
                step.title,
                step.agent_id,
                step.capability,
                step.status,
                step.verification,
                step.result.as_deref().unwrap_or("无")
            ));
        }
        markdown.push('\n');
    } else {
        markdown.push_str("- 无关联 Agent 会话，无法展开步骤计划。\n\n");
    }

    markdown.push_str("# 代码变更\n\n");
    markdown.push_str(&format!(
        "- 准备候选版本：{}\n- 验证候选版本：{}\n- 提升后稳定版本：{}\n",
        audit.prepared_candidate_version.as_deref().unwrap_or("无"),
        audit.candidate_version.as_deref().unwrap_or("无"),
        audit.stable_version_after.as_deref().unwrap_or("无")
    ));
    markdown.push_str("- 详细代码差异以对应版本归档、Git 提交和 Agent 会话事件为准。\n\n");

    markdown.push_str("# 测试结果\n\n");
    markdown.push_str(&format!(
        "- 候选循环结果：{}\n- 预检开放错误数量：{}\n",
        audit.cycle_result.as_deref().unwrap_or("无"),
        audit.open_error_count
    ));
    if let Some(session) = session {
        let run_events = session
            .events
            .iter()
            .filter_map(|event| event.run.as_ref())
            .collect::<Vec<_>>();
        if run_events.is_empty() {
            markdown.push_str("- 会话没有记录独立 Runtime 运行事件。\n");
        } else {
            for run in run_events {
                markdown.push_str(&format!(
                    "- Runtime 运行 {}：版本 {}，退出码 {:?}，超时 {}，报告 {}\n",
                    run.run_id, run.version, run.exit_code, run.timed_out, run.report_file
                ));
            }
        }
    } else {
        markdown.push_str("- 无关联会话，无法读取 Runtime 运行事件。\n");
    }
    markdown.push('\n');

    markdown.push_str("# 错误信息\n\n");
    let audit_error = audit.error.as_deref().unwrap_or("无");
    let session_error = session
        .and_then(|session| session.error.as_deref())
        .unwrap_or("无");
    markdown.push_str(&format!(
        "- 审计错误：{}\n- 会话错误：{}\n\n",
        audit_error, session_error
    ));

    markdown.push_str("# 审计记录\n\n");
    markdown.push_str(&format!(
        "- 审计文件：{}\n- 记忆来源：{}\n- 成功经验数量：{}\n- 失败风险数量：{}\n- 优化建议数量：{}\n- 可复用经验数量：{}\n\n",
        audit.file.display(),
        if audit.memory_source_versions.is_empty() {
            "无".to_string()
        } else {
            audit.memory_source_versions.join("、")
        },
        audit.success_experience_count,
        audit.failure_experience_count,
        audit.optimization_suggestion_count,
        audit.reusable_experience_count
    ));

    markdown.push_str("# 下一步建议\n\n");
    if audit.status == AiSelfUpgradeAuditStatus::Succeeded {
        markdown.push_str("- 继续执行 `preflight` 和开放错误查询，确认新稳定版本可以继续进化。\n");
        markdown.push_str("- 根据任务队列和最新记忆选择下一轮最小 patch 级目标。\n");
    } else {
        markdown.push_str("- 先修复审计错误或会话错误，再重新执行自我升级。\n");
        markdown.push_str("- 修复后必须重新运行测试、验证和预检。\n");
    }
    markdown
}

fn patch_draft_allowed_write_roots(version: &str) -> Result<Vec<String>, AiPatchDraftError> {
    let major = version_major_key(version).map_err(AiPatchDraftError::Version)?;
    Ok(vec![format!(
        "workspaces/{major}/artifacts/agents/patch-drafts/"
    )])
}

fn patch_draft_required_sections() -> Vec<String> {
    vec![
        "# 补丁目标".to_string(),
        "# 计划".to_string(),
        "# 允许写入范围".to_string(),
        "# 代码草案".to_string(),
        "# 测试草案".to_string(),
        "# 验证命令".to_string(),
        "# 风险与回滚".to_string(),
    ]
}

fn build_ai_patch_draft_prompt(
    preflight: &PreflightReport,
    target_version: &str,
    insights: &MemoryInsightReport,
    goal: &str,
    allowed_write_roots: &[String],
    required_sections: &[String],
) -> String {
    let mut prompt = String::new();
    prompt.push_str("你是 SelfForge 的 AI 补丁草案 Agent。\n");
    prompt.push_str("请基于当前状态和近期记忆，生成一个中文 Markdown 补丁草案。\n");
    prompt.push_str("必须遵守：只生成草案，不要声称已经修改文件；禁止输出 Emoji；禁止输出 API Key、密钥、完整请求体或敏感配置；禁止修改 runtime 和 supervisor 受保护边界；禁止要求绕过测试；默认只做 patch 级变更。\n");
    prompt.push_str("草案只能写入下方允许的草案产物目录。代码块中的内容均视为候选草案，不代表真实写入源码。\n\n");
    prompt.push_str("# 当前状态\n");
    prompt.push_str(&format!(
        "- 当前稳定版本：{}\n- 草案目标版本：{}\n- 状态：{}\n- 候选版本：{}\n- 未解决错误：{}\n- 用户目标：{}\n",
        preflight.current_version,
        target_version,
        preflight.status,
        preflight.candidate_version.as_deref().unwrap_or("无"),
        preflight.open_errors.len(),
        goal
    ));
    prompt.push_str("\n# 允许写入范围\n");
    for root in allowed_write_roots {
        prompt.push_str(&format!("- {root}\n"));
    }
    prompt.push_str("\n# 必须包含章节\n");
    for section in required_sections {
        prompt.push_str(&format!("- {section}\n"));
    }
    prompt.push_str("\n# 近期成功经验\n");
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
    prompt.push_str("\n# 输出要求\n");
    prompt.push_str("请直接输出中文 Markdown，必须先写计划，再写测试草案。不要输出说明前缀，不要包裹在代码围栏中。\n");
    prompt
}

fn validate_ai_patch_draft_text(text: &str) -> Result<String, AiPatchDraftError> {
    let trimmed = text.trim();
    let response_preview = truncate_chars(trimmed, 160);
    if trimmed.is_empty() {
        return Err(AiPatchDraftError::InvalidDraft {
            reason: "响应为空".to_string(),
            response_preview,
        });
    }
    if !contains_chinese_text(trimmed) {
        return Err(AiPatchDraftError::InvalidDraft {
            reason: "响应缺少中文内容".to_string(),
            response_preview,
        });
    }
    if trimmed.chars().any(is_disallowed_symbol) {
        return Err(AiPatchDraftError::InvalidDraft {
            reason: "响应包含 Emoji 或禁用符号".to_string(),
            response_preview,
        });
    }
    for required in ["计划", "测试"] {
        if !has_markdown_section(trimmed, required) {
            return Err(AiPatchDraftError::InvalidDraft {
                reason: format!("响应缺少 {required} 章节"),
                response_preview,
            });
        }
    }

    Ok(trimmed.to_string())
}

#[derive(Debug)]
struct PatchPreviewCodeBlock {
    language: Option<String>,
    content: String,
}

fn extract_patch_preview_code_blocks(markdown: &str) -> Vec<PatchPreviewCodeBlock> {
    let mut in_code_section = false;
    let mut in_fence = false;
    let mut language = None;
    let mut content = Vec::new();
    let mut blocks = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if !in_fence {
            if let Some(title) = markdown_heading_title(trimmed) {
                if in_code_section && !title.contains("代码草案") {
                    break;
                }
                in_code_section = title.contains("代码草案");
                continue;
            }
            if !in_code_section {
                continue;
            }
        }

        if in_code_section && trimmed.starts_with("```") {
            if in_fence {
                let joined = content.join("\n").trim().to_string();
                if !joined.is_empty() {
                    blocks.push(PatchPreviewCodeBlock {
                        language: language.clone(),
                        content: joined,
                    });
                }
                content.clear();
                language = None;
                in_fence = false;
            } else {
                let tag = trimmed.trim_start_matches("```").trim();
                language = if tag.is_empty() {
                    None
                } else {
                    Some(tag.to_string())
                };
                in_fence = true;
            }
            continue;
        }

        if in_fence {
            content.push(line.to_string());
        }
    }

    blocks
}

fn build_ai_patch_preview_markdown(
    audit: &AiPatchAuditRecord,
    draft: &AiPatchDraftRecord,
    status: AiPatchPreviewStatus,
    code_block_count: usize,
    changes: &[AiPatchPreviewChange],
    error: Option<&str>,
    draft_markdown: Option<&str>,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁应用预演\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 目标版本：{}\n- 审计记录：{}\n- 草案记录：{}\n- 预演状态：{}\n- 错误信息：{}\n\n",
        audit.version,
        audit.target_version,
        audit.id,
        draft.id,
        status,
        error.unwrap_or("无")
    ));

    markdown.push_str("# 审计结果\n\n");
    markdown.push_str(&format!(
        "- 审计状态：{}\n- 规范化写入范围数量：{}\n- 活跃冲突数量：{}\n- 发现数量：{}\n",
        audit.status,
        audit.normalized_write_scope.len(),
        audit.active_conflict_count,
        audit.finding_count
    ));
    for scope in &audit.normalized_write_scope {
        markdown.push_str(&format!("- 写入范围：{scope}\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 预演变更\n\n");
    markdown.push_str(&format!(
        "- 代码块数量：{}\n- 变更数量：{}\n",
        code_block_count,
        changes.len()
    ));
    if changes.is_empty() {
        markdown.push_str("- 本次未生成可应用变更。\n\n");
    } else if let Some(draft_markdown) = draft_markdown {
        let code_blocks = extract_patch_preview_code_blocks(draft_markdown);
        for change in changes {
            markdown.push_str(&format!(
                "\n## 预演文件 {}\n\n- 来源代码块：{}\n- 语言：{}\n- 内容字节：{}\n\n",
                change.path,
                change.code_block_index,
                change.language.as_deref().unwrap_or("未标注"),
                change.content_bytes
            ));
            if let Some(block) = code_blocks.get(change.code_block_index.saturating_sub(1)) {
                markdown.push_str("```");
                if let Some(language) = block.language.as_deref() {
                    markdown.push_str(language);
                }
                markdown.push('\n');
                markdown.push_str(&block.content);
                markdown.push_str("\n```\n");
            } else {
                markdown.push_str("- 未找到对应代码块。\n");
            }
        }
        markdown.push('\n');
    }

    markdown.push_str("# 测试建议\n\n");
    markdown.push_str("- 预演成功后必须先在候选工作区或沙箱中应用，再执行 `cargo fmt --check`、`cargo test`、`cargo run -- validate` 和 `cargo run -- preflight`。\n");
    markdown.push_str("- 本命令只生成可审计预演，不代表源码已经被修改。\n\n");

    markdown.push_str("# 回滚说明\n\n");
    markdown.push_str("- 如果后续候选应用或测试失败，保留本预演记录作为审计证据，并通过 `rollback` 或候选失败流程恢复稳定版本。\n");
    markdown
}

fn read_patch_draft_markdown(
    root: &Path,
    draft: &AiPatchDraftRecord,
) -> Result<String, (PathBuf, io::Error)> {
    let Some(draft_file) = draft.draft_file.as_ref() else {
        return Err((
            PathBuf::from("无"),
            io::Error::new(io::ErrorKind::NotFound, "补丁草案缺少 Markdown 文件"),
        ));
    };
    let path = root.join(draft_file);
    fs::read_to_string(&path).map_err(|source| (path, source))
}

fn make_patch_application_id(
    root: &Path,
    version: &str,
) -> Result<String, AiPatchApplicationError> {
    let major = version_major_key(version).map_err(AiPatchApplicationError::Version)?;
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let id_seed = clock.as_nanos();
    for attempt in 0..1000 {
        let id = format!("patch-application-{id_seed}-{attempt:03}");
        let record_path = root
            .join("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-applications")
            .join(format!("{id}.json"));
        let application_dir = root
            .join("workspaces")
            .join(&major)
            .join("source")
            .join("patch-applications")
            .join(&id);
        if !record_path.exists() && !application_dir.exists() {
            return Ok(id);
        }
    }

    Err(AiPatchApplicationError::Store(
        AiPatchApplicationStoreError::IdExhausted {
            version: version.to_string(),
        },
    ))
}

fn patch_application_safe_relative_path(value: &str) -> Result<PathBuf, AiPatchApplicationError> {
    let normalized = normalize_patch_scope_path(value).map_err(|reason| {
        AiPatchApplicationError::InvalidPath {
            path: value.to_string(),
            reason,
        }
    })?;
    if normalized.starts_with("runtime/")
        || normalized.starts_with("supervisor/")
        || normalized.starts_with("state/")
        || normalized.starts_with(".git/")
        || normalized == ".env"
    {
        return Err(AiPatchApplicationError::InvalidPath {
            path: value.to_string(),
            reason: "候选应用禁止写入受保护路径。".to_string(),
        });
    }

    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        path.push(part);
    }
    Ok(path)
}

fn patch_application_verification_commands() -> Vec<String> {
    vec![
        "cargo fmt --check".to_string(),
        "cargo test".to_string(),
        "cargo run -- validate".to_string(),
        "cargo run -- preflight".to_string(),
    ]
}

fn patch_application_verification_specs(
    commands: &[String],
) -> Result<Vec<AiPatchVerificationCommandSpec>, AiPatchVerificationError> {
    commands
        .iter()
        .map(|command| match command.as_str() {
            "cargo fmt --check" => Ok(AiPatchVerificationCommandSpec {
                command: command.clone(),
                program: "cargo".to_string(),
                args: vec!["fmt".to_string(), "--check".to_string()],
            }),
            "cargo test" => Ok(AiPatchVerificationCommandSpec {
                command: command.clone(),
                program: "cargo".to_string(),
                args: vec!["test".to_string()],
            }),
            "cargo run -- validate" => Ok(AiPatchVerificationCommandSpec {
                command: command.clone(),
                program: "cargo".to_string(),
                args: vec!["run".to_string(), "--".to_string(), "validate".to_string()],
            }),
            "cargo run -- preflight" => Ok(AiPatchVerificationCommandSpec {
                command: command.clone(),
                program: "cargo".to_string(),
                args: vec!["run".to_string(), "--".to_string(), "preflight".to_string()],
            }),
            other => Err(AiPatchVerificationError::UnsupportedCommand(
                other.to_string(),
            )),
        })
        .collect()
}

fn run_patch_verification_command(
    root: &Path,
    spec: &AiPatchVerificationCommandSpec,
    timeout_ms: u64,
) -> Result<AiPatchVerificationCommandRecord, AiPatchVerificationError> {
    let started_at_unix_seconds = current_unix_seconds();
    let started = Instant::now();
    let mut child = match Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(source) => {
            return Ok(AiPatchVerificationCommandRecord {
                command: spec.command.clone(),
                program: spec.program.clone(),
                args: spec.args.clone(),
                started_at_unix_seconds,
                duration_ms: started.elapsed().as_millis() as u64,
                timeout_ms,
                exit_code: None,
                timed_out: false,
                stdout_bytes: 0,
                stderr_bytes: source.to_string().len(),
                stdout_preview: String::new(),
                stderr_preview: format!("启动失败：{source}"),
                status: AiPatchVerificationStatus::Failed,
            });
        }
    };

    let timeout = Duration::from_millis(timeout_ms);
    loop {
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let output =
                child
                    .wait_with_output()
                    .map_err(|source| AiPatchVerificationError::Io {
                        path: root.to_path_buf(),
                        source,
                    })?;
            return Ok(build_patch_verification_run(
                spec,
                started_at_unix_seconds,
                started.elapsed().as_millis() as u64,
                timeout_ms,
                output.status.code(),
                true,
                &output.stdout,
                &output.stderr,
            ));
        }

        match child
            .try_wait()
            .map_err(|source| AiPatchVerificationError::Io {
                path: root.to_path_buf(),
                source,
            })? {
            Some(_) => {
                let output =
                    child
                        .wait_with_output()
                        .map_err(|source| AiPatchVerificationError::Io {
                            path: root.to_path_buf(),
                            source,
                        })?;
                return Ok(build_patch_verification_run(
                    spec,
                    started_at_unix_seconds,
                    started.elapsed().as_millis() as u64,
                    timeout_ms,
                    output.status.code(),
                    false,
                    &output.stdout,
                    &output.stderr,
                ));
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn build_patch_verification_run(
    spec: &AiPatchVerificationCommandSpec,
    started_at_unix_seconds: u64,
    duration_ms: u64,
    timeout_ms: u64,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> AiPatchVerificationCommandRecord {
    let status = if !timed_out && exit_code == Some(0) {
        AiPatchVerificationStatus::Passed
    } else {
        AiPatchVerificationStatus::Failed
    };
    AiPatchVerificationCommandRecord {
        command: spec.command.clone(),
        program: spec.program.clone(),
        args: spec.args.clone(),
        started_at_unix_seconds,
        duration_ms,
        timeout_ms,
        exit_code,
        timed_out,
        stdout_bytes: stdout.len(),
        stderr_bytes: stderr.len(),
        stdout_preview: command_output_preview(stdout),
        stderr_preview: command_output_preview(stderr),
        status,
    }
}

fn patch_verification_run_passed(run: &AiPatchVerificationCommandRecord) -> bool {
    run.status == AiPatchVerificationStatus::Passed && !run.timed_out && run.exit_code == Some(0)
}

fn unsupported_patch_verification_run(
    command: &str,
    timeout_ms: u64,
) -> AiPatchVerificationCommandRecord {
    let message = format!("验证命令不受支持：{command}");
    AiPatchVerificationCommandRecord {
        command: command.to_string(),
        program: String::new(),
        args: Vec::new(),
        started_at_unix_seconds: current_unix_seconds(),
        duration_ms: 0,
        timeout_ms,
        exit_code: None,
        timed_out: false,
        stdout_bytes: 0,
        stderr_bytes: message.len(),
        stdout_preview: String::new(),
        stderr_preview: message,
        status: AiPatchVerificationStatus::Failed,
    }
}

fn command_output_preview(bytes: &[u8]) -> String {
    const LIMIT: usize = 1200;
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    let trimmed = text.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }

    trimmed.chars().take(LIMIT).collect::<String>() + "..."
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn make_patch_source_plan_id(root: &Path, version: &str) -> Result<String, AiPatchSourcePlanError> {
    let major = version_major_key(version).map_err(|error| {
        AiPatchSourcePlanError::Store(AiPatchSourcePlanStoreError::Version(error))
    })?;
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let id_seed = clock.as_nanos();
    for attempt in 0..1000 {
        let id = format!("patch-source-plan-{id_seed}-{attempt:03}");
        let record_path = root
            .join("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-source-plans")
            .join(format!("{id}.json"));
        let plan_dir = root
            .join("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-source-plans")
            .join(&id);
        if !record_path.exists() && !plan_dir.exists() {
            return Ok(id);
        }
    }

    Err(AiPatchSourcePlanError::Store(
        AiPatchSourcePlanStoreError::IdExhausted {
            version: version.to_string(),
        },
    ))
}

fn make_patch_source_execution_id(
    root: &Path,
    version: &str,
) -> Result<String, AiPatchSourceExecutionError> {
    let major = version_major_key(version).map_err(|error| {
        AiPatchSourceExecutionError::Store(AiPatchSourceExecutionStoreError::Version(error))
    })?;
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let id_seed = clock.as_nanos();
    for attempt in 0..1000 {
        let id = format!("patch-source-execution-{id_seed}-{attempt:03}");
        let record_path = root
            .join("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-source-executions")
            .join(format!("{id}.json"));
        let execution_dir = root
            .join("workspaces")
            .join(&major)
            .join("artifacts")
            .join("agents")
            .join("patch-source-executions")
            .join(&id);
        if !record_path.exists() && !execution_dir.exists() {
            return Ok(id);
        }
    }

    Err(AiPatchSourceExecutionError::Store(
        AiPatchSourceExecutionStoreError::IdExhausted {
            version: version.to_string(),
        },
    ))
}

#[derive(Debug, Clone)]
struct PreparedPatchSourceExecutionFile {
    target_path: PathBuf,
    before_contents: Vec<u8>,
    new_contents: Vec<u8>,
    record_file: AiPatchSourceExecutionFile,
}

fn rollback_patch_source_execution(
    prepared_files: &[PreparedPatchSourceExecutionFile],
) -> Vec<String> {
    let mut steps = Vec::new();
    for prepared in prepared_files.iter().rev() {
        if prepared.record_file.target_existed_before {
            match fs::write(&prepared.target_path, &prepared.before_contents) {
                Ok(()) => steps.push(prepared.record_file.rollback_action.clone()),
                Err(source) => steps.push(format!(
                    "回滚失败，无法恢复 {}：{}。",
                    prepared.record_file.target_file.display(),
                    source
                )),
            }
        } else if prepared.target_path.exists() {
            match fs::remove_file(&prepared.target_path) {
                Ok(()) => steps.push(prepared.record_file.rollback_action.clone()),
                Err(source) => steps.push(format!(
                    "回滚失败，无法删除新建文件 {}：{}。",
                    prepared.record_file.target_file.display(),
                    source
                )),
            }
        } else {
            steps.push(format!(
                "回滚跳过，新建文件已经不存在：{}。",
                prepared.record_file.target_file.display()
            ));
        }
    }
    if steps.is_empty() {
        steps.push("没有需要回滚的源码文件。".to_string());
    }
    steps
}

fn patch_source_plan_prerequisites() -> Vec<String> {
    vec![
        "候选应用状态必须为已应用。".to_string(),
        "候选应用验证状态必须为已通过。".to_string(),
        "源码覆盖前必须人工复核覆盖计划、差异摘要和回滚清单。".to_string(),
        "真实覆盖后必须重新执行格式化、测试、系统验证和预检。".to_string(),
    ]
}

fn build_ai_patch_application_markdown(
    preview: &AiPatchPreviewRecord,
    candidate_version: &str,
    status: AiPatchApplicationStatus,
    application_dir: Option<&PathBuf>,
    files: &[AiPatchApplicationFile],
    verification_commands: &[String],
    rollback_hint: &str,
    error: Option<&str>,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁候选应用记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 候选版本：{}\n- 预演记录：{}\n- 审计记录：{}\n- 草案记录：{}\n- 应用状态：{}\n- 应用目录：{}\n- 错误信息：{}\n\n",
        preview.version,
        candidate_version,
        preview.id,
        preview.audit_id,
        preview.draft_id,
        status,
        application_dir
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "无".to_string()),
        error.unwrap_or("无")
    ));

    markdown.push_str("# 应用文件\n\n");
    if files.is_empty() {
        markdown.push_str("- 本次没有写入候选应用镜像文件。\n\n");
    } else {
        for file in files {
            markdown.push_str(&format!(
                "- 来源路径：{}；镜像文件：{}；字节：{}\n",
                file.source_path,
                file.mirror_file.display(),
                file.content_bytes
            ));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 验证命令\n\n");
    for command in verification_commands {
        markdown.push_str(&format!("- `{command}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 回滚准备\n\n");
    markdown.push_str(&format!("- {rollback_hint}\n"));
    markdown.push_str("- 当前命令只写入候选工作区镜像，后续真实覆盖仓库源码前必须再次审计写入范围并执行完整验证。\n");
    markdown
}

fn build_ai_patch_application_record_markdown(record: &AiPatchApplicationRecord) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁候选应用记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 候选版本：{}\n- 预演记录：{}\n- 审计记录：{}\n- 草案记录：{}\n- 应用状态：{}\n- 验证状态：{}\n- 验证时间：{}\n- 应用目录：{}\n- 错误信息：{}\n\n",
        record.version,
        record.candidate_version,
        record.preview_id,
        record.audit_id,
        record.draft_id,
        record.status,
        record.verification_status,
        record
            .verified_at_unix_seconds
            .map(|value| format!("unix:{value}"))
            .unwrap_or_else(|| "未验证".to_string()),
        record
            .application_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "无".to_string()),
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 应用文件\n\n");
    if record.files.is_empty() {
        markdown.push_str("- 本次没有写入候选应用镜像文件。\n\n");
    } else {
        for file in &record.files {
            markdown.push_str(&format!(
                "- 来源路径：{}；镜像文件：{}；字节：{}\n",
                file.source_path,
                file.mirror_file.display(),
                file.content_bytes
            ));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 验证命令\n\n");
    if record.verification_commands.is_empty() {
        markdown.push_str("- 无。\n\n");
    } else {
        for command in &record.verification_commands {
            markdown.push_str(&format!("- `{command}`\n"));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 验证结果\n\n");
    if record.verification_runs.is_empty() {
        markdown.push_str("- 尚未执行验证命令。\n\n");
    } else {
        for run in &record.verification_runs {
            markdown.push_str(&format!(
                "- 命令：`{}`；状态：{}；退出码：{}；超时：{}；耗时毫秒：{}；标准输出字节：{}；标准错误字节：{}\n",
                run.command,
                run.status,
                run.exit_code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "无".to_string()),
                if run.timed_out { "是" } else { "否" },
                run.duration_ms,
                run.stdout_bytes,
                run.stderr_bytes
            ));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 回滚准备\n\n");
    markdown.push_str(&format!("- {}\n", record.rollback_hint));
    markdown.push_str("- 任何验证失败都必须先保留本记录，再通过回滚或阻断流程恢复稳定版本。\n");
    markdown
}

fn build_ai_patch_source_plan_markdown(record: &AiPatchSourcePlanRecord) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖准备记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 候选版本：{}\n- 候选应用记录：{}\n- 预演记录：{}\n- 审计记录：{}\n- 草案记录：{}\n- 准备状态：{}\n- 计划目录：{}\n- 错误信息：{}\n\n",
        record.version,
        record.candidate_version,
        record.application_id,
        record.preview_id,
        record.audit_id,
        record.draft_id,
        record.status,
        record
            .plan_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "无".to_string()),
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 前置条件\n\n");
    for prerequisite in &record.prerequisites {
        markdown.push_str(&format!("- {prerequisite}\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 覆盖计划\n\n");
    if record.files.is_empty() {
        markdown.push_str("- 本次没有可覆盖文件。\n\n");
    } else {
        for file in &record.files {
            markdown.push_str(&format!(
                "- 来源路径：{}；镜像文件：{}；目标文件：{}；目标已存在：{}；原始字节：{}；新字节：{}\n",
                file.source_path,
                file.mirror_file.display(),
                file.target_file.display(),
                if file.target_exists { "是" } else { "否" },
                file.original_bytes,
                file.new_bytes
            ));
            markdown.push_str(&format!("  - 差异摘要：{}\n", file.diff_summary));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 回滚清单\n\n");
    if record.rollback_steps.is_empty() {
        markdown.push_str("- 本次没有回滚步骤。\n\n");
    } else {
        for step in &record.rollback_steps {
            markdown.push_str(&format!("- {step}\n"));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 下一步\n\n");
    markdown.push_str("- 该记录只允许作为源码覆盖前的人工复核材料，禁止视为已经覆盖源码。\n");
    markdown.push_str("- 真实覆盖源码必须继续通过独立命令执行，并在完成后重新运行完整验证。\n");
    markdown
}

fn build_ai_patch_source_execution_markdown(record: &AiPatchSourceExecutionRecord) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖执行记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 候选版本：{}\n- 覆盖准备记录：{}\n- 候选应用记录：{}\n- 预演记录：{}\n- 审计记录：{}\n- 草案记录：{}\n- 执行状态：{}\n- 验证状态：{}\n- 是否回滚：{}\n- 执行目录：{}\n- 错误信息：{}\n\n",
        record.version,
        record.candidate_version,
        record.source_plan_id,
        record.application_id,
        record.preview_id,
        record.audit_id,
        record.draft_id,
        record.status,
        record.verification_status,
        if record.rollback_performed { "是" } else { "否" },
        record
            .execution_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "无".to_string()),
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 覆盖文件\n\n");
    if record.files.is_empty() {
        markdown.push_str("- 本次没有执行源码覆盖。\n\n");
    } else {
        for file in &record.files {
            markdown.push_str(&format!(
                "- 来源路径：{}；镜像文件：{}；目标文件：{}；动作：{}；覆盖前存在：{}；覆盖前字节：{}；覆盖后字节：{}；执行级备份：{}\n",
                file.source_path,
                file.mirror_file.display(),
                file.target_file.display(),
                file.action,
                if file.target_existed_before { "是" } else { "否" },
                file.before_bytes,
                file.after_bytes,
                file.execution_backup_file
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "无".to_string())
            ));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 验证结果\n\n");
    if record.verification_runs.is_empty() {
        markdown.push_str("- 本次没有执行验证命令。\n\n");
    } else {
        for run in &record.verification_runs {
            markdown.push_str(&format!(
                "- 命令：`{}`；状态：{}；退出码：{}；超时：{}；耗时毫秒：{}；标准输出字节：{}；标准错误字节：{}\n",
                run.command,
                run.status,
                run.exit_code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "无".to_string()),
                if run.timed_out { "是" } else { "否" },
                run.duration_ms,
                run.stdout_bytes,
                run.stderr_bytes
            ));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 回滚记录\n\n");
    if record.rollback_steps.is_empty() {
        markdown.push_str("- 本次没有执行回滚。\n\n");
    } else {
        for step in &record.rollback_steps {
            markdown.push_str(&format!("- {step}\n"));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 下一步\n\n");
    if record.status == AiPatchSourceExecutionStatus::Applied {
        markdown.push_str("- 覆盖已通过验证，可进入版本提升衔接记录和后续候选生成。\n");
    } else if record.status == AiPatchSourceExecutionStatus::RolledBack {
        markdown.push_str("- 覆盖已回滚，必须先分析验证失败原因，再重新生成补丁或准备记录。\n");
    } else {
        markdown.push_str("- 覆盖被阻断，必须先修复阻断原因，再重新准备或执行。\n");
    }
    markdown
}

fn build_ai_patch_source_promotion_markdown(record: &AiPatchSourcePromotionRecord) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖提升衔接记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 下一候选版本：{}\n- 源码覆盖执行：{}\n- 覆盖准备记录：{}\n- 候选应用记录：{}\n- 预演记录：{}\n- 审计记录：{}\n- 草案记录：{}\n- 衔接状态：{}\n- 验证状态：{}\n- 验证运行数：{}\n- 覆盖文件数：{}\n- 是否回滚：{}\n- 错误信息：{}\n\n",
        record.version,
        record.next_candidate_version,
        record.source_execution_id,
        record.source_plan_id,
        record.application_id,
        record.preview_id,
        record.audit_id,
        record.draft_id,
        record.status,
        record.verification_status,
        record.verification_run_count,
        record.file_count,
        if record.rollback_performed { "是" } else { "否" },
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 下一候选目标\n\n");
    markdown.push_str(&record.next_candidate_goal);
    markdown.push_str("\n\n");

    markdown.push_str("# 就绪检查\n\n");
    if record.readiness_checks.is_empty() {
        markdown.push_str("- 无通过项。\n");
    } else {
        for check in &record.readiness_checks {
            markdown.push_str(&format!("- {check}\n"));
        }
    }
    markdown.push('\n');

    markdown.push_str("# 验证摘要\n\n");
    if record.verification_commands.is_empty() {
        markdown.push_str("- 无验证命令。\n\n");
    } else {
        for command in &record.verification_commands {
            markdown.push_str(&format!("- {command}\n"));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 变更文件\n\n");
    if record.changed_files.is_empty() {
        markdown.push_str("- 无覆盖文件。\n\n");
    } else {
        for file in &record.changed_files {
            markdown.push_str(&format!("- {file}\n"));
        }
        markdown.push('\n');
    }

    markdown.push_str("# 提交信息\n\n");
    if let Some(title) = &record.suggested_commit_title {
        markdown.push_str(&format!("- 标题：{title}\n"));
    } else {
        markdown.push_str("- 标题：无，当前记录未就绪。\n");
    }
    if let Some(body) = &record.suggested_commit_body {
        markdown.push_str("- 正文：\n\n");
        markdown.push_str(body);
        markdown.push_str("\n\n");
    } else {
        markdown.push_str("- 正文：无，当前记录未就绪。\n\n");
    }

    markdown.push_str("# 下一步\n\n");
    if record.status == AiPatchSourcePromotionStatus::Ready {
        markdown.push_str("- 可以基于本记录进入下一候选生成、完整验证和受控提交。\n");
    } else {
        markdown.push_str("- 必须先修复阻断原因，再重新执行源码覆盖或生成提升衔接记录。\n");
    }
    markdown
}

fn build_ai_patch_source_candidate_markdown(record: &AiPatchSourceCandidateRecord) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖候选准备记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 提升衔接记录：{}\n- 源码覆盖执行：{}\n- 覆盖准备记录：{}\n- 候选应用记录：{}\n- 候选版本：{}\n- 准备状态：{}\n- 候选工作区：{}\n- 候选验证路径数：{}\n- 新建路径数：{}\n- 已有路径数：{}\n- 错误信息：{}\n\n",
        record.version,
        record.promotion_id,
        record.source_execution_id,
        record.source_plan_id,
        record.application_id,
        record.candidate_version,
        record.status,
        record
            .candidate_workspace
            .as_deref()
            .unwrap_or("无"),
        record.candidate_checked_path_count,
        record.created_path_count,
        record.existing_path_count,
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 状态变化\n\n");
    markdown.push_str(&format!(
        "- 准备前稳定版本：{}\n- 准备前状态：{}\n- 准备前候选版本：{}\n- 准备后稳定版本：{}\n- 准备后状态：{}\n- 准备后候选版本：{}\n\n",
        record.stable_version_before,
        record.state_status_before,
        record.candidate_version_before.as_deref().unwrap_or("无"),
        record.stable_version_after,
        record.state_status_after,
        record.candidate_version_after.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 候选目标\n\n");
    markdown.push_str(&record.candidate_goal);
    markdown.push_str("\n\n");

    markdown.push_str("# 就绪检查\n\n");
    if record.readiness_checks.is_empty() {
        markdown.push_str("- 无通过项。\n");
    } else {
        for check in &record.readiness_checks {
            markdown.push_str(&format!("- {check}\n"));
        }
    }
    markdown.push('\n');

    markdown.push_str("# 后续命令\n\n");
    for command in &record.follow_up_commands {
        markdown.push_str(&format!("- `{command}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 下一步\n\n");
    match record.status {
        AiPatchSourceCandidateStatus::Prepared | AiPatchSourceCandidateStatus::Reused => {
            markdown.push_str(
                "- 候选版本已经可验证，必须继续执行预检、候选验证、受控 cycle 和归档更新。\n",
            );
        }
        AiPatchSourceCandidateStatus::Blocked => {
            markdown.push_str(
                "- 候选准备被阻断，必须先修复阻断原因，再重新基于提升衔接记录准备候选。\n",
            );
        }
    }
    markdown
}

fn build_ai_patch_source_cycle_markdown(record: &AiPatchSourceCycleRecord) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖候选 cycle 记录\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 候选准备记录：{}\n- 提升衔接记录：{}\n- 源码覆盖执行：{}\n- 候选版本：{}\n- cycle 状态：{}\n- cycle 结果：{}\n- 错误信息：{}\n\n",
        record.version,
        record.candidate_record_id,
        record.promotion_id,
        record.source_execution_id,
        record.candidate_version,
        record.status,
        record
            .cycle_result
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "无".to_string()),
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 状态变化\n\n");
    markdown.push_str(&format!(
        "- cycle 前稳定版本：{}\n- cycle 前状态：{}\n- cycle 前候选版本：{}\n- cycle 后稳定版本：{}\n- cycle 后状态：{}\n- cycle 后候选版本：{}\n\n",
        record.stable_version_before,
        record.state_status_before,
        record.candidate_version_before.as_deref().unwrap_or("无"),
        record.stable_version_after,
        record.state_status_after,
        record.candidate_version_after.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 预检摘要\n\n");
    markdown.push_str(&format!(
        "- 当前版本检查路径数：{}\n- 候选版本检查路径数：{}\n- 是否允许继续：{}\n- 开放错误数：{}\n\n",
        record.preflight_current_checked_path_count,
        record.preflight_candidate_checked_path_count,
        if record.preflight_can_advance { "是" } else { "否" },
        record.open_error_count
    ));

    markdown.push_str("# cycle 结果\n\n");
    markdown.push_str(&format!(
        "- 候选验证路径数：{}\n- 回滚或失败原因：{}\n\n",
        record.cycle_candidate_checked_path_count,
        record.failure.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 就绪检查\n\n");
    if record.readiness_checks.is_empty() {
        markdown.push_str("- 无通过项。\n");
    } else {
        for check in &record.readiness_checks {
            markdown.push_str(&format!("- {check}\n"));
        }
    }
    markdown.push('\n');

    markdown.push_str("# 后续命令\n\n");
    for command in &record.follow_up_commands {
        markdown.push_str(&format!("- `{command}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 下一步\n\n");
    match record.status {
        AiPatchSourceCycleStatus::Promoted => {
            markdown.push_str(
                "- 候选版本已提升为稳定版本，继续执行预检、记忆压缩和下一轮最小任务选择。\n",
            );
        }
        AiPatchSourceCycleStatus::RolledBack => {
            markdown.push_str("- 候选版本已回滚，必须分析失败原因并重新生成候选准备链路。\n");
        }
        AiPatchSourceCycleStatus::Blocked => {
            markdown.push_str(
                "- 候选 cycle 被阻断，必须先修复阻断原因，再重新执行同一条候选准备记录。\n",
            );
        }
    }
    markdown
}

fn patch_source_cycle_follow_up_goal(
    cycle: &AiPatchSourceCycleRecord,
    status: AiPatchSourceCycleFollowUpStatus,
) -> String {
    match status {
        AiPatchSourceCycleFollowUpStatus::Promoted => format!(
            "基于源码覆盖 cycle {} 的提升结果，执行记忆压缩、开放错误检查，并生成下一轮最小 patch 任务。",
            cycle.id
        ),
        AiPatchSourceCycleFollowUpStatus::RolledBack => format!(
            "基于源码覆盖 cycle {} 的回滚原因修复候选链路，重新准备候选并验证。",
            cycle.id
        ),
        AiPatchSourceCycleFollowUpStatus::Blocked => format!(
            "修复源码覆盖 cycle {} 的阻断条件，再重新执行同一候选准备记录。",
            cycle.id
        ),
    }
}

fn patch_source_cycle_follow_up_task(
    cycle: &AiPatchSourceCycleRecord,
    status: AiPatchSourceCycleFollowUpStatus,
) -> String {
    match status {
        AiPatchSourceCycleFollowUpStatus::Promoted => format!(
            "为稳定版本 {} 生成下一轮 patch 级任务，并把 cycle 总结写入记忆和任务队列。",
            cycle.stable_version_after
        ),
        AiPatchSourceCycleFollowUpStatus::RolledBack => {
            "定位候选验证失败原因，修复后重新执行源码覆盖候选准备和 cycle。".to_string()
        }
        AiPatchSourceCycleFollowUpStatus::Blocked => {
            "修复阻断条件，重新执行 `agent-patch-source-cycle`。".to_string()
        }
    }
}

fn patch_source_cycle_follow_up_commands(
    cycle: &AiPatchSourceCycleRecord,
    status: AiPatchSourceCycleFollowUpStatus,
) -> Vec<String> {
    match status {
        AiPatchSourceCycleFollowUpStatus::Promoted => vec![
            "cargo run -- preflight".to_string(),
            "cargo run -- memory-compact --current --keep 5".to_string(),
            "cargo run -- memory-insights --current --limit 5".to_string(),
            "cargo run -- agent-work-init --current --threads 3 \"源码覆盖 cycle 后续协作\""
                .to_string(),
        ],
        AiPatchSourceCycleFollowUpStatus::RolledBack => vec![
            "cargo run -- preflight".to_string(),
            "cargo run -- errors --current --open --limit 5".to_string(),
            format!(
                "cargo run -- agent-patch-source-candidate --version {} {}",
                cycle.version, cycle.promotion_id
            ),
            format!(
                "cargo run -- agent-patch-source-cycle --version {} {}",
                cycle.version, cycle.candidate_record_id
            ),
        ],
        AiPatchSourceCycleFollowUpStatus::Blocked => vec![
            "cargo run -- preflight".to_string(),
            format!(
                "cargo run -- agent-patch-source-cycle --version {} {}",
                cycle.version, cycle.candidate_record_id
            ),
        ],
    }
}

fn build_ai_patch_source_cycle_summary_markdown(
    record: &AiPatchSourceCycleFollowUpRecord,
    cycle: &AiPatchSourceCycleRecord,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖 cycle 后续总结\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- cycle 记录：{}\n- 候选准备记录：{}\n- 提升衔接记录：{}\n- 候选版本：{}\n- 后续状态：{}\n- cycle 结果：{}\n\n",
        record.version,
        record.cycle_id,
        record.candidate_record_id,
        record.promotion_id,
        record.candidate_version,
        record.status,
        record
            .cycle_result
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "无".to_string())
    ));

    markdown.push_str("# cycle 摘要\n\n");
    markdown.push_str(&format!(
        "- cycle 前稳定版本：{}\n- cycle 前状态：{}\n- cycle 后稳定版本：{}\n- cycle 后状态：{}\n- cycle 后候选版本：{}\n- 候选验证路径数：{}\n- 失败或阻断原因：{}\n\n",
        cycle.stable_version_before,
        cycle.state_status_before,
        record.stable_version_after,
        record.state_status_after,
        record.candidate_version_after.as_deref().unwrap_or("无"),
        cycle.cycle_candidate_checked_path_count,
        record.failure.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 状态与风险\n\n");
    markdown.push_str(&format!(
        "- 预检是否允许继续：{}\n- 开放错误数：{}\n- 是否建议压缩记忆：{}\n\n",
        if record.preflight_can_advance {
            "是"
        } else {
            "否"
        },
        record.open_error_count,
        if record.memory_compaction_recommended {
            "是"
        } else {
            "否"
        }
    ));

    markdown.push_str("# 记忆与任务建议\n\n");
    markdown.push_str(&format!(
        "- 下一目标：{}\n- 下一任务：{}\n\n",
        record.next_goal, record.next_task
    ));

    markdown.push_str("# 后续命令\n\n");
    for command in &record.follow_up_commands {
        markdown.push_str(&format!("- `{command}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 下一步\n\n");
    match record.status {
        AiPatchSourceCycleFollowUpStatus::Promoted => {
            markdown.push_str(
                "- 已完成提升，应先压缩记忆并读取经验，再进入下一轮 patch 级任务选择。\n",
            );
        }
        AiPatchSourceCycleFollowUpStatus::RolledBack => {
            markdown.push_str("- 已完成回滚，应优先修复失败原因，再重新准备候选和执行 cycle。\n");
        }
        AiPatchSourceCycleFollowUpStatus::Blocked => {
            markdown.push_str(
                "- cycle 被阻断，应先修复状态、预检或开放错误问题，再重新执行候选 cycle。\n",
            );
        }
    }
    markdown
}

fn patch_source_task_draft_title(
    source_status: AiPatchSourceCycleFollowUpStatus,
    status: AiPatchSourceTaskDraftStatus,
    suggested_target_version: &str,
) -> String {
    if status == AiPatchSourceTaskDraftStatus::Blocked {
        return "修复源码覆盖后续总结中的任务草案输入".to_string();
    }

    match source_status {
        AiPatchSourceCycleFollowUpStatus::Promoted => {
            format!("生成 {suggested_target_version} 下一轮 patch 级自我升级任务草案")
        }
        AiPatchSourceCycleFollowUpStatus::RolledBack => {
            "生成源码覆盖回滚后的修复任务草案".to_string()
        }
        AiPatchSourceCycleFollowUpStatus::Blocked => "生成源码覆盖阻断后的修复任务草案".to_string(),
    }
}

fn patch_source_task_draft_description(
    source_status: AiPatchSourceCycleFollowUpStatus,
    next_goal: &str,
    next_task: &str,
) -> String {
    let source_context = match source_status {
        AiPatchSourceCycleFollowUpStatus::Promoted => "来源总结显示候选版本已提升",
        AiPatchSourceCycleFollowUpStatus::RolledBack => "来源总结显示候选版本已回滚",
        AiPatchSourceCycleFollowUpStatus::Blocked => "来源总结显示候选 cycle 被阻断",
    };
    format!("{source_context}。下一目标：{next_goal} 下一任务：{next_task}")
}

fn patch_source_task_draft_acceptance_checks() -> Vec<String> {
    vec![
        "cargo fmt --check".to_string(),
        "cargo test".to_string(),
        "cargo run -- validate".to_string(),
        "cargo run -- preflight".to_string(),
        "cargo run -- errors --current --open --limit 5".to_string(),
    ]
}

fn patch_source_task_draft_follow_up_commands(
    suggested_target_version: &str,
    proposed_task_title: &str,
) -> Vec<String> {
    vec![
        "cargo run -- preflight".to_string(),
        format!("cargo run -- agent-plan \"{proposed_task_title}\""),
        format!(
            "cargo run -- agent-patch-draft \"执行 {suggested_target_version} 的受控最小任务\""
        ),
        "cargo run -- agent-patch-source-task-draft-record TASK_DRAFT_ID".to_string(),
    ]
}

fn build_ai_patch_source_task_draft_markdown(
    record: &AiPatchSourceTaskDraftRecord,
    summary: &AiPatchSourceCycleFollowUpRecord,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖下一任务草案\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 来源总结：{}\n- 来源 cycle：{}\n- 草案状态：{}\n- 来源状态：{}\n- 提升后稳定版本：{}\n- 建议目标版本：{}\n- 是否需要审计：{}\n- 错误信息：{}\n\n",
        record.version,
        record.summary_id,
        record.cycle_id,
        record.status,
        record.source_status,
        record.stable_version_after,
        record.suggested_target_version,
        if record.required_audit { "是" } else { "否" },
        record.error.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 来源总结\n\n");
    markdown.push_str(&format!(
        "- 来源总结文件：{}\n- 来源 JSON 文件：{}\n- 来源下一目标：{}\n- 来源下一任务：{}\n\n",
        summary.markdown_file.display(),
        summary.file.display(),
        summary.next_goal,
        summary.next_task
    ));

    markdown.push_str("# 任务草案\n\n");
    markdown.push_str(&format!(
        "- 草案标题：{}\n- 草案描述：{}\n\n",
        record.proposed_task_title, record.proposed_task_description
    ));

    markdown.push_str("# 验收检查\n\n");
    for check in &record.acceptance_checks {
        markdown.push_str(&format!("- `{check}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 后续命令\n\n");
    for command in &record.follow_up_commands {
        markdown.push_str(&format!("- `{command}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 下一步\n\n");
    if record.status == AiPatchSourceTaskDraftStatus::Blocked {
        markdown.push_str(
            "- 草案输入不完整，必须先补齐来源总结中的下一目标和下一任务，再重新生成草案。\n",
        );
    } else {
        markdown.push_str(
            "- 该记录只是任务草案，必须先经过人工或自动审计，再进入真实补丁生成和源码覆盖流程。\n",
        );
    }
    markdown
}

fn patch_source_task_audit_findings(
    task_draft: &AiPatchSourceTaskDraftRecord,
    expected_target_version: &str,
) -> Vec<AiPatchSourceTaskAuditFinding> {
    let required_checks = patch_source_task_draft_acceptance_checks();
    let mut findings = Vec::new();

    findings.push(AiPatchSourceTaskAuditFinding {
        check: "草案状态".to_string(),
        passed: task_draft.status == AiPatchSourceTaskDraftStatus::Drafted,
        message: if task_draft.status == AiPatchSourceTaskDraftStatus::Drafted {
            "草案状态允许进入审计。".to_string()
        } else {
            "草案状态不是已生成草案，禁止进入补丁草案流程。".to_string()
        },
    });
    findings.push(AiPatchSourceTaskAuditFinding {
        check: "审计要求".to_string(),
        passed: task_draft.required_audit,
        message: if task_draft.required_audit {
            "草案要求审计，符合流程。".to_string()
        } else {
            "草案未标记为需要审计，禁止批准。".to_string()
        },
    });
    findings.push(AiPatchSourceTaskAuditFinding {
        check: "目标版本".to_string(),
        passed: task_draft.suggested_target_version == expected_target_version,
        message: if task_draft.suggested_target_version == expected_target_version {
            "建议目标版本符合 patch 递增规则。".to_string()
        } else {
            format!(
                "建议目标版本应为 {}，当前为 {}。",
                expected_target_version, task_draft.suggested_target_version
            )
        },
    });
    findings.push(non_empty_task_audit_finding(
        "草案标题",
        &task_draft.proposed_task_title,
        "草案标题存在。",
        "草案标题为空，禁止批准。",
    ));
    findings.push(non_empty_task_audit_finding(
        "草案描述",
        &task_draft.proposed_task_description,
        "草案描述存在。",
        "草案描述为空，禁止批准。",
    ));
    findings.push(non_empty_task_audit_finding(
        "来源目标",
        &task_draft.source_next_goal,
        "来源目标存在。",
        "来源目标为空，禁止批准。",
    ));
    findings.push(non_empty_task_audit_finding(
        "来源任务",
        &task_draft.source_next_task,
        "来源任务存在。",
        "来源任务为空，禁止批准。",
    ));
    for check in required_checks {
        let passed = task_draft
            .acceptance_checks
            .iter()
            .any(|existing| existing == &check);
        findings.push(AiPatchSourceTaskAuditFinding {
            check: format!("验收检查 {check}"),
            passed,
            message: if passed {
                format!("已包含 `{check}`。")
            } else {
                format!("缺少必要验收检查 `{check}`。")
            },
        });
    }
    let has_agent_plan = task_draft
        .follow_up_commands
        .iter()
        .any(|command| command.contains("agent-plan"));
    findings.push(AiPatchSourceTaskAuditFinding {
        check: "后续计划命令".to_string(),
        passed: has_agent_plan,
        message: if has_agent_plan {
            "后续命令包含 `agent-plan`。".to_string()
        } else {
            "后续命令缺少 `agent-plan`，禁止批准。".to_string()
        },
    });
    let has_patch_draft = task_draft
        .follow_up_commands
        .iter()
        .any(|command| command.contains("agent-patch-draft"));
    findings.push(AiPatchSourceTaskAuditFinding {
        check: "后续补丁草案命令".to_string(),
        passed: has_patch_draft,
        message: if has_patch_draft {
            "后续命令包含 `agent-patch-draft`。".to_string()
        } else {
            "后续命令缺少 `agent-patch-draft`，禁止批准。".to_string()
        },
    });

    findings
}

fn non_empty_task_audit_finding(
    check: &str,
    value: &str,
    passed_message: &str,
    failed_message: &str,
) -> AiPatchSourceTaskAuditFinding {
    let passed = normalize_optional_text(value).is_some();
    AiPatchSourceTaskAuditFinding {
        check: check.to_string(),
        passed,
        message: if passed {
            passed_message.to_string()
        } else {
            failed_message.to_string()
        },
    }
}

fn patch_source_task_audit_goal(task_draft: &AiPatchSourceTaskDraftRecord) -> String {
    format!(
        "{}：{}",
        task_draft.proposed_task_title, task_draft.source_next_task
    )
}

fn patch_source_task_audit_follow_up_commands(
    status: AiPatchSourceTaskAuditStatus,
    task_draft: &AiPatchSourceTaskDraftRecord,
    approved_goal: &str,
) -> Vec<String> {
    match status {
        AiPatchSourceTaskAuditStatus::Approved => vec![
            "cargo run -- preflight".to_string(),
            format!(
                "cargo run -- agent-plan \"{}\"",
                task_draft.proposed_task_title
            ),
            format!("cargo run -- agent-patch-draft \"{approved_goal}\""),
            "cargo run -- agent-patch-source-task-audit-record TASK_AUDIT_ID".to_string(),
        ],
        AiPatchSourceTaskAuditStatus::Blocked => vec![
            "cargo run -- preflight".to_string(),
            format!(
                "cargo run -- agent-patch-source-task-draft-record {}",
                task_draft.id
            ),
        ],
    }
}

fn build_ai_patch_source_task_audit_markdown(
    record: &AiPatchSourceTaskAuditRecord,
    task_draft: &AiPatchSourceTaskDraftRecord,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# AI 补丁源码覆盖任务草案审计\n\n");
    markdown.push_str("# 基本信息\n\n");
    markdown.push_str(&format!(
        "- 源版本：{}\n- 任务草案：{}\n- 来源总结：{}\n- 来源 cycle：{}\n- 审计状态：{}\n- 草案状态：{}\n- 建议目标版本：{}\n- 阻断原因：{}\n\n",
        record.version,
        record.task_draft_id,
        record.summary_id,
        record.cycle_id,
        record.status,
        record.source_task_status,
        record.suggested_target_version,
        record.blocked_reason.as_deref().unwrap_or("无")
    ));

    markdown.push_str("# 来源草案\n\n");
    markdown.push_str(&format!(
        "- 草案文件：{}\n- 草案 JSON：{}\n- 草案标题：{}\n- 草案描述：{}\n- 来源目标：{}\n- 来源任务：{}\n\n",
        task_draft.markdown_file.display(),
        task_draft.file.display(),
        task_draft.proposed_task_title,
        task_draft.proposed_task_description,
        task_draft.source_next_goal,
        task_draft.source_next_task
    ));

    markdown.push_str("# 审计发现\n\n");
    for finding in &record.findings {
        markdown.push_str(&format!(
            "- {}：{}，{}\n",
            finding.check,
            if finding.passed {
                "通过"
            } else {
                "未通过"
            },
            finding.message
        ));
    }
    markdown.push('\n');

    markdown.push_str("# 批准目标\n\n");
    markdown.push_str(&format!("- {}\n\n", record.approved_goal));

    markdown.push_str("# 后续命令\n\n");
    for command in &record.follow_up_commands {
        markdown.push_str(&format!("- `{command}`\n"));
    }
    markdown.push('\n');

    markdown.push_str("# 下一步\n\n");
    match record.status {
        AiPatchSourceTaskAuditStatus::Approved => {
            markdown.push_str(
                "- 草案已通过审计，可以在再次预检后进入 AI 补丁草案流程，后续仍必须执行测试和验证。\n",
            );
        }
        AiPatchSourceTaskAuditStatus::Blocked => {
            markdown
                .push_str("- 草案未通过审计，必须先修复草案或重新生成任务草案，再重新执行审计。\n");
        }
    }
    markdown
}

#[derive(Debug)]
struct PatchWriteScopeAudit {
    normalized_write_scope: Vec<String>,
    findings: Vec<AiPatchAuditFinding>,
}

fn extract_patch_audit_write_scope(markdown: &str) -> Vec<String> {
    let mut in_scope_section = false;
    let mut scopes = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = markdown_heading_title(trimmed) {
            if in_scope_section && !title.contains("允许写入范围") && !title.contains("写入范围")
            {
                break;
            }
            in_scope_section = title.contains("允许写入范围") || title == "写入范围";
            continue;
        }
        if !in_scope_section || trimmed.is_empty() || trimmed.starts_with("```") {
            continue;
        }
        for scope in split_patch_scope_line(trimmed) {
            if !scope.is_empty() {
                scopes.push(scope);
            }
        }
    }

    scopes
}

fn audit_patch_write_scope(
    requested_write_scope: &[String],
    version: &str,
) -> Result<PatchWriteScopeAudit, AiPatchAuditError> {
    let mut findings = Vec::new();
    let mut normalized_write_scope = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if requested_write_scope.is_empty() {
        findings.push(AiPatchAuditFinding {
            severity: AiPatchAuditSeverity::Error,
            kind: AiPatchAuditFindingKind::MissingWriteScope,
            message: "补丁草案缺少可审计的允许写入范围。".to_string(),
            path: None,
            task_id: None,
            task_title: None,
            worker_id: None,
        });
    }

    let protected_roots = patch_audit_protected_roots(version)?;
    for raw_scope in requested_write_scope {
        match normalize_patch_scope_path(raw_scope) {
            Ok(scope) => {
                if patch_scope_is_protected(&scope, &protected_roots) {
                    findings.push(AiPatchAuditFinding {
                        severity: AiPatchAuditSeverity::Error,
                        kind: AiPatchAuditFindingKind::ProtectedPath,
                        message: "补丁草案请求修改受保护路径。".to_string(),
                        path: Some(scope.clone()),
                        task_id: None,
                        task_title: None,
                        worker_id: None,
                    });
                }
                if seen.insert(scope.clone()) {
                    normalized_write_scope.push(scope);
                }
            }
            Err(reason) => findings.push(AiPatchAuditFinding {
                severity: AiPatchAuditSeverity::Error,
                kind: AiPatchAuditFindingKind::InvalidPath,
                message: reason,
                path: Some(raw_scope.clone()),
                task_id: None,
                task_title: None,
                worker_id: None,
            }),
        }
    }

    Ok(PatchWriteScopeAudit {
        normalized_write_scope,
        findings,
    })
}

fn audit_patch_scope_conflicts(
    normalized_write_scope: &[String],
    queue_report: &AgentWorkQueueReport,
) -> Vec<AiPatchAuditFinding> {
    let mut findings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for task in queue_report
        .queue
        .tasks
        .iter()
        .filter(|task| task.status == AgentWorkTaskStatus::Claimed)
    {
        for requested_scope in normalized_write_scope {
            if scopes_overlap_one_to_many(requested_scope, &task.write_scope) {
                let key = format!("{}:{requested_scope}", task.id);
                if seen.insert(key) {
                    findings.push(AiPatchAuditFinding {
                        severity: AiPatchAuditSeverity::Error,
                        kind: AiPatchAuditFindingKind::ActiveConflict,
                        message: "补丁草案写入范围与已领取协作任务重叠。".to_string(),
                        path: Some(requested_scope.clone()),
                        task_id: Some(task.id.clone()),
                        task_title: Some(task.title.clone()),
                        worker_id: task.claimed_by.clone(),
                    });
                }
            }
        }
    }

    findings
}

fn patch_audit_protected_roots(version: &str) -> Result<Vec<String>, AiPatchAuditError> {
    let major = version_major_key(version).map_err(AiPatchAuditError::Version)?;
    Ok(vec![
        ".git/".to_string(),
        ".env".to_string(),
        "runtime/".to_string(),
        "supervisor/".to_string(),
        "state/".to_string(),
        "target/".to_string(),
        format!("workspaces/{major}/sandbox/"),
        format!("workspaces/{major}/logs/"),
    ])
}

fn markdown_heading_title(line: &str) -> Option<String> {
    if !line.starts_with('#') {
        return None;
    }
    let title = line.trim_start_matches('#').trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn split_patch_scope_line(line: &str) -> Vec<String> {
    let mut value = line
        .trim()
        .trim_start_matches(|character| matches!(character, '-' | '*' | '+'))
        .trim()
        .to_string();
    while value
        .chars()
        .next()
        .map(|character| character.is_ascii_digit() || matches!(character, '.' | ')' | '、'))
        .unwrap_or(false)
    {
        value = value
            .chars()
            .skip(1)
            .collect::<String>()
            .trim_start()
            .to_string();
    }
    value = value
        .replace('`', "")
        .replace('"', "")
        .replace('\'', "")
        .replace('“', "")
        .replace('”', "")
        .trim()
        .to_string();

    value
        .split(|character| matches!(character, '，' | ',' | '；' | ';'))
        .filter_map(scope_candidate_from_segment)
        .collect()
}

fn scope_candidate_from_segment(segment: &str) -> Option<String> {
    let mut value = segment.trim();
    if value.is_empty() || matches!(value, "无" | "暂无") {
        return None;
    }
    if let Some((left, right)) = value.split_once('：') {
        if !looks_like_path(left) || left.contains("路径") || left.contains("文件") {
            value = right.trim();
        }
    } else if let Some((left, right)) = value.split_once(':') {
        if !looks_like_path(left) || left.contains("path") || left.contains("file") {
            value = right.trim();
        }
    }
    let first = value.split_whitespace().next().unwrap_or("").trim();
    let first = first
        .trim_matches(|character| matches!(character, '。' | '，' | ',' | '；' | ';' | '：' | ':'));
    if first.is_empty() || matches!(first, "无" | "暂无") {
        None
    } else {
        Some(first.to_string())
    }
}

fn looks_like_path(value: &str) -> bool {
    let value = value.trim();
    value.contains('/')
        || value.contains('\\')
        || value.starts_with('.')
        || value.ends_with(".rs")
        || value.ends_with(".md")
        || matches!(
            value,
            "Cargo.toml" | "Cargo.lock" | "README.md" | "Agents.md"
        )
}

fn normalize_patch_scope_path(value: &str) -> Result<String, String> {
    let mut scope = value
        .trim()
        .replace('\\', "/")
        .trim_matches(|character| matches!(character, '`' | '"' | '\'' | '“' | '”'))
        .trim()
        .to_string();
    while scope.starts_with("./") {
        scope = scope.trim_start_matches("./").to_string();
    }
    scope = scope.trim_end_matches('/').to_string();
    if scope.is_empty() {
        return Err("写入范围为空。".to_string());
    }
    if scope.starts_with('/') || scope.starts_with('~') || scope.chars().nth(1) == Some(':') {
        return Err("写入范围必须是仓库相对路径，禁止使用绝对路径。".to_string());
    }
    if scope
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("写入范围包含非法路径片段。".to_string());
    }
    if scope
        .chars()
        .any(|character| matches!(character, '<' | '>' | '|' | '?' | '*'))
    {
        return Err("写入范围包含非法文件名字符。".to_string());
    }

    Ok(scope)
}

fn patch_scope_is_protected(scope: &str, protected_roots: &[String]) -> bool {
    protected_roots.iter().any(|root| {
        let root = normalize_scope_for_compare(root);
        let scope = normalize_scope_for_compare(scope);
        scope == root || scope.starts_with(&(root + "/"))
    })
}

fn scopes_overlap_one_to_many(left: &str, right: &[String]) -> bool {
    right
        .iter()
        .any(|right_scope| scopes_overlap_pair(left, right_scope))
}

fn scopes_overlap_pair(left: &str, right: &str) -> bool {
    let left = normalize_scope_for_compare(left);
    let right = normalize_scope_for_compare(right);
    left == right || left.starts_with(&(right.clone() + "/")) || right.starts_with(&(left + "/"))
}

fn normalize_scope_for_compare(scope: &str) -> String {
    scope
        .trim()
        .trim_end_matches(|character| matches!(character, '/' | '\\'))
        .replace('\\', "/")
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

fn contains_chinese_text(value: &str) -> bool {
    value
        .chars()
        .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
}

fn has_markdown_section(value: &str, section: &str) -> bool {
    value.lines().any(|line| {
        let line = line.trim_start();
        let line = line.trim_start_matches('#').trim_start();
        line.starts_with(section)
    })
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
            AiSelfUpgradeError::Summary(error) => {
                write!(formatter, "AI 自我升级总结报告失败：{error}")
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
            AiSelfUpgradeError::Summary(error) => Some(error),
            AiSelfUpgradeError::Blocked { .. } | AiSelfUpgradeError::EmptyGoal { .. } => None,
        }
    }
}

impl fmt::Display for AiSelfUpgradeSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiSelfUpgradeSummaryError::Audit(error) => {
                write!(formatter, "读取自我升级审计记录失败：{error}")
            }
            AiSelfUpgradeSummaryError::Session(error) => {
                write!(formatter, "读取自我升级会话失败：{error}")
            }
            AiSelfUpgradeSummaryError::Store(error) => {
                write!(formatter, "写入自我升级总结报告失败：{error}")
            }
        }
    }
}

impl Error for AiSelfUpgradeSummaryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiSelfUpgradeSummaryError::Audit(error) => Some(error),
            AiSelfUpgradeSummaryError::Session(error) => Some(error),
            AiSelfUpgradeSummaryError::Store(error) => Some(error),
        }
    }
}

impl fmt::Display for AiPatchDraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchDraftError::Preflight(error) => {
                write!(formatter, "AI 补丁草案预检失败：{error}")
            }
            AiPatchDraftError::Memory(error) => {
                write!(formatter, "AI 补丁草案读取记忆失败：{error}")
            }
            AiPatchDraftError::Ai(error) => write!(formatter, "AI 补丁草案请求失败：{error}"),
            AiPatchDraftError::Store(error) => {
                write!(formatter, "AI 补丁草案记录失败：{error}")
            }
            AiPatchDraftError::TaskAudit(error) => {
                write!(formatter, "AI 补丁草案读取任务草案审计失败：{error}")
            }
            AiPatchDraftError::Version(error) => write!(formatter, "{error}"),
            AiPatchDraftError::Blocked {
                version,
                open_errors,
            } => write!(
                formatter,
                "版本 {version} 存在 {} 条未解决错误，AI 补丁草案已停止",
                open_errors.len()
            ),
            AiPatchDraftError::TaskAuditNotApproved {
                id,
                status,
                blocked_reason,
            } => write!(
                formatter,
                "任务草案审计 {id} 状态为 {status}，禁止生成 AI 补丁草案，原因：{}",
                blocked_reason.as_deref().unwrap_or("未记录")
            ),
            AiPatchDraftError::EmptyTaskAuditGoal { id } => {
                write!(
                    formatter,
                    "任务草案审计 {id} 的批准目标为空，禁止生成 AI 补丁草案"
                )
            }
            AiPatchDraftError::InvalidDraft {
                reason,
                response_preview,
            } => write!(
                formatter,
                "AI 补丁草案响应不合规：{reason}，响应摘要：{response_preview}"
            ),
        }
    }
}

impl Error for AiPatchDraftError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchDraftError::Preflight(error) => Some(error),
            AiPatchDraftError::Memory(error) => Some(error),
            AiPatchDraftError::Ai(error) => Some(error),
            AiPatchDraftError::Store(error) => Some(error),
            AiPatchDraftError::TaskAudit(error) => Some(error),
            AiPatchDraftError::Version(error) => Some(error),
            AiPatchDraftError::Blocked { .. }
            | AiPatchDraftError::TaskAuditNotApproved { .. }
            | AiPatchDraftError::EmptyTaskAuditGoal { .. }
            | AiPatchDraftError::InvalidDraft { .. } => None,
        }
    }
}

impl fmt::Display for AiPatchAuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchAuditError::Draft(error) => {
                write!(formatter, "AI 补丁审计读取草案失败：{error}")
            }
            AiPatchAuditError::Store(error) => write!(formatter, "AI 补丁审计记录失败：{error}"),
            AiPatchAuditError::WorkQueue(error) => {
                write!(formatter, "AI 补丁审计读取协作队列失败：{error}")
            }
            AiPatchAuditError::Version(error) => write!(formatter, "{error}"),
            AiPatchAuditError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁审计读取文件失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchAuditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchAuditError::Draft(error) => Some(error),
            AiPatchAuditError::Store(error) => Some(error),
            AiPatchAuditError::WorkQueue(error) => Some(error),
            AiPatchAuditError::Version(error) => Some(error),
            AiPatchAuditError::Io { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for AiPatchPreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchPreviewError::Audit(error) => {
                write!(formatter, "AI 补丁应用预演读取审计失败：{error}")
            }
            AiPatchPreviewError::Draft(error) => {
                write!(formatter, "AI 补丁应用预演读取草案失败：{error}")
            }
            AiPatchPreviewError::Store(error) => {
                write!(formatter, "AI 补丁应用预演记录失败：{error}")
            }
            AiPatchPreviewError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁应用预演读取文件失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchPreviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchPreviewError::Audit(error) => Some(error),
            AiPatchPreviewError::Draft(error) => Some(error),
            AiPatchPreviewError::Store(error) => Some(error),
            AiPatchPreviewError::Io { source, .. } => Some(source),
        }
    }
}

impl fmt::Display for AiPatchApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchApplicationError::Preflight(error) => {
                write!(formatter, "AI 补丁候选应用前置检查失败：{error}")
            }
            AiPatchApplicationError::Evolution(error) => {
                write!(formatter, "AI 补丁候选应用准备候选版本失败：{error}")
            }
            AiPatchApplicationError::Preview(error) => {
                write!(formatter, "AI 补丁候选应用读取预演失败：{error}")
            }
            AiPatchApplicationError::Draft(error) => {
                write!(formatter, "AI 补丁候选应用读取草案失败：{error}")
            }
            AiPatchApplicationError::Store(error) => {
                write!(formatter, "AI 补丁候选应用记录失败：{error}")
            }
            AiPatchApplicationError::Version(error) => write!(formatter, "{error}"),
            AiPatchApplicationError::Forge(error) => {
                write!(formatter, "AI 补丁候选应用验证失败：{error}")
            }
            AiPatchApplicationError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁候选应用文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchApplicationError::InvalidPath { path, reason } => {
                write!(formatter, "AI 补丁候选应用路径不合法 {path}：{reason}")
            }
        }
    }
}

impl Error for AiPatchApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchApplicationError::Preflight(error) => Some(error),
            AiPatchApplicationError::Evolution(error) => Some(error),
            AiPatchApplicationError::Preview(error) => Some(error),
            AiPatchApplicationError::Draft(error) => Some(error),
            AiPatchApplicationError::Store(error) => Some(error),
            AiPatchApplicationError::Version(error) => Some(error),
            AiPatchApplicationError::Forge(error) => Some(error),
            AiPatchApplicationError::Io { source, .. } => Some(source),
            AiPatchApplicationError::InvalidPath { .. } => None,
        }
    }
}

impl fmt::Display for AiPatchVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchVerificationError::Store(error) => {
                write!(formatter, "AI 补丁候选应用验证记录失败：{error}")
            }
            AiPatchVerificationError::Preview(error) => {
                write!(formatter, "AI 补丁候选应用验证读取预演失败：{error}")
            }
            AiPatchVerificationError::UnsupportedCommand(command) => {
                write!(formatter, "AI 补丁候选应用验证命令不受支持：{command}")
            }
            AiPatchVerificationError::Io { path, source } => write!(
                formatter,
                "AI 补丁候选应用验证执行失败 {}：{}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for AiPatchVerificationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchVerificationError::Store(error) => Some(error),
            AiPatchVerificationError::Preview(error) => Some(error),
            AiPatchVerificationError::Io { source, .. } => Some(source),
            AiPatchVerificationError::UnsupportedCommand(_) => None,
        }
    }
}

impl fmt::Display for AiPatchSourcePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourcePlanError::Application(error) => {
                write!(formatter, "AI 补丁源码覆盖准备读取候选应用失败：{error}")
            }
            AiPatchSourcePlanError::Store(error) => {
                write!(formatter, "AI 补丁源码覆盖准备记录失败：{error}")
            }
            AiPatchSourcePlanError::Io { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖准备文件读写失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourcePlanError::InvalidPath { path, reason } => {
                write!(formatter, "AI 补丁源码覆盖准备路径不合法 {path}：{reason}")
            }
        }
    }
}

impl Error for AiPatchSourcePlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourcePlanError::Application(error) => Some(error),
            AiPatchSourcePlanError::Store(error) => Some(error),
            AiPatchSourcePlanError::Io { source, .. } => Some(source),
            AiPatchSourcePlanError::InvalidPath { .. } => None,
        }
    }
}

impl fmt::Display for AiPatchSourceExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceExecutionError::SourcePlan(error) => {
                write!(formatter, "AI 补丁源码覆盖执行读取准备记录失败：{error}")
            }
            AiPatchSourceExecutionError::Store(error) => {
                write!(formatter, "AI 补丁源码覆盖执行记录失败：{error}")
            }
            AiPatchSourceExecutionError::Verification(error) => {
                write!(formatter, "AI 补丁源码覆盖执行验证失败：{error}")
            }
            AiPatchSourceExecutionError::Io { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖执行文件读写失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceExecutionError::InvalidPath { path, reason } => {
                write!(formatter, "AI 补丁源码覆盖执行路径不合法 {path}：{reason}")
            }
        }
    }
}

impl Error for AiPatchSourceExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceExecutionError::SourcePlan(error) => Some(error),
            AiPatchSourceExecutionError::Store(error) => Some(error),
            AiPatchSourceExecutionError::Verification(error) => Some(error),
            AiPatchSourceExecutionError::Io { source, .. } => Some(source),
            AiPatchSourceExecutionError::InvalidPath { .. } => None,
        }
    }
}

impl fmt::Display for AiPatchSourcePromotionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourcePromotionError::SourceExecution(error) => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖提升衔接读取执行记录失败：{error}"
                )
            }
            AiPatchSourcePromotionError::Store(error) => {
                write!(formatter, "AI 补丁源码覆盖提升衔接记录失败：{error}")
            }
            AiPatchSourcePromotionError::Version(error) => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖提升衔接计算下一版本失败：{error}"
                )
            }
        }
    }
}

impl Error for AiPatchSourcePromotionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourcePromotionError::SourceExecution(error) => Some(error),
            AiPatchSourcePromotionError::Store(error) => Some(error),
            AiPatchSourcePromotionError::Version(error) => Some(error),
        }
    }
}

impl fmt::Display for AiPatchSourceCandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCandidateError::Promotion(error) => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖候选准备读取衔接记录失败：{error}"
                )
            }
            AiPatchSourceCandidateError::Store(error) => {
                write!(formatter, "AI 补丁源码覆盖候选准备记录失败：{error}")
            }
            AiPatchSourceCandidateError::State(error) => {
                write!(formatter, "AI 补丁源码覆盖候选准备读取状态失败：{error}")
            }
            AiPatchSourceCandidateError::ErrorArchive(error) => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖候选准备查询开放错误失败：{error}"
                )
            }
            AiPatchSourceCandidateError::Version(error) => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖候选准备计算下一版本失败：{error}"
                )
            }
        }
    }
}

impl Error for AiPatchSourceCandidateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceCandidateError::Promotion(error) => Some(error),
            AiPatchSourceCandidateError::Store(error) => Some(error),
            AiPatchSourceCandidateError::State(error) => Some(error),
            AiPatchSourceCandidateError::ErrorArchive(error) => Some(error),
            AiPatchSourceCandidateError::Version(error) => Some(error),
        }
    }
}

impl fmt::Display for AiPatchSourceCycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCycleError::Candidate(error) => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖候选 cycle 读取候选准备记录失败：{error}"
                )
            }
            AiPatchSourceCycleError::Store(error) => {
                write!(formatter, "AI 补丁源码覆盖候选 cycle 记录失败：{error}")
            }
            AiPatchSourceCycleError::State(error) => {
                write!(formatter, "AI 补丁源码覆盖候选 cycle 读取状态失败：{error}")
            }
        }
    }
}

impl Error for AiPatchSourceCycleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceCycleError::Candidate(error) => Some(error),
            AiPatchSourceCycleError::Store(error) => Some(error),
            AiPatchSourceCycleError::State(error) => Some(error),
        }
    }
}

impl fmt::Display for AiPatchSourceCycleSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCycleSummaryError::Cycle(error) => {
                write!(formatter, "读取源码覆盖 cycle 记录失败：{error}")
            }
            AiPatchSourceCycleSummaryError::Store(error) => {
                write!(formatter, "写入源码覆盖 cycle 后续总结失败：{error}")
            }
        }
    }
}

impl Error for AiPatchSourceCycleSummaryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceCycleSummaryError::Cycle(error) => Some(error),
            AiPatchSourceCycleSummaryError::Store(error) => Some(error),
        }
    }
}

impl fmt::Display for AiPatchSourceTaskDraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceTaskDraftError::Summary(error) => {
                write!(formatter, "读取源码覆盖 cycle 后续总结失败：{error}")
            }
            AiPatchSourceTaskDraftError::Store(error) => {
                write!(formatter, "写入源码覆盖下一任务草案失败：{error}")
            }
            AiPatchSourceTaskDraftError::Version(error) => {
                write!(formatter, "计算源码覆盖下一任务目标版本失败：{error}")
            }
        }
    }
}

impl Error for AiPatchSourceTaskDraftError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceTaskDraftError::Summary(error) => Some(error),
            AiPatchSourceTaskDraftError::Store(error) => Some(error),
            AiPatchSourceTaskDraftError::Version(error) => Some(error),
        }
    }
}

impl fmt::Display for AiPatchSourceTaskAuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceTaskAuditError::TaskDraft(error) => {
                write!(formatter, "读取源码覆盖下一任务草案失败：{error}")
            }
            AiPatchSourceTaskAuditError::Store(error) => {
                write!(formatter, "写入源码覆盖任务草案审计失败：{error}")
            }
            AiPatchSourceTaskAuditError::Version(error) => {
                write!(formatter, "计算源码覆盖任务草案审计目标版本失败：{error}")
            }
        }
    }
}

impl Error for AiPatchSourceTaskAuditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceTaskAuditError::TaskDraft(error) => Some(error),
            AiPatchSourceTaskAuditError::Store(error) => Some(error),
            AiPatchSourceTaskAuditError::Version(error) => Some(error),
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

impl fmt::Display for AgentStepRunStop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStepRunStop::SessionCompleted => write!(formatter, "会话已完成"),
            AgentStepRunStop::StepLimitReached => write!(formatter, "达到最大步数"),
            AgentStepRunStop::NoPendingStep { session_id } => {
                write!(formatter, "会话 {session_id} 没有待执行步骤")
            }
            AgentStepRunStop::InputRequired {
                step_order,
                tool_id,
                input,
            } => write!(
                formatter,
                "步骤 {step_order} 的工具 {tool_id} 需要输入 {input}"
            ),
            AgentStepRunStop::NoRunnableTool { step_order } => {
                write!(formatter, "步骤 {step_order} 没有可自动执行的工具")
            }
            AgentStepRunStop::Failed { message } => write!(formatter, "执行失败：{message}"),
        }
    }
}

impl fmt::Display for AgentStepRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStepRunError::InvalidStepLimit => {
                write!(formatter, "受控多步运行的最大步数必须大于 0")
            }
            AgentStepRunError::Step(error) => {
                write!(formatter, "受控多步运行初始化失败：{error}")
            }
        }
    }
}

impl Error for AgentStepRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentStepRunError::InvalidStepLimit => None,
            AgentStepRunError::Step(error) => Some(error),
        }
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
