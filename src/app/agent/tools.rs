use super::types::{AgentCapability, AgentDefinition, AgentPlan};
use crate::{VersionError, version_major_key};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const TOOL_CONFIG_FILE: &str = "tool-config.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    #[serde(default)]
    pub capabilities: Vec<AgentCapability>,
    #[serde(default)]
    pub agent_ids: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolBinding {
    pub agent_id: String,
    #[serde(default)]
    pub tool_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentToolConfig {
    #[serde(default)]
    pub tools: Vec<AgentToolDefinition>,
    #[serde(default)]
    pub agent_bindings: Vec<AgentToolBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolReport {
    pub version: String,
    pub config_path: PathBuf,
    pub config_exists: bool,
    pub tools: Vec<AgentToolDefinition>,
    pub assignments: Vec<AgentToolAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolAssignment {
    pub agent_id: String,
    pub tool_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolConfigInitReport {
    pub version: String,
    pub config_path: PathBuf,
    pub created: bool,
}

#[derive(Debug)]
pub enum AgentToolError {
    Version(VersionError),
    WorkspaceMissing {
        version: String,
        path: PathBuf,
    },
    UnknownAgent {
        agent_id: String,
    },
    UnknownTool {
        tool_id: String,
    },
    DisabledTool {
        tool_id: String,
    },
    DuplicateTool {
        tool_id: String,
    },
    InvalidToolId {
        tool_id: String,
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

pub fn load_agent_tool_report(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
    agents: &[AgentDefinition],
) -> Result<AgentToolReport, AgentToolError> {
    let root = root.as_ref();
    let version = version.as_ref().to_string();
    let config_path = tool_config_path(root, &version)?;
    let config_exists = config_path.exists();
    let config = if config_exists {
        let contents = fs::read_to_string(&config_path).map_err(|source| AgentToolError::Io {
            path: config_path.clone(),
            source,
        })?;
        serde_json::from_str::<AgentToolConfig>(&contents).map_err(|source| {
            AgentToolError::Parse {
                path: config_path.clone(),
                source,
            }
        })?
    } else {
        AgentToolConfig::default()
    };

    build_tool_report(version, config_path, config_exists, config, agents)
}

pub fn initialize_agent_tool_config(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
) -> Result<AgentToolConfigInitReport, AgentToolError> {
    let root = root.as_ref();
    let version = version.as_ref().to_string();
    let config_path = tool_config_path(root, &version)?;
    if config_path.exists() {
        return Ok(AgentToolConfigInitReport {
            version,
            config_path,
            created: false,
        });
    }

    let parent = config_path.parent().ok_or_else(|| AgentToolError::Io {
        path: config_path.clone(),
        source: io::Error::new(io::ErrorKind::NotFound, "缺少工具配置父目录"),
    })?;
    fs::create_dir_all(parent).map_err(|source| AgentToolError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let config = AgentToolConfig::default();
    let contents =
        serde_json::to_string_pretty(&config).map_err(|source| AgentToolError::Serialize {
            path: config_path.clone(),
            source,
        })? + "\n";
    fs::write(&config_path, contents).map_err(|source| AgentToolError::Io {
        path: config_path.clone(),
        source,
    })?;

    Ok(AgentToolConfigInitReport {
        version,
        config_path,
        created: true,
    })
}

pub fn apply_tools_to_plan(plan: &mut AgentPlan, report: &AgentToolReport) {
    for step in &mut plan.steps {
        step.tool_ids = report.tool_ids_for_agent(&step.agent_id);
    }
}

impl AgentToolReport {
    pub fn tool_ids_for_agent(&self, agent_id: &str) -> Vec<String> {
        self.assignments
            .iter()
            .find(|assignment| assignment.agent_id == agent_id)
            .map(|assignment| assignment.tool_ids.clone())
            .unwrap_or_default()
    }
}

impl AgentToolDefinition {
    fn builtin(
        id: &str,
        name: &str,
        description: &str,
        kind: &str,
        capabilities: Vec<AgentCapability>,
        agent_ids: Vec<&str>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            kind: kind.to_string(),
            capabilities,
            agent_ids: agent_ids.into_iter().map(str::to_string).collect(),
            enabled: true,
        }
    }

    fn capability_matches(&self, capability: AgentCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

fn build_tool_report(
    version: String,
    config_path: PathBuf,
    config_exists: bool,
    config: AgentToolConfig,
    agents: &[AgentDefinition],
) -> Result<AgentToolReport, AgentToolError> {
    let agent_ids = agents
        .iter()
        .map(|agent| agent.id.as_str())
        .collect::<HashSet<_>>();
    let mut tools = builtin_tools();
    let mut config_tool_ids = HashSet::new();

    for tool in config.tools {
        validate_tool(&tool, &agent_ids)?;
        if !config_tool_ids.insert(tool.id.clone()) {
            return Err(AgentToolError::DuplicateTool { tool_id: tool.id });
        }
        if let Some(existing) = tools.iter_mut().find(|existing| existing.id == tool.id) {
            *existing = tool;
        } else {
            tools.push(tool);
        }
    }

    let tool_by_id = tools
        .iter()
        .map(|tool| (tool.id.as_str(), tool))
        .collect::<HashMap<_, _>>();
    for binding in &config.agent_bindings {
        if !agent_ids.contains(binding.agent_id.as_str()) {
            return Err(AgentToolError::UnknownAgent {
                agent_id: binding.agent_id.clone(),
            });
        }
        for tool_id in &binding.tool_ids {
            let Some(tool) = tool_by_id.get(tool_id.as_str()) else {
                return Err(AgentToolError::UnknownTool {
                    tool_id: tool_id.clone(),
                });
            };
            if !tool.enabled {
                return Err(AgentToolError::DisabledTool {
                    tool_id: tool_id.clone(),
                });
            }
        }
    }

    let mut assignments = Vec::new();
    for agent in agents {
        let mut tool_ids = Vec::new();
        for tool in tools.iter().filter(|tool| tool.enabled) {
            let explicitly_allowed = tool.agent_ids.iter().any(|id| id == &agent.id);
            let capability_allowed = agent
                .capabilities
                .iter()
                .any(|capability| tool.capability_matches(*capability));
            if explicitly_allowed || capability_allowed {
                push_unique(&mut tool_ids, &tool.id);
            }
        }
        for binding in config
            .agent_bindings
            .iter()
            .filter(|binding| binding.agent_id == agent.id)
        {
            for tool_id in &binding.tool_ids {
                push_unique(&mut tool_ids, tool_id);
            }
        }
        assignments.push(AgentToolAssignment {
            agent_id: agent.id.clone(),
            tool_ids,
        });
    }

    Ok(AgentToolReport {
        version,
        config_path,
        config_exists,
        tools,
        assignments,
    })
}

fn validate_tool(
    tool: &AgentToolDefinition,
    agent_ids: &HashSet<&str>,
) -> Result<(), AgentToolError> {
    if !valid_tool_id(&tool.id) {
        return Err(AgentToolError::InvalidToolId {
            tool_id: tool.id.clone(),
        });
    }
    for agent_id in &tool.agent_ids {
        if !agent_ids.contains(agent_id.as_str()) {
            return Err(AgentToolError::UnknownAgent {
                agent_id: agent_id.clone(),
            });
        }
    }
    Ok(())
}

fn valid_tool_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn builtin_tools() -> Vec<AgentToolDefinition> {
    vec![
        AgentToolDefinition::builtin(
            "memory.context",
            "最近记忆读取",
            "读取 major 聚合记忆中的最近历史记录。",
            "memory",
            vec![AgentCapability::Memory, AgentCapability::Planning],
            vec!["architect", "archivist"],
        ),
        AgentToolDefinition::builtin(
            "memory.insights",
            "记忆经验提取",
            "提取成功经验、失败风险、优化建议和可复用经验。",
            "memory",
            vec![AgentCapability::Memory, AgentCapability::Planning],
            vec!["architect", "archivist"],
        ),
        AgentToolDefinition::builtin(
            "runtime.run",
            "受控运行",
            "通过 Runtime 在目标 workspace 中执行明确程序并记录运行证据。",
            "runtime",
            vec![AgentCapability::Runtime, AgentCapability::Testing],
            vec!["builder", "verifier"],
        ),
        AgentToolDefinition::builtin(
            "agent.session",
            "会话审计",
            "读取和记录 Agent 会话、事件时间线和计划上下文。",
            "agent",
            vec![AgentCapability::Planning, AgentCapability::Documentation],
            vec!["architect", "archivist"],
        ),
        AgentToolDefinition::builtin(
            "forge.archive",
            "Forge 归档",
            "将任务、记忆、错误和版本记录写入 major 聚合文件。",
            "archive",
            vec![AgentCapability::Documentation, AgentCapability::Memory],
            vec!["archivist"],
        ),
        AgentToolDefinition::builtin(
            "ai.request",
            "统一 AI 请求",
            "通过统一 AI Provider 模块发起受控非流式请求。",
            "ai",
            vec![
                AgentCapability::Planning,
                AgentCapability::Implementation,
                AgentCapability::Review,
            ],
            vec!["architect", "builder", "reviewer"],
        ),
    ]
}

fn tool_config_path(root: &Path, version: &str) -> Result<PathBuf, AgentToolError> {
    let major = version_major_key(version)?;
    let workspace = root.join("workspaces").join(&major);
    if !workspace.is_dir() {
        return Err(AgentToolError::WorkspaceMissing {
            version: version.to_string(),
            path: workspace,
        });
    }

    Ok(workspace
        .join("artifacts")
        .join("agents")
        .join(TOOL_CONFIG_FILE))
}

fn default_enabled() -> bool {
    true
}

impl fmt::Display for AgentToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentToolError::Version(error) => write!(formatter, "{error}"),
            AgentToolError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的工具配置工作区不存在：{}",
                path.display()
            ),
            AgentToolError::UnknownAgent { agent_id } => {
                write!(formatter, "工具配置引用了未知 Agent：{agent_id}")
            }
            AgentToolError::UnknownTool { tool_id } => {
                write!(formatter, "工具配置引用了未知工具：{tool_id}")
            }
            AgentToolError::DisabledTool { tool_id } => {
                write!(formatter, "工具配置引用了已禁用工具：{tool_id}")
            }
            AgentToolError::DuplicateTool { tool_id } => {
                write!(formatter, "工具配置中存在重复工具：{tool_id}")
            }
            AgentToolError::InvalidToolId { tool_id } => {
                write!(formatter, "工具标识不合法：{tool_id}")
            }
            AgentToolError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            AgentToolError::Parse { path, source } => {
                write!(
                    formatter,
                    "解析工具配置 {} 失败：{}",
                    path.display(),
                    source
                )
            }
            AgentToolError::Serialize { path, source } => {
                write!(
                    formatter,
                    "序列化工具配置 {} 失败：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AgentToolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentToolError::Version(error) => Some(error),
            AgentToolError::Io { source, .. } => Some(source),
            AgentToolError::Parse { source, .. } => Some(source),
            AgentToolError::Serialize { source, .. } => Some(source),
            AgentToolError::WorkspaceMissing { .. }
            | AgentToolError::UnknownAgent { .. }
            | AgentToolError::UnknownTool { .. }
            | AgentToolError::DisabledTool { .. }
            | AgentToolError::DuplicateTool { .. }
            | AgentToolError::InvalidToolId { .. } => None,
        }
    }
}

impl From<VersionError> for AgentToolError {
    fn from(error: VersionError) -> Self {
        AgentToolError::Version(error)
    }
}
