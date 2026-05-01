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
const SELF_UPGRADE_DIRECTORY: &str = "self-upgrades";
const SELF_UPGRADE_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiSelfUpgradeAuditStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSelfUpgradeAuditRecord {
    pub id: String,
    pub version: String,
    pub created_at_unix_seconds: u64,
    pub status: AiSelfUpgradeAuditStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub provider_id: String,
    pub model: String,
    pub protocol: String,
    pub prompt_bytes: usize,
    #[serde(default)]
    pub memory_source_versions: Vec<String>,
    pub success_experience_count: usize,
    pub failure_experience_count: usize,
    pub optimization_suggestion_count: usize,
    pub reusable_experience_count: usize,
    pub open_error_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_response_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared_candidate_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_version_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSelfUpgradeAuditSummary {
    pub id: String,
    pub version: String,
    pub created_at_unix_seconds: u64,
    pub status: AiSelfUpgradeAuditStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_version_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiSelfUpgradeAuditError {
    Version(VersionError),
    WorkspaceMissing {
        version: String,
        path: PathBuf,
    },
    IdExhausted {
        version: String,
    },
    InvalidRecordId {
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
pub struct AiSelfUpgradeAuditStore {
    root: PathBuf,
}

impl AiSelfUpgradeAuditStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiSelfUpgradeAuditRecord,
    ) -> Result<AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| AiSelfUpgradeAuditError::Io {
            path: layout.records_dir.clone(),
            source,
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id_seed = clock.as_nanos();
        let mut selected = None;
        for attempt in 0..1000 {
            let id = format!("self-upgrade-{id_seed}-{attempt:03}");
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if !path.exists() {
                selected = Some((id, relative_file, path));
                break;
            }
        }
        let Some((id, relative_file, path)) = selected else {
            return Err(AiSelfUpgradeAuditError::IdExhausted {
                version: record.version,
            });
        };

        record.id = id;
        record.created_at_unix_seconds = clock.as_secs();
        record.file = relative_file;
        self.write_json(&path, &record)?;
        self.append_summary(&layout.index_path, record.summary())?;

        Ok(record)
    }

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AiSelfUpgradeAuditSummary>, AiSelfUpgradeAuditError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiSelfUpgradeAuditError::Io {
                path: layout.index_path.clone(),
                source,
            }
        })?;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for line in contents
            .lines()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let entry =
                serde_json::from_str::<AiSelfUpgradeAuditSummary>(line).map_err(|source| {
                    AiSelfUpgradeAuditError::Parse {
                        path: layout.index_path.clone(),
                        source,
                    }
                })?;
            if entry.version == version && seen.insert(entry.id.clone()) {
                entries.push(entry);
                if entries.len() >= limit {
                    break;
                }
            }
        }

        Ok(entries)
    }

    pub fn load(
        &self,
        version: impl AsRef<str>,
        id: &str,
    ) -> Result<AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiSelfUpgradeAuditError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents = fs::read_to_string(&path).map_err(|source| AiSelfUpgradeAuditError::Io {
            path: path.clone(),
            source,
        })?;
        let record =
            serde_json::from_str::<AiSelfUpgradeAuditRecord>(&contents).map_err(|source| {
                AiSelfUpgradeAuditError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiSelfUpgradeAuditError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(&self, version: &str) -> Result<AiSelfUpgradeAuditLayout, AiSelfUpgradeAuditError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiSelfUpgradeAuditError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(SELF_UPGRADE_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(SELF_UPGRADE_INDEX_FILE);

        Ok(AiSelfUpgradeAuditLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiSelfUpgradeAuditError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiSelfUpgradeAuditError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiSelfUpgradeAuditError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiSelfUpgradeAuditSummary,
    ) -> Result<(), AiSelfUpgradeAuditError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiSelfUpgradeAuditError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiSelfUpgradeAuditError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiSelfUpgradeAuditError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiSelfUpgradeAuditError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiSelfUpgradeAuditRecord {
    pub fn summary(&self) -> AiSelfUpgradeAuditSummary {
        AiSelfUpgradeAuditSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            proposed_goal: self.proposed_goal.clone(),
            session_id: self.session_id.clone(),
            candidate_version: self.candidate_version.clone(),
            stable_version_after: self.stable_version_after.clone(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiSelfUpgradeAuditStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiSelfUpgradeAuditStatus::Succeeded => write!(formatter, "成功"),
            AiSelfUpgradeAuditStatus::Failed => write!(formatter, "失败"),
        }
    }
}

impl fmt::Display for AiSelfUpgradeAuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiSelfUpgradeAuditError::Version(error) => write!(formatter, "{error}"),
            AiSelfUpgradeAuditError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 自我升级记录：{}",
                path.display()
            ),
            AiSelfUpgradeAuditError::IdExhausted { version } => {
                write!(formatter, "版本 {version} 无法生成唯一 AI 自我升级记录编号")
            }
            AiSelfUpgradeAuditError::InvalidRecordId { id } => {
                write!(formatter, "AI 自我升级记录编号不合法：{id}")
            }
            AiSelfUpgradeAuditError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 AI 自我升级记录 {id}")
            }
            AiSelfUpgradeAuditError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 自我升级记录文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiSelfUpgradeAuditError::Serialize { path, source } => {
                write!(
                    formatter,
                    "AI 自我升级记录序列化失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiSelfUpgradeAuditError::Parse { path, source } => {
                write!(
                    formatter,
                    "AI 自我升级记录解析失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiSelfUpgradeAuditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiSelfUpgradeAuditError::Version(error) => Some(error),
            AiSelfUpgradeAuditError::Io { source, .. } => Some(source),
            AiSelfUpgradeAuditError::Serialize { source, .. } => Some(source),
            AiSelfUpgradeAuditError::Parse { source, .. } => Some(source),
            AiSelfUpgradeAuditError::WorkspaceMissing { .. }
            | AiSelfUpgradeAuditError::IdExhausted { .. }
            | AiSelfUpgradeAuditError::InvalidRecordId { .. }
            | AiSelfUpgradeAuditError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiSelfUpgradeAuditError {
    fn from(error: VersionError) -> Self {
        AiSelfUpgradeAuditError::Version(error)
    }
}

#[derive(Debug)]
struct AiSelfUpgradeAuditLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiSelfUpgradeAuditError> {
    let valid = id.starts_with("self-upgrade-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiSelfUpgradeAuditError::InvalidRecordId { id: id.to_string() })
    }
}
