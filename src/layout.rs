use crate::CURRENT_VERSION;
use crate::documentation::{DocumentationError, validate_chinese_markdown};
use crate::version::version_series_file_name;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SelfForge {
    root: PathBuf,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapReport {
    pub version: String,
    pub created_paths: Vec<PathBuf>,
    pub existing_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub version: String,
    pub checked_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum ForgeError {
    Io { path: PathBuf, source: io::Error },
    Validation { missing_paths: Vec<PathBuf> },
    Documentation { source: DocumentationError },
}

impl SelfForge {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self::for_version(root, CURRENT_VERSION)
    }

    pub fn for_version(root: impl AsRef<Path>, version: impl Into<String>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            version: version.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn bootstrap(&self) -> Result<BootstrapReport, ForgeError> {
        let mut created_paths = Vec::new();
        let mut existing_paths = Vec::new();

        for directory in self.required_directories() {
            ensure_directory(&directory, &mut created_paths, &mut existing_paths)?;
        }

        for file in self.seed_files() {
            ensure_file(
                &file.path,
                &file.contents,
                &mut created_paths,
                &mut existing_paths,
            )?;
        }

        Ok(BootstrapReport {
            version: self.version.clone(),
            created_paths,
            existing_paths,
        })
    }

    pub fn validate(&self) -> Result<ValidationReport, ForgeError> {
        let mut checked_paths = Vec::new();
        let mut missing_paths = Vec::new();

        for path in self
            .required_directories()
            .into_iter()
            .chain(self.required_files())
        {
            if path.exists() {
                checked_paths.push(path);
            } else {
                missing_paths.push(path);
            }
        }

        if !missing_paths.is_empty() {
            return Err(ForgeError::Validation { missing_paths });
        }

        validate_chinese_markdown(&self.root)
            .map_err(|source| ForgeError::Documentation { source })?;

        Ok(ValidationReport {
            version: self.version.clone(),
            checked_paths,
        })
    }

    fn required_directories(&self) -> Vec<PathBuf> {
        vec![
            self.root.join("runtime"),
            self.root.join("supervisor"),
            self.root.join("workspaces"),
            self.root.join("workspaces").join(&self.version),
            self.root.join("forge"),
            self.root.join("forge").join("memory"),
            self.root.join("forge").join("tasks"),
            self.root.join("forge").join("errors"),
            self.root.join("forge").join("errors").join(&self.version),
            self.root.join("forge").join("versions"),
            self.root.join("state"),
        ]
    }

    fn required_files(&self) -> Vec<PathBuf> {
        vec![
            self.root.join("runtime").join("README.md"),
            self.root.join("supervisor").join("README.md"),
            self.root
                .join("workspaces")
                .join(&self.version)
                .join("README.md"),
            self.root
                .join("forge")
                .join("memory")
                .join(format!("{}.md", self.version)),
            self.root
                .join("forge")
                .join("tasks")
                .join(format!("{}.md", self.version)),
            self.root
                .join("forge")
                .join("errors")
                .join(&self.version)
                .join("README.md"),
            self.root
                .join("forge")
                .join("versions")
                .join(version_record_file_name(&self.version)),
            self.root.join("state").join("state.json"),
        ]
    }

    fn seed_files(&self) -> Vec<SeedFile> {
        vec![
            SeedFile {
                path: self.root.join("runtime").join("README.md"),
                contents: RUNTIME_README.to_string(),
            },
            SeedFile {
                path: self.root.join("supervisor").join("README.md"),
                contents: SUPERVISOR_README.to_string(),
            },
            SeedFile {
                path: self
                    .root
                    .join("workspaces")
                    .join(&self.version)
                    .join("README.md"),
                contents: workspace_readme(&self.version),
            },
            SeedFile {
                path: self
                    .root
                    .join("forge")
                    .join("memory")
                    .join(format!("{}.md", self.version)),
                contents: memory_template(&self.version, "无"),
            },
            SeedFile {
                path: self
                    .root
                    .join("forge")
                    .join("tasks")
                    .join(format!("{}.md", self.version)),
                contents: task_template(&self.version),
            },
            SeedFile {
                path: self
                    .root
                    .join("forge")
                    .join("errors")
                    .join(&self.version)
                    .join("README.md"),
                contents: errors_readme(&self.version),
            },
            SeedFile {
                path: self
                    .root
                    .join("forge")
                    .join("versions")
                    .join(version_record_file_name(&self.version)),
                contents: version_template(&self.version),
            },
            SeedFile {
                path: self.root.join("state").join("state.json"),
                contents: state_json(&self.version),
            },
        ]
    }
}

impl fmt::Display for ForgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeError::Io { path, source } => write!(formatter, "{}: {}", path.display(), source),
            ForgeError::Validation { missing_paths } => {
                write!(formatter, "missing required paths: ")?;
                for (index, path) in missing_paths.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    write!(formatter, "{}", relative_display(path))?;
                }
                Ok(())
            }
            ForgeError::Documentation { source } => write!(formatter, "{source}"),
        }
    }
}

