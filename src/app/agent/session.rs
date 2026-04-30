use super::registry::AgentRegistry;
use super::types::{AgentCapability, AgentError, AgentPlan};
use crate::{VersionError, version_major_key};
use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStepStatus {
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub version: String,
    pub goal: String,
    pub status: AgentSessionStatus,
    pub created_at_unix_seconds: u64,
    pub plan: AgentPlan,
    pub steps: Vec<AgentSessionStep>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionSummary {
    pub id: String,
    pub version: String,
    pub goal: String,
    pub status: AgentSessionStatus,
    pub created_at_unix_seconds: u64,
    pub step_count: usize,
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
        let Some((id, file, path)) = selected else {
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
            })
            .collect::<Vec<_>>();

        let session = AgentSession {
            id: id.clone(),
            version: version.clone(),
            goal: plan.goal.clone(),
            status: AgentSessionStatus::Planned,
            created_at_unix_seconds,
            plan,
            steps,
            file: file.clone(),
        };
        self.write_json(&path, &session)?;
        self.append_summary(&layout.index_path, session.summary())?;

        Ok(session)
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
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents =
            fs::read_to_string(&layout.index_path).map_err(|source| AgentSessionError::Io {
                path: layout.index_path.clone(),
                source,
            })?;
        let mut entries = Vec::new();
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            let entry = serde_json::from_str::<AgentSessionSummary>(line).map_err(|source| {
                AgentSessionError::Parse {
                    path: layout.index_path.clone(),
                    source,
                }
            })?;
            if entry.version == version {
                entries.push(entry);
            }
        }
        entries.reverse();
        entries.truncate(limit);
        Ok(entries)
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
    fn summary(&self) -> AgentSessionSummary {
        AgentSessionSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            goal: self.goal.clone(),
            status: self.status,
            created_at_unix_seconds: self.created_at_unix_seconds,
            step_count: self.steps.len(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AgentSessionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSessionStatus::Planned => formatter.write_str("已计划"),
        }
    }
}

impl fmt::Display for AgentStepStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStepStatus::Pending => formatter.write_str("待执行"),
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
