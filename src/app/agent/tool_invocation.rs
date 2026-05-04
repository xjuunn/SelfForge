use super::session::AgentRunReference;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolInvocation {
    pub agent_id: String,
    pub tool_id: String,
    pub version: String,
    pub input: AgentToolInvocationInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentToolInvocationInput {
    Empty,
    MemoryContext {
        limit: usize,
    },
    MemoryInsights {
        limit: usize,
    },
    AgentSessions {
        limit: usize,
        all_major: bool,
    },
    RuntimeRun {
        session_version: String,
        session_id: String,
        target_version: String,
        step_order: usize,
        program: String,
        args: Vec<String>,
        timeout_ms: u64,
    },
    AiRequestPreview {
        prompt: String,
    },
    CodeSearch {
        query: String,
        limit: usize,
    },
    CodeRead {
        path: String,
        max_bytes: usize,
    },
    ForgeArchiveStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolInvocationReport {
    pub agent_id: String,
    pub tool_id: String,
    pub version: String,
    pub summary: String,
    #[serde(default)]
    pub details: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<AgentRunReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStepExecutionRequest {
    pub session_version: String,
    pub session_id: String,
    pub target_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
    pub limit: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStepExecutionReport {
    pub session_id: String,
    pub session_version: String,
    pub step_order: usize,
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_worker_id: Option<String>,
    pub tool: AgentToolInvocationReport,
    pub session_completed: bool,
}
