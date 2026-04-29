use crate::layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport, major_file_name};
use crate::runtime::Runtime;
use crate::state::{ForgeState, StateError};
use crate::version::{VersionBump, VersionError, next_version_after_with_bump, version_major_key};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct EvolutionEngine {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionReport {
    pub current_version: String,
    pub next_version: String,
    pub workspace: PathBuf,
    pub created_paths: Vec<PathBuf>,
    pub existing_paths: Vec<PathBuf>,
    pub candidate_validation: ValidationReport,
    pub state: ForgeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionReport {
    pub previous_version: String,
    pub promoted_version: String,
    pub state: ForgeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackReport {
    pub current_version: String,
    pub rolled_back_version: String,
    pub state: ForgeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleResult {
    Promoted,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleReport {
    pub previous_version: String,
    pub candidate_version: String,
    pub result: CycleResult,
    pub candidate_validation: Option<ValidationReport>,
    pub failure: Option<String>,
    pub state: ForgeState,
}

#[derive(Debug)]
pub enum EvolutionError {
    State(StateError),
    Forge(ForgeError),
    Version(VersionError),
    CandidateAlreadyPrepared { version: String },
    MissingCandidate,
    Io { path: PathBuf, source: io::Error },
}

impl EvolutionEngine {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn prepare_next_version(&self, goal: &str) -> Result<EvolutionReport, EvolutionError> {
        self.prepare_next_version_with_bump(goal, VersionBump::Patch)
    }

    pub fn prepare_next_version_with_bump(
        &self,
        goal: &str,
        bump: VersionBump,
    ) -> Result<EvolutionReport, EvolutionError> {
        let mut state = ForgeState::load(&self.root)?;

        if let Some(version) = &state.candidate_version {
            if state.status == "candidate_prepared" {
                return Err(EvolutionError::CandidateAlreadyPrepared {
                    version: version.clone(),
                });
            }
        }

        let current_version = state.current_version.clone();
        let next_version = next_version_after_with_bump(&current_version, bump)?;
        let current_workspace = format!("workspaces/{}", version_major_key(&current_version)?);

        let runtime = Runtime::new(&self.root);
        runtime.verify_layout_for_version(&current_version)?;

        let BootstrapReport {
            created_paths,
            existing_paths,
            ..
        } = SelfForge::for_version(&self.root, &next_version).bootstrap()?;

        write_candidate_documents(&self.root, &current_version, &next_version, goal)?;

        let candidate_validation = runtime.verify_layout_for_version(&next_version)?;
        let candidate_workspace = format!("workspaces/{}", version_major_key(&next_version)?);

        state.status = "candidate_prepared".to_string();
        state.workspace = current_workspace;
        state.version_scheme = Some("semantic:vMAJOR.MINOR.PATCH".to_string());
        state.candidate_version = Some(next_version.clone());
        state.candidate_workspace = Some(candidate_workspace.clone());
        state.last_verified = Some(format!("candidate:{next_version}"));
        state.save(&self.root)?;

        Ok(EvolutionReport {
            current_version,
            next_version,
            workspace: self.root.join(candidate_workspace),
            created_paths,
            existing_paths,
            candidate_validation,
            state,
        })
    }

    pub fn promote_candidate(&self) -> Result<PromotionReport, EvolutionError> {
        let mut state = ForgeState::load(&self.root)?;
        let Some(candidate_version) = state.candidate_version.clone() else {
            return Err(EvolutionError::MissingCandidate);
        };
        let Some(candidate_workspace) = state.candidate_workspace.clone() else {
            return Err(EvolutionError::MissingCandidate);
        };

        let runtime = Runtime::new(&self.root);
        runtime.verify_layout_for_version(&candidate_version)?;

        let previous_version = state.current_version.clone();
        state.parent_version = Some(previous_version.clone());
        state.current_version = candidate_version.clone();
        state.workspace = candidate_workspace;
        state.status = "active".to_string();
        state.last_verified = Some(format!("promoted:{candidate_version}"));
        state.candidate_version = None;
        state.candidate_workspace = None;
        state.version_scheme = Some("semantic:vMAJOR.MINOR.PATCH".to_string());
        state.save(&self.root)?;

        Ok(PromotionReport {
            previous_version,
            promoted_version: candidate_version,
            state,
        })
    }

    pub fn rollback_candidate(&self, reason: &str) -> Result<RollbackReport, EvolutionError> {
        let mut state = ForgeState::load(&self.root)?;
        let Some(candidate_version) = state.candidate_version.clone() else {
            return Err(EvolutionError::MissingCandidate);
        };

        let current_version = state.current_version.clone();
        state.status = "rolled_back".to_string();
        state.last_verified = Some(format!("rollback:{candidate_version}:{reason}"));
        state.candidate_version = None;
        state.candidate_workspace = None;
        state.version_scheme = Some("semantic:vMAJOR.MINOR.PATCH".to_string());
        state.save(&self.root)?;

        Ok(RollbackReport {
            current_version,
            rolled_back_version: candidate_version,
            state,
        })
    }

    pub fn run_candidate_cycle(&self) -> Result<CycleReport, EvolutionError> {
        let state = ForgeState::load(&self.root)?;
        let previous_version = state.current_version.clone();
        let Some(candidate_version) = state.candidate_version.clone() else {
            return Err(EvolutionError::MissingCandidate);
        };

        let runtime = Runtime::new(&self.root);
        runtime.verify_layout_for_version(&previous_version)?;

        match runtime.verify_layout_for_version(&candidate_version) {
            Ok(candidate_validation) => {
                let promotion = self.promote_candidate()?;
                Ok(CycleReport {
                    previous_version,
                    candidate_version,
                    result: CycleResult::Promoted,
                    candidate_validation: Some(candidate_validation),
                    failure: None,
                    state: promotion.state,
                })
            }
            Err(error) => {
                let failure = error.to_string();
                let rollback = self.rollback_candidate(&failure)?;
                Ok(CycleReport {
                    previous_version,
                    candidate_version,
                    result: CycleResult::RolledBack,
                    candidate_validation: None,
                    failure: Some(failure),
                    state: rollback.state,
                })
            }
        }
    }
}

impl fmt::Display for EvolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvolutionError::State(error) => write!(formatter, "{error}"),
            EvolutionError::Forge(error) => write!(formatter, "{error}"),
            EvolutionError::Version(error) => write!(formatter, "{error}"),
            EvolutionError::CandidateAlreadyPrepared { version } => {
                write!(formatter, "candidate version {version} is already prepared")
            }
            EvolutionError::MissingCandidate => {
                write!(formatter, "no candidate version is prepared")
            }
            EvolutionError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
        }
    }
}

impl Error for EvolutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EvolutionError::State(error) => Some(error),
            EvolutionError::Forge(error) => Some(error),
            EvolutionError::Version(error) => Some(error),
            EvolutionError::CandidateAlreadyPrepared { .. } => None,
            EvolutionError::MissingCandidate => None,
            EvolutionError::Io { source, .. } => Some(source),
        }
    }
}

