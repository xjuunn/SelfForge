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
const PATCH_SOURCE_PLAN_DIRECTORY: &str = "patch-source-plans";
const PATCH_SOURCE_PLAN_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchSourcePlanStatus {
    Prepared,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourcePlanFile {
    pub source_path: String,
    pub mirror_file: PathBuf,
    pub target_file: PathBuf,
    pub target_exists: bool,
    pub original_bytes: usize,
    pub new_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_backup_file: Option<PathBuf>,
    pub diff_summary: String,
    pub rollback_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourcePlanRecord {
    pub id: String,
    pub version: String,
    pub application_id: String,
    pub candidate_version: String,
    pub preview_id: String,
    pub audit_id: String,
    pub draft_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourcePlanStatus,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub files: Vec<AiPatchSourcePlanFile>,
    #[serde(default)]
    pub rollback_steps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourcePlanSummary {
    pub id: String,
    pub version: String,
    pub application_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourcePlanStatus,
    pub file_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchSourcePlanStoreError {
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
pub struct AiPatchSourcePlanStore {
    root: PathBuf,
}

impl AiPatchSourcePlanStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchSourcePlanRecord,
        report_markdown: Option<&str>,
    ) -> Result<AiPatchSourcePlanRecord, AiPatchSourcePlanStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchSourcePlanStoreError::Io {
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
                let id = format!("patch-source-plan-{id_seed}-{attempt:03}");
                let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
                let path = self.root.join(&relative_file);
                if !path.exists() {
                    selected = Some((id, relative_file, path));
                    break;
                }
            }
            selected.ok_or_else(|| AiPatchSourcePlanStoreError::IdExhausted {
                version: record.version.clone(),
            })?
        } else {
            validate_record_id(&record.id)?;
            let id = record.id.clone();
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if path.exists() {
                return Err(AiPatchSourcePlanStoreError::InvalidRecordId { id });
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

    pub fn list(
        &self,
        version: impl AsRef<str>,
        limit: usize,
    ) -> Result<Vec<AiPatchSourcePlanSummary>, AiPatchSourcePlanStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchSourcePlanStoreError::Io {
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
                serde_json::from_str::<AiPatchSourcePlanSummary>(line).map_err(|source| {
                    AiPatchSourcePlanStoreError::Parse {
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
    ) -> Result<AiPatchSourcePlanRecord, AiPatchSourcePlanStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchSourcePlanStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchSourcePlanStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record =
            serde_json::from_str::<AiPatchSourcePlanRecord>(&contents).map_err(|source| {
                AiPatchSourcePlanStoreError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiPatchSourcePlanStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(
        &self,
        version: &str,
    ) -> Result<AiPatchSourcePlanLayout, AiPatchSourcePlanStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchSourcePlanStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_SOURCE_PLAN_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_SOURCE_PLAN_INDEX_FILE);

        Ok(AiPatchSourcePlanLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchSourcePlanStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchSourcePlanStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourcePlanStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchSourcePlanStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourcePlanStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchSourcePlanSummary,
    ) -> Result<(), AiPatchSourcePlanStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchSourcePlanStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchSourcePlanStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchSourcePlanStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchSourcePlanStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchSourcePlanRecord {
    pub fn summary(&self) -> AiPatchSourcePlanSummary {
        AiPatchSourcePlanSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            application_id: self.application_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            file_count: self.files.len(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchSourcePlanStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourcePlanStatus::Prepared => write!(formatter, "已准备"),
            AiPatchSourcePlanStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchSourcePlanStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourcePlanStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchSourcePlanStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁源码覆盖准备：{}",
                path.display()
            ),
            AiPatchSourcePlanStoreError::IdExhausted { version } => {
                write!(
                    formatter,
                    "版本 {version} 无法生成唯一 AI 补丁源码覆盖准备编号"
                )
            }
            AiPatchSourcePlanStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁源码覆盖准备编号不合法：{id}")
            }
            AiPatchSourcePlanStoreError::NotFound { version, id } => {
                write!(formatter, "版本 {version} 未找到 AI 补丁源码覆盖准备 {id}")
            }
            AiPatchSourcePlanStoreError::Io { path, source } => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖准备文件读写失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchSourcePlanStoreError::Serialize { path, source } => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖准备序列化失败 {}：{}",
                    path.display(),
                    source
                )
            }
            AiPatchSourcePlanStoreError::Parse { path, source } => {
                write!(
                    formatter,
                    "AI 补丁源码覆盖准备解析失败 {}：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AiPatchSourcePlanStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourcePlanStoreError::Version(error) => Some(error),
            AiPatchSourcePlanStoreError::Io { source, .. } => Some(source),
            AiPatchSourcePlanStoreError::Serialize { source, .. } => Some(source),
            AiPatchSourcePlanStoreError::Parse { source, .. } => Some(source),
            AiPatchSourcePlanStoreError::WorkspaceMissing { .. }
            | AiPatchSourcePlanStoreError::IdExhausted { .. }
            | AiPatchSourcePlanStoreError::InvalidRecordId { .. }
            | AiPatchSourcePlanStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchSourcePlanStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchSourcePlanStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchSourcePlanLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchSourcePlanStoreError> {
    let valid = id.starts_with("patch-source-plan-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchSourcePlanStoreError::InvalidRecordId { id: id.to_string() })
    }
}
