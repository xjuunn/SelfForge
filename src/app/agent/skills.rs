use crate::{VersionError, version_major_key};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

const SKILL_INDEX_FILE: &str = "skill-index.json";
const DEFAULT_SKILL_LIMIT: usize = 5;
const DEFAULT_TOKEN_BUDGET: usize = 2_000;
const DEFAULT_SKILL_TOKEN_ESTIMATE: usize = 128;
const MAX_SKILL_CONTENT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentSkillIndex {
    #[serde(default)]
    pub skills: Vec<AgentSkillMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSkillMetadata {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub content_path: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub estimated_tokens: usize,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSkillIndexReport {
    pub version: String,
    pub index_path: PathBuf,
    pub index_exists: bool,
    pub skill_count: usize,
    pub enabled_skill_count: usize,
    pub loaded_skill_count: usize,
    pub estimated_index_tokens: usize,
    pub skills: Vec<AgentSkillMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSkillSelectionRequest {
    pub version: String,
    pub goal: String,
    pub limit: usize,
    pub token_budget: usize,
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSkillSelectionReport {
    pub version: String,
    pub goal: String,
    pub index_skill_count: usize,
    pub candidate_skill_count: usize,
    pub selected_skill_count: usize,
    pub loaded_skill_count: usize,
    pub skipped_for_budget: usize,
    pub estimated_context_tokens: usize,
    pub skills: Vec<AgentSkillSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSkillSelection {
    pub metadata: AgentSkillMetadata,
    pub score: usize,
    pub reason: String,
    pub content: Option<String>,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScoredSkill {
    metadata: AgentSkillMetadata,
    score: usize,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentSkillSearchQuery {
    goal: String,
    goal_terms: Vec<String>,
    required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedAgentSkill<'a> {
    metadata: &'a AgentSkillMetadata,
    name: PreparedSkillText,
    summary: PreparedSkillText,
    tags: Vec<PreparedSkillText>,
    triggers: Vec<PreparedSkillText>,
    capabilities: Vec<PreparedSkillText>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedSkillText {
    value: String,
    terms: Vec<String>,
}

#[derive(Debug)]
pub enum AgentSkillError {
    Version(VersionError),
    WorkspaceMissing {
        version: String,
        path: PathBuf,
    },
    InvalidSkillId {
        skill_id: String,
    },
    DuplicateSkill {
        skill_id: String,
    },
    InvalidSkillPath {
        skill_id: String,
        path: PathBuf,
    },
    EmptyGoal,
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

pub fn initialize_agent_skill_index(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
) -> Result<AgentSkillIndexReport, AgentSkillError> {
    let root = root.as_ref();
    let version = version.as_ref();
    let index_path = skill_index_path(root, version)?;
    if !index_path.exists() {
        let parent = index_path.parent().ok_or_else(|| AgentSkillError::Io {
            path: index_path.clone(),
            source: io::Error::new(io::ErrorKind::NotFound, "缺少技能索引父目录"),
        })?;
        fs::create_dir_all(parent).map_err(|source| AgentSkillError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let contents =
            serde_json::to_string_pretty(&AgentSkillIndex::default()).map_err(|source| {
                AgentSkillError::Serialize {
                    path: index_path.clone(),
                    source,
                }
            })? + "\n";
        fs::write(&index_path, contents).map_err(|source| AgentSkillError::Io {
            path: index_path.clone(),
            source,
        })?;
    }

    load_agent_skill_index(root, version)
}

pub fn load_agent_skill_index(
    root: impl AsRef<Path>,
    version: impl AsRef<str>,
) -> Result<AgentSkillIndexReport, AgentSkillError> {
    let root = root.as_ref();
    let version = version.as_ref().to_string();
    let index_path = skill_index_path(root, &version)?;
    let index_exists = index_path.exists();
    let index = if index_exists {
        let contents = fs::read_to_string(&index_path).map_err(|source| AgentSkillError::Io {
            path: index_path.clone(),
            source,
        })?;
        serde_json::from_str::<AgentSkillIndex>(&contents).map_err(|source| {
            AgentSkillError::Parse {
                path: index_path.clone(),
                source,
            }
        })?
    } else {
        AgentSkillIndex::default()
    };

    validate_skill_index(root, &index)?;
    let enabled_skill_count = index.skills.iter().filter(|skill| skill.enabled).count();
    let estimated_index_tokens = index.skills.iter().map(estimate_metadata_tokens).sum();

    Ok(AgentSkillIndexReport {
        version,
        index_path,
        index_exists,
        skill_count: index.skills.len(),
        enabled_skill_count,
        loaded_skill_count: 0,
        estimated_index_tokens,
        skills: index.skills,
    })
}

pub fn select_agent_skills(
    root: impl AsRef<Path>,
    request: AgentSkillSelectionRequest,
) -> Result<AgentSkillSelectionReport, AgentSkillError> {
    let root = root.as_ref();
    if request.goal.trim().is_empty() {
        return Err(AgentSkillError::EmptyGoal);
    }
    let limit = if request.limit == 0 {
        DEFAULT_SKILL_LIMIT
    } else {
        request.limit
    };
    let token_budget = if request.token_budget == 0 {
        DEFAULT_TOKEN_BUDGET
    } else {
        request.token_budget
    };
    let query = AgentSkillSearchQuery::new(&request.goal, &request.required_capabilities);
    let index = load_agent_skill_index(root, &request.version)?;
    let mut candidates = index
        .skills
        .iter()
        .filter(|skill| skill.enabled)
        .map(PreparedAgentSkill::new)
        .filter(|skill| capability_filter_matches(skill, &query.required_capabilities))
        .filter_map(|skill| score_skill(&skill, &query))
        .collect::<Vec<_>>();

    candidates.sort_by_key(|skill| (Reverse(skill.score), Reverse(skill.metadata.priority)));

    let candidate_skill_count = candidates.len();
    let mut selected = Vec::new();
    let mut estimated_context_tokens = 0;
    let mut skipped_for_budget = 0;

    for candidate in candidates {
        if selected.len() >= limit {
            break;
        }
        let estimate = skill_token_estimate(&candidate.metadata);
        if estimated_context_tokens + estimate > token_budget {
            skipped_for_budget += 1;
            continue;
        }

        let content = match &candidate.metadata.content_path {
            Some(path) => Some(read_skill_content(root, &candidate.metadata.id, path)?),
            None => None,
        };
        estimated_context_tokens += estimate;
        selected.push(AgentSkillSelection {
            metadata: candidate.metadata,
            score: candidate.score,
            reason: candidate.reason,
            content,
            estimated_tokens: estimate,
        });
    }

    Ok(AgentSkillSelectionReport {
        version: request.version,
        goal: request.goal,
        index_skill_count: index.skill_count,
        candidate_skill_count,
        selected_skill_count: selected.len(),
        loaded_skill_count: selected
            .iter()
            .filter(|skill| skill.content.is_some())
            .count(),
        skipped_for_budget,
        estimated_context_tokens,
        skills: selected,
    })
}

pub fn format_agent_skill_context(skills: &AgentSkillSelectionReport) -> String {
    if skills.skills.is_empty() {
        return format!(
            "- 未召回技能；索引技能 {} 个，候选 {} 个，默认只使用当前状态和近期记忆。\n",
            skills.index_skill_count, skills.candidate_skill_count
        );
    }

    let mut lines = vec![format!(
        "- 技能索引 {} 个，候选 {} 个，已选择 {} 个，已加载正文 {} 个，上下文 token 估算 {}。",
        skills.index_skill_count,
        skills.candidate_skill_count,
        skills.selected_skill_count,
        skills.loaded_skill_count,
        skills.estimated_context_tokens
    )];
    for skill in &skills.skills {
        lines.push(format!(
            "- 技能 {}：{}；分数 {}；原因 {}；估算 token {}。",
            skill.metadata.id,
            skill.metadata.name,
            skill.score,
            skill.reason,
            skill.estimated_tokens
        ));
        if !skill.metadata.summary.trim().is_empty() {
            lines.push(format!("  摘要：{}", skill.metadata.summary.trim()));
        }
        if let Some(content) = &skill.content {
            lines.push(format!(
                "  正文：{}",
                truncate_skill_context(content.trim(), 1_200)
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn validate_skill_index(root: &Path, index: &AgentSkillIndex) -> Result<(), AgentSkillError> {
    let mut ids = std::collections::HashSet::new();
    for skill in &index.skills {
        if !valid_skill_id(&skill.id) {
            return Err(AgentSkillError::InvalidSkillId {
                skill_id: skill.id.clone(),
            });
        }
        if !ids.insert(skill.id.clone()) {
            return Err(AgentSkillError::DuplicateSkill {
                skill_id: skill.id.clone(),
            });
        }
        if let Some(path) = &skill.content_path {
            validate_relative_skill_path(root, &skill.id, path)?;
        }
    }
    Ok(())
}

fn score_skill(
    skill: &PreparedAgentSkill<'_>,
    query: &AgentSkillSearchQuery,
) -> Option<ScoredSkill> {
    let mut score = 0;
    let mut reasons = Vec::new();

    if query.contains_prepared(&skill.name) {
        score += 8;
        reasons.push("名称匹配");
    }
    for trigger in &skill.triggers {
        if query.contains_prepared(trigger) {
            score += 6;
            reasons.push("触发词匹配");
        }
    }
    for tag in &skill.tags {
        if query.contains_prepared(tag) {
            score += 3;
            reasons.push("标签匹配");
        }
    }
    for capability in &skill.capabilities {
        if query.contains_prepared(capability) {
            score += 2;
            reasons.push("能力匹配");
        }
    }
    if query.contains_prepared(&skill.summary) {
        score += 1;
        reasons.push("摘要匹配");
    }

    (score > 0).then(|| ScoredSkill {
        metadata: skill.metadata.clone(),
        score,
        reason: unique_reasons(reasons).join("、"),
    })
}

fn capability_filter_matches(skill: &PreparedAgentSkill<'_>, required: &[String]) -> bool {
    required.is_empty()
        || required.iter().all(|required| {
            skill
                .capabilities
                .iter()
                .any(|capability| capability.value == *required)
        })
}

impl AgentSkillSearchQuery {
    fn new(goal: &str, required_capabilities: &[String]) -> Self {
        let normalized_goal = normalize_search_text(goal);
        Self {
            goal_terms: split_search_terms(&normalized_goal),
            goal: normalized_goal,
            required_capabilities: normalize_unique_terms(required_capabilities),
        }
    }

    fn contains_prepared(&self, term: &PreparedSkillText) -> bool {
        if term.value.is_empty() {
            return false;
        }
        self.goal.contains(&term.value)
            || term
                .terms
                .iter()
                .any(|part| self.goal_terms.iter().any(|goal| goal.contains(part)))
    }
}

impl<'a> PreparedAgentSkill<'a> {
    fn new(metadata: &'a AgentSkillMetadata) -> Self {
        Self {
            metadata,
            name: PreparedSkillText::new(&metadata.name),
            summary: PreparedSkillText::new(&metadata.summary),
            tags: metadata
                .tags
                .iter()
                .map(|value| PreparedSkillText::new(value))
                .collect(),
            triggers: metadata
                .triggers
                .iter()
                .map(|value| PreparedSkillText::new(value))
                .collect(),
            capabilities: metadata
                .capabilities
                .iter()
                .map(|value| PreparedSkillText::new(value))
                .collect(),
        }
    }
}

impl PreparedSkillText {
    fn new(value: &str) -> Self {
        let value = normalize_search_text(value);
        Self {
            terms: split_search_terms(&value),
            value,
        }
    }
}

fn normalize_unique_terms(values: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = normalize_search_text(value);
        if !value.is_empty() && !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    normalized
}

fn normalize_search_text(value: &str) -> String {
    value.trim().to_lowercase()
}

fn split_search_terms(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn unique_reasons(reasons: Vec<&str>) -> Vec<&str> {
    let mut unique = Vec::new();
    for reason in reasons {
        if !unique.contains(&reason) {
            unique.push(reason);
        }
    }
    unique
}

fn read_skill_content(root: &Path, skill_id: &str, path: &str) -> Result<String, AgentSkillError> {
    let resolved = validate_relative_skill_path(root, skill_id, path)?;
    read_skill_content_prefix(&resolved).map_err(|source| AgentSkillError::Io {
        path: resolved.clone(),
        source,
    })
}

fn read_skill_content_prefix(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buffer = Vec::with_capacity(MAX_SKILL_CONTENT_BYTES + 4);
    file.by_ref()
        .take((MAX_SKILL_CONTENT_BYTES + 4) as u64)
        .read_to_end(&mut buffer)?;
    let truncated = buffer.len() > MAX_SKILL_CONTENT_BYTES;
    if truncated {
        buffer.truncate(MAX_SKILL_CONTENT_BYTES);
    }
    trim_to_utf8_boundary(&mut buffer, truncated)?;
    String::from_utf8(buffer).map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source))
}

fn trim_to_utf8_boundary(buffer: &mut Vec<u8>, truncated: bool) -> io::Result<()> {
    loop {
        match std::str::from_utf8(buffer) {
            Ok(_) => return Ok(()),
            Err(error) if truncated && error.error_len().is_none() => {
                buffer.truncate(error.valid_up_to());
            }
            Err(error) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, error));
            }
        }
    }
}

fn validate_relative_skill_path(
    root: &Path,
    skill_id: &str,
    value: &str,
) -> Result<PathBuf, AgentSkillError> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AgentSkillError::InvalidSkillPath {
            skill_id: skill_id.to_string(),
            path: path.to_path_buf(),
        });
    }
    let resolved = root.join(path);
    if !resolved.starts_with(root) {
        return Err(AgentSkillError::InvalidSkillPath {
            skill_id: skill_id.to_string(),
            path: resolved,
        });
    }
    Ok(resolved)
}

fn skill_index_path(root: &Path, version: &str) -> Result<PathBuf, AgentSkillError> {
    let major = version_major_key(version)?;
    let workspace = root.join("workspaces").join(&major);
    if !workspace.is_dir() {
        return Err(AgentSkillError::WorkspaceMissing {
            version: version.to_string(),
            path: workspace,
        });
    }
    Ok(workspace
        .join("artifacts")
        .join("agents")
        .join("skills")
        .join(SKILL_INDEX_FILE))
}

fn estimate_metadata_tokens(skill: &AgentSkillMetadata) -> usize {
    estimate_tokens(&format!(
        "{} {} {} {} {}",
        skill.id,
        skill.name,
        skill.summary,
        skill.tags.join(" "),
        skill.triggers.join(" ")
    ))
    .max(1)
}

fn skill_token_estimate(skill: &AgentSkillMetadata) -> usize {
    if skill.estimated_tokens == 0 {
        DEFAULT_SKILL_TOKEN_ESTIMATE
    } else {
        skill.estimated_tokens
    }
}

fn estimate_tokens(value: &str) -> usize {
    value.chars().count().div_ceil(4)
}

fn truncate_skill_context(value: &str, max: usize) -> String {
    let mut output = value.chars().take(max).collect::<String>();
    if value.chars().count() > max {
        output.push_str("...");
    }
    output
}

fn valid_skill_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn default_enabled() -> bool {
    true
}

fn default_priority() -> i32 {
    0
}

impl AgentSkillSelectionRequest {
    pub fn new(version: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            goal: goal.into(),
            limit: DEFAULT_SKILL_LIMIT,
            token_budget: DEFAULT_TOKEN_BUDGET,
            required_capabilities: Vec::new(),
        }
    }
}

impl fmt::Display for AgentSkillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSkillError::Version(error) => write!(formatter, "{error}"),
            AgentSkillError::WorkspaceMissing { version, path } => write!(
                formatter,
                "版本 {version} 的技能索引工作区不存在：{}",
                path.display()
            ),
            AgentSkillError::InvalidSkillId { skill_id } => {
                write!(formatter, "技能标识不合法：{skill_id}")
            }
            AgentSkillError::DuplicateSkill { skill_id } => {
                write!(formatter, "技能索引中存在重复技能：{skill_id}")
            }
            AgentSkillError::InvalidSkillPath { skill_id, path } => write!(
                formatter,
                "技能 {skill_id} 的正文路径不允许越过项目根目录：{}",
                path.display()
            ),
            AgentSkillError::EmptyGoal => write!(formatter, "技能召回目标不能为空"),
            AgentSkillError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            AgentSkillError::Parse { path, source } => {
                write!(
                    formatter,
                    "解析技能索引 {} 失败：{}",
                    path.display(),
                    source
                )
            }
            AgentSkillError::Serialize { path, source } => {
                write!(
                    formatter,
                    "序列化技能索引 {} 失败：{}",
                    path.display(),
                    source
                )
            }
        }
    }
}

impl Error for AgentSkillError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AgentSkillError::Version(error) => Some(error),
            AgentSkillError::Io { source, .. } => Some(source),
            AgentSkillError::Parse { source, .. } => Some(source),
            AgentSkillError::Serialize { source, .. } => Some(source),
            AgentSkillError::WorkspaceMissing { .. }
            | AgentSkillError::InvalidSkillId { .. }
            | AgentSkillError::DuplicateSkill { .. }
            | AgentSkillError::InvalidSkillPath { .. }
            | AgentSkillError::EmptyGoal => None,
        }
    }
}

impl From<VersionError> for AgentSkillError {
    fn from(error: VersionError) -> Self {
        AgentSkillError::Version(error)
    }
}
