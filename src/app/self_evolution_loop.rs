use super::agent::{
    AiPatchApplicationStatus, AiPatchAuditStatus, AiPatchDraftStatus, AiPatchPreviewStatus,
    AiPatchSourceCandidateStatus, AiPatchSourceCycleFollowUpStatus, AiPatchSourceCycleStatus,
    AiPatchSourceExecutionStatus, AiPatchSourcePlanStatus, AiPatchSourcePromotionStatus,
    AiPatchVerificationStatus,
};
use super::minimal_loop::SelfForgeApp;
#[cfg(test)]
use super::minimal_loop::{AiSelfUpgradeError, AiSelfUpgradeReport};
use crate::state::{ForgeState, StateError};
use crate::version::{VersionError, version_major_key};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod git_pr;

pub use git_pr::{
    SelfEvolutionLoopGitPrEvent, SelfEvolutionLoopGitPrEventStatus, SelfEvolutionLoopGitPrMode,
    SelfEvolutionLoopGitPrRequest,
};

const AGENT_ARTIFACT_DIRECTORY: &str = "agents";
const SELF_EVOLUTION_LOOP_DIRECTORY: &str = "self-evolution-loops";
const SELF_EVOLUTION_LOOP_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelfEvolutionLoopStatus {
    Running,
    Succeeded,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelfEvolutionLoopStepStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfEvolutionLoopRequest {
    pub hint: String,
    pub max_cycles: usize,
    pub max_failures: usize,
    pub timeout_ms: u64,
    pub resume: bool,
    #[serde(default)]
    pub git_pr: SelfEvolutionLoopGitPrRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfEvolutionLoopStepRecord {
    pub cycle: usize,
    pub status: SelfEvolutionLoopStepStatus,
    pub started_at_unix_seconds: u64,
    pub completed_at_unix_seconds: Option<u64>,
    pub stable_version_before: String,
    pub stable_version_after: Option<String>,
    pub audit_id: Option<String>,
    pub summary_id: Option<String>,
    #[serde(default)]
    pub phase_events: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_draft_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_audit_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_preview_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_application_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_source_plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_source_execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_source_promotion_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_source_candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_source_cycle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_source_summary_id: Option<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfEvolutionLoopRecord {
    pub id: String,
    pub version: String,
    pub status: SelfEvolutionLoopStatus,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub hint: String,
    pub max_cycles: usize,
    pub max_failures: usize,
    pub timeout_ms: u64,
    pub completed_cycles: usize,
    pub failed_cycles: usize,
    pub consecutive_failures: usize,
    pub resumed: bool,
    #[serde(default)]
    pub git_pr: SelfEvolutionLoopGitPrRequest,
    #[serde(default)]
    pub git_pr_events: Vec<SelfEvolutionLoopGitPrEvent>,
    #[serde(default)]
    pub pr_url: Option<String>,
    pub last_error: Option<String>,
    pub steps: Vec<SelfEvolutionLoopStepRecord>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfEvolutionLoopReport {
    pub record: SelfEvolutionLoopRecord,
    pub index_file: PathBuf,
    pub resumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfEvolutionLoopSummary {
    pub id: String,
    pub version: String,
    pub status: SelfEvolutionLoopStatus,
    pub updated_at_unix_seconds: u64,
    pub completed_cycles: usize,
    pub failed_cycles: usize,
    pub consecutive_failures: usize,
    pub git_pr_mode: SelfEvolutionLoopGitPrMode,
    pub pr_url: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum SelfEvolutionLoopError {
    InvalidRequest(String),
    State(StateError),
    Version(VersionError),
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    NotFound {
        version: String,
        id: String,
    },
    GitPr(git_pr::SelfEvolutionLoopGitPrError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfEvolutionLoopCycleReport {
    stable_version_after: String,
    audit_id: Option<String>,
    summary_id: Option<String>,
    phase_events: Vec<String>,
    patch_draft_id: Option<String>,
    patch_audit_id: Option<String>,
    patch_preview_id: Option<String>,
    patch_application_id: Option<String>,
    patch_source_plan_id: Option<String>,
    patch_source_execution_id: Option<String>,
    patch_source_promotion_id: Option<String>,
    patch_source_candidate_id: Option<String>,
    patch_source_cycle_id: Option<String>,
    patch_source_summary_id: Option<String>,
    changed_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfEvolutionLoopCycleError {
    message: String,
}

impl SelfForgeApp {
    pub fn run_self_evolution_loop(
        &self,
        request: SelfEvolutionLoopRequest,
    ) -> Result<SelfEvolutionLoopReport, SelfEvolutionLoopError> {
        self.run_self_evolution_loop_with_cycle_executor(request, |app, hint, timeout_ms| {
            app.run_coding_self_evolution_cycle(hint, timeout_ms)
        })
    }

    #[cfg(test)]
    pub(crate) fn run_self_evolution_loop_with_executor<F>(
        &self,
        request: SelfEvolutionLoopRequest,
        mut executor: F,
    ) -> Result<SelfEvolutionLoopReport, SelfEvolutionLoopError>
    where
        F: FnMut(&SelfForgeApp, &str, u64) -> Result<AiSelfUpgradeReport, AiSelfUpgradeError>,
    {
        self.run_self_evolution_loop_with_cycle_executor(request, |app, hint, timeout_ms| {
            executor(app, hint, timeout_ms)
                .map(SelfEvolutionLoopCycleReport::from_self_upgrade_report)
                .map_err(SelfEvolutionLoopCycleError::from)
        })
    }

    pub(crate) fn run_self_evolution_loop_with_cycle_executor<F>(
        &self,
        request: SelfEvolutionLoopRequest,
        mut executor: F,
    ) -> Result<SelfEvolutionLoopReport, SelfEvolutionLoopError>
    where
        F: FnMut(
            &SelfForgeApp,
            &str,
            u64,
        ) -> Result<SelfEvolutionLoopCycleReport, SelfEvolutionLoopCycleError>,
    {
        validate_self_evolution_loop_request(&request)?;
        let state = ForgeState::load(self.root()).map_err(SelfEvolutionLoopError::State)?;
        let store = SelfEvolutionLoopStore::new(self.root().to_path_buf(), &state.current_version)?;
        let mut resumed = false;
        let mut record = if request.resume {
            if let Some(mut record) = store.load_latest_running()? {
                resumed = true;
                record.resumed = true;
                let max_failures = record.max_failures;
                recover_interrupted_steps(&mut record, max_failures);
                record
            } else {
                new_self_evolution_loop_record(&request, &state.current_version, &store)
            }
        } else {
            new_self_evolution_loop_record(&request, &state.current_version, &store)
        };

        if resumed {
            git_pr::recover_interrupted_git_pr_events(&mut record);
        }
        git_pr::prepare_git_pr_flow(self.root(), &store, &mut record)?;
        store.save(&mut record)?;
        while record.status == SelfEvolutionLoopStatus::Running
            && record.completed_cycles < record.max_cycles
            && record.consecutive_failures < record.max_failures
        {
            let before_state =
                ForgeState::load(self.root()).map_err(SelfEvolutionLoopError::State)?;
            let cycle = record.steps.len() + 1;
            record.steps.push(SelfEvolutionLoopStepRecord {
                cycle,
                status: SelfEvolutionLoopStepStatus::Running,
                started_at_unix_seconds: current_unix_seconds(),
                completed_at_unix_seconds: None,
                stable_version_before: before_state.current_version.clone(),
                stable_version_after: None,
                audit_id: None,
                summary_id: None,
                phase_events: Vec::new(),
                patch_draft_id: None,
                patch_audit_id: None,
                patch_preview_id: None,
                patch_application_id: None,
                patch_source_plan_id: None,
                patch_source_execution_id: None,
                patch_source_promotion_id: None,
                patch_source_candidate_id: None,
                patch_source_cycle_id: None,
                patch_source_summary_id: None,
                changed_files: Vec::new(),
                error: None,
            });
            store.save(&mut record)?;

            let result = catch_unwind(AssertUnwindSafe(|| {
                executor(self, &record.hint, record.timeout_ms)
            }));
            match result {
                Ok(Ok(cycle_report)) => {
                    match complete_successful_self_evolution_step(
                        self.root(),
                        &store,
                        &mut record,
                        cycle_report,
                    ) {
                        Ok(()) => {
                            record.completed_cycles += 1;
                            record.consecutive_failures = 0;
                            record.last_error = None;
                        }
                        Err(error) => {
                            record_failed_self_evolution_step(&mut record, error.to_string());
                        }
                    }
                }
                Ok(Err(error)) => {
                    record_failed_self_evolution_step(&mut record, error.to_string());
                }
                Err(_) => {
                    record_failed_self_evolution_step(
                        &mut record,
                        "自我进化循环捕获到内部 panic，已记录失败并保持可恢复。".to_string(),
                    );
                }
            }

            if record.completed_cycles >= record.max_cycles {
                record.status = SelfEvolutionLoopStatus::Succeeded;
            } else if record.consecutive_failures >= record.max_failures {
                record.status = SelfEvolutionLoopStatus::Stopped;
            }
            store.save(&mut record)?;
        }

        if record.status == SelfEvolutionLoopStatus::Succeeded {
            if let Err(error) = git_pr::finalize_git_pr_flow(self, &store, &mut record) {
                record.status = SelfEvolutionLoopStatus::Stopped;
                record.last_error = Some(truncate_chars(&error.to_string(), 400));
                store.save(&mut record)?;
            }
        }

        Ok(SelfEvolutionLoopReport {
            record,
            index_file: store.index_path,
            resumed,
        })
    }

    pub fn self_evolution_loop_records(
        &self,
        version: &str,
        limit: usize,
    ) -> Result<Vec<SelfEvolutionLoopSummary>, SelfEvolutionLoopError> {
        let store = SelfEvolutionLoopStore::new(self.root().to_path_buf(), version)?;
        store.list(limit)
    }

    pub fn self_evolution_loop_record(
        &self,
        version: &str,
        id: &str,
    ) -> Result<SelfEvolutionLoopRecord, SelfEvolutionLoopError> {
        let store = SelfEvolutionLoopStore::new(self.root().to_path_buf(), version)?;
        store.load(id)
    }

    pub(crate) fn run_coding_self_evolution_cycle(
        &self,
        hint: &str,
        timeout_ms: u64,
    ) -> Result<SelfEvolutionLoopCycleReport, SelfEvolutionLoopCycleError> {
        let state = ForgeState::load(self.root())
            .map_err(|error| SelfEvolutionLoopCycleError::new(error.to_string()))?;
        let version = state.current_version.clone();
        let mut events = Vec::new();
        push_cycle_event(
            &mut events,
            format!("读取当前状态：稳定版本 {version}，准备执行真实编码循环。"),
        );

        let goal = build_coding_self_evolution_goal(self.root(), hint);
        push_cycle_event(
            &mut events,
            "已读取项目文件结构、Agents.md 和 README.md，并写入本轮 AI 补丁目标上下文。"
                .to_string(),
        );

        let draft = self.ai_patch_draft(&goal, timeout_ms).map_err(|error| {
            SelfEvolutionLoopCycleError::new(format!("AI 补丁草案失败：{error}"))
        })?;
        push_cycle_event(
            &mut events,
            format!(
                "AI 返回补丁草案：{}，状态 {:?}。",
                draft.record.id, draft.record.status
            ),
        );
        if draft.record.status != AiPatchDraftStatus::Succeeded {
            return Err(SelfEvolutionLoopCycleError::new(
                draft
                    .record
                    .error
                    .clone()
                    .unwrap_or_else(|| "AI 补丁草案未成功，停止本轮循环。".to_string()),
            ));
        }

        let audit = self
            .ai_patch_audit(&version, &draft.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("AI 补丁审计失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "完成补丁审计：{}，状态 {:?}。",
                audit.record.id, audit.record.status
            ),
        );
        if audit.record.status != AiPatchAuditStatus::Passed {
            let message = audit
                .record
                .findings
                .iter()
                .map(|finding| finding.message.clone())
                .collect::<Vec<_>>()
                .join("；");
            let message = if message.trim().is_empty() {
                "AI 补丁审计未通过，禁止进入源码修改。".to_string()
            } else {
                message
            };
            return Err(SelfEvolutionLoopCycleError::new(message));
        }

        let preview = self
            .ai_patch_preview(&version, &audit.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("AI 补丁预览失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "完成补丁预览：{}，状态 {:?}，变更 {} 个文件。",
                preview.record.id,
                preview.record.status,
                preview.record.changes.len()
            ),
        );
        if preview.record.status != AiPatchPreviewStatus::Previewed
            || preview.record.changes.is_empty()
        {
            return Err(SelfEvolutionLoopCycleError::new(
                preview
                    .record
                    .error
                    .clone()
                    .unwrap_or_else(|| "AI 补丁没有可预览的源码变更，禁止提升版本。".to_string()),
            ));
        }

        let application = self
            .ai_patch_apply(&version, &preview.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("AI 补丁应用失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "应用补丁到候选镜像：{}，状态 {:?}，文件 {} 个。",
                application.record.id,
                application.record.status,
                application.record.files.len()
            ),
        );
        if application.record.status != AiPatchApplicationStatus::Applied
            || application.record.files.is_empty()
        {
            return Err(SelfEvolutionLoopCycleError::new(
                application
                    .record
                    .error
                    .clone()
                    .unwrap_or_else(|| "AI 补丁未应用任何文件，禁止提升版本。".to_string()),
            ));
        }

        let verification = self
            .ai_patch_verify(&version, &application.record.id, timeout_ms)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("AI 补丁验证失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "候选镜像验证完成：状态 {:?}，执行命令 {} 条。",
                verification.status, verification.executed_count
            ),
        );
        if verification.status != AiPatchVerificationStatus::Passed {
            return Err(SelfEvolutionLoopCycleError::new(
                "AI 补丁验证未通过，禁止覆盖源码和提升版本。".to_string(),
            ));
        }

        let source_plan = self
            .ai_patch_source_plan(&version, &application.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("源码覆盖计划失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "源码覆盖计划完成：{}，状态 {:?}，文件 {} 个。",
                source_plan.record.id,
                source_plan.record.status,
                source_plan.record.files.len()
            ),
        );
        if source_plan.record.status != AiPatchSourcePlanStatus::Prepared
            || source_plan.record.files.is_empty()
        {
            return Err(SelfEvolutionLoopCycleError::new(
                source_plan
                    .record
                    .error
                    .clone()
                    .unwrap_or_else(|| "源码覆盖计划没有可执行文件，禁止提升版本。".to_string()),
            ));
        }

        let source_execution = self
            .ai_patch_source_execute(&version, &source_plan.record.id, timeout_ms)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("源码覆盖执行失败：{error}"))
            })?;
        let changed_files = source_execution
            .record
            .files
            .iter()
            .map(|file| file.source_path.clone())
            .collect::<Vec<_>>();
        push_cycle_event(
            &mut events,
            format!(
                "源码覆盖执行完成：{}，状态 {:?}，验证 {:?}，文件 {} 个。",
                source_execution.record.id,
                source_execution.record.status,
                source_execution.record.verification_status,
                changed_files.len()
            ),
        );
        if source_execution.record.status != AiPatchSourceExecutionStatus::Applied
            || source_execution.record.verification_status != AiPatchVerificationStatus::Passed
            || source_execution.record.rollback_performed
            || changed_files.is_empty()
        {
            return Err(SelfEvolutionLoopCycleError::new(
                source_execution.record.error.clone().unwrap_or_else(|| {
                    "源码覆盖未通过验证或没有真实文件变更，禁止提升版本。".to_string()
                }),
            ));
        }

        let promotion = self
            .ai_patch_source_promotion(&version, &source_execution.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("源码提升衔接失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "源码提升衔接完成：{}，状态 {:?}。",
                promotion.record.id, promotion.record.status
            ),
        );
        if promotion.record.status != AiPatchSourcePromotionStatus::Ready {
            return Err(SelfEvolutionLoopCycleError::new(
                promotion
                    .record
                    .error
                    .clone()
                    .unwrap_or_else(|| "源码提升衔接未就绪，禁止准备候选版本。".to_string()),
            ));
        }

        let candidate = self
            .ai_patch_source_candidate(&version, &promotion.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("候选版本准备失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "候选版本准备完成：{}，状态 {:?}，候选 {}。",
                candidate.record.id, candidate.record.status, candidate.record.candidate_version
            ),
        );
        if !matches!(
            candidate.record.status,
            AiPatchSourceCandidateStatus::Prepared | AiPatchSourceCandidateStatus::Reused
        ) {
            return Err(SelfEvolutionLoopCycleError::new(
                candidate
                    .record
                    .error
                    .clone()
                    .unwrap_or_else(|| "候选版本未准备完成，禁止执行版本循环。".to_string()),
            ));
        }

        let cycle = self
            .ai_patch_source_cycle(&version, &candidate.record.id)
            .map_err(|error| {
                SelfEvolutionLoopCycleError::new(format!("候选版本循环失败：{error}"))
            })?;
        push_cycle_event(
            &mut events,
            format!(
                "候选版本循环完成：{}，状态 {:?}，稳定版本 {} -> {}。",
                cycle.record.id,
                cycle.record.status,
                cycle.record.stable_version_before,
                cycle.record.stable_version_after
            ),
        );
        if cycle.record.status != AiPatchSourceCycleStatus::Promoted {
            return Err(SelfEvolutionLoopCycleError::new(
                cycle
                    .record
                    .error
                    .clone()
                    .or_else(|| cycle.record.failure.clone())
                    .unwrap_or_else(|| "候选版本未提升，停止本轮循环。".to_string()),
            ));
        }

        let summary = self
            .ai_patch_source_cycle_summary(&version, &cycle.record.id)
            .map_err(|error| SelfEvolutionLoopCycleError::new(format!("循环总结失败：{error}")))?;
        push_cycle_event(
            &mut events,
            format!(
                "循环总结完成：{}，状态 {:?}，下一任务建议已记录。",
                summary.record.id, summary.record.status
            ),
        );
        if summary.record.status != AiPatchSourceCycleFollowUpStatus::Promoted {
            return Err(SelfEvolutionLoopCycleError::new(
                summary
                    .record
                    .failure
                    .clone()
                    .unwrap_or_else(|| "循环总结未确认提升结果。".to_string()),
            ));
        }

        Ok(SelfEvolutionLoopCycleReport {
            stable_version_after: cycle.record.stable_version_after.clone(),
            audit_id: None,
            summary_id: None,
            phase_events: events,
            patch_draft_id: Some(draft.record.id),
            patch_audit_id: Some(audit.record.id),
            patch_preview_id: Some(preview.record.id),
            patch_application_id: Some(application.record.id),
            patch_source_plan_id: Some(source_plan.record.id),
            patch_source_execution_id: Some(source_execution.record.id),
            patch_source_promotion_id: Some(promotion.record.id),
            patch_source_candidate_id: Some(candidate.record.id),
            patch_source_cycle_id: Some(cycle.record.id),
            patch_source_summary_id: Some(summary.record.id),
            changed_files,
        })
    }
}

impl fmt::Display for SelfEvolutionLoopStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SelfEvolutionLoopStatus::Running => "运行中",
            SelfEvolutionLoopStatus::Succeeded => "已完成",
            SelfEvolutionLoopStatus::Stopped => "已停止",
        };
        formatter.write_str(text)
    }
}

impl fmt::Display for SelfEvolutionLoopStepStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SelfEvolutionLoopStepStatus::Running => "运行中",
            SelfEvolutionLoopStepStatus::Succeeded => "已成功",
            SelfEvolutionLoopStepStatus::Failed => "已失败",
        };
        formatter.write_str(text)
    }
}

