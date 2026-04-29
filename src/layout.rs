use crate::CURRENT_VERSION;
use crate::documentation::{DocumentationError, validate_chinese_markdown};
use crate::version::{version_major_file_name, version_major_key};
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
    WorkspaceRoot { unexpected_paths: Vec<PathBuf> },
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

    pub fn workspace_name(&self) -> String {
        workspace_name(&self.version)
    }

    pub fn workspace_path(&self) -> PathBuf {
        self.root.join("workspaces").join(self.workspace_name())
    }

    pub fn archive_file_name(&self) -> String {
        major_file_name(&self.version)
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

        self.validate_workspace_root()?;

        validate_chinese_markdown(&self.root)
            .map_err(|source| ForgeError::Documentation { source })?;

        Ok(ValidationReport {
            version: self.version.clone(),
            checked_paths,
        })
    }

    fn required_directories(&self) -> Vec<PathBuf> {
        let mut directories = vec![
            self.root.join("runtime"),
            self.root.join("supervisor"),
            self.root.join("workspaces"),
            self.workspace_path(),
            self.root.join("forge"),
            self.root.join("forge").join("memory"),
            self.root.join("forge").join("tasks"),
            self.root.join("forge").join("errors"),
            self.root.join("forge").join("versions"),
            self.root.join("state"),
        ];
        directories.extend(
            WORKSPACE_ROOT_DIRECTORIES
                .iter()
                .map(|directory| self.workspace_path().join(directory)),
        );
        directories
    }

    fn required_files(&self) -> Vec<PathBuf> {
        let archive_file = self.archive_file_name();
        let mut files = vec![
            self.root.join("README.md"),
            self.root.join("runtime").join("README.md"),
            self.root.join("supervisor").join("README.md"),
            self.workspace_path().join("README.md"),
            self.workspace_path().join(".gitignore"),
            self.root.join("forge").join("memory").join(&archive_file),
            self.root.join("forge").join("tasks").join(&archive_file),
            self.root.join("forge").join("errors").join(&archive_file),
            self.root.join("forge").join("versions").join(&archive_file),
            self.root.join("state").join("state.json"),
        ];
        files.extend(
            WORKSPACE_ROOT_DIRECTORIES
                .iter()
                .map(|directory| self.workspace_path().join(directory).join("README.md")),
        );
        files
    }

    fn seed_files(&self) -> Vec<SeedFile> {
        let archive_file = self.archive_file_name();
        let workspace_name = self.workspace_name();
        let mut files = vec![
            SeedFile {
                path: self.root.join("README.md"),
                contents: root_readme(&self.version, &workspace_name),
            },
            SeedFile {
                path: self.root.join("runtime").join("README.md"),
                contents: RUNTIME_README.to_string(),
            },
            SeedFile {
                path: self.root.join("supervisor").join("README.md"),
                contents: SUPERVISOR_README.to_string(),
            },
            SeedFile {
                path: self.workspace_path().join("README.md"),
                contents: workspace_readme(&workspace_name),
            },
            SeedFile {
                path: self.workspace_path().join(".gitignore"),
                contents: WORKSPACE_GITIGNORE.to_string(),
            },
            SeedFile {
                path: self.root.join("forge").join("memory").join(&archive_file),
                contents: memory_template(&workspace_name),
            },
            SeedFile {
                path: self.root.join("forge").join("tasks").join(&archive_file),
                contents: task_template(&workspace_name),
            },
            SeedFile {
                path: self.root.join("forge").join("errors").join(&archive_file),
                contents: errors_template(&workspace_name),
            },
            SeedFile {
                path: self.root.join("forge").join("versions").join(&archive_file),
                contents: version_template(&workspace_name),
            },
            SeedFile {
                path: self.root.join("state").join("state.json"),
                contents: state_json(&self.version, &workspace_name),
            },
        ];
        files.extend(WORKSPACE_ROOT_DIRECTORIES.iter().map(|directory| SeedFile {
            path: self.workspace_path().join(directory).join("README.md"),
            contents: workspace_area_readme(directory),
        }));
        files
    }

    fn validate_workspace_root(&self) -> Result<(), ForgeError> {
        let workspace_path = self.workspace_path();
        let mut unexpected_paths = Vec::new();

        let entries = fs::read_dir(&workspace_path).map_err(|source| ForgeError::Io {
            path: workspace_path.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| ForgeError::Io {
                path: workspace_path.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_allowed_workspace_root_entry(&name) {
                unexpected_paths.push(entry.path());
            }
        }

        if !unexpected_paths.is_empty() {
            return Err(ForgeError::WorkspaceRoot { unexpected_paths });
        }

        Ok(())
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
            ForgeError::WorkspaceRoot { unexpected_paths } => {
                write!(formatter, "unexpected workspace root entries: ")?;
                for (index, path) in unexpected_paths.iter().enumerate() {
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
            ForgeError::WorkspaceRoot { .. } => None,
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

pub fn workspace_name(version: &str) -> String {
    version_major_key(version).unwrap_or_else(|_| version.to_string())
}

pub fn major_file_name(version: &str) -> String {
    version_major_file_name(version).unwrap_or_else(|_| format!("{version}.md"))
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

const RUNTIME_README: &str =
    "# 运行时边界\n\nSelfForge 运行时负责验证工作区、执行受控命令并记录可审计结果。\n";

const SUPERVISOR_README: &str =
    "# 监督器边界\n\nSelfForge 监督器负责管理候选版本生命周期、验证流程、提升与回滚状态迁移。\n";

const WORKSPACE_ROOT_DIRECTORIES: &[&str] = &["source", "tests", "sandbox", "artifacts", "logs"];
const WORKSPACE_ROOT_FILES: &[&str] = &[
    "README.md",
    ".gitignore",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
];

const WORKSPACE_GITIGNORE: &str = "# SelfForge 工作区忽略规则\n\n/logs/*\n!/logs/README.md\n/sandbox/runs/\n/sandbox/tmp/\n/artifacts/tmp/\n";

fn root_readme(version: &str, workspace_name: &str) -> String {
    format!(
        "# SelfForge\n\nSelfForge 是一个受控自进化软件系统，用于生成、验证、记录、提升和回滚版本。\n\n# 当前状态\n\n- 当前版本：{version}\n- 核心语言：Rust\n- 状态文件：`state/state.json`\n- 归档目录：`forge/`\n- 工作区：`workspaces/{workspace_name}/`\n\n# 常用命令\n\n```txt\ncargo run -- validate\ncargo test\ncargo run -- advance \"目标\"\ncargo run -- cycle\n```\n\n# 约束\n\n所有文档必须使用中文，禁止使用 Emoji。小版本记录追加到 `forge/*/{workspace_name}.md`，工作区复用 `workspaces/{workspace_name}/`。\n"
    )
}

fn workspace_readme(workspace_name: &str) -> String {
    format!(
        "# SelfForge {workspace_name} 工作区\n\n本目录按 major 版本聚合工作区内容。小版本更新不再创建新的工作区目录，只在 forge 聚合文件中追加记录。\n\n# 顶层结构\n\n- `source/`：存放受控生成或待验证的源码。\n- `tests/`：存放工作区内的测试、样例和夹具。\n- `sandbox/`：存放运行时临时执行目录，运行记录按 run id 分层。\n- `artifacts/`：存放可保留的构建产物和验证产物。\n- `logs/`：存放本地原始日志，摘要必须写入 forge。\n\n# 约束\n\n- 根目录只允许固定入口文件和固定一级目录。\n- 禁止将生成文件直接堆放在工作区根目录。\n- 认知类记录必须写入 forge，不允许散落在 workspace。\n"
    )
}

fn workspace_area_readme(area: &str) -> String {
    match area {
        "source" => "# source 源码区\n\n本目录存放受控生成或待验证的源码。新增内容必须继续按模块分层，禁止把大量源码文件直接堆在本目录根部。\n".to_string(),
        "tests" => "# tests 测试区\n\n本目录存放工作区内的测试、样例和夹具。测试应按功能或模块分层，测试结论必须同步记录到 forge 记忆。\n".to_string(),
        "sandbox" => "# sandbox 沙箱区\n\n本目录存放运行时临时执行目录。每次执行应使用独立 run id 分层，临时内容不得作为长期记忆保存。\n".to_string(),
        "artifacts" => "# artifacts 产物区\n\n本目录存放需要保留的构建产物、验证产物或导出物。产物必须按任务或模块分层，禁止直接堆在根部。\n".to_string(),
        "logs" => "# logs 日志区\n\n本目录存放本地原始日志。日志摘要、错误分析和结论必须写入 forge，避免日志成为唯一审计来源。\n".to_string(),
        _ => format!("# {area} 工作区目录\n\n本目录属于 SelfForge 工作区固定骨架，内容必须按职责继续分层。\n"),
    }
}

fn is_allowed_workspace_root_entry(name: &str) -> bool {
    WORKSPACE_ROOT_FILES.contains(&name) || WORKSPACE_ROOT_DIRECTORIES.contains(&name)
}

fn memory_template(major: &str) -> String {
    format!(
        "# {major} 记忆记录\n\n# 记录规则\n\n- 本文件集中记录 {major} 大版本下的小版本记忆。\n- 每次 patch 或 minor 更新只追加一个版本小节，禁止创建新的小版本记忆文件。\n"
    )
}

fn task_template(major: &str) -> String {
    format!(
        "# {major} 任务记录\n\n# 记录规则\n\n- 本文件集中记录 {major} 大版本下的小版本任务。\n- 每次 patch 或 minor 更新只追加一个任务小节，禁止创建新的小版本任务文件。\n"
    )
}

fn errors_template(major: &str) -> String {
    format!(
        "# {major} 错误记录\n\n# 记录规则\n\n- 本文件集中记录 {major} 大版本下的小版本错误。\n- 每个错误仍需包含错误信息、出现阶段、原因分析、解决方案、是否已解决。\n- 小版本更新只追加错误小节，禁止创建新的小版本错误目录。\n"
    )
}

fn version_template(major: &str) -> String {
    format!(
        "# {major} 版本记录\n\n# 记录规则\n\n- 本文件集中记录 {major} 大版本下的小版本变化。\n- 小版本更新只追加到本文件，禁止创建新的小版本版本文件。\n"
    )
}

fn state_json(version: &str, workspace_name: &str) -> String {
    format!(
        "{{\n  \"current_version\": \"{version}\",\n  \"parent_version\": null,\n  \"status\": \"initialized\",\n  \"workspace\": \"workspaces/{workspace_name}\",\n  \"last_verified\": null,\n  \"version_scheme\": \"semantic:vMAJOR.MINOR.PATCH\"\n}}\n"
    )
}
