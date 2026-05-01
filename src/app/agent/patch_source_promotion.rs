use super::patch_application::AiPatchVerificationStatus;
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
const PATCH_SOURCE_PROMOTION_DIRECTORY: &str = "patch-source-promotions";
const PATCH_SOURCE_PROMOTION_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchSourcePromotionStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourcePromotionRecord {
    pub id: String,
    pub version: String,
    pub source_execution_id: String,
    pub source_plan_id: String,
    pub application_id: String,
    pub candidate_version: String,
    pub preview_id: String,
    pub audit_id: String,
    pub draft_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourcePromotionStatus,
    pub next_candidate_version: String,
    pub next_candidate_goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_commit_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_commit_body: Option<String>,
    pub verification_status: AiPatchVerificationStatus,
    pub verification_run_count: usize,
    #[serde(default)]
    pub verification_commands: Vec<String>,
    pub file_count: usize,
    #[serde(default)]
    pub changed_files: Vec<String>,
    pub rollback_performed: bool,
    #[serde(default)]
    pub readiness_checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourcePromotionSummary {
    pub id: String,
    pub version: String,
    pub source_execution_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourcePromotionStatus,
    pub next_candidate_version: String,
    pub verification_status: AiPatchVerificationStatus,
    pub verification_run_count: usize,
    pub file_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchSourcePromotionStoreError {
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
pub struct AiPatchSourcePromotionStore {
    root: PathBuf,
}

impl AiPatchSourcePromotionStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchSourcePromotionRecord,
        report_markdown: Option<&str>,
    ) -> Result<AiPatchSourcePromotionRecord, AiPatchSourcePromotionStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchSourcePromotionStoreError::Io {
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
                let id = format!("patch-source-promotion-{id_seed}-{attempt:03}");
                let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
                let path = self.root.join(&relative_file);
                if !path.exists() {
                    selected = Some((id, relative_file, path));
                    break;
                }
            }
            selected.ok_or_else(|| AiPatchSourcePromotionStoreError::IdExhausted {
                version: record.version.clone(),
            })?
        } else {
            validate_record_id(&record.id)?;
            let id = record.id.clone();
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if path.exists() {
                return Err(AiPatchSourcePromotionStoreError::InvalidRecordId { id });
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
    ) -> Result<Vec<AiPatchSourcePromotionSummary>, AiPatchSourcePromotionStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchSourcePromotionStoreError::Io {
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
                serde_json::from_str::<AiPatchSourcePromotionSummary>(line).map_err(|source| {
                    AiPatchSourcePromotionStoreError::Parse {
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
    ) -> Result<AiPatchSourcePromotionRecord, AiPatchSourcePromotionStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchSourcePromotionStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchSourcePromotionStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record =
            serde_json::from_str::<AiPatchSourcePromotionRecord>(&contents).map_err(|source| {
                AiPatchSourcePromotionStoreError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiPatchSourcePromotionStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(
        &self,
        version: &str,
    ) -> Result<AiPatchSourcePromotionLayout, AiPatchSourcePromotionStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchSourcePromotionStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_SOURCE_PROMOTION_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_SOURCE_PROMOTION_INDEX_FILE);

        Ok(AiPatchSourcePromotionLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchSourcePromotionStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchSourcePromotionStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourcePromotionStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchSourcePromotionStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourcePromotionStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchSourcePromotionSummary,
    ) -> Result<(), AiPatchSourcePromotionStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchSourcePromotionStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchSourcePromotionStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchSourcePromotionStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchSourcePromotionStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchSourcePromotionRecord {
    pub fn summary(&self) -> AiPatchSourcePromotionSummary {
        AiPatchSourcePromotionSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            source_execution_id: self.source_execution_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            next_candidate_version: self.next_candidate_version.clone(),
            verification_status: self.verification_status,
            verification_run_count: self.verification_run_count,
            file_count: self.file_count,
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchSourcePromotionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourcePromotionStatus::Ready => write!(formatter, "已就绪"),
            AiPatchSourcePromotionStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchSourcePromotionStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourcePromotionStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchSourcePromotionStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁源码覆盖提升衔接：{}",
                path.display()
            ),
            AiPatchSourcePromotionStoreError::IdExhausted { version } => write!(
                formatter,
                "版本 {version} 无法生成唯一 AI 补丁源码覆盖提升衔接编号"
            ),
            AiPatchSourcePromotionStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁源码覆盖提升衔接编号不合法：{id}")
            }
            AiPatchSourcePromotionStoreError::NotFound { version, id } => write!(
                formatter,
                "版本 {version} 未找到 AI 补丁源码覆盖提升衔接 {id}"
            ),
            AiPatchSourcePromotionStoreError::Io { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖提升衔接文件读写失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourcePromotionStoreError::Serialize { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖提升衔接序列化失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourcePromotionStoreError::Parse { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖提升衔接解析失败 {}：{}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for AiPatchSourcePromotionStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourcePromotionStoreError::Version(error) => Some(error),
            AiPatchSourcePromotionStoreError::Io { source, .. } => Some(source),
            AiPatchSourcePromotionStoreError::Serialize { source, .. } => Some(source),
            AiPatchSourcePromotionStoreError::Parse { source, .. } => Some(source),
            AiPatchSourcePromotionStoreError::WorkspaceMissing { .. }
            | AiPatchSourcePromotionStoreError::IdExhausted { .. }
            | AiPatchSourcePromotionStoreError::InvalidRecordId { .. }
            | AiPatchSourcePromotionStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchSourcePromotionStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchSourcePromotionStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchSourcePromotionLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchSourcePromotionStoreError> {
    let valid = id.starts_with("patch-source-promotion-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchSourcePromotionStoreError::InvalidRecordId { id: id.to_string() })
    }
}
