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
const PATCH_DRAFT_DIRECTORY: &str = "patch-drafts";
const PATCH_DRAFT_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchDraftStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchDraftRecord {
    pub id: String,
    pub version: String,
    pub target_version: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchDraftStatus,
    pub goal: String,
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
    #[serde(default)]
    pub allowed_write_roots: Vec<String>,
    #[serde(default)]
    pub required_sections: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_response_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchDraftSummary {
    pub id: String,
    pub version: String,
    pub target_version: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchDraftStatus,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchDraftStoreError {
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
pub struct AiPatchDraftStore {
    root: PathBuf,
}

impl AiPatchDraftStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchDraftRecord,
        draft_markdown: Option<&str>,
    ) -> Result<AiPatchDraftRecord, AiPatchDraftStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| AiPatchDraftStoreError::Io {
            path: layout.records_dir.clone(),
            source,
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id_seed = clock.as_nanos();
        let mut selected = None;
        for attempt in 0..1000 {
            let id = format!("patch-draft-{id_seed}-{attempt:03}");
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if !path.exists() {
                selected = Some((id, relative_file, path));
                break;
            }
        }
        let Some((id, relative_file, path)) = selected else {
            return Err(AiPatchDraftStoreError::IdExhausted {
                version: record.version,
            });
        };

        record.id = id;
        record.created_at_unix_seconds = clock.as_secs();
        record.file = relative_file;
        if let Some(draft_markdown) = draft_markdown {
            let relative_draft_file = layout
                .relative_records_dir
                .join(format!("{}.md", record.id));
            let draft_path = self.root.join(&relative_draft_file);
            self.write_text(&draft_path, draft_markdown)?;
            record.draft_file = Some(relative_draft_file);
        }
        self.write_json(&path, &record)?;
        self.append_summary(&layout.index_path, record.summary())?;

        Ok(record)
    }

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AiPatchDraftSummary>, AiPatchDraftStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchDraftStoreError::Io {
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
            let entry = serde_json::from_str::<AiPatchDraftSummary>(line).map_err(|source| {
                AiPatchDraftStoreError::Parse {
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
    ) -> Result<AiPatchDraftRecord, AiPatchDraftStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchDraftStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents = fs::read_to_string(&path).map_err(|source| AiPatchDraftStoreError::Io {
            path: path.clone(),
            source,
        })?;
        let record = serde_json::from_str::<AiPatchDraftRecord>(&contents).map_err(|source| {
            AiPatchDraftStoreError::Parse {
                path: path.clone(),
                source,
            }
        })?;
        if record.version != version || record.id != id {
            return Err(AiPatchDraftStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(&self, version: &str) -> Result<AiPatchDraftLayout, AiPatchDraftStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchDraftStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_DRAFT_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_DRAFT_INDEX_FILE);

        Ok(AiPatchDraftLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchDraftStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchDraftStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchDraftStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchDraftStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchDraftStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchDraftSummary,
    ) -> Result<(), AiPatchDraftStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchDraftStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchDraftStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchDraftStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchDraftStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchDraftRecord {
    pub fn summary(&self) -> AiPatchDraftSummary {
        AiPatchDraftSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            target_version: self.target_version.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            goal: self.goal.clone(),
            draft_file: self.draft_file.clone(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchDraftStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchDraftStatus::Succeeded => write!(formatter, "成功"),
            AiPatchDraftStatus::Failed => write!(formatter, "失败"),
        }
    }
}

impl fmt::Display for AiPatchDraftStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchDraftStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchDraftStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁草案：{}",
                path.display()
            ),
            AiPatchDraftStoreError::IdExhausted { version } => {
                write!(formatter, "版本 {version} 无法生成唯一 AI 补丁草案编号")
            }
            AiPatchDraftStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁草案编号不合法：{id}")
            }
            AiPatchDraftStoreError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 AI 补丁草案 {id}")
            }
            AiPatchDraftStoreError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁草案文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchDraftStoreError::Serialize { path, source } => {
                write!(
                    formatter,
                    "AI 补丁草案序列化失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchDraftStoreError::Parse { path, source } => {
                write!(
                    formatter,
                    "AI 补丁草案解析失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchDraftStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchDraftStoreError::Version(error) => Some(error),
            AiPatchDraftStoreError::Io { source, .. } => Some(source),
            AiPatchDraftStoreError::Serialize { source, .. } => Some(source),
            AiPatchDraftStoreError::Parse { source, .. } => Some(source),
            AiPatchDraftStoreError::WorkspaceMissing { .. }
            | AiPatchDraftStoreError::IdExhausted { .. }
            | AiPatchDraftStoreError::InvalidRecordId { .. }
            | AiPatchDraftStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchDraftStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchDraftStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchDraftLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchDraftStoreError> {
    let valid = id.starts_with("patch-draft-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchDraftStoreError::InvalidRecordId { id: id.to_string() })
    }
}
