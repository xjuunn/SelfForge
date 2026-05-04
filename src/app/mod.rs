mod agent;
mod ai_provider;
mod error_archive;
mod memory;
mod minimal_loop;

pub use agent::{
    AgentCapability, AgentDefinition, AgentError, AgentPlan, AgentPlanStep, AgentRegistry,
    AgentRunReference, AgentSession, AgentSessionError, AgentSessionEvent, AgentSessionEventKind,
    AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus, AgentSessionStep,
    AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext, AgentStepExecutionReport,
    AgentStepExecutionRequest, AgentStepStatus, AgentToolAssignment, AgentToolBinding,
    AgentToolConfig, AgentToolConfigInitReport, AgentToolDefinition, AgentToolError,
    AgentToolInvocation, AgentToolInvocationInput, AgentToolInvocationReport, AgentToolReport,
    AgentWorkClaimReport, AgentWorkCompactionReport, AgentWorkCoordinator, AgentWorkError,
    AgentWorkEvent, AgentWorkQueue, AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTask,
    AgentWorkTaskStatus, AiPatchApplicationFile, AiPatchApplicationRecord,
    AiPatchApplicationStatus, AiPatchApplicationStore, AiPatchApplicationStoreError,
    AiPatchApplicationSummary, AiPatchAuditFinding, AiPatchAuditFindingKind, AiPatchAuditRecord,
    AiPatchAuditSeverity, AiPatchAuditStatus, AiPatchAuditStore, AiPatchAuditStoreError,
    AiPatchAuditSummary, AiPatchDraftRecord, AiPatchDraftStatus, AiPatchDraftStore,
    AiPatchDraftStoreError, AiPatchDraftSummary, AiPatchPreviewChange, AiPatchPreviewRecord,
    AiPatchPreviewStatus, AiPatchPreviewStore, AiPatchPreviewStoreError, AiPatchPreviewSummary,
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
};
pub use ai_provider::{
    AiConfigError, AiConfigReport, AiExecutionError, AiExecutionReport, AiProviderRegistry,
    AiProviderStatus, AiRawHttpResponse, AiRequestError, AiRequestSpec, AiResponseError,
    AiTextResponse,
};
pub use error_archive::{
    ArchivedErrorEntry, ErrorArchive, ErrorArchiveError, ErrorArchiveReport, ErrorListQuery,
    ErrorResolutionReport,
};
pub use memory::{
    MemoryCompactionError, MemoryCompactionReport, MemoryContextEntry, MemoryContextError,
    MemoryContextReport, MemoryInsight, MemoryInsightReport,
};
pub use minimal_loop::{
    AgentEvolutionError, AgentEvolutionReport, AgentPlanReport, AgentPlanReportError,
    AgentRunError, AgentRunReport, AgentSingleEvolutionReport, AgentStepExecutionError,
    AgentStepRunError, AgentStepRunReport, AgentStepRunStop, AgentToolInvocationError,
    AgentVerificationReport, AgentWorkFinalizeCheckError, AgentWorkFinalizeCheckReport,
    AiPatchApplicationError, AiPatchApplicationReport, AiPatchAuditError, AiPatchAuditReport,
    AiPatchDraftError, AiPatchDraftPreview, AiPatchDraftReport, AiPatchPreviewError,
    AiPatchPreviewReport, AiPatchSourceCandidateError, AiPatchSourceCandidateReport,
    AiPatchSourceCycleError, AiPatchSourceCycleReport, AiPatchSourceCycleSummaryError,
    AiPatchSourceCycleSummaryReport, AiPatchSourceExecutionError, AiPatchSourceExecutionReport,
    AiPatchSourcePlanError, AiPatchSourcePlanReport, AiPatchSourcePromotionError,
    AiPatchSourcePromotionReport, AiPatchSourceTaskAuditError, AiPatchSourceTaskAuditReport,
    AiPatchSourceTaskDraftError, AiPatchSourceTaskDraftReport, AiPatchVerificationCommandSpec,
    AiPatchVerificationError, AiPatchVerificationReport, AiSelfUpgradeError, AiSelfUpgradePreview,
    AiSelfUpgradeReport, AiSelfUpgradeSummaryError, AiSelfUpgradeSummaryReport, BranchCheckError,
    BranchCheckReport, MinimalLoopError, MinimalLoopOutcome, MinimalLoopReport, PreflightReport,
    SelfForgeApp, normalize_ai_self_upgrade_goal,
};
