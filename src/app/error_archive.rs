use crate::{
    ExecutionError, RunIndexEntry, RunQuery, Supervisor, VersionError, version_major_file_name,
    version_major_key,
};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const RUN_QUERY_LIMIT: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorArchiveReport {
    pub version: String,
    pub run_id: String,
    pub archive_path: PathBuf,
    pub appended: bool,
}

#[derive(Debug)]
pub enum ErrorArchiveError {
    Execution(ExecutionError),
    Version(VersionError),
    NoFailedRun { version: String },
    RunNotFound { version: String, run_id: String },
    RunNotFailed { version: String, run_id: String },
    Io { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone)]
pub struct ErrorArchive {
    root: PathBuf,
    supervisor: Supervisor,
}

impl ErrorArchive {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            supervisor: Supervisor::new(&root),
            root,
        }
    }

    pub fn record_failed_run(
        &self,
        version: impl AsRef<str>,
        run_id: Option<&str>,
        stage: &str,
        solution: &str,
    ) -> Result<ErrorArchiveReport, ErrorArchiveError> {
        let version = version.as_ref().to_string();
        let entry = match run_id {
            Some(run_id) => self.find_run(&version, run_id)?,
            None => self.latest_failed_run(&version)?,
        };

        if !entry.is_failed() {
            return Err(ErrorArchiveError::RunNotFailed {
                version,
                run_id: entry.run_id,
            });
        }

        let archive_path = self
            .root
            .join("forge")
            .join("errors")
            .join(version_major_file_name(&version)?);
        let mut contents =
            fs::read_to_string(&archive_path).map_err(|source| ErrorArchiveError::Io {
                path: archive_path.clone(),
                source,
            })?;

        if contents.contains(&format!("运行编号：{}", entry.run_id)) {
            return Ok(ErrorArchiveReport {
                version,
                run_id: entry.run_id,
                archive_path,
                appended: false,
            });
        }

        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        if !contents.ends_with("\n\n") {
            contents.push('\n');
        }

        contents.push_str(&error_section(
            &version,
            &version_major_key(&version)?,
            &entry,
            stage,
            solution,
        ));
        fs::write(&archive_path, contents).map_err(|source| ErrorArchiveError::Io {
            path: archive_path.clone(),
            source,
        })?;

        Ok(ErrorArchiveReport {
            version,
            run_id: entry.run_id,
            archive_path,
            appended: true,
        })
    }

    fn latest_failed_run(&self, version: &str) -> Result<RunIndexEntry, ErrorArchiveError> {
        let mut entries = self
            .supervisor
            .query_runs(version, RunQuery::failed(1))
            .map_err(ErrorArchiveError::Execution)?;
        entries.pop().ok_or_else(|| ErrorArchiveError::NoFailedRun {
            version: version.to_string(),
        })
    }

    fn find_run(&self, version: &str, run_id: &str) -> Result<RunIndexEntry, ErrorArchiveError> {
        let entries = self
            .supervisor
            .query_runs(version, RunQuery::recent(RUN_QUERY_LIMIT))
            .map_err(ErrorArchiveError::Execution)?;
        entries
            .into_iter()
            .find(|entry| entry.run_id == run_id)
            .ok_or_else(|| ErrorArchiveError::RunNotFound {
                version: version.to_string(),
                run_id: run_id.to_string(),
            })
    }
}

impl fmt::Display for ErrorArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorArchiveError::Execution(error) => write!(formatter, "{error}"),
            ErrorArchiveError::Version(error) => write!(formatter, "{error}"),
            ErrorArchiveError::NoFailedRun { version } => {
                write!(formatter, "版本 {version} 没有可归档的失败运行记录")
            }
            ErrorArchiveError::RunNotFound { version, run_id } => {
                write!(formatter, "版本 {version} 未找到运行记录 {run_id}")
            }
            ErrorArchiveError::RunNotFailed { version, run_id } => {
                write!(formatter, "版本 {version} 的运行记录 {run_id} 不是失败记录")
            }
            ErrorArchiveError::Io { path, source } => {
                write!(formatter, "{}: {}", path.display(), source)
            }
        }
    }
}

impl Error for ErrorArchiveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ErrorArchiveError::Execution(error) => Some(error),
            ErrorArchiveError::Version(error) => Some(error),
            ErrorArchiveError::NoFailedRun { .. } => None,
            ErrorArchiveError::RunNotFound { .. } => None,
            ErrorArchiveError::RunNotFailed { .. } => None,
            ErrorArchiveError::Io { source, .. } => Some(source),
        }
    }
}

impl From<VersionError> for ErrorArchiveError {
    fn from(error: VersionError) -> Self {
        ErrorArchiveError::Version(error)
    }
}

fn error_section(
    version: &str,
    workspace: &str,
    entry: &RunIndexEntry,
    stage: &str,
    solution: &str,
) -> String {
    let stage = default_when_blank(stage, "Runtime 受控执行");
    let solution = default_when_blank(solution, "待分析并修复后重新运行验证。");
    let report_path = format!(
        "workspaces/{workspace}/sandbox/runs/{}",
        entry.report_file.replace('\\', "/")
    );
    let args = if entry.args.is_empty() {
        "无".to_string()
    } else {
        entry.args.join(" ")
    };

    format!(
        "## {version} 运行错误 {run_id}\n\n# 错误信息\n\n- 运行编号：{run_id}\n- 版本：{version}\n- 程序：`{program}`\n- 参数：`{args}`\n- 退出码：{exit_code:?}\n- 是否超时：{timed_out}\n- 标准输出字节：{stdout_bytes}\n- 标准错误字节：{stderr_bytes}\n- 报告文件：`{report_path}`\n\n# 出现阶段\n\n{stage}\n\n# 原因分析\n\n运行索引显示该命令出现非零退出或超时。详细原因需要结合运行报告、标准输出和标准错误继续分析。\n\n# 解决方案\n\n{solution}\n\n# 是否已解决\n\n否。该记录为失败运行归档草稿，修复并重新验证后需要更新结论。\n",
        run_id = entry.run_id,
        program = entry.program,
        exit_code = entry.exit_code,
        timed_out = entry.timed_out,
        stdout_bytes = entry.stdout_bytes,
        stderr_bytes = entry.stderr_bytes
    )
}

fn default_when_blank<'a>(value: &'a str, default: &'a str) -> &'a str {
    let value = value.trim();
    if value.is_empty() { default } else { value }
}
