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
const PATCH_APPLICATION_DIRECTORY: &str = "patch-applications";
const PATCH_APPLICATION_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchApplicationStatus {
    Applied,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchVerificationStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
}

impl Default for AiPatchVerificationStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchApplicationFile {
    pub source_path: String,
    pub mirror_file: PathBuf,
    pub content_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchVerificationCommandRecord {
    pub command: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub started_at_unix_seconds: u64,
    pub duration_ms: u64,
    pub timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub stdout_preview: String,
    pub stderr_preview: String,
    pub status: AiPatchVerificationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchApplicationRecord {
    pub id: String,
    pub version: String,
    pub candidate_version: String,
    pub preview_id: String,
    pub audit_id: String,
    pub draft_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchApplicationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_dir: Option<PathBuf>,
    pub applied_file_count: usize,
    #[serde(default)]
    pub files: Vec<AiPatchApplicationFile>,
    #[serde(default)]
    pub validation_checked_paths: Vec<PathBuf>,
    #[serde(default)]
    pub verification_commands: Vec<String>,
    #[serde(default)]
    pub verification_runs: Vec<AiPatchVerificationCommandRecord>,
    #[serde(default)]
    pub verification_status: AiPatchVerificationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at_unix_seconds: Option<u64>,
    pub rollback_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchApplicationSummary {
    pub id: String,
    pub version: String,
    pub candidate_version: String,
    pub preview_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchApplicationStatus,
    pub applied_file_count: usize,
    #[serde(default)]
    pub verification_status: AiPatchVerificationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchApplicationStoreError {
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
pub struct AiPatchApplicationStore {
    root: PathBuf,
}

impl AiPatchApplicationStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchApplicationRecord,
        report_markdown: Option<&str>,
    ) -> Result<AiPatchApplicationRecord, AiPatchApplicationStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchApplicationStoreError::Io {
                path: layout.records_dir.clone(),
                source,
            }
        })?;

        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id_seed = clock.as_nanos();
        let (id, relative_file, path) = if record.id.is_empty() {
            let mut selected = None;
            for attempt in 0..1000 {
                let id = format!("patch-application-{id_seed}-{attempt:03}");
                let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
                let path = self.root.join(&relative_file);
                if !path.exists() {
                    selected = Some((id, relative_file, path));
                    break;
                }
            }
            selected.ok_or_else(|| AiPatchApplicationStoreError::IdExhausted {
                version: record.version.clone(),
            })?
        } else {
            validate_record_id(&record.id)?;
            let id = record.id.clone();
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if path.exists() {
                return Err(AiPatchApplicationStoreError::InvalidRecordId { id });
            }
            (id, relative_file, path)
        };

        record.id = id;
        record.created_at_unix_seconds = clock.as_secs();
        record.file = relative_file;
        if let Some(report_markdown) = report_markdown {
            let relative_report_file = layout
                .relative_records_dir
                .join(format!("{}.md", record.id));
            let report_path = self.root.join(&relative_report_file);
            self.write_text(&report_path, report_markdown)?;
            record.report_file = Some(relative_report_file);
        }
        self.write_json(&path, &record)?;
        self.append_summary(&layout.index_path, record.summary())?;

        Ok(record)
    }