impl Error for ForgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ForgeError::Io { source, .. } => Some(source),
            ForgeError::Validation { .. } => None,
            ForgeError::Documentation { source } => Some(source),
        }
    }
}

struct SeedFile {
    path: PathBuf,
    contents: String,
}

fn ensure_directory(
    path: &Path,
    created_paths: &mut Vec<PathBuf>,
    existing_paths: &mut Vec<PathBuf>,
) -> Result<(), ForgeError> {
    if path.exists() {
        existing_paths.push(path.to_path_buf());
        return Ok(());
    }

    fs::create_dir_all(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    created_paths.push(path.to_path_buf());
    Ok(())
}

fn ensure_file(
    path: &Path,
    contents: &str,
    created_paths: &mut Vec<PathBuf>,
    existing_paths: &mut Vec<PathBuf>,
) -> Result<(), ForgeError> {
    if path.exists() {
        existing_paths.push(path.to_path_buf());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ForgeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, contents).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    created_paths.push(path.to_path_buf());
    Ok(())
}

fn relative_display(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let start = parts
        .iter()
        .position(|part| {
            matches!(
                part.as_str(),
                "runtime" | "supervisor" | "workspaces" | "forge" | "state"
            )
        })
        .unwrap_or(0);
    parts[start..].join("/")
}

fn version_record_file_name(version: &str) -> String {
    version_series_file_name(version).unwrap_or_else(|_| format!("{version}.md"))
}

const RUNTIME_README: &str = "# 运行时边界\n\nSelfForge 运行时是受保护的执行边界，负责验证工作区、文档归档和后续沙箱执行结果。\n";

const SUPERVISOR_README: &str =
    "# 监督器边界\n\nSelfForge 监督器负责管理候选版本生命周期、验证流程、提升与回滚状态迁移。\n";

fn workspace_readme(version: &str) -> String {
    format!(
        "# SelfForge {version} 工作区\n\n该目录是 {version} 的隔离工作区，只允许放置本版本受控生成与验证所需的文件。\n"
    )
}

fn memory_template(version: &str, parent_version: &str) -> String {
    format!(
        "# 版本信息\n- 版本号：{version}\n- 时间：待验证后补充\n- 父版本：{parent_version}\n\n# 目标\n\n待计划生成后补充。\n\n# 计划（Plan）\n\n1. 读取历史记忆。\n2. 确定目标。\n3. 生成候选版本。\n4. 执行测试与验证。\n5. 记录结果。\n\n# 执行过程\n\n待验证后补充。\n\n# 代码变更\n\n待验证后补充。\n\n# 测试结果\n\n待验证后补充。\n\n# 错误总结\n\n待验证后补充。\n\n# 评估\n\n待验证后补充。\n\n# 优化建议\n\n待验证后补充。\n\n# 可复用经验\n\n待验证后补充。\n"
    )
}

fn task_template(version: &str) -> String {
    format!(
        "# 任务来源\n\nSelfForge 受控进化流程。\n\n# 任务描述\n\n生成并验证 {version} 的最小候选版本归档。\n\n# 输入\n\n- 当前状态文件\n- 最近版本记忆\n\n# 输出\n\n- {version} 工作区\n- {version} forge 文档\n- 更新后的持久化状态\n\n# 计划（Plan）\n\n1. 验证当前稳定版本。\n2. 生成候选版本目录和文档。\n3. 持久化候选版本状态。\n4. 执行测试与验证。\n"
    )
}

fn errors_readme(version: &str) -> String {
    format!(
        "# {version} 错误记录\n\n若出现错误，必须在本目录新增 error-XXX.md，并包含错误信息、出现阶段、原因分析、解决方案、是否已解决。\n"
    )
}

fn version_template(version: &str) -> String {
    let series = version_series_file_name(version)
        .ok()
        .and_then(|file_name| file_name.strip_suffix(".md").map(ToOwned::to_owned))
        .unwrap_or_else(|| version.to_string());

    format!(
        "# {series} 版本记录\n\n# 记录规则\n\n- 本文件集中记录 {series}.x 的 patch 更新，避免为每次小版本生成独立版本文件。\n- minor 或 major 版本变化时，才创建新的版本系列文件。\n\n## {version}\n\n# 版本变化\n\n- 初始化 {version} 候选版本归档。\n\n# 新增功能\n\n- 待验证后补充。\n\n# 修复内容\n\n- 待验证后补充。\n"
    )
}

fn state_json(version: &str) -> String {
    format!(
        "{{\n  \"current_version\": \"{version}\",\n  \"parent_version\": null,\n  \"status\": \"initialized\",\n  \"workspace\": \"workspaces/{version}\",\n  \"last_verified\": null,\n  \"version_scheme\": \"semantic:vMAJOR.MINOR.PATCH\"\n}}\n"
    )
}
