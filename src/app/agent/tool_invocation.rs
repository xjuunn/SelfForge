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
