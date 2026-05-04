use super::minimal_loop::{AiSelfUpgradeError, AiSelfUpgradeReport, SelfForgeApp};
use crate::state::{ForgeState, StateError};
use crate::version::{VersionError, version_major_key};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
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

impl SelfForgeApp {
    pub fn run_self_evolution_loop(
        &self,
        request: SelfEvolutionLoopRequest,
    ) -> Result<SelfEvolutionLoopReport, SelfEvolutionLoopError> {
        self.run_self_evolution_loop_with_executor(request, |app, hint, timeout_ms| {
            app.ai_self_upgrade(hint, timeout_ms)
        })
    }

    pub(crate) fn run_self_evolution_loop_with_executor<F>(
        &self,
        request: SelfEvolutionLoopRequest,
        mut executor: F,
    ) -> Result<SelfEvolutionLoopReport, SelfEvolutionLoopError>
    where
        F: FnMut(&SelfForgeApp, &str, u64) -> Result<AiSelfUpgradeReport, AiSelfUpgradeError>,
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
                error: None,
            });
            store.save(&mut record)?;

            let result = catch_unwind(AssertUnwindSafe(|| {
                executor(self, &record.hint, record.timeout_ms)
            }));
            match result {
                Ok(Ok(upgrade)) => {
                    match complete_successful_self_evolution_step(
                        self.root(),
                        &store,
                        &mut record,
                        upgrade,
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
    upgrade: AiSelfUpgradeReport,
) -> Result<(), git_pr::SelfEvolutionLoopGitPrError> {
    let cycle = {
        let step = record.steps.last_mut().expect("running step should exist");
        step.status = SelfEvolutionLoopStepStatus::Succeeded;
        step.completed_at_unix_seconds = Some(current_unix_seconds());
        step.stable_version_after = Some(upgrade.evolution.cycle.state.current_version.clone());
        step.audit_id = Some(upgrade.audit.id.clone());
        step.summary_id = Some(upgrade.summary.id.clone());
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
