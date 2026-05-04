use crate::VersionError;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

pub(super) const DEFAULT_WORK_LEASE_SECONDS: u64 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentWorkTaskStatus {
    Pending,
    Claimed,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkQueue {
    pub version: String,
    pub goal: String,
    pub thread_count: usize,
    #[serde(default = "default_work_lease_seconds")]
    pub lease_duration_seconds: u64,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    pub conflict_policy: String,
    pub prompt_policy: String,
    pub tasks: Vec<AgentWorkTask>,
    #[serde(default)]
    pub events: Vec<AgentWorkEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub preferred_agent_id: String,
    pub priority: usize,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub write_scope: Vec<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    pub status: AgentWorkTaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_at_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkEvent {
    pub order: usize,
    pub timestamp_unix_seconds: u64,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkQueueReport {
    pub version: String,
    pub queue_path: PathBuf,
    pub created: bool,
    pub queue: AgentWorkQueue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkClaimReport {
    pub version: String,
    pub queue_path: PathBuf,
    pub worker_id: String,
    pub task: AgentWorkTask,
    pub remaining_available: usize,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkReapReport {
    pub version: String,
    pub queue_path: PathBuf,
    pub released_tasks: Vec<AgentWorkTask>,
    pub queue: AgentWorkQueue,
}

#[derive(Debug, Clone)]
pub struct AgentWorkCoordinator {
    pub(super) root: PathBuf,
}

#[derive(Debug)]
pub enum AgentWorkError {
    Version(VersionError),
    WorkspaceMissing {
        version: String,
        path: PathBuf,
    },
    MissingQueue {
        version: String,
        path: PathBuf,
    },
    InvalidThreadCount,
    InvalidLeaseSeconds,
    InvalidWorkerId {
        worker_id: String,
    },
    InvalidTaskId {
        task_id: String,
    },
    TaskNotFound {
        task_id: String,
    },
    TaskNotClaimedByWorker {
        task_id: String,
        worker_id: String,
    },
    NoAvailableTask {
        version: String,
    },
    QueueNotCompleted {
        version: String,
    },
    LockBusy {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
}

fn default_work_lease_seconds() -> u64 {
    DEFAULT_WORK_LEASE_SECONDS
}

impl fmt::Display for AgentWorkTaskStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentWorkTaskStatus::Pending => formatter.write_str("待领取"),
            AgentWorkTaskStatus::Claimed => formatter.write_str("已领取"),
            AgentWorkTaskStatus::Completed => formatter.write_str("已完成"),
            AgentWorkTaskStatus::Blocked => formatter.write_str("已阻断"),
        }
    }
}

impl fmt::Display for AgentWorkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentWorkError::Version(error) => write!(formatter, "{error}"),
            AgentWorkError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的协作工作区不存在：{}",
                path.display()
            ),
            AgentWorkError::MissingQueue { version, path } => {
                write!(
                    formatter,
                    "版本 {version} 的协作队列不存在：{}",
                    path.display()
                )
            }
            AgentWorkError::InvalidThreadCount => write!(formatter, "线程数量必须大于 0"),
            AgentWorkError::InvalidLeaseSeconds => write!(formatter, "任务租约秒数必须大于 0"),
            AgentWorkError::InvalidWorkerId { worker_id } => {
                write!(formatter, "工作线程标识不合法：{worker_id}")
            }
            AgentWorkError::InvalidTaskId { task_id } => {
                write!(formatter, "任务标识不合法：{task_id}")
            }
            AgentWorkError::TaskNotFound { task_id } => {
                write!(formatter, "协作任务不存在：{task_id}")
            }
            AgentWorkError::TaskNotClaimedByWorker { task_id, worker_id } => write!(
                formatter,
                "任务 {task_id} 未被工作线程 {worker_id} 领取，禁止完成或释放"
            ),
            AgentWorkError::NoAvailableTask { version } => {
                write!(formatter, "版本 {version} 当前没有可领取任务")
            }
            AgentWorkError::QueueNotCompleted { version } => {
                write!(formatter, "版本 {version} 的协作队列尚未全部完成，禁止重开")
            }
            AgentWorkError::LockBusy { path } => {
                write!(formatter, "协作队列锁繁忙：{}", path.display())
            }
            AgentWorkError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            AgentWorkError::Parse { path, source } => {
                write!(
                    formatter,
                    "解析协作队列 {} 失败：{}",
                    path.display(),
                    source
                )
            }
            AgentWorkError::Serialize { path, source } => write!(
                formatter,
                "序列化协作队列 {} 失败：{}",
                path.display(),
                source
            ),
        }
    }
}

impl Error for AgentWorkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentWorkError::Version(error) => Some(error),
            AgentWorkError::Io { source, .. } => Some(source),
            AgentWorkError::Parse { source, .. } => Some(source),
            AgentWorkError::Serialize { source, .. } => Some(source),
            AgentWorkError::WorkspaceMissing { .. }
            | AgentWorkError::MissingQueue { .. }
            | AgentWorkError::InvalidThreadCount
            | AgentWorkError::InvalidLeaseSeconds
            | AgentWorkError::InvalidWorkerId { .. }
            | AgentWorkError::InvalidTaskId { .. }
            | AgentWorkError::TaskNotFound { .. }
            | AgentWorkError::TaskNotClaimedByWorker { .. }
            | AgentWorkError::NoAvailableTask { .. }
            | AgentWorkError::QueueNotCompleted { .. }
            | AgentWorkError::LockBusy { .. } => None,
        }
    }
}

impl From<VersionError> for AgentWorkError {
    fn from(error: VersionError) -> Self {
        AgentWorkError::Version(error)
    }
}