impl fmt::Display for SelfEvolutionLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfEvolutionLoopError::InvalidRequest(message) => formatter.write_str(message),
            SelfEvolutionLoopError::State(error) => write!(formatter, "{error}"),
            SelfEvolutionLoopError::Version(error) => write!(formatter, "{error}"),
            SelfEvolutionLoopError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            SelfEvolutionLoopError::Json { path, source } => {
                write!(formatter, "解析 {} 失败: {}", path.display(), source)
            }
            SelfEvolutionLoopError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到自我进化循环记录 {id}")
            }
            SelfEvolutionLoopError::GitPr(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for SelfEvolutionLoopError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SelfEvolutionLoopError::State(error) => Some(error),
            SelfEvolutionLoopError::Version(error) => Some(error),
            SelfEvolutionLoopError::Io { source, .. } => Some(source),
            SelfEvolutionLoopError::Json { source, .. } => Some(source),
            SelfEvolutionLoopError::GitPr(error) => Some(error),
            SelfEvolutionLoopError::InvalidRequest(_) | SelfEvolutionLoopError::NotFound { .. } => {
                None
            }
        }
    }
}

struct SelfEvolutionLoopStore {
    version: String,
    records_dir: PathBuf,
    index_path: PathBuf,
}

impl SelfEvolutionLoopStore {
    fn new(root: PathBuf, version: &str) -> Result<Self, SelfEvolutionLoopError> {
        let major = version_major_key(version).map_err(SelfEvolutionLoopError::Version)?;
        let records_dir = root
            .join("workspaces")
            .join(major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY)
            .join(SELF_EVOLUTION_LOOP_DIRECTORY);
        let index_path = records_dir.join(SELF_EVOLUTION_LOOP_INDEX_FILE);
        Ok(Self {
            version: version.to_string(),
            records_dir,
            index_path,
        })
    }

