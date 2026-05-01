use super::registry::AgentRegistry;
use super::types::{AgentCapability, AgentError, AgentPlan};
use crate::{VersionError, version_major_key};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AGENT_ARTIFACT_DIRECTORY: &str = "agents";
const SESSION_DIRECTORY: &str = "sessions";
const SESSION_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentSessionStatus {
    Planned,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentSessionEventKind {
    SessionCreated,
    SessionStatusChanged,
    StepUpdated,
    RuntimeRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub version: String,
    pub goal: String,
    pub status: AgentSessionStatus,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub plan: AgentPlan,
    pub steps: Vec<AgentSessionStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub events: Vec<AgentSessionEvent>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionStep {
    pub order: usize,
    pub agent_id: String,
    pub title: String,
    pub capability: AgentCapability,
    pub status: AgentStepStatus,
    pub verification: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionEvent {
    pub order: usize,
    pub timestamp_unix_seconds: u64,
    pub kind: AgentSessionEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_order: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<AgentRunReference>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunReference {
    pub run_id: String,
    pub version: String,
    pub report_file: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionSummary {
    pub id: String,
    pub version: String,
    pub goal: String,
    pub status: AgentSessionStatus,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub step_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub event_count: usize,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AgentSessionError {
    Agent(AgentError),
    Version(VersionError),
    WorkspaceMissing {
        version: String,
        path: PathBuf,
    },
    IdExhausted {
        version: String,
    },
    InvalidSessionId {
        id: String,
    },
    StepNotFound {
        id: String,
        order: usize,
    },
    NotFound {
        version: String,
        id: String,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone)]
pub struct AgentSessionStore {
    root: PathBuf,
}

impl AgentSessionStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn start(
        &self,
        version: impl AsRef<str>,
        goal: &str,
    ) -> Result<AgentSession, AgentSessionError> {
        let version = version.as_ref().to_string();
        let plan = AgentRegistry::standard().plan_for_goal(goal)?;
        let layout = self.layout(&version)?;
        fs::create_dir_all(&layout.sessions_dir).map_err(|source| AgentSessionError::Io {
            path: layout.sessions_dir.clone(),
            source,
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let created_at_unix_seconds = clock.as_secs();
        let id_seed = clock.as_nanos();
        let mut selected = None;
        for attempt in 0..1000 {
            let id = format!("agent-session-{id_seed}-{attempt:03}");
            let file = layout.relative_sessions_dir.join(format!("{id}.json"));
            let path = self.root.join(&file);
            if !path.exists() {
                selected = Some((id, file, path));
                break;
            }
        }
        let Some((id, file, _path)) = selected else {
            return Err(AgentSessionError::IdExhausted { version });
        };

        let steps = plan
            .steps
            .iter()
            .map(|step| AgentSessionStep {
                order: step.order,
                agent_id: step.agent_id.clone(),
                title: step.title.clone(),
                capability: step.capability,
                status: AgentStepStatus::Pending,
                verification: step.verification.clone(),
                result: None,
            })
            .collect::<Vec<_>>();

        let mut session = AgentSession {
            id: id.clone(),
            version: version.clone(),
            goal: plan.goal.clone(),
            status: AgentSessionStatus::Planned,
            created_at_unix_seconds,
            updated_at_unix_seconds: created_at_unix_seconds,
            plan,
            steps,
            outcome: None,
            error: None,
            events: Vec::new(),
            file: file.clone(),
        };
        session.record_event(
            AgentSessionEventKind::SessionCreated,
            None,
            None,
            "会话已创建并生成 Agent 协作计划。",
        );
        self.save(&session)?;

        Ok(session)
    }

    pub fn save(&self, session: &AgentSession) -> Result<(), AgentSessionError> {
        validate_session_id(&session.id)?;
        let layout = self.layout(&session.version)?;
        fs::create_dir_all(&layout.sessions_dir).map_err(|source| AgentSessionError::Io {
            path: layout.sessions_dir.clone(),
            source,
        })?;
        let path = layout.sessions_dir.join(format!("{}.json", session.id));
        self.write_json(&path, session)?;
        self.append_summary(&layout.index_path, session.summary())
    }

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AgentSessionSummary>, AgentSessionError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        self.list_from_index(&layout.index_path, limit, |entry| entry.version == version)
    }

    pub fn list_all_major(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AgentSessionSummary>, AgentSessionError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        self.list_from_index(&layout.index_path, limit, |_| true)
    }

    pub fn load(
        &self,
        version: impl AsRef<str>,
        id: &str,
    ) -> Result<AgentSession, AgentSessionError> {
        let version = version.as_ref().to_string();
        validate_session_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.sessions_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AgentSessionError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents = fs::read_to_string(&path).map_err(|source| AgentSessionError::Io {
            path: path.clone(),
            source,
        })?;
        let session = serde_json::from_str::<AgentSession>(&contents).map_err(|source| {
            AgentSessionError::Parse {
                path: path.clone(),
                source,
            }
        })?;
        if session.version != version {
            return Err(AgentSessionError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(session)
    }

    fn list_from_index<F>(
        &self,
        index_path: &Path,
        limit: usize,
        mut should_include: F,
    ) -> Result<Vec<AgentSessionSummary>, AgentSessionError>
    where
        F: FnMut(&AgentSessionSummary) -> bool,
    {
        if !index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(index_path).map_err(|source| AgentSessionError::Io {
            path: index_path.to_path_buf(),
            source,
        })?;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for line in contents
            .lines()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let entry = serde_json::from_str::<AgentSessionSummary>(line).map_err(|source| {
                AgentSessionError::Parse {
                    path: index_path.to_path_buf(),
                    source,
                }
            })?;
            if should_include(&entry) && seen.insert(entry.id.clone()) {
                entries.push(entry);
                if entries.len() >= limit {
                    break;
                }
            }
        }
        Ok(entries)
    }

    fn layout(&self, version: &str) -> Result<AgentSessionLayout, AgentSessionError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AgentSessionError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let agents_dir = self.root.join(&relative_agents_dir);
        let relative_sessions_dir = relative_agents_dir.join(SESSION_DIRECTORY);
        let sessions_dir = self.root.join(&relative_sessions_dir);
        let index_path = agents_dir.join(SESSION_INDEX_FILE);

        Ok(AgentSessionLayout {
            sessions_dir,
            relative_sessions_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), AgentSessionError> {
        let contents =
            serde_json::to_string_pretty(value).map_err(|source| AgentSessionError::Serialize {
                path: path.to_path_buf(),
                source,
            })? + "\n";
        fs::write(path, contents).map_err(|source| AgentSessionError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AgentSessionSummary,
    ) -> Result<(), AgentSessionError> {
        let line =
            serde_json::to_string(&summary).map_err(|source| AgentSessionError::Serialize {
                path: path.to_path_buf(),
                source,
            })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AgentSessionError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AgentSessionError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AgentSession {
    pub fn mark_running(&mut self) {
        self.status = AgentSessionStatus::Running;
        self.error = None;
        self.record_event(
            AgentSessionEventKind::SessionStatusChanged,
            None,
            None,
            "会话进入运行中状态。",
        );
    }

    pub fn mark_completed(&mut self, outcome: impl Into<String>) {
        let outcome = outcome.into();
        self.status = AgentSessionStatus::Completed;
        self.outcome = Some(outcome.clone());
        self.error = None;
        self.record_event(
            AgentSessionEventKind::SessionStatusChanged,
            None,
            None,
            format!("会话已完成：{outcome}"),
        );
    }

    pub fn mark_failed(&mut self, error: impl Into<String>) {
        let error = error.into();
        self.status = AgentSessionStatus::Failed;
        self.error = Some(error.clone());
        self.record_event(
            AgentSessionEventKind::SessionStatusChanged,
            None,
            None,
            format!("会话已失败：{error}"),
        );
    }

    pub fn update_step(
        &mut self,
        order: usize,
        status: AgentStepStatus,
        result: impl Into<String>,
    ) -> Result<(), AgentSessionError> {
        let result = result.into();
        let status_text = status.to_string();
        {
            let Some(step) = self.steps.iter_mut().find(|step| step.order == order) else {
                return Err(AgentSessionError::StepNotFound {
                    id: self.id.clone(),
                    order,
                });
            };

            step.status = status;
            step.result = Some(result.clone());
        }
        self.record_event(
            AgentSessionEventKind::StepUpdated,
            Some(order),
            None,
            format!("步骤 {order} 状态更新为 {status_text}：{result}"),
        );
        Ok(())
    }

    pub fn update_step_with_run(
        &mut self,
        order: usize,
        status: AgentStepStatus,
        result: impl Into<String>,
        run: AgentRunReference,
    ) -> Result<(), AgentSessionError> {
        let result = result.into();
        let status_text = status.to_string();
        {
            let Some(step) = self.steps.iter_mut().find(|step| step.order == order) else {
                return Err(AgentSessionError::StepNotFound {
                    id: self.id.clone(),
                    order,
                });
            };

            step.status = status;
            step.result = Some(result.clone());
        }
        let run_id = run.run_id.clone();
        self.record_event(
            AgentSessionEventKind::RuntimeRun,
            Some(order),
            Some(run),
            format!("步骤 {order} 关联运行记录 {run_id}，状态更新为 {status_text}：{result}"),
        );
        Ok(())
    }

    fn summary(&self) -> AgentSessionSummary {
        AgentSessionSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            goal: self.goal.clone(),
            status: self.status,
            created_at_unix_seconds: self.created_at_unix_seconds,
            updated_at_unix_seconds: self.updated_at_unix_seconds,
            step_count: self.steps.len(),
            outcome: self.outcome.clone(),
            error: self.error.clone(),
            event_count: self.events.len(),
            file: self.file.clone(),
        }
    }

    fn record_event(
        &mut self,
        kind: AgentSessionEventKind,
        step_order: Option<usize>,
        run: Option<AgentRunReference>,
        message: impl Into<String>,
    ) {
        let timestamp_unix_seconds = current_unix_seconds();
        self.updated_at_unix_seconds = timestamp_unix_seconds;
        self.events.push(AgentSessionEvent {
            order: self.events.len() + 1,
            timestamp_unix_seconds,
            kind,
            step_order,
            run,
            message: message.into(),
        });
    }
}

impl fmt::Display for AgentSessionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSessionStatus::Planned => formatter.write_str("已计划"),
            AgentSessionStatus::Running => formatter.write_str("运行中"),
            AgentSessionStatus::Completed => formatter.write_str("已完成"),
            AgentSessionStatus::Failed => formatter.write_str("已失败"),
        }
    }
}

impl fmt::Display for AgentStepStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStepStatus::Pending => formatter.write_str("待执行"),
            AgentStepStatus::Running => formatter.write_str("运行中"),
            AgentStepStatus::Completed => formatter.write_str("已完成"),
            AgentStepStatus::Failed => formatter.write_str("已失败"),
        }
    }
}

impl fmt::Display for AgentSessionEventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSessionEventKind::SessionCreated => formatter.write_str("会话创建"),
            AgentSessionEventKind::SessionStatusChanged => formatter.write_str("会话状态"),
            AgentSessionEventKind::StepUpdated => formatter.write_str("步骤更新"),
            AgentSessionEventKind::RuntimeRun => formatter.write_str("运行记录"),
        }
    }
}

impl fmt::Display for AgentSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSessionError::Agent(error) => write!(formatter, "{error}"),
            AgentSessionError::Version(error) => write!(formatter, "{error}"),
            AgentSessionError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在：{}",
                path.display()
            ),
            AgentSessionError::IdExhausted { version } => {
                write!(formatter, "版本 {version} 无法生成唯一 Agent 会话标识")
            }
            AgentSessionError::InvalidSessionId { id } => {
                write!(formatter, "Agent 会话标识不合法：{id}")
            }
            AgentSessionError::StepNotFound { id, order } => {
                write!(formatter, "Agent 会话 {id} 未找到步骤 {order}")
            }
            AgentSessionError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 Agent 会话 {id}")
            }
            AgentSessionError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            AgentSessionError::Serialize { path, source } => {
                write!(formatter, "序列化 {} 失败：{}", path.display(), source)
            }
            AgentSessionError::Parse { path, source } => {
                write!(formatter, "解析 {} 失败：{}", path.display(), source)
            }
        }
    }
}

impl Error for AgentSessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentSessionError::Agent(error) => Some(error),
            AgentSessionError::Version(error) => Some(error),
            AgentSessionError::WorkspaceMissing { .. } => None,
            AgentSessionError::IdExhausted { .. } => None,
            AgentSessionError::InvalidSessionId { .. } => None,
            AgentSessionError::StepNotFound { .. } => None,
            AgentSessionError::NotFound { .. } => None,
            AgentSessionError::Io { source, .. } => Some(source),
            AgentSessionError::Serialize { source, .. } => Some(source),
            AgentSessionError::Parse { source, .. } => Some(source),
        }
    }
}

impl From<AgentError> for AgentSessionError {
    fn from(error: AgentError) -> Self {
        AgentSessionError::Agent(error)
    }
}

impl From<VersionError> for AgentSessionError {
    fn from(error: VersionError) -> Self {
        AgentSessionError::Version(error)
    }
}

struct AgentSessionLayout {
    sessions_dir: PathBuf,
    relative_sessions_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_session_id(id: &str) -> Result<(), AgentSessionError> {
    let valid = !id.is_empty()
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    if valid {
        Ok(())
    } else {
        Err(AgentSessionError::InvalidSessionId { id: id.to_string() })
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