    pub fn update(
        &self,
        mut record: AiPatchApplicationRecord,
        report_markdown: Option<&str>,
    ) -> Result<AiPatchApplicationRecord, AiPatchApplicationStoreError> {
        validate_record_id(&record.id)?;
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchApplicationStoreError::Io {
                path: layout.records_dir.clone(),
                source,
            }
        })?;

        let relative_file = layout
            .relative_records_dir
            .join(format!("{}.json", record.id));
        let path = self.root.join(&relative_file);
        if !path.exists() {
            return Err(AiPatchApplicationStoreError::NotFound {
                version: record.version.clone(),
                id: record.id.clone(),
            });
        }

        record.file = relative_file;
        if let Some(report_markdown) = report_markdown {
            let relative_report_file = record.report_file.clone().unwrap_or_else(|| {
                layout
                    .relative_records_dir
                    .join(format!("{}.md", record.id))
            });
            let report_path = self.root.join(&relative_report_file);
            self.write_text(&report_path, report_markdown)?;
            record.report_file = Some(relative_report_file);
        }
        self.write_json(&path, &record)?;
        self.append_summary(&layout.index_path, record.summary())?;

        Ok(record)
    }

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AiPatchApplicationSummary>, AiPatchApplicationStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchApplicationStoreError::Io {
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
                serde_json::from_str::<AiPatchApplicationSummary>(line).map_err(|source| {
                    AiPatchApplicationStoreError::Parse {
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
    ) -> Result<AiPatchApplicationRecord, AiPatchApplicationStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchApplicationStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchApplicationStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record =
            serde_json::from_str::<AiPatchApplicationRecord>(&contents).map_err(|source| {
                AiPatchApplicationStoreError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiPatchApplicationStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(
        &self,
        version: &str,
    ) -> Result<AiPatchApplicationLayout, AiPatchApplicationStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchApplicationStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_APPLICATION_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_APPLICATION_INDEX_FILE);

        Ok(AiPatchApplicationLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchApplicationStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchApplicationStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchApplicationStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchApplicationStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchApplicationStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchApplicationSummary,
    ) -> Result<(), AiPatchApplicationStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchApplicationStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchApplicationStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchApplicationStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchApplicationStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchApplicationRecord {
    pub fn summary(&self) -> AiPatchApplicationSummary {
        AiPatchApplicationSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            candidate_version: self.candidate_version.clone(),
            preview_id: self.preview_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            applied_file_count: self.applied_file_count,
            verification_status: self.verification_status,
            application_dir: self.application_dir.clone(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchApplicationStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchApplicationStatus::Applied => write!(formatter, "已应用"),
            AiPatchApplicationStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchVerificationStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchVerificationStatus::Pending => write!(formatter, "待验证"),
            AiPatchVerificationStatus::Passed => write!(formatter, "已通过"),
            AiPatchVerificationStatus::Failed => write!(formatter, "未通过"),
            AiPatchVerificationStatus::Skipped => write!(formatter, "已跳过"),
        }
    }
}

impl fmt::Display for AiPatchApplicationStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchApplicationStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchApplicationStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁候选应用：{}",
                path.display()
            ),
            AiPatchApplicationStoreError::IdExhausted { version } => {
                write!(formatter, "版本 {version} 无法生成唯一 AI 补丁候选应用编号")
            }
            AiPatchApplicationStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁候选应用编号不合法：{id}")
            }
            AiPatchApplicationStoreError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 AI 补丁候选应用 {id}")
            }
            AiPatchApplicationStoreError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁候选应用文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchApplicationStoreError::Serialize { path, source } => {
                write!(
                    formatter,
                    "AI 补丁候选应用序列化失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchApplicationStoreError::Parse { path, source } => {
                write!(
                    formatter,
                    "AI 补丁候选应用解析失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchApplicationStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchApplicationStoreError::Version(error) => Some(error),
            AiPatchApplicationStoreError::Io { source, .. } => Some(source),
            AiPatchApplicationStoreError::Serialize { source, .. } => Some(source),
            AiPatchApplicationStoreError::Parse { source, .. } => Some(source),
            AiPatchApplicationStoreError::WorkspaceMissing { .. }
            | AiPatchApplicationStoreError::IdExhausted { .. }
            | AiPatchApplicationStoreError::InvalidRecordId { .. }
            | AiPatchApplicationStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchApplicationStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchApplicationStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchApplicationLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchApplicationStoreError> {
    let valid = id.starts_with("patch-application-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchApplicationStoreError::InvalidRecordId { id: id.to_string() })
    }
}
