use crate::layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport};
use crate::runtime::Runtime;
use crate::state::{ForgeState, StateError};
use crate::version::{
    VersionBump, VersionError, next_version_after_with_bump, version_series_file_name,
};
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

        let runtime = Runtime::new(&self.root);
        runtime.verify_layout_for_version(&current_version)?;

        let BootstrapReport {
            created_paths,
            existing_paths,
            ..
        } = SelfForge::for_version(&self.root, &next_version).bootstrap()?;

        write_candidate_documents(&self.root, &current_version, &next_version, goal)?;

        let candidate_validation = runtime.verify_layout_for_version(&next_version)?;
        let candidate_workspace = format!("workspaces/{next_version}");

        state.status = "candidate_prepared".to_string();
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
    write_document_if_missing(
        &root
            .join("forge")
            .join("memory")
            .join(format!("{next_version}.md")),
        &memory_document(current_version, next_version, goal, &timestamp),
    )?;
    write_document_if_missing(
        &root
            .join("forge")
            .join("tasks")
            .join(format!("{next_version}.md")),
        &task_document(current_version, next_version, goal),
    )?;
    write_document_if_missing(
        &root
            .join("forge")
            .join("errors")
            .join(next_version)
            .join("README.md"),
        &errors_readme(next_version),
    )?;
    append_version_record(root, current_version, next_version)?;
    Ok(())
}

fn write_document_if_missing(path: &Path, contents: &str) -> Result<(), EvolutionError> {
    if path.exists() {
        return Ok(());
    }

    write_document(path, contents)
}

fn append_version_record(
    root: &Path,
    current_version: &str,
    next_version: &str,
) -> Result<(), EvolutionError> {
    let series_file_name = version_series_file_name(next_version)?;
    let series = series_file_name
        .strip_suffix(".md")
        .unwrap_or(&series_file_name)
        .to_string();
    let path = root.join("forge").join("versions").join(&series_file_name);
    let entry = version_document(current_version, next_version);
    let marker = format!("## {next_version}");

    if path.exists() {
        let mut contents = fs::read_to_string(&path).map_err(|source| EvolutionError::Io {
            path: path.clone(),
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

    write_document(&path, &version_series_document(&series, &entry))
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
    let document = format!(
        "# 版本信息\n- 版本号：{next_version}\n- 时间：{timestamp}\n- 父版本：{current_version}\n\n# 目标\n\n{goal}\n\n# 计划（Plan）\n\n1. 读取最近版本记忆和持久化状态。\n2. 验证当前稳定版本仍可运行。\n3. 按语义化版本规则计算下一候选版本。\n4. 生成候选工作区与 forge 归档文档。\n5. 验证候选版本布局和中文文档规范。\n6. 将 state/state.json 更新为 candidate_prepared。\n7. 保持当前稳定版本不变，等待提升或回滚。\n\n# 执行过程\n\n已生成候选版本骨架，尚未执行版本提升。\n\n# 代码变更\n\n待最终验证后补充。\n\n# 测试结果\n\n待最终验证后补充。\n\n# 错误总结\n\n待最终验证后补充。\n\n# 评估\n\n候选版本生成完成后仍需通过验证与提升流程才能成为当前版本。\n\n# 优化建议\n\n继续完善沙箱执行、资源限制和并行验证。\n\n# 可复用经验\n\n候选版本文档已存在时必须保留，不能覆盖前序计划。\n"
    );
    if !document.is_empty() {
        return document;
    }

    format!(
        "# 版本信息\n- 版本号：{next_version}\n- 时间：{timestamp}\n- 父版本：{current_version}\n\n# 目标\n\n{goal}\n\n# 计划（Plan）\n\n1. 读取最近版本记忆和持久化状态。\n2. 验证当前稳定版本仍可运行。\n3. 按语义化版本规则计算下一候选版本。\n4. 生成候选工作区与 forge 归档文档。\n5. 验证候选版本布局和中文文档规范。\n6. 将 state/state.json 更新为 candidate_prepared。\n7. 保持当前稳定版本不变，等待提升或回滚。\n\n# 执行过程\n\n已生成候选版本骨架，尚未执行版本提升。\n\n# 代码变更\n\n待最终验证后补充。\n\n# 测试结果\n\n待最终验证后补充。\n\n# 错误总结\n\n待最终验证后补充。\n\n# 评估\n\n候选版本生成完成后仍需通过验证与提升流程才能成为当前版本。\n\n# 优化建议\n\n继续完善沙箱执行、资源限制和并行验证。\n\n# 可复用经验\n\n候选版本文档已存在时必须保留，不能覆盖前序计划。\n"
    )
}

fn task_document(current_version: &str, next_version: &str, goal: &str) -> String {
    let version_file =
        version_series_file_name(next_version).unwrap_or_else(|_| format!("{next_version}.md"));

    format!(
        "# 任务来源\n\nSelfForge 受控进化流程。\n\n# 任务描述\n\n从当前稳定版本 {current_version} 生成下一候选版本 {next_version}，默认只递增 patch。\n\n# 输入\n\n- 当前稳定版本：{current_version}\n- 用户目标：{goal}\n- 状态文件：state/state.json\n\n# 输出\n\n- workspaces/{next_version}\n- forge/memory/{next_version}.md\n- forge/errors/{next_version}\n- forge/versions/{version_file}\n- 更新后的 state/state.json\n\n# 计划（Plan）\n\n1. 读取状态。\n2. 验证当前稳定版本。\n3. 计算下一 patch 版本。\n4. 生成候选版本目录和文档。\n5. 验证候选版本。\n6. 持久化候选状态。\n"
    )
}

fn errors_readme(next_version: &str) -> String {
    format!(
        "# {next_version} 错误记录\n\n每个错误必须独立记录为 error-XXX.md，并包含错误信息、出现阶段、原因分析、解决方案、是否已解决。\n"
    )
}

fn version_series_document(series: &str, first_entry: &str) -> String {
    format!(
        "# {series} 版本记录\n\n# 记录规则\n\n- 本文件集中记录 {series}.x 的 patch 更新，避免为每次小版本生成独立版本文件。\n- minor 或 major 版本变化时，才创建新的版本系列文件。\n\n{first_entry}"
    )
}

fn version_document(current_version: &str, next_version: &str) -> String {
    format!(
        "## {next_version}\n\n# 版本变化\n\n- 从 {current_version} 生成 {next_version} 候选版本。\n- 新增候选工作区与 forge 归档文档。\n- state/state.json 保持 current_version 为 {current_version}，并记录 candidate_version 为 {next_version}。\n\n# 新增功能\n\n- 候选版本生成。\n- 中文文档规范验证。\n\n# 修复内容\n\n- 暂无。\n"
    )
}
