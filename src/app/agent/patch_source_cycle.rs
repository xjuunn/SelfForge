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
const PATCH_SOURCE_CYCLE_DIRECTORY: &str = "patch-source-cycles";
const PATCH_SOURCE_CYCLE_INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchSourceCycleStatus {
    Promoted,
    RolledBack,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPatchSourceCycleResult {
    Promoted,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPatchSourceCycleRecord {
    pub id: String,
    pub version: String,
    pub candidate_record_id: String,
    pub promotion_id: String,
    pub source_execution_id: String,
    pub candidate_version: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourceCycleStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_result: Option<AiPatchSourceCycleResult>,
    pub stable_version_before: String,
    pub state_status_before: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version_before: Option<String>,
    pub stable_version_after: String,
    pub state_status_after: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version_after: Option<String>,
    pub preflight_current_checked_path_count: usize,
    pub preflight_candidate_checked_path_count: usize,
    pub preflight_can_advance: bool,
    pub open_error_count: usize,
    pub cycle_candidate_checked_path_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
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
pub struct AiPatchSourceCycleSummary {
    pub id: String,
    pub version: String,
    pub candidate_record_id: String,
    pub created_at_unix_seconds: u64,
    pub status: AiPatchSourceCycleStatus,
    pub candidate_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_result: Option<AiPatchSourceCycleResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub file: PathBuf,
}

#[derive(Debug)]
pub enum AiPatchSourceCycleStoreError {
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
pub struct AiPatchSourceCycleStore {
    root: PathBuf,
}

impl AiPatchSourceCycleStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn create(
        &self,
        mut record: AiPatchSourceCycleRecord,
        report_markdown: Option<&str>,
    ) -> Result<AiPatchSourceCycleRecord, AiPatchSourceCycleStoreError> {
        let layout = self.layout(&record.version)?;
        fs::create_dir_all(&layout.records_dir).map_err(|source| {
            AiPatchSourceCycleStoreError::Io {
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
                let id = format!("patch-source-cycle-{id_seed}-{attempt:03}");
                let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
                let path = self.root.join(&relative_file);
                if !path.exists() {
                    selected = Some((id, relative_file, path));
                    break;
                }
            }
            selected.ok_or_else(|| AiPatchSourceCycleStoreError::IdExhausted {
                version: record.version.clone(),
            })?
        } else {
            validate_record_id(&record.id)?;
            let id = record.id.clone();
            let relative_file = layout.relative_records_dir.join(format!("{id}.json"));
            let path = self.root.join(&relative_file);
            if path.exists() {
                return Err(AiPatchSourceCycleStoreError::InvalidRecordId { id });
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
    ) -> Result<Vec<AiPatchSourceCycleSummary>, AiPatchSourceCycleStoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let version = version.as_ref().to_string();
        let layout = self.layout(&version)?;
        if !layout.index_path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&layout.index_path).map_err(|source| {
            AiPatchSourceCycleStoreError::Io {
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
                serde_json::from_str::<AiPatchSourceCycleSummary>(line).map_err(|source| {
                    AiPatchSourceCycleStoreError::Parse {
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
    ) -> Result<AiPatchSourceCycleRecord, AiPatchSourceCycleStoreError> {
        let version = version.as_ref().to_string();
        validate_record_id(id)?;
        let layout = self.layout(&version)?;
        let path = layout.records_dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(AiPatchSourceCycleStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        let contents =
            fs::read_to_string(&path).map_err(|source| AiPatchSourceCycleStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let record =
            serde_json::from_str::<AiPatchSourceCycleRecord>(&contents).map_err(|source| {
                AiPatchSourceCycleStoreError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;
        if record.version != version || record.id != id {
            return Err(AiPatchSourceCycleStoreError::NotFound {
                version,
                id: id.to_string(),
            });
        }

        Ok(record)
    }

    fn layout(
        &self,
        version: &str,
    ) -> Result<AiPatchSourceCycleLayout, AiPatchSourceCycleStoreError> {
        let major = version_major_key(version)?;
        let workspace = self.root.join("workspaces").join(&major);
        if !workspace.is_dir() {
            return Err(AiPatchSourceCycleStoreError::WorkspaceMissing {
                version: version.to_string(),
                path: workspace,
            });
        }

        let relative_agents_dir = PathBuf::from("workspaces")
            .join(&major)
            .join("artifacts")
            .join(AGENT_ARTIFACT_DIRECTORY);
        let relative_records_dir = relative_agents_dir.join(PATCH_SOURCE_CYCLE_DIRECTORY);
        let records_dir = self.root.join(&relative_records_dir);
        let index_path = records_dir.join(PATCH_SOURCE_CYCLE_INDEX_FILE);

        Ok(AiPatchSourceCycleLayout {
            records_dir,
            relative_records_dir,
            index_path,
        })
    }

    fn write_json<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), AiPatchSourceCycleStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            AiPatchSourceCycleStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourceCycleStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), AiPatchSourceCycleStoreError> {
        let contents = value
            .trim_end_matches(|character| character == '\r' || character == '\n')
            .to_string()
            + "\n";
        fs::write(path, contents).map_err(|source| AiPatchSourceCycleStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn append_summary(
        &self,
        path: &Path,
        summary: AiPatchSourceCycleSummary,
    ) -> Result<(), AiPatchSourceCycleStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AiPatchSourceCycleStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let line = serde_json::to_string(&summary).map_err(|source| {
            AiPatchSourceCycleStoreError::Serialize {
                path: path.to_path_buf(),
                source,
            }
        })? + "\n";
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| AiPatchSourceCycleStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        file.write_all(line.as_bytes())
            .map_err(|source| AiPatchSourceCycleStoreError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
}

impl AiPatchSourceCycleRecord {
    pub fn summary(&self) -> AiPatchSourceCycleSummary {
        AiPatchSourceCycleSummary {
            id: self.id.clone(),
            version: self.version.clone(),
            candidate_record_id: self.candidate_record_id.clone(),
            created_at_unix_seconds: self.created_at_unix_seconds,
            status: self.status,
            candidate_version: self.candidate_version.clone(),
            cycle_result: self.cycle_result.clone(),
            error: self.error.clone(),
            file: self.file.clone(),
        }
    }
}

impl fmt::Display for AiPatchSourceCycleStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCycleStatus::Promoted => write!(formatter, "已提升"),
            AiPatchSourceCycleStatus::RolledBack => write!(formatter, "已回滚"),
            AiPatchSourceCycleStatus::Blocked => write!(formatter, "已阻断"),
        }
    }
}

impl fmt::Display for AiPatchSourceCycleResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCycleResult::Promoted => write!(formatter, "已提升"),
            AiPatchSourceCycleResult::RolledBack => write!(formatter, "已回滚"),
        }
    }
}

impl fmt::Display for AiPatchSourceCycleStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiPatchSourceCycleStoreError::Version(error) => write!(formatter, "{error}"),
            AiPatchSourceCycleStoreError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工作区不存在，无法写入 AI 补丁源码覆盖候选 cycle 记录：{}",
                path.display()
            ),
            AiPatchSourceCycleStoreError::IdExhausted { version } => write!(
                formatter,
                "版本 {version} 无法生成唯一 AI 补丁源码覆盖候选 cycle 编号"
            ),
            AiPatchSourceCycleStoreError::InvalidRecordId { id } => {
                write!(formatter, "AI 补丁源码覆盖候选 cycle 编号不合法：{id}")
            }
            AiPatchSourceCycleStoreError::NotFound { version, id } => write!(
                formatter,
                "版本 {version} 未找到 AI 补丁源码覆盖候选 cycle {id}"
            ),
            AiPatchSourceCycleStoreError::Io { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖候选 cycle 文件读写失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceCycleStoreError::Serialize { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖候选 cycle 序列化失败 {}：{}",
                path.display(),
                source
            ),
            AiPatchSourceCycleStoreError::Parse { path, source } => write!(
                formatter,
                "AI 补丁源码覆盖候选 cycle 解析失败 {}：{}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for AiPatchSourceCycleStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AiPatchSourceCycleStoreError::Version(error) => Some(error),
            AiPatchSourceCycleStoreError::Io { source, .. } => Some(source),
            AiPatchSourceCycleStoreError::Serialize { source, .. } => Some(source),
            AiPatchSourceCycleStoreError::Parse { source, .. } => Some(source),
            AiPatchSourceCycleStoreError::WorkspaceMissing { .. }
            | AiPatchSourceCycleStoreError::IdExhausted { .. }
            | AiPatchSourceCycleStoreError::InvalidRecordId { .. }
            | AiPatchSourceCycleStoreError::NotFound { .. } => None,
        }
    }
}

impl From<VersionError> for AiPatchSourceCycleStoreError {
    fn from(error: VersionError) -> Self {
        AiPatchSourceCycleStoreError::Version(error)
    }
}

#[derive(Debug)]
struct AiPatchSourceCycleLayout {
    records_dir: PathBuf,
    relative_records_dir: PathBuf,
    index_path: PathBuf,
}

fn validate_record_id(id: &str) -> Result<(), AiPatchSourceCycleStoreError> {
    let valid = id.starts_with("patch-source-cycle-")
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(AiPatchSourceCycleStoreError::InvalidRecordId { id: id.to_string() })
    }
}