    fn save(&self, record: &mut SelfEvolutionLoopRecord) -> Result<(), SelfEvolutionLoopError> {
        fs::create_dir_all(&self.records_dir).map_err(|source| SelfEvolutionLoopError::Io {
            path: self.records_dir.clone(),
            source,
        })?;
        record.updated_at_unix_seconds = current_unix_seconds();
        let json = serde_json::to_string_pretty(record).map_err(|source| {
            SelfEvolutionLoopError::Json {
                path: record.file.clone(),
                source,
            }
        })? + "\n";
        fs::write(&record.file, json).map_err(|source| SelfEvolutionLoopError::Io {
            path: record.file.clone(),
            source,
        })?;
        let index_line = serde_json::to_string(&self.index_entry(record)).map_err(|source| {
            SelfEvolutionLoopError::Json {
                path: self.index_path.clone(),
                source,
            }
        })? + "\n";
        append_text(&self.index_path, &index_line)
    }

    fn load_latest_running(
        &self,
    ) -> Result<Option<SelfEvolutionLoopRecord>, SelfEvolutionLoopError> {
        let mut records = self.load_records()?;
        records.retain(|record| record.status == SelfEvolutionLoopStatus::Running);
        records.sort_by(|left, right| {
            right
                .updated_at_unix_seconds
                .cmp(&left.updated_at_unix_seconds)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(records.into_iter().next())
    }

    fn list(&self, limit: usize) -> Result<Vec<SelfEvolutionLoopSummary>, SelfEvolutionLoopError> {
        let mut records = self.load_records()?;
        records.sort_by(|left, right| {
            right
                .updated_at_unix_seconds
                .cmp(&left.updated_at_unix_seconds)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(records
            .into_iter()
            .take(limit)
            .map(|record| record.summary())
            .collect())
    }

    fn load(&self, id: &str) -> Result<SelfEvolutionLoopRecord, SelfEvolutionLoopError> {
        validate_loop_record_id(id)?;
        let path = self.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(SelfEvolutionLoopError::NotFound {
                version: self.version.clone(),
                id: id.to_string(),
            });
        }
        self.load_record_file(path)
    }

    fn load_records(&self) -> Result<Vec<SelfEvolutionLoopRecord>, SelfEvolutionLoopError> {
        if !self.records_dir.exists() {
            return Ok(Vec::new());
        }
        let entries =
            fs::read_dir(&self.records_dir).map_err(|source| SelfEvolutionLoopError::Io {
                path: self.records_dir.clone(),
                source,
            })?;
        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| SelfEvolutionLoopError::Io {
                path: self.records_dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            records.push(self.load_record_file(path)?);
        }
        Ok(records)
    }

    fn load_record_file(
        &self,
        path: PathBuf,
    ) -> Result<SelfEvolutionLoopRecord, SelfEvolutionLoopError> {
        let text = fs::read_to_string(&path).map_err(|source| SelfEvolutionLoopError::Io {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| SelfEvolutionLoopError::Json { path, source })
    }

    fn index_entry(&self, record: &SelfEvolutionLoopRecord) -> SelfEvolutionLoopIndexEntry {
        SelfEvolutionLoopIndexEntry {
            id: record.id.clone(),
            version: record.version.clone(),
            status: record.status,
            updated_at_unix_seconds: record.updated_at_unix_seconds,
            completed_cycles: record.completed_cycles,
            failed_cycles: record.failed_cycles,
            consecutive_failures: record.consecutive_failures,
            file: record.file.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelfEvolutionLoopIndexEntry {
    id: String,
    version: String,
    status: SelfEvolutionLoopStatus,
    updated_at_unix_seconds: u64,
    completed_cycles: usize,
    failed_cycles: usize,
    consecutive_failures: usize,
    file: PathBuf,
}

impl SelfEvolutionLoopRecord {
    pub fn summary(&self) -> SelfEvolutionLoopSummary {
        SelfEvolutionLoopSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            status: self.status,
            updated_at_unix_seconds: self.updated_at_unix_seconds,
            completed_cycles: self.completed_cycles,
            failed_cycles: self.failed_cycles,
            consecutive_failures: self.consecutive_failures,
            git_pr_mode: self.git_pr.mode,
            pr_url: self.pr_url.clone(),
            file: self.file.clone(),
        }
    }
}

fn new_self_evolution_loop_record(
    request: &SelfEvolutionLoopRequest,
    version: &str,
    store: &SelfEvolutionLoopStore,
) -> SelfEvolutionLoopRecord {
    let now = current_unix_seconds();
    let id = format!("self-loop-{now}");
    SelfEvolutionLoopRecord {
        id: id.clone(),
        version: version.to_string(),
        status: SelfEvolutionLoopStatus::Running,
        created_at_unix_seconds: now,
        updated_at_unix_seconds: now,
        hint: request.hint.clone(),
        max_cycles: request.max_cycles,
        max_failures: request.max_failures,
        timeout_ms: request.timeout_ms,
        completed_cycles: 0,
        failed_cycles: 0,
        consecutive_failures: 0,
        resumed: false,
        git_pr: request.git_pr.clone(),
        git_pr_events: Vec::new(),
        pr_url: None,
        last_error: None,
        steps: Vec::new(),
        file: store.records_dir.join(format!("{id}.json")),
    }
}

fn recover_interrupted_steps(record: &mut SelfEvolutionLoopRecord, max_failures: usize) {
    let mut recovered = false;
    for step in &mut record.steps {
        if step.status == SelfEvolutionLoopStepStatus::Running {
            step.status = SelfEvolutionLoopStepStatus::Failed;
            step.completed_at_unix_seconds = Some(current_unix_seconds());
            step.error = Some("上次循环在运行中中断，本次恢复时已标记为失败。".to_string());
            recovered = true;
        }
    }
    if recovered {
        record.failed_cycles += 1;
        record.consecutive_failures += 1;
        record.last_error = Some("已恢复上次中断的自我进化循环。".to_string());
        if record.consecutive_failures >= max_failures {
            record.status = SelfEvolutionLoopStatus::Stopped;
        }
    }
}

fn complete_successful_self_evolution_step(
    root: &std::path::Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    cycle_report: SelfEvolutionLoopCycleReport,
) -> Result<(), git_pr::SelfEvolutionLoopGitPrError> {
    let cycle = {
        let step = record.steps.last_mut().expect("running step should exist");
        step.status = SelfEvolutionLoopStepStatus::Succeeded;
        step.completed_at_unix_seconds = Some(current_unix_seconds());
        step.stable_version_after = Some(cycle_report.stable_version_after.clone());
        step.audit_id = cycle_report.audit_id.clone();
        step.summary_id = cycle_report.summary_id.clone();
        step.phase_events = cycle_report.phase_events.clone();
        step.patch_draft_id = cycle_report.patch_draft_id.clone();
        step.patch_audit_id = cycle_report.patch_audit_id.clone();
        step.patch_preview_id = cycle_report.patch_preview_id.clone();
        step.patch_application_id = cycle_report.patch_application_id.clone();
        step.patch_source_plan_id = cycle_report.patch_source_plan_id.clone();
        step.patch_source_execution_id = cycle_report.patch_source_execution_id.clone();
        step.patch_source_promotion_id = cycle_report.patch_source_promotion_id.clone();
        step.patch_source_candidate_id = cycle_report.patch_source_candidate_id.clone();
        step.patch_source_cycle_id = cycle_report.patch_source_cycle_id.clone();
        step.patch_source_summary_id = cycle_report.patch_source_summary_id.clone();
        step.changed_files = cycle_report.changed_files.clone();
        step.cycle
    };
    store
        .save(record)
        .map_err(|error| git_pr::SelfEvolutionLoopGitPrError::Record(error.to_string()))?;
    git_pr::commit_successful_cycle(root, store, record, cycle)
}

fn record_failed_self_evolution_step(record: &mut SelfEvolutionLoopRecord, error: String) {
    let now = current_unix_seconds();
    if let Some(step) = record.steps.last_mut() {
        step.status = SelfEvolutionLoopStepStatus::Failed;
        step.completed_at_unix_seconds = Some(now);
        step.stable_version_after = Some(step.stable_version_before.clone());
        step.error = Some(truncate_chars(&error, 400));
    }
    record.failed_cycles += 1;
    record.consecutive_failures += 1;
    record.last_error = Some(truncate_chars(&error, 400));
}

#[cfg(test)]
impl SelfEvolutionLoopCycleReport {
    fn from_self_upgrade_report(report: AiSelfUpgradeReport) -> Self {
        Self {
            stable_version_after: report.evolution.cycle.state.current_version.clone(),
            audit_id: Some(report.audit.id.clone()),
            summary_id: Some(report.summary.id.clone()),
            phase_events: vec!["兼容测试执行器：已完成旧版 AI 自我升级目标决策流程。".to_string()],
            patch_draft_id: None,
            patch_audit_id: None,
            patch_preview_id: None,
            patch_application_id: None,
            patch_source_plan_id: None,
            patch_source_execution_id: None,
            patch_source_promotion_id: None,
            patch_source_candidate_id: None,
            patch_source_cycle_id: None,
            patch_source_summary_id: None,
            changed_files: Vec::new(),
        }
    }
}

impl SelfEvolutionLoopCycleError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SelfEvolutionLoopCycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SelfEvolutionLoopCycleError {}

#[cfg(test)]
impl From<AiSelfUpgradeError> for SelfEvolutionLoopCycleError {
    fn from(error: AiSelfUpgradeError) -> Self {
        Self::new(error.to_string())
    }
}

fn validate_self_evolution_loop_request(
    request: &SelfEvolutionLoopRequest,
) -> Result<(), SelfEvolutionLoopError> {
    if request.max_cycles == 0 {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "--max-cycles 必须大于 0".to_string(),
        ));
    }
    if request.max_failures == 0 {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "--max-failures 必须大于 0".to_string(),
        ));
    }
    if request.timeout_ms == 0 {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "--timeout-ms 必须大于 0".to_string(),
        ));
    }
    if request.git_pr.mode == SelfEvolutionLoopGitPrMode::PullRequest && !request.git_pr.confirmed {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "PR 自主收束必须显式传入 --confirm-finalize。".to_string(),
        ));
    }
    if request.git_pr.mode == SelfEvolutionLoopGitPrMode::PullRequest && !request.git_pr.wait_checks
    {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "PR 自主合并必须等待 required checks，禁止跳过检查。".to_string(),
        ));
    }
    if request.git_pr.mode == SelfEvolutionLoopGitPrMode::PullRequest
        && !request.git_pr.issue_ref.contains("#1")
    {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "PR 自主收束必须关联 Issue #1。".to_string(),
        ));
    }
    if request.git_pr.command_timeout_ms == 0 {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "--git-timeout-ms 必须大于 0".to_string(),
        ));
    }
    if request.git_pr.check_timeout_ms == 0 {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "--check-timeout-ms 必须大于 0".to_string(),
        ));
    }
    if request.git_pr.check_interval_seconds == 0 {
        return Err(SelfEvolutionLoopError::InvalidRequest(
            "--check-interval-seconds 必须大于 0".to_string(),
        ));
    }
    Ok(())
}

