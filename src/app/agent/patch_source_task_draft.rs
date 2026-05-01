use super::patch_source_cycle_summary::AiPatchSourceCycleFollowUpStatus;
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
const PATCH_SOURCE_TASK_DRAFT_DIRECTORY: &str = "patch-source-task-drafts";
const PATCH_SOURCE_TASK_DRAFT_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchSourceTaskDraftStatus {
    Drafted,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourceTaskDraftRecord {
    pub id: String,
    pub version: String,
    pub summary_id: String,
    pub cycle_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourceTaskDraftStatus,
    pub source_status: AiPatchSourceCycleFollowUpStatus,
    pub stable_version_after: String,
    pub source_next_goal: String,
    pub source_next_task: String,
    pub proposed_task_title: String,
    pub proposed_task_description: String,
    pub suggested_target_version: String,
    pub required_audit: bool,
    #[serde(default)]
    pub acceptance_checks: Vec<String>,
    #[serde(default)]
    pub follow_up_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub markdown_file: PathBuf,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourceTaskDraftSummary {
    pub id: String,
    pub version: String,
    pub summary_id: String,
    pub cycle_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourceTaskDraftStatus,
    pub source_status: AiPatchSourceCycleFollowUpStatus,
    pub proposed_task_title: String,
    pub suggested_target_version: String,
    pub markdown_file: PathBuf,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchSourceTaskDraftStoreError {
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
pub struct AiPatchSourceTaskDraftStore {
    root: PathBuf,
}

impl AiPatchSourceTaskDraftStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchSourceTaskDraftRecord,
        markdown: &str,
    ) -> Result<AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchSourceTaskDraftStoreError::Io {
                path: layout.records_dir.clone(),
                source,
            }
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id_seed = clock.as_nanos();
        let mut selected = None;
        for attempt in 0..1000 {
            let id = format!("patch-source-task-draft-{id_seed}-{attempt:03}");
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let relative_markdown_file = layout.relative_records_dir.join(format!("{id}.md"));
            let path = self.root.join(&relative_file);
            if !path.exists() {
                selected = Some((id, relative_file, relative_markdown_file, path));
                break;
            }
        }
        let Some((id, relative_file, relative_markdown_file, path)) = selected else {
            return Err(AiPatchSourceTaskDraftStoreError::IdExhausted {
                version: record.version,
            });
        };

        record.id = id;
        record.created_at_unix_seconds = clock.as_secs();
        record.file = relative_file;
        record.markdown_file = relative_markdown_file;
        self.write_text(&self.root.join(&record.markdown_file), markdown)?;
        self.write_json(&path, &record)?;
        self.append_summary(&layout.index_path, record.summary())?;

        Ok(record)
    }

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AiPatchSourceTaskDraftSummary>, AiPatchSourceTaskDraftStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchSourceTaskDraftStoreError::Io {
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
                serde_json::from_str::<AiPatchSourceTaskDraftSummary>(line).map_err(|source| {
                    AiPatchSourceTaskDraftStoreError::Parse {
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
    ) -> Result<AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchSourceTaskDraftStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchSourceTaskDraftStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record =
            serde_json::from_str::<AiPatchSourceTaskDraftRecord>(&contents).map_err(|source| {
                AiPatchSourceTaskDraftStoreError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiPatchSourceTaskDraftStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(
        &self,
        version: &str,
    ) -> Result<AiPatchSourceTaskDraftLayout, AiPatchSourceTaskDraftStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchSourceTaskDraftStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_SOURCE_TASK_DRAFT_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_SOURCE_TASK_DRAFT_INDEX_FILE);

        Ok(AiPatchSourceTaskDraftLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchSourceTaskDraftStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchSourceTaskDraftStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourceTaskDraftStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchSourceTaskDraftStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourceTaskDraftStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchSourceTaskDraftSummary,
    ) -> Result<(), AiPatchSourceTaskDraftStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchSourceTaskDraftStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchSourceTaskDraftStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchSourceTaskDraftStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchSourceTaskDraftStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchSourceTaskDraftRecord {
    pub fn summary(&self) -> AiPatchSourceTaskDraftSummary {
        AiPatchSourceTaskDraftSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            summary_id: self.summary_id.clone(),
            cycle_id: self.cycle_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            source_status: self.source_status,
            proposed_task_title: self.proposed_task_title.clone(),
            suggested_target_version: self.suggested_target_version.clone(),
            markdown_file: self.markdown_file.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchSourceTaskDraftStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceTaskDraftStatus::Drafted => write!(formatter, "已生成草案"),
            AiPatchSourceTaskDraftStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchSourceTaskDraftStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceTaskDraftStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchSourceTaskDraftStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入源码覆盖下一任务草案：{}",
                path.display()
            ),
            AiPatchSourceTaskDraftStoreError::IdExhausted { version } => {
                write!(
                    formatter,
                    "版本 {version} 无法生成唯一源码覆盖下一任务草案编号"
                )
            }
            AiPatchSourceTaskDraftStoreError::InvalidRecordId { id } => {
                write!(formatter, "源码覆盖下一任务草案编号不合法：{id}")
            }
            AiPatchSourceTaskDraftStoreError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到源码覆盖下一任务草案 {id}")
            }
            AiPatchSourceTaskDraftStoreError::Io { path, source } => write!(
                formatter,
                "源码覆盖下一任务草案文件读写失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceTaskDraftStoreError::Serialize { path, source } => write!(
                formatter,
                "源码覆盖下一任务草案序列化失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceTaskDraftStoreError::Parse { path, source } => write!(
                formatter,
                "源码覆盖下一任务草案解析失败 {}：{}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for AiPatchSourceTaskDraftStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceTaskDraftStoreError::Version(error) => Some(error),
            AiPatchSourceTaskDraftStoreError::Io { source, .. } => Some(source),
            AiPatchSourceTaskDraftStoreError::Serialize { source, .. } => Some(source),
            AiPatchSourceTaskDraftStoreError::Parse { source, .. } => Some(source),
            AiPatchSourceTaskDraftStoreError::WorkspaceMissing { .. }
            | AiPatchSourceTaskDraftStoreError::IdExhausted { .. }
            | AiPatchSourceTaskDraftStoreError::InvalidRecordId { .. }
            | AiPatchSourceTaskDraftStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchSourceTaskDraftStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchSourceTaskDraftStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchSourceTaskDraftLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchSourceTaskDraftStoreError> {
    let valid = id.starts_with("patch-source-task-draft-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchSourceTaskDraftStoreError::InvalidRecordId { id: id.to_string() })
    }
}
