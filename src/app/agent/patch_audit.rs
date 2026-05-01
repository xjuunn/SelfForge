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
const PATCH_AUDIT_DIRECTORY: &str = "patch-audits";
const PATCH_AUDIT_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchAuditStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchAuditSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchAuditFindingKind {
    MissingDraftFile,
    MissingWriteScope,
    InvalidPath,
    ProtectedPath,
    ActiveConflict,
    QueueUnavailable,
    DraftNotSuccessful,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchAuditFinding {
    pub severity: AiPatchAuditSeverity,
    pub kind: AiPatchAuditFindingKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchAuditRecord {
    pub id: String,
    pub version: String,
    pub target_version: String,
    pub draft_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_task_audit_id: Option<String>,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchAuditStatus,
    #[serde(default)]
    pub requested_write_scope: Vec<String>,
    #[serde(default)]
    pub normalized_write_scope: Vec<String>,
    #[serde(default)]
    pub protected_roots: Vec<String>,
    pub active_conflict_count: usize,
    pub finding_count: usize,
    #[serde(default)]
    pub findings: Vec<AiPatchAuditFinding>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchAuditSummary {
    pub id: String,
    pub version: String,
    pub target_version: String,
    pub draft_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_task_audit_id: Option<String>,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchAuditStatus,
    pub active_conflict_count: usize,
    pub finding_count: usize,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchAuditStoreError {
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
pub struct AiPatchAuditStore {
    root: PathBuf,
}

impl AiPatchAuditStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchAuditRecord,
    ) -> Result<AiPatchAuditRecord, AiPatchAuditStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| AiPatchAuditStoreError::Io {
            path: layout.records_dir.clone(),
            source,
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id_seed = clock.as_nanos();
        let mut selected = None;
        for attempt in 0..1000 {
            let id = format!("patch-audit-{id_seed}-{attempt:03}");
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if !path.exists() {
                selected = Some((id, relative_file, path));
                break;
            }
        }
        let Some((id, relative_file, path)) = selected else {
            return Err(AiPatchAuditStoreError::IdExhausted {
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
    ) -> Result<Vec<AiPatchAuditSummary>, AiPatchAuditStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchAuditStoreError::Io {
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
            let entry = serde_json::from_str::<AiPatchAuditSummary>(line).map_err(|source| {
                AiPatchAuditStoreError::Parse {
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
    ) -> Result<AiPatchAuditRecord, AiPatchAuditStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchAuditStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents = fs::read_to_string(&path).map_err(|source| AiPatchAuditStoreError::Io {
            path: path.clone(),
            source,
        })?;
        let record = serde_json::from_str::<AiPatchAuditRecord>(&contents).map_err(|source| {
            AiPatchAuditStoreError::Parse {
                path: path.clone(),
                source,
            }
        })?;
        if record.version != version || record.id != id {
            return Err(AiPatchAuditStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(&self, version: &str) -> Result<AiPatchAuditLayout, AiPatchAuditStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchAuditStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_AUDIT_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_AUDIT_INDEX_FILE);

        Ok(AiPatchAuditLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchAuditStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchAuditStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchAuditStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchAuditSummary,
    ) -> Result<(), AiPatchAuditStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchAuditStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchAuditStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchAuditStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchAuditStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchAuditRecord {
    pub fn summary(&self) -> AiPatchAuditSummary {
        AiPatchAuditSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            target_version: self.target_version.clone(),
            draft_id: self.draft_id.clone(),
            source_task_audit_id: self.source_task_audit_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            active_conflict_count: self.active_conflict_count,
            finding_count: self.finding_count,
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchAuditStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchAuditStatus::Passed => write!(formatter, "通过"),
            AiPatchAuditStatus::Failed => write!(formatter, "失败"),
        }
    }
}

impl fmt::Display for AiPatchAuditSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchAuditSeverity::Warning => write!(formatter, "警告"),
            AiPatchAuditSeverity::Error => write!(formatter, "错误"),
        }
    }
}

impl fmt::Display for AiPatchAuditFindingKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchAuditFindingKind::MissingDraftFile => write!(formatter, "缺少草案文件"),
            AiPatchAuditFindingKind::MissingWriteScope => write!(formatter, "缺少写入范围"),
            AiPatchAuditFindingKind::InvalidPath => write!(formatter, "非法路径"),
            AiPatchAuditFindingKind::ProtectedPath => write!(formatter, "受保护路径"),
            AiPatchAuditFindingKind::ActiveConflict => write!(formatter, "活跃冲突"),
            AiPatchAuditFindingKind::QueueUnavailable => write!(formatter, "协作队列不可用"),
            AiPatchAuditFindingKind::DraftNotSuccessful => write!(formatter, "草案未成功"),
        }
    }
}

impl fmt::Display for AiPatchAuditStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchAuditStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchAuditStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁审计：{}",
                path.display()
            ),
            AiPatchAuditStoreError::IdExhausted { version } => {
                write!(formatter, "版本 {version} 无法生成唯一 AI 补丁审计编号")
            }
            AiPatchAuditStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁审计编号不合法：{id}")
            }
            AiPatchAuditStoreError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 AI 补丁审计 {id}")
            }
            AiPatchAuditStoreError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁审计文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchAuditStoreError::Serialize { path, source } => {
                write!(
                    formatter,
                    "AI 补丁审计序列化失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchAuditStoreError::Parse { path, source } => {
                write!(
                    formatter,
                    "AI 补丁审计解析失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchAuditStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchAuditStoreError::Version(error) => Some(error),
            AiPatchAuditStoreError::Io { source, .. } => Some(source),
            AiPatchAuditStoreError::Serialize { source, .. } => Some(source),
            AiPatchAuditStoreError::Parse { source, .. } => Some(source),
            AiPatchAuditStoreError::WorkspaceMissing { .. }
            | AiPatchAuditStoreError::IdExhausted { .. }
            | AiPatchAuditStoreError::InvalidRecordId { .. }
            | AiPatchAuditStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchAuditStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchAuditStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchAuditLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchAuditStoreError> {
    let valid = id.starts_with("patch-audit-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchAuditStoreError::InvalidRecordId { id: id.to_string() })
    }
}
