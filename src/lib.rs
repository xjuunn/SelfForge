pub mod app;
pub mod documentation;
pub mod evolution;
pub mod layout;
pub mod runtime;
pub mod state;
pub mod supervisor;
pub mod version;

pub use documentation::{
    DocumentationError, DocumentationReport, DocumentationViolation, validate_chinese_markdown,
};
pub use evolution::{
    CycleReport, CycleResult, EvolutionEngine, EvolutionError, EvolutionReport, PromotionReport,
    RollbackReport,
};
pub use layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport};
pub use runtime::{ExecutionError, ExecutionReport, RunIndexEntry, RunQuery, Runtime};
pub use state::{ForgeState, StateError};
pub use supervisor::Supervisor;
pub use version::{
    ForgeVersion, VersionBump, VersionError, next_version_after, next_version_after_with_bump,
    version_major_file_name, version_major_key,
};

pub const CURRENT_VERSION: &str = "v0.1.65";

pub use app::{
    AgentCapability, AgentDefinition, AgentError, AgentEvolutionError, AgentEvolutionReport,
    AgentPlan, AgentPlanReport, AgentPlanReportError, AgentPlanStep, AgentRegistry, AgentRunError,
    AgentRunReference, AgentRunReport, AgentSession, AgentSessionError, AgentSessionEvent,
    AgentSessionEventKind, AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus,
    AgentSessionStep, AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext,
    AgentSingleEvolutionReport, AgentStepExecutionError, AgentStepExecutionReport,
    AgentStepExecutionRequest, AgentStepRunError, AgentStepRunReport, AgentStepRunStop,
    AgentStepStatus, AgentToolAssignment, AgentToolBinding, AgentToolConfig,
    AgentToolConfigInitReport, AgentToolDefinition, AgentToolError, AgentToolInvocation,
    AgentToolInvocationError, AgentToolInvocationInput, AgentToolInvocationReport, AgentToolReport,
    AgentVerificationReport, AgentWorkClaimReport, AgentWorkCompactionReport, AgentWorkCoordinator,
    AgentWorkError, AgentWorkEvent, AgentWorkQueue, AgentWorkQueueReport, AgentWorkReapReport,
    AgentWorkTask, AgentWorkTaskStatus, AiConfigError, AiConfigReport, AiExecutionError,
    AiExecutionReport, AiPatchApplicationError, AiPatchApplicationFile, AiPatchApplicationRecord,
    AiPatchApplicationReport, AiPatchApplicationStatus, AiPatchApplicationStore,
    AiPatchApplicationStoreError, AiPatchApplicationSummary, AiPatchAuditError,
    AiPatchAuditFinding, AiPatchAuditFindingKind, AiPatchAuditRecord, AiPatchAuditReport,
    AiPatchAuditSeverity, AiPatchAuditStatus, AiPatchAuditStore, AiPatchAuditStoreError,
    AiPatchAuditSummary, AiPatchDraftError, AiPatchDraftPreview, AiPatchDraftRecord,
    AiPatchDraftReport, AiPatchDraftStatus, AiPatchDraftStore, AiPatchDraftStoreError,
    AiPatchDraftSummary, AiPatchPreviewChange, AiPatchPreviewError, AiPatchPreviewRecord,
    AiPatchPreviewReport, AiPatchPreviewStatus, AiPatchPreviewStore, AiPatchPreviewStoreError,
    AiPatchPreviewSummary, AiPatchSourceCandidateError, AiPatchSourceCandidateRecord,
    AiPatchSourceCandidateReport, AiPatchSourceCandidateStatus, AiPatchSourceCandidateStore,
    AiPatchSourceCandidateStoreError, AiPatchSourceCandidateSummary, AiPatchSourceCycleError,
    AiPatchSourceCycleFollowUpRecord, AiPatchSourceCycleFollowUpStatus,
    AiPatchSourceCycleFollowUpStore, AiPatchSourceCycleFollowUpStoreError,
    AiPatchSourceCycleFollowUpSummary, AiPatchSourceCycleRecord, AiPatchSourceCycleReport,
    AiPatchSourceCycleResult, AiPatchSourceCycleStatus, AiPatchSourceCycleStore,
    AiPatchSourceCycleStoreError, AiPatchSourceCycleSummary, AiPatchSourceCycleSummaryError,
    AiPatchSourceCycleSummaryReport, AiPatchSourceExecutionError, AiPatchSourceExecutionFile,
    AiPatchSourceExecutionRecord, AiPatchSourceExecutionReport, AiPatchSourceExecutionStatus,
    AiPatchSourceExecutionStore, AiPatchSourceExecutionStoreError, AiPatchSourceExecutionSummary,
    AiPatchSourcePlanError, AiPatchSourcePlanFile, AiPatchSourcePlanRecord,
    AiPatchSourcePlanReport, AiPatchSourcePlanStatus, AiPatchSourcePlanStore,
    AiPatchSourcePlanStoreError, AiPatchSourcePlanSummary, AiPatchSourcePromotionError,
    AiPatchSourcePromotionRecord, AiPatchSourcePromotionReport, AiPatchSourcePromotionStatus,
    AiPatchSourcePromotionStore, AiPatchSourcePromotionStoreError, AiPatchSourcePromotionSummary,
    AiPatchSourceTaskAuditError, AiPatchSourceTaskAuditFinding, AiPatchSourceTaskAuditRecord,
    AiPatchSourceTaskAuditReport, AiPatchSourceTaskAuditStatus, AiPatchSourceTaskAuditStore,
    AiPatchSourceTaskAuditStoreError, AiPatchSourceTaskAuditSummary, AiPatchSourceTaskDraftError,
    AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftReport, AiPatchSourceTaskDraftStatus,
    AiPatchSourceTaskDraftStore, AiPatchSourceTaskDraftStoreError, AiPatchSourceTaskDraftSummary,
    AiPatchVerificationCommandRecord, AiPatchVerificationCommandSpec, AiPatchVerificationError,
    AiPatchVerificationReport, AiPatchVerificationStatus, AiProviderRegistry, AiProviderStatus,
    AiRawHttpResponse, AiRequestError, AiRequestSpec, AiResponseError, AiSelfUpgradeAuditError,
    AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditStatus, AiSelfUpgradeAuditStore,
    AiSelfUpgradeAuditSummary, AiSelfUpgradeError, AiSelfUpgradePreview, AiSelfUpgradeReport,
    AiSelfUpgradeSummaryError, AiSelfUpgradeSummaryIndexEntry, AiSelfUpgradeSummaryRecord,
    AiSelfUpgradeSummaryReport, AiSelfUpgradeSummaryStatus, AiSelfUpgradeSummaryStore,
    AiSelfUpgradeSummaryStoreError, AiTextResponse, ArchivedErrorEntry, BranchCheckError,
    BranchCheckReport, ErrorArchive, ErrorArchiveError, ErrorArchiveReport, ErrorListQuery,
    ErrorResolutionReport, MemoryCompactionError, MemoryCompactionReport, MemoryContextEntry,
    MemoryContextError, MemoryContextReport, MemoryInsight, MemoryInsightReport, MinimalLoopError,
    MinimalLoopOutcome, MinimalLoopReport, PreflightReport, SelfForgeApp,
    normalize_ai_self_upgrade_goal,
};

#[cfg(test)]
mod tests;