impl From<StateError> for EvolutionError {
    fn from(error: StateError) -> Self {
        EvolutionError::State(error)
    }
}

impl From<ForgeError> for EvolutionError {
    fn from(error: ForgeError) -> Self {
        EvolutionError::Forge(error)
    }
}

impl From<VersionError> for EvolutionError {
    fn from(error: VersionError) -> Self {
        EvolutionError::Version(error)
    }
}

fn write_candidate_documents(
    root: &Path,
    current_version: &str,
    next_version: &str,
    goal: &str,
) -> Result<(), EvolutionError> {
    let timestamp = timestamp();
    append_archive_record(
        root,
        "memory",
        next_version,
        &memory_document(current_version, next_version, goal, &timestamp),
    )?;
    append_archive_record(
        root,
        "tasks",
        next_version,
        &task_document(current_version, next_version, goal),
    )?;
    append_archive_record(root, "errors", next_version, &errors_document(next_version))?;
    append_version_record(root, current_version, next_version)?;
    Ok(())
}

fn append_archive_record(
    root: &Path,
    area: &str,
    next_version: &str,
    entry: &str,
) -> Result<(), EvolutionError> {
    let path = root
        .join("forge")
        .join(area)
        .join(major_file_name(next_version));
    append_record(&path, next_version, entry)
}

fn append_version_record(
    root: &Path,
    current_version: &str,
    next_version: &str,
) -> Result<(), EvolutionError> {
    let path = root
        .join("forge")
        .join("versions")
        .join(major_file_name(next_version));
    let entry = version_document(current_version, next_version);
    append_record(&path, next_version, &entry)
}

