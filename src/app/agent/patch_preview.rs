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
const PATCH_PREVIEW_DIRECTORY: &str = "patch-previews";
const PATCH_PREVIEW_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchPreviewStatus {
    Previewed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchPreviewChange {
    pub path: String,
    pub code_block_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub content_bytes: usize,
    pub content_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchPreviewRecord {
    pub id: String,
    pub version: String,
    pub target_version: String,
    pub audit_id: String,
    pub draft_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchPreviewStatus,
    #[serde(default)]
    pub normalized_write_scope: Vec<String>,
    pub code_block_count: usize,
    pub change_count: usize,
    #[serde(default)]
    pub changes: Vec<AiPatchPreviewChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchPreviewSummary {
    pub id: String,
    pub version: String,
    pub target_version: String,
    pub audit_id: String,
    pub draft_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchPreviewStatus,
    pub change_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchPreviewStoreError {
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
pub struct AiPatchPreviewStore {
    root: PathBuf,
}

impl AiPatchPreviewStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchPreviewRecord,
        preview_markdown: Option<&str>,
    ) -> Result<AiPatchPreviewRecord, AiPatchPreviewStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| AiPatchPreviewStoreError::Io {
            path: layout.records_dir.clone(),
            source,
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id_seed = clock.as_nanos();
        let mut selected = None;
        for attempt in 0..1000 {
            let id = format!("patch-preview-{id_seed}-{attempt:03}");
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if !path.exists() {
                selected = Some((id, relative_file, path));
                break;
            }
        }
        let Some((id, relative_file, path)) = selected else {
            return Err(AiPatchPreviewStoreError::IdExhausted {
                version: record.version,
            });
        };

        record.id = id;
        record.created_at_unix_seconds = clock.as_secs();
        record.file = relative_file;
        if let Some(preview_markdown) = preview_markdown {
            let relative_preview_file = layout
                .relative_records_dir
                .join(format!("{}.md", record.id));
            let preview_path = self.root.join(&relative_preview_file);
            self.write_text(&preview_path, preview_markdown)?;
            record.preview_file = Some(relative_preview_file);
        }
        self.write_json(&path, &record)?;
        self.append_summary(&layout.index_path, record.summary())?;

        Ok(record)
    }

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AiPatchPreviewSummary>, AiPatchPreviewStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchPreviewStoreError::Io {
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
            let entry = serde_json::from_str::<AiPatchPreviewSummary>(line).map_err(|source| {
                AiPatchPreviewStoreError::Parse {
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
    ) -> Result<AiPatchPreviewRecord, AiPatchPreviewStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchPreviewStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchPreviewStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record = serde_json::from_str::<AiPatchPreviewRecord>(&contents).map_err(|source| {
            AiPatchPreviewStoreError::Parse {
                path: path.clone(),
                source,
            }
        })?;
        if record.version != version || record.id != id {
            return Err(AiPatchPreviewStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(&self, version: &str) -> Result<AiPatchPreviewLayout, AiPatchPreviewStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchPreviewStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_PREVIEW_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_PREVIEW_INDEX_FILE);

        Ok(AiPatchPreviewLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchPreviewStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchPreviewStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchPreviewStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchPreviewStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchPreviewStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchPreviewSummary,
    ) -> Result<(), AiPatchPreviewStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchPreviewStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchPreviewStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchPreviewStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchPreviewStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchPreviewRecord {
    pub fn summary(&self) -> AiPatchPreviewSummary {
        AiPatchPreviewSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            target_version: self.target_version.clone(),
            audit_id: self.audit_id.clone(),
            draft_id: self.draft_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            change_count: self.change_count,
            preview_file: self.preview_file.clone(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchPreviewStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchPreviewStatus::Previewed => write!(formatter, "已预演"),
            AiPatchPreviewStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchPreviewStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchPreviewStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchPreviewStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁应用预演：{}",
                path.display()
            ),
            AiPatchPreviewStoreError::IdExhausted { version } => {
                write!(formatter, "版本 {version} 无法生成唯一 AI 补丁应用预演编号")
            }
            AiPatchPreviewStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁应用预演编号不合法：{id}")
            }
            AiPatchPreviewStoreError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 AI 补丁应用预演 {id}")
            }
            AiPatchPreviewStoreError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁应用预演文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchPreviewStoreError::Serialize { path, source } => {
                write!(
                    formatter,
                    "AI 补丁应用预演序列化失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchPreviewStoreError::Parse { path, source } => {
                write!(
                    formatter,
                    "AI 补丁应用预演解析失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchPreviewStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchPreviewStoreError::Version(error) => Some(error),
            AiPatchPreviewStoreError::Io { source, .. } => Some(source),
            AiPatchPreviewStoreError::Serialize { source, .. } => Some(source),
            AiPatchPreviewStoreError::Parse { source, .. } => Some(source),
            AiPatchPreviewStoreError::WorkspaceMissing { .. }
            | AiPatchPreviewStoreError::IdExhausted { .. }
            | AiPatchPreviewStoreError::InvalidRecordId { .. }
            | AiPatchPreviewStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchPreviewStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchPreviewStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchPreviewLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchPreviewStoreError> {
    let valid = id.starts_with("patch-preview-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchPreviewStoreError::InvalidRecordId { id: id.to_string() })
    }
}
