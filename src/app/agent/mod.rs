mod code_tools;
mod coordination;
mod patch_application;
mod patch_audit;
mod patch_draft;
mod patch_preview;
mod patch_source_candidate;
mod patch_source_cycle;
mod patch_source_cycle_summary;
mod patch_source_execution;
mod patch_source_plan;
mod patch_source_promotion;
mod patch_source_task_audit;
mod patch_source_task_draft;
mod registry;
mod self_upgrade_audit;
mod self_upgrade_summary;
mod session;
mod skills;
mod tool_invocation;
mod tools;
mod types;

pub use code_tools::{
    AgentCodeDiffReport, AgentCodeListEntry, AgentCodeListReport, AgentCodeReadReport,
    AgentCodeSearchMatch, AgentCodeSearchReport, AgentCodeToolError, inspect_project_code_diff,
    list_project_code_files, read_project_code_file, search_project_code,
};
pub use coordination::{
    AgentWorkClaimReport, AgentWorkCompactionReport, AgentWorkCoordinator, AgentWorkError,
    AgentWorkEvent, AgentWorkQueue, AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTask,
    AgentWorkTaskStatus,
};
pub use patch_application::{
    AiPatchApplicationFile, AiPatchApplicationRecord, AiPatchApplicationStatus,
    AiPatchApplicationStore, AiPatchApplicationStoreError, AiPatchApplicationSummary,
    AiPatchVerificationCommandRecord, AiPatchVerificationStatus,
};
pub use patch_audit::{
    AiPatchAuditFinding, AiPatchAuditFindingKind, AiPatchAuditRecord, AiPatchAuditSeverity,
    AiPatchAuditStatus, AiPatchAuditStore, AiPatchAuditStoreError, AiPatchAuditSummary,
};
pub use patch_draft::{
    AiPatchDraftRecord, AiPatchDraftStatus, AiPatchDraftStore, AiPatchDraftStoreError,
    AiPatchDraftSummary,
};
pub use patch_preview::{
    AiPatchPreviewChange, AiPatchPreviewRecord, AiPatchPreviewStatus, AiPatchPreviewStore,
    AiPatchPreviewStoreError, AiPatchPreviewSummary,
};
pub use patch_source_candidate::{
    AiPatchSourceCandidateRecord, AiPatchSourceCandidateStatus, AiPatchSourceCandidateStore,
    AiPatchSourceCandidateStoreError, AiPatchSourceCandidateSummary,
};
pub use patch_source_cycle::{
    AiPatchSourceCycleRecord, AiPatchSourceCycleResult, AiPatchSourceCycleStatus,
    AiPatchSourceCycleStore, AiPatchSourceCycleStoreError, AiPatchSourceCycleSummary,
};
pub use patch_source_cycle_summary::{
    AiPatchSourceCycleFollowUpRecord, AiPatchSourceCycleFollowUpStatus,
    AiPatchSourceCycleFollowUpStore, AiPatchSourceCycleFollowUpStoreError,
    AiPatchSourceCycleFollowUpSummary,
};
pub use patch_source_execution::{
    AiPatchSourceExecutionFile, AiPatchSourceExecutionRecord, AiPatchSourceExecutionStatus,
    AiPatchSourceExecutionStore, AiPatchSourceExecutionStoreError, AiPatchSourceExecutionSummary,
};
pub use patch_source_plan::{
    AiPatchSourcePlanFile, AiPatchSourcePlanRecord, AiPatchSourcePlanStatus,
    AiPatchSourcePlanStore, AiPatchSourcePlanStoreError, AiPatchSourcePlanSummary,
};
pub use patch_source_promotion::{
    AiPatchSourcePromotionRecord, AiPatchSourcePromotionStatus, AiPatchSourcePromotionStore,
    AiPatchSourcePromotionStoreError, AiPatchSourcePromotionSummary,
};
pub use patch_source_task_audit::{
    AiPatchSourceTaskAuditFinding, AiPatchSourceTaskAuditRecord, AiPatchSourceTaskAuditStatus,
    AiPatchSourceTaskAuditStore, AiPatchSourceTaskAuditStoreError, AiPatchSourceTaskAuditSummary,
};
pub use patch_source_task_draft::{
    AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftStatus, AiPatchSourceTaskDraftStore,
    AiPatchSourceTaskDraftStoreError, AiPatchSourceTaskDraftSummary,
};
pub use registry::AgentRegistry;
pub use self_upgrade_audit::{
    AiSelfUpgradeAuditError, AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditStatus,
    AiSelfUpgradeAuditStore, AiSelfUpgradeAuditSummary,
};
pub use self_upgrade_summary::{
    AiSelfUpgradeSummaryIndexEntry, AiSelfUpgradeSummaryRecord, AiSelfUpgradeSummaryStatus,
    AiSelfUpgradeSummaryStore, AiSelfUpgradeSummaryStoreError,
};
pub use session::{
    AgentRunReference, AgentSession, AgentSessionError, AgentSessionEvent, AgentSessionEventKind,
    AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus, AgentSessionStep,
    AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext, AgentStepStatus,
};
pub use skills::{
    AgentSkillError, AgentSkillIndex, AgentSkillIndexReport, AgentSkillMetadata,
    AgentSkillSelection, AgentSkillSelectionReport, AgentSkillSelectionRequest,
    format_agent_skill_context, initialize_agent_skill_index, load_agent_skill_index,
    select_agent_skills,
};
pub use tool_invocation::{
    AgentStepExecutionReport, AgentStepExecutionRequest, AgentToolInvocation,
    AgentToolInvocationInput, AgentToolInvocationReport,
};
pub use tools::{
    AgentToolAssignment, AgentToolBinding, AgentToolConfig, AgentToolConfigInitReport,
    AgentToolDefinition, AgentToolError, AgentToolReport, apply_tools_to_plan,
    initialize_agent_tool_config, load_agent_tool_report,
};
pub use types::{AgentCapability, AgentDefinition, AgentError, AgentPlan, AgentPlanStep};
