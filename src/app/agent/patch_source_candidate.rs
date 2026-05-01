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
const PATCH_SOURCE_CANDIDATE_DIRECTORY: &str = "patch-source-candidates";
const PATCH_SOURCE_CANDIDATE_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchSourceCandidateStatus {
    Prepared,
    Reused,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourceCandidateRecord {
    pub id: String,
    pub version: String,
    pub promotion_id: String,
    pub source_execution_id: String,
    pub source_plan_id: String,
    pub application_id: String,
    pub candidate_version: String,
    pub candidate_goal: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourceCandidateStatus,
    pub stable_version_before: String,
    pub state_status_before: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version_before: Option<String>,
    pub stable_version_after: String,
    pub state_status_after: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_workspace: Option<String>,
    pub candidate_checked_path_count: usize,
    pub created_path_count: usize,
    pub existing_path_count: usize,
    #[serde(default)]
    pub readiness_checks: Vec<String>,
    #[serde(default)]
    pub follow_up_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourceCandidateSummary {
    pub id: String,
    pub version: String,
    pub promotion_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourceCandidateStatus,
    pub candidate_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchSourceCandidateStoreError {
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
pub struct AiPatchSourceCandidateStore {
    root: PathBuf,
}

impl AiPatchSourceCandidateStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchSourceCandidateRecord,
        report_markdown: Option<&str>,
    ) -> Result<AiPatchSourceCandidateRecord, AiPatchSourceCandidateStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchSourceCandidateStoreError::Io {
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
                let id = format!("patch-source-candidate-{id_seed}-{attempt:03}");
                let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
                let path = self.root.join(&relative_file);
                if !path.exists() {
                    selected = Some((id, relative_file, path));
                    break;
                }
            }
            selected.ok_or_else(|| AiPatchSourceCandidateStoreError::IdExhausted {
                version: record.version.clone(),
            })?
        } else {
            validate_record_id(&record.id)?;
            let id = record.id.clone();
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if path.exists() {
                return Err(AiPatchSourceCandidateStoreError::InvalidRecordId { id });
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
    ) -> Result<Vec<AiPatchSourceCandidateSummary>, AiPatchSourceCandidateStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchSourceCandidateStoreError::Io {
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
                serde_json::from_str::<AiPatchSourceCandidateSummary>(line).map_err(|source| {
                    AiPatchSourceCandidateStoreError::Parse {
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
    ) -> Result<AiPatchSourceCandidateRecord, AiPatchSourceCandidateStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchSourceCandidateStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchSourceCandidateStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record =
            serde_json::from_str::<AiPatchSourceCandidateRecord>(&contents).map_err(|source| {
                AiPatchSourceCandidateStoreError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiPatchSourceCandidateStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(
        &self,
        version: &str,
    ) -> Result<AiPatchSourceCandidateLayout, AiPatchSourceCandidateStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchSourceCandidateStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_SOURCE_CANDIDATE_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_SOURCE_CANDIDATE_INDEX_FILE);

        Ok(AiPatchSourceCandidateLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchSourceCandidateStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchSourceCandidateStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourceCandidateStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchSourceCandidateStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourceCandidateStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchSourceCandidateSummary,
    ) -> Result<(), AiPatchSourceCandidateStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchSourceCandidateStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchSourceCandidateStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchSourceCandidateStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchSourceCandidateStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchSourceCandidateRecord {
    pub fn summary(&self) -> AiPatchSourceCandidateSummary {
        AiPatchSourceCandidateSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            promotion_id: self.promotion_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            candidate_version: self.candidate_version.clone(),
            candidate_version_after: self.candidate_version_after.clone(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchSourceCandidateStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCandidateStatus::Prepared => write!(formatter, "已准备"),
            AiPatchSourceCandidateStatus::Reused => write!(formatter, "已复用"),
            AiPatchSourceCandidateStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchSourceCandidateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCandidateStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchSourceCandidateStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁源码覆盖候选准备：{}",
                path.display()
            ),
            AiPatchSourceCandidateStoreError::IdExhausted { version } => write!(
                formatter,
                "版本 {version} 无法生成唯一 AI 补丁源码覆盖候选准备编号"
            ),
            AiPatchSourceCandidateStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁源码覆盖候选准备编号不合法：{id}")
            }
            AiPatchSourceCandidateStoreError::NotFound { version, id } => write!(
                formatter,
                "版本 {version} 未找到 AI 补丁源码覆盖候选准备 {id}"
            ),
            AiPatchSourceCandidateStoreError::Io { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖候选准备文件读写失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceCandidateStoreError::Serialize { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖候选准备序列化失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceCandidateStoreError::Parse { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖候选准备解析失败 {}：{}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for AiPatchSourceCandidateStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceCandidateStoreError::Version(error) => Some(error),
            AiPatchSourceCandidateStoreError::Io { source, .. } => Some(source),
            AiPatchSourceCandidateStoreError::Serialize { source, .. } => Some(source),
            AiPatchSourceCandidateStoreError::Parse { source, .. } => Some(source),
            AiPatchSourceCandidateStoreError::WorkspaceMissing { .. }
            | AiPatchSourceCandidateStoreError::IdExhausted { .. }
            | AiPatchSourceCandidateStoreError::InvalidRecordId { .. }
            | AiPatchSourceCandidateStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchSourceCandidateStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchSourceCandidateStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchSourceCandidateLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchSourceCandidateStoreError> {
    let valid = id.starts_with("patch-source-candidate-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchSourceCandidateStoreError::InvalidRecordId { id: id.to_string() })
    }
}