fn append_record(path: &Path, version: &str, entry: &str) -> Result<(), EvolutionError> {
    let marker = format!("## {version}");

    if path.exists() {
        let mut contents = fs::read_to_string(path).map_err(|source| EvolutionError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if contents.contains(&marker) {
            return Ok(());
        }
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push('\n');
        contents.push_str(&entry);
        return write_document(&path, &contents);
    }

    write_document(path, entry)
}

fn write_document(path: &Path, contents: &str) -> Result<(), EvolutionError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| EvolutionError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, contents).map_err(|source| EvolutionError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("unix:{}", duration.as_secs()),
        Err(_) => "unix:0".to_string(),
    }
}

fn memory_document(
    current_version: &str,
    next_version: &str,
    goal: &str,
    timestamp: &str,
) -> String {
    format!(
        "## {next_version}\n\n# 版本信息\n- 版本号：{next_version}\n- 时间：{timestamp}\n- 父版本：{current_version}\n\n# 目标\n\n{goal}\n\n# 计划（Plan）\n\n1. 读取最近版本记忆和持久化状态。\n2. 验证当前稳定版本仍可运行。\n3. 生成或复用 major 工作区与聚合归档文件。\n4. 追加候选版本记忆、任务、错误和版本记录。\n5. 验证候选版本布局和中文文档规范。\n6. 将 state/state.json 更新为 candidate_prepared。\n\n# 执行过程\n\n已按 major 聚合规则生成候选记录，未创建新的小版本工作区目录。\n\n# 代码变更\n\n待最终验证后补充。\n\n# 测试结果\n\n待最终验证后补充。\n\n# 错误总结\n\n待最终验证后补充。\n\n# 评估\n\n候选版本生成完成后仍需通过验证与提升流程才能成为当前版本。\n\n# 优化建议\n\n继续减少小版本碎片化文件和目录。\n\n# 可复用经验\n\n小版本记录应追加到 major 聚合文件，避免目录数量随 patch 增长。\n"
    )
}

fn task_document(current_version: &str, next_version: &str, goal: &str) -> String {
    let archive_file = major_file_name(next_version);
    let workspace = version_major_key(next_version).unwrap_or_else(|_| next_version.to_string());

    format!(
        "## {next_version}\n\n# 任务来源\n\nSelfForge 受控进化流程。\n\n# 任务描述\n\n从当前稳定版本 {current_version} 生成下一候选版本 {next_version}，并按 major 聚合规则追加归档。\n\n# 输入\n\n- 当前稳定版本：{current_version}\n- 用户目标：{goal}\n- 状态文件：state/state.json\n\n# 输出\n\n- workspaces/{workspace}\n- forge/memory/{archive_file}\n- forge/tasks/{archive_file}\n- forge/errors/{archive_file}\n- forge/versions/{archive_file}\n- 更新后的 state/state.json\n\n# 计划（Plan）\n\n1. 读取状态。\n2. 验证当前稳定版本。\n3. 计算下一 patch 版本。\n4. 复用 major 工作区并追加聚合文档。\n5. 验证候选版本。\n6. 持久化候选状态。\n"
    )
}

fn errors_document(next_version: &str) -> String {
    format!(
        "## {next_version}\n\n# 错误信息\n\n本候选生成阶段暂无错误。\n\n# 出现阶段\n\n候选生成。\n\n# 原因分析\n\n暂无。\n\n# 解决方案\n\n暂无。\n\n# 是否已解决\n\n是。\n"
    )
}

fn version_document(current_version: &str, next_version: &str) -> String {
    let workspace = version_major_key(next_version).unwrap_or_else(|_| next_version.to_string());

    format!(
        "## {next_version}\n\n# 版本变化\n\n- 从 {current_version} 生成 {next_version} 候选版本。\n- 复用 major 工作区 `workspaces/{workspace}`，不再创建小版本工作区目录。\n- 小版本记忆、任务、错误、版本记录均追加到 major 聚合文件。\n- state/state.json 保持 current_version 为 {current_version}，并记录 candidate_version 为 {next_version}。\n\n# 新增功能\n\n- major 聚合工作区规则。\n- major 聚合 forge 文档规则。\n\n# 修复内容\n\n- 减少小版本目录和文件爆炸。\n"
    )
}
