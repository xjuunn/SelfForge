use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentCapability {
    Architecture,
    Planning,
    Implementation,
    Testing,
    Review,
    Documentation,
    Memory,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub capabilities: Vec<AgentCapability>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlan {
    pub goal: String,
    pub agents: Vec<AgentDefinition>,
    pub steps: Vec<AgentPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlanStep {
    pub order: usize,
    pub agent_id: String,
    pub title: String,
    pub capability: AgentCapability,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub verification: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    EmptyGoal,
    EmptyAgentId,
    DuplicateAgent { id: String },
    MissingCapability { capability: AgentCapability },
}

impl AgentDefinition {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        purpose: impl Into<String>,
        capabilities: Vec<AgentCapability>,
        inputs: Vec<&str>,
        outputs: Vec<&str>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            purpose: purpose.into(),
            capabilities,
            inputs: inputs.into_iter().map(str::to_string).collect(),
            outputs: outputs.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn has_capability(&self, capability: AgentCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

impl AgentCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentCapability::Architecture => "架构",
            AgentCapability::Planning => "计划",
            AgentCapability::Implementation => "实现",
            AgentCapability::Testing => "测试",
            AgentCapability::Review => "审查",
            AgentCapability::Documentation => "归档",
            AgentCapability::Memory => "记忆",
            AgentCapability::Runtime => "运行",
        }
    }
}

impl fmt::Display for AgentCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::EmptyGoal => write!(formatter, "Agent 计划目标不能为空"),
            AgentError::EmptyAgentId => write!(formatter, "Agent 标识不能为空"),
            AgentError::DuplicateAgent { id } => {
                write!(formatter, "Agent 标识重复：{id}")
            }
            AgentError::MissingCapability { capability } => {
                write!(formatter, "缺少具备 {capability} 能力的 Agent")
            }
        }
    }
}

impl Error for AgentError {}