fn validate_loop_record_id(id: &str) -> Result<(), SelfEvolutionLoopError> {
    let valid = !id.trim().is_empty()
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    if valid {
        Ok(())
    } else {
        Err(SelfEvolutionLoopError::InvalidRequest(format!(
            "自我进化循环记录编号非法：{id}"
        )))
    }
}

fn append_text(path: &PathBuf, text: &str) -> Result<(), SelfEvolutionLoopError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SelfEvolutionLoopError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| SelfEvolutionLoopError::Io {
            path: path.clone(),
            source,
        })?;
    file.write_all(text.as_bytes())
        .map_err(|source| SelfEvolutionLoopError::Io {
            path: path.clone(),
            source,
        })
}

fn build_coding_self_evolution_goal(root: &Path, hint: &str) -> String {
    let normalized_hint = if hint.trim().is_empty() {
        "继续实现一个小而可验证的 AI coding 常用能力"
    } else {
        hint.trim()
    };
    let agents = read_context_file(root, "Agents.md", 8_000)
        .or_else(|| read_context_file(root, "AGENTS.md", 8_000))
        .unwrap_or_else(|| "未找到 Agents.md。".to_string());
    let readme = read_context_file(root, "README.md", 8_000)
        .unwrap_or_else(|| "未找到 README.md。".to_string());
    let structure = collect_project_structure(root, 220);

    format!(
        "用户目标：{normalized_hint}\n\n\
         本轮必须像编码智能体一样执行真实源码改动，不允许只生成目标、不允许只推进状态版本。\n\
         必须先基于以下项目上下文规划，再输出可审计的补丁草案；补丁应保持小步、可验证、可回滚。\n\
         只有当补丁包含真实源码文件变更、候选镜像验证通过、源码覆盖验证通过后，系统才允许准备和提升下一个版本。\n\n\
         # 已读取项目文件结构\n{structure}\n\n\
         # 已读取 Agents.md\n{agents}\n\n\
         # 已读取 README.md\n{readme}\n\n\
         # 本轮实现要求\n\
         - 优先补齐 AI coding 常用能力，例如项目上下文读取、代码检索、差异审查、测试建议、补丁生成、补丁验证或循环恢复。\n\
         - 需要修改 Rust 源码和必要测试；禁止只修改 forge 归档或 state 状态。\n\
         - 不要修改 `.env`、密钥、target 目录或 Git 元数据。\n\
         - 输出代码草案时必须包含目标文件路径和完整替换内容，便于后续补丁流水线应用。\n"
    )
}

