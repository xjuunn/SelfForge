#![allow(dead_code)]

use crate::CURRENT_VERSION;
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

        if missing_paths.is_empty() {
            Ok(ValidationReport {
                version: self.version.clone(),
                checked_paths,
            })
        } else {
            Err(ForgeError::Validation { missing_paths })
        }
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
                .join(format!("{}.md", self.version)),
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
                contents: memory_template(&self.version, "none"),
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
                    .join(format!("{}.md", self.version)),
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
            ForgeError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
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
        }
    }
}

impl Error for ForgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ForgeError::Io { source, .. } => Some(source),
            ForgeError::Validation { .. } => None,
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

const RUNTIME_README: &str = "# Runtime\n\nSelfForge Runtime is the protected execution boundary for generated work. In v1 it validates the persisted project layout before later versions add sandboxed execution.\n";

const SUPERVISOR_README: &str = "# Supervisor\n\nSelfForge Supervisor is the protected process-control boundary. In v1 it initializes and verifies the current version through the Rust runtime layer.\n";

#[allow(dead_code)]
const WORKSPACE_README: &str = "# SelfForge v1 Workspace\n\nThis directory is the isolated workspace for the first controlled generation.\n";

const MEMORY_TEMPLATE: &str = "# 版本信息\n- 版本号：v1\n- 时间：2026-04-29\n- 父版本：无\n\n# 目标\n\n建立 SelfForge 的最基础架构。\n\n# 计划（Plan）\n\n1. 建立标准目录结构。\n2. 实现 Rust CLI、Runtime 验证层与 Supervisor 编排层。\n3. 写入持久化状态与 forge 文档。\n4. 编写并执行测试。\n5. 记录版本与提交。\n\n# 执行过程\n\n待最终验证后补充。\n\n# 代码变更\n\n待最终验证后补充。\n\n# 测试结果\n\n待最终验证后补充。\n\n# 错误总结\n\n待最终验证后补充。\n\n# 评估\n\n待最终验证后补充。\n\n# 优化建议\n\n待最终验证后补充。\n\n# 可复用经验\n\n待最终验证后补充。\n";

const TASK_TEMPLATE: &str = "# 任务来源\n\n用户指定第一个任务：完成 SelfForge 系统的最基础架构。\n\n# 任务描述\n\n建立最小可运行、可验证、可审计的 SelfForge v1 基础骨架。\n\n# 输入\n\n- 顶层 SelfForge 系统提示词与项目约束\n- 当前 Rust crate\n\n# 输出\n\n- 标准目录结构\n- 持久化状态\n- forge 文档归档\n- Rust CLI 与测试\n";

const ERRORS_README: &str = "# v1 错误记录\n\n当前版本没有已确认的未解决错误。若出现错误，必须在本目录新增 error-XXX.md，并包含错误信息、出现阶段、原因分析、解决方案、是否已解决。\n";

const VERSION_TEMPLATE: &str = "# v1\n\n# 版本变化\n\n- 建立 SelfForge v1 基础架构。\n\n# 新增功能\n\n- Rust CLI 初始化、验证与状态查看。\n- Runtime 验证层。\n- Supervisor 编排层。\n- forge 文档归档入口。\n\n# 修复内容\n\n- 无。\n";

const STATE_JSON: &str = "{\n  \"current_version\": \"v1\",\n  \"parent_version\": null,\n  \"status\": \"initialized\",\n  \"workspace\": \"workspaces/v1\",\n  \"last_verified\": null\n}\n";

fn workspace_readme(version: &str) -> String {
    format!(
        "# SelfForge {version} Workspace\n\nThis directory is the isolated workspace for controlled generation {version}.\n"
    )
}

fn memory_template(version: &str, parent_version: &str) -> String {
    format!(
        "# 版本信息\n- 版本号：{version}\n- 时间：待验证后补充\n- 父版本：{parent_version}\n\n# 目标\n\n待计划生成后补充。\n\n# 计划（Plan）\n\n1. 读取历史记忆。\n2. 确定目标。\n3. 生成候选版本。\n4. 执行测试与验证。\n5. 记录结果。\n\n# 执行过程\n\n待验证后补充。\n\n# 代码变更\n\n待验证后补充。\n\n# 测试结果\n\n待验证后补充。\n\n# 错误总结\n\n待验证后补充。\n\n# 评估\n\n待验证后补充。\n\n# 优化建议\n\n待验证后补充。\n\n# 可复用经验\n\n待验证后补充。\n"
    )
}

fn task_template(version: &str) -> String {
    format!(
        "# 任务来源\n\nSelfForge 受控进化流程。\n\n# 任务描述\n\n生成并验证 {version} 的最小候选版本归档。\n\n# 输入\n\n- 当前状态文件\n- 最近版本记忆\n\n# 输出\n\n- {version} workspace\n- {version} forge 文档\n- 更新后的持久化状态\n\n# 计划（Plan）\n\n1. 验证当前稳定版本。\n2. 生成候选版本目录和文档。\n3. 持久化候选版本状态。\n4. 执行测试与验证。\n"
    )
}

fn errors_readme(version: &str) -> String {
    format!(
        "# {version} 错误记录\n\n若出现错误，必须在本目录新增 error-XXX.md，并包含错误信息、出现阶段、原因分析、解决方案、是否已解决。\n"
    )
}

fn version_template(version: &str) -> String {
    format!(
        "# {version}\n\n# 版本变化\n\n- 初始化 {version} 候选版本归档。\n\n# 新增功能\n\n- 待验证后补充。\n\n# 修复内容\n\n- 待验证后补充。\n"
    )
}

fn state_json(version: &str) -> String {
    format!(
        "{{\n  \"current_version\": \"{version}\",\n  \"parent_version\": null,\n  \"status\": \"initialized\",\n  \"workspace\": \"workspaces/{version}\",\n  \"last_verified\": null\n}}\n"
    )
}
