use crate::layout::{ForgeError, SelfForge, ValidationReport, workspace_name};
use serde_json::json;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Runtime {
    forge: SelfForge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReport {
    pub version: String,
    pub workspace: PathBuf,
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub run_dir: PathBuf,
}

#[derive(Debug)]
pub enum ExecutionError {
    Forge(ForgeError),
    EmptyProgram,
    WorkspacePath {
        workspace: PathBuf,
        workspaces_root: PathBuf,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    Spawn {
        program: String,
        source: io::Error,
    },
    Wait {
        program: String,
        source: io::Error,
    },
}

impl Runtime {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            forge: SelfForge::new(root),
        }
    }

    pub fn verify_layout(&self) -> Result<ValidationReport, ForgeError> {
        self.forge.validate()
    }

    pub fn verify_layout_for_version(
        &self,
        version: impl AsRef<str>,
    ) -> Result<ValidationReport, ForgeError> {
        SelfForge::for_version(self.forge.root(), version.as_ref()).validate()
    }

    pub fn execute_in_workspace(
        &self,
        version: impl AsRef<str>,
        program: impl AsRef<str>,
        args: &[String],
        timeout_ms: u64,
    ) -> Result<ExecutionReport, ExecutionError> {
        let version = version.as_ref().to_string();
        let program = program.as_ref().trim().to_string();
        if program.is_empty() {
            return Err(ExecutionError::EmptyProgram);
        }

        let workspace = self.canonical_workspace(&version)?;
        self.verify_layout_for_version(&version)
            .map_err(ExecutionError::Forge)?;

        let timeout = Duration::from_millis(timeout_ms);
        let mut child = Command::new(&program)
            .args(args)
            .current_dir(&workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ExecutionError::Spawn {
                program: program.clone(),
                source,
            })?;

        let started = Instant::now();
        loop {
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .map_err(|source| ExecutionError::Wait {
                        program: program.clone(),
                        source,
                    })?;
                let report = execution_report(version, workspace, program, args, output, true);
                return self.persist_execution_report(report);
            }

            match child.try_wait().map_err(|source| ExecutionError::Wait {
                program: program.clone(),
                source,
            })? {
                Some(_) => {
                    let output =
                        child
                            .wait_with_output()
                            .map_err(|source| ExecutionError::Wait {
                                program: program.clone(),
                                source,
                            })?;
                    let report = execution_report(version, workspace, program, args, output, false);
                    return self.persist_execution_report(report);
                }
                None => thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn persist_execution_report(
        &self,
        mut report: ExecutionReport,
    ) -> Result<ExecutionReport, ExecutionError> {
        let run_dir = next_run_dir(&report.workspace)?;
        fs::create_dir_all(&run_dir).map_err(|source| ExecutionError::Io {
            path: run_dir.clone(),
            source,
        })?;

        fs::write(run_dir.join("stdout.txt"), &report.stdout).map_err(|source| {
            ExecutionError::Io {
                path: run_dir.join("stdout.txt"),
                source,
            }
        })?;
        fs::write(run_dir.join("stderr.txt"), &report.stderr).map_err(|source| {
            ExecutionError::Io {
                path: run_dir.join("stderr.txt"),
                source,
            }
        })?;

        report.run_dir = run_dir;
        let contents = serde_json::to_string_pretty(&json!({
            "version": &report.version,
            "workspace": &report.workspace,
            "program": &report.program,
            "args": &report.args,
            "exit_code": report.exit_code,
            "timed_out": report.timed_out,
            "stdout_file": "stdout.txt",
            "stderr_file": "stderr.txt"
        }))
        .map_err(|source| ExecutionError::Serialize {
            path: report.run_dir.join("report.json"),
            source,
        })? + "\n";

        fs::write(report.run_dir.join("report.json"), contents).map_err(|source| {
            ExecutionError::Io {
                path: report.run_dir.join("report.json"),
                source,
            }
        })?;

        Ok(report)
    }

    fn canonical_workspace(&self, version: &str) -> Result<PathBuf, ExecutionError> {
        let workspaces_root = self
            .forge
            .root()
            .join("workspaces")
            .canonicalize()
            .map_err(|source| ExecutionError::Io {
                path: self.forge.root().join("workspaces"),
                source,
            })?;
        let workspace = workspaces_root.join(workspace_name(version));
        let canonical_workspace =
            workspace
                .canonicalize()
                .map_err(|source| ExecutionError::Io {
                    path: workspace.clone(),
                    source,
                })?;

        if !canonical_workspace.starts_with(&workspaces_root) {
            return Err(ExecutionError::WorkspacePath {
                workspace: canonical_workspace,
                workspaces_root,
            });
        }

        Ok(canonical_workspace)
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::Forge(error) => write!(formatter, "{error}"),
            ExecutionError::EmptyProgram => write!(formatter, "执行命令不能为空"),
            ExecutionError::WorkspacePath {
                workspace,
                workspaces_root,
            } => write!(
                formatter,
                "工作区 {} 不在允许的根目录 {} 内",
                workspace.display(),
                workspaces_root.display()
            ),
            ExecutionError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
            ExecutionError::Serialize { path, source } => {
                write!(
                    formatter,
                    "序列化执行记录 {} 失败: {}",
                    path.display(),
                    source
                )
            }
            ExecutionError::Spawn { program, source } => {
                write!(formatter, "启动命令 {program} 失败: {source}")
            }
            ExecutionError::Wait { program, source } => {
                write!(formatter, "等待命令 {program} 失败: {source}")
            }
        }
    }
}

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ExecutionError::Forge(error) => Some(error),
            ExecutionError::EmptyProgram => None,
            ExecutionError::WorkspacePath { .. } => None,
            ExecutionError::Io { source, .. } => Some(source),
            ExecutionError::Serialize { source, .. } => Some(source),
            ExecutionError::Spawn { source, .. } => Some(source),
            ExecutionError::Wait { source, .. } => Some(source),
        }
    }
}

fn execution_report(
    version: String,
    workspace: PathBuf,
    program: String,
    args: &[String],
    output: std::process::Output,
    timed_out: bool,
) -> ExecutionReport {
    ExecutionReport {
        version,
        workspace,
        program,
        args: args.to_vec(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        timed_out,
        run_dir: PathBuf::new(),
    }
}

fn next_run_dir(workspace: &Path) -> Result<PathBuf, ExecutionError> {
    let runs_root = workspace.join("sandbox").join("runs");
    fs::create_dir_all(&runs_root).map_err(|source| ExecutionError::Io {
        path: runs_root.clone(),
        source,
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    for index in 0..1000 {
        let candidate = runs_root.join(format!("run-{timestamp}-{index:03}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Ok(runs_root.join(format!("run-{timestamp}-fallback")))
}