fn read_context_file(root: &Path, relative: &str, max_chars: usize) -> Option<String> {
    let path = root.join(relative);
    let text = fs::read_to_string(path).ok()?;
    Some(truncate_chars(&text, max_chars))
}

fn collect_project_structure(root: &Path, limit: usize) -> String {
    let mut result = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(relative) = stack.pop() {
        if result.len() >= limit {
            break;
        }
        let directory = root.join(&relative);
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        let mut children = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if should_skip_structure_entry(&relative, &name) {
                    return None;
                }
                let child_relative = relative.join(&name);
                let is_dir = entry.file_type().ok()?.is_dir();
                Some((child_relative, is_dir))
            })
            .collect::<Vec<_>>();
        children.sort_by(|left, right| left.0.cmp(&right.0));
        for (child, is_dir) in children.into_iter().rev() {
            if result.len() >= limit {
                break;
            }
            let suffix = if is_dir { "/" } else { "" };
            result.push(format!("- {}{suffix}", child.display()));
            if is_dir {
                stack.push(child);
            }
        }
    }
    if result.is_empty() {
        "未能读取项目结构。".to_string()
    } else {
        result.join("\n")
    }
}

fn should_skip_structure_entry(parent: &Path, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        ".git" | "target" | ".env" | ".env.local" | ".env.production"
    ) {
        return true;
    }
    if parent.as_os_str().is_empty() {
        return false;
    }
    let parent_text = parent.to_string_lossy().replace('\\', "/");
    parent_text.starts_with("target") || parent_text.starts_with(".git")
}

fn push_cycle_event(events: &mut Vec<String>, message: String) {
    eprintln!("SelfForge AI 过程 {message}");
    events.push(message);
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            result.push_str("...");
            break;
        }
        result.push(ch);
    }
    result
}
