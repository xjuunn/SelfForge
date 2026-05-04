use super::{
    SelfEvolutionLoopError, SelfEvolutionLoopRecord, SelfEvolutionLoopStore, current_unix_seconds,
    truncate_chars,
};
use crate::state::ForgeState;
use crate::{AgentWorkFinalizeCheckError, SelfForgeApp};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_REMOTE: &str = "origin";
const DEFAULT_BASE_BRANCH: &str = "master";
const DEFAULT_ISSUE_REF: &str = "Refs #1";
const DEFAULT_ISSUE_URL: &str = "https://github.com/xjuunn/SelfForge/issues/1";
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_CHECK_TIMEOUT_MS: u64 = 900_000;
const DEFAULT_CHECK_INTERVAL_SECONDS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SelfEvolutionLoopGitPrMode {
    #[default]
    Disabled,
    LocalCommit,
    PullRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfEvolutionLoopGitPrRequest {
    pub mode: SelfEvolutionLoopGitPrMode,
    pub branch_name: Option<String>,
    pub worker_id: Option<String>,
    pub task_id: Option<String>,
    pub remote: String,
    pub base_branch: String,
    pub issue_ref: String,
    pub issue_url: String,
    pub wait_checks: bool,
    pub merge: bool,
    pub delete_remote_branch: bool,
    pub confirmed: bool,
    pub command_timeout_ms: u64,
    pub check_timeout_ms: u64,
    pub check_interval_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelfEvolutionLoopGitPrEventStatus {
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfEvolutionLoopGitPrEvent {
    pub order: usize,
    pub action: String,
    pub status: SelfEvolutionLoopGitPrEventStatus,
    pub started_at_unix_seconds: u64,
    pub completed_at_unix_seconds: Option<u64>,
    pub command: Option<Vec<String>>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_preview: Option<String>,
    pub stderr_preview: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug)]
pub enum SelfEvolutionLoopGitPrError {
    Record(String),
    InvalidState(String),
    Finalize(AgentWorkFinalizeCheckError),
    FinalizeBlocked(Vec<String>),
    CommandSpawn {
        action: String,
        program: String,
        source: io::Error,
    },
    CommandFailed {
        action: String,
        program: String,
        args: Vec<String>,
        exit_code: Option<i32>,
        timed_out: bool,
        stderr: String,
    },
}

#[derive(Debug, Clone)]
struct CommandOutcome {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timed_out: bool,
}

impl Default for SelfEvolutionLoopGitPrRequest {
    fn default() -> Self {
        Self {
            mode: SelfEvolutionLoopGitPrMode::Disabled,
            branch_name: None,
            worker_id: Some("ai-1".to_string()),
            task_id: None,
            remote: DEFAULT_REMOTE.to_string(),
            base_branch: DEFAULT_BASE_BRANCH.to_string(),
            issue_ref: DEFAULT_ISSUE_REF.to_string(),
            issue_url: DEFAULT_ISSUE_URL.to_string(),
            wait_checks: true,
            merge: true,
            delete_remote_branch: true,
            confirmed: false,
            command_timeout_ms: DEFAULT_COMMAND_TIMEOUT_MS,
            check_timeout_ms: DEFAULT_CHECK_TIMEOUT_MS,
            check_interval_seconds: DEFAULT_CHECK_INTERVAL_SECONDS,
        }
    }
}

pub(super) fn prepare_git_pr_flow(
    root: &Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    if record.git_pr.mode == SelfEvolutionLoopGitPrMode::Disabled {
        return Ok(());
    }

    let current_branch = git_text(root, &["branch", "--show-current"])?;
    let current_branch = current_branch.trim();
    let current_branch = if current_branch.is_empty() {
        git_text(root, &["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_string()
    } else {
        current_branch.to_string()
    };
    let base_branch = record.git_pr.base_branch.clone();
    let branch_name = resolve_branch_name(record, &current_branch);

    if current_branch == branch_name {
        append_message_event(
            store,
            record,
            "准备任务分支",
            SelfEvolutionLoopGitPrEventStatus::Skipped,
            format!("当前已在任务分支 {branch_name}。"),
        )?;
        return Ok(());
    }

    if current_branch == base_branch {
        switch_to_task_branch(root, &branch_name)?;
        append_message_event(
            store,
            record,
            "准备任务分支",
            SelfEvolutionLoopGitPrEventStatus::Succeeded,
            format!("已从 {base_branch} 切换到任务分支 {branch_name}。"),
        )?;
        return Ok(());
    }

    if current_branch.starts_with("codex/") && record.git_pr.branch_name.is_none() {
        record.git_pr.branch_name = Some(current_branch.clone());
        append_message_event(
            store,
            record,
            "复用当前任务分支",
            SelfEvolutionLoopGitPrEventStatus::Succeeded,
            format!("已复用当前任务分支 {current_branch}。"),
        )?;
        return Ok(());
    }

    Err(SelfEvolutionLoopGitPrError::InvalidState(format!(
        "当前分支 {current_branch} 既不是基础分支 {base_branch}，也不是目标任务分支 {branch_name}。"
    )))
}

pub(super) fn commit_successful_cycle(
    root: &Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    cycle: usize,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    if record.git_pr.mode == SelfEvolutionLoopGitPrMode::Disabled {
        return Ok(());
    }

    let status = git_text(root, &["status", "--porcelain", "--untracked-files=all"])?;
    if status.trim().is_empty() {
        append_message_event(
            store,
            record,
            "本轮本地提交",
            SelfEvolutionLoopGitPrEventStatus::Skipped,
            format!("第 {cycle} 轮没有需要提交的文件改动。"),
        )?;
        return Ok(());
    }

    let timeout_ms = record.git_pr.command_timeout_ms;
    run_required_command(
        root,
        store,
        record,
        "暂存本轮改动",
        "git",
        &["add", "--all"],
        timeout_ms,
    )?;
    run_required_command(
        root,
        store,
        record,
        "提交本轮改动",
        "git",
        &[
            "commit",
            "-m",
            &format!("feat(self-loop): 自我进化循环第 {cycle} 轮本地收束"),
            "-m",
            "未提升最终任务组版本，因此提交信息不携带版本号。",
        ],
        timeout_ms,
    )?;

    Ok(())
}

pub(super) fn finalize_git_pr_flow(
    app: &SelfForgeApp,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    match record.git_pr.mode {
        SelfEvolutionLoopGitPrMode::Disabled => Ok(()),
        SelfEvolutionLoopGitPrMode::LocalCommit => commit_pending_changes(
            app.root(),
            store,
            record,
            "提交循环收束记录",
            "chore(self-loop): 记录自我进化循环收束状态",
        ),
        SelfEvolutionLoopGitPrMode::PullRequest => finalize_pull_request(app, store, record),
    }
}

pub(super) fn recover_interrupted_git_pr_events(record: &mut SelfEvolutionLoopRecord) {
    for event in &mut record.git_pr_events {
        if event.status == SelfEvolutionLoopGitPrEventStatus::Running {
            event.status = SelfEvolutionLoopGitPrEventStatus::Failed;
            event.completed_at_unix_seconds = Some(current_unix_seconds());
            event.message =
                Some("上次 Git/PR 步骤在运行中中断，本次恢复时已标记为失败。".to_string());
        }
    }
}

fn finalize_pull_request(
    app: &SelfForgeApp,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    if !record.git_pr.confirmed {
        return Err(SelfEvolutionLoopGitPrError::InvalidState(
            "PR 自主收束需要显式传入 --confirm-finalize。".to_string(),
        ));
    }

    let state = ForgeState::load(app.root()).map_err(|error| {
        SelfEvolutionLoopGitPrError::InvalidState(format!("读取版本状态失败：{error}"))
    })?;
    let finalize = app
        .agent_work_finalize_check(&state.current_version)
        .map_err(SelfEvolutionLoopGitPrError::Finalize)?;
    if !finalize.can_finalize {
        append_message_event(
            store,
            record,
            "任务组收束检查",
            SelfEvolutionLoopGitPrEventStatus::Failed,
            finalize.blockers.join("；"),
        )?;
        return Err(SelfEvolutionLoopGitPrError::FinalizeBlocked(
            finalize.blockers,
        ));
    }
    append_message_event(
        store,
        record,
        "任务组收束检查",
        SelfEvolutionLoopGitPrEventStatus::Succeeded,
        format!(
            "任务组可收束：已完成 {}，已阻断 {}，开放错误 {}。",
            finalize.completed_count,
            finalize.blocked_count,
            finalize.open_errors.len()
        ),
    )?;

    run_final_validation_commands(app.root(), store, record)?;
    commit_pending_changes(
        app.root(),
        store,
        record,
        "提交最终收束记录",
        "chore(self-loop): 记录自我进化循环最终收束",
    )?;

    let branch =
        record.git_pr.branch_name.clone().ok_or_else(|| {
            SelfEvolutionLoopGitPrError::InvalidState("缺少任务分支名。".to_string())
        })?;
    let remote = record.git_pr.remote.clone();
    let base_branch = record.git_pr.base_branch.clone();
    let command_timeout_ms = record.git_pr.command_timeout_ms;
    run_required_command(
        app.root(),
        store,
        record,
        "统一推送任务分支",
        "git",
        &["push", "-u", &remote, &branch],
        command_timeout_ms,
    )?;

    let title = format!("{} 自主进化循环收束", state.current_version);
    let body = build_pr_body(record, &state.current_version, &branch);
    let create = run_required_command(
        app.root(),
        store,
        record,
        "创建 Pull Request",
        "gh",
        &[
            "pr",
            "create",
            "--base",
            &base_branch,
            "--head",
            &branch,
            "--title",
            &title,
            "--body",
            &body,
        ],
        command_timeout_ms,
    )?;
    let pr_url = extract_pr_url(&create.stdout).unwrap_or_else(|| create.stdout.trim().to_string());
    if !pr_url.trim().is_empty() {
        record.pr_url = Some(pr_url.clone());
        save_record(store, record)?;
    }

    if record.git_pr.wait_checks {
        let interval = record.git_pr.check_interval_seconds.to_string();
        let check_timeout_ms = record.git_pr.check_timeout_ms;
        run_required_command(
            app.root(),
            store,
            record,
            "等待 required checks",
            "gh",
            &[
                "pr",
                "checks",
                &pr_url,
                "--watch",
                "--fail-fast",
                "--interval",
                &interval,
            ],
            check_timeout_ms,
        )?;
    }

    if record.git_pr.merge {
        run_required_command(
            app.root(),
            store,
            record,
            "合并 Pull Request",
            "gh",
            &["pr", "merge", &pr_url, "--merge", "--delete-branch"],
            command_timeout_ms,
        )?;
    }

    if record.git_pr.delete_remote_branch {
        ensure_remote_branch_deleted(app.root(), store, record, &remote, &branch)?;
    }

    Ok(())
}

fn run_final_validation_commands(
    root: &Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    let commands: [(&str, &str, &[&str]); 5] = [
        ("最终验证 cargo fmt", "cargo", &["fmt", "--check"]),
        ("最终验证 cargo test", "cargo", &["test"]),
        ("最终验证 validate", "cargo", &["run", "--", "validate"]),
        ("最终验证 preflight", "cargo", &["run", "--", "preflight"]),
        (
            "最终验证开放错误",
            "cargo",
            &["run", "--", "errors", "--current", "--open"],
        ),
    ];
    for (action, program, args) in commands {
        let timeout_ms = record.git_pr.command_timeout_ms;
        run_required_command(root, store, record, action, program, args, timeout_ms)?;
    }
    Ok(())
}

fn commit_pending_changes(
    root: &Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    action: &str,
    title: &str,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    let status = git_text(root, &["status", "--porcelain", "--untracked-files=all"])?;
    if status.trim().is_empty() {
        append_message_event(
            store,
            record,
            action,
            SelfEvolutionLoopGitPrEventStatus::Skipped,
            "没有需要提交的收束记录。".to_string(),
        )?;
        return Ok(());
    }
    let timeout_ms = record.git_pr.command_timeout_ms;
    run_required_command(
        root,
        store,
        record,
        "暂存收束记录",
        "git",
        &["add", "--all"],
        timeout_ms,
    )?;
    run_required_command(
        root,
        store,
        record,
        action,
        "git",
        &[
            "commit",
            "-m",
            title,
            "-m",
            "阶段性本地提交，未提升最终任务组版本号。",
        ],
        timeout_ms,
    )?;
    Ok(())
}

fn ensure_remote_branch_deleted(
    root: &Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    remote: &str,
    branch: &str,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    let output = git_text(root, &["ls-remote", "--heads", remote, branch])?;
    if output.trim().is_empty() {
        append_message_event(
            store,
            record,
            "确认远程分支删除",
            SelfEvolutionLoopGitPrEventStatus::Succeeded,
            format!("远程分支 {remote}/{branch} 已不存在。"),
        )?;
        return Ok(());
    }

    let timeout_ms = record.git_pr.command_timeout_ms;
    run_required_command(
        root,
        store,
        record,
        "删除远程任务分支",
        "git",
        &["push", remote, "--delete", branch],
        timeout_ms,
    )?;
    Ok(())
}

fn switch_to_task_branch(root: &Path, branch: &str) -> Result<(), SelfEvolutionLoopGitPrError> {
    if git_success(root, &["rev-parse", "--verify", branch])? {
        let output = git_output(root, &["switch", branch])?;
        if output.status.success() {
            return Ok(());
        }
        return Err(command_failed(
            "切换已有任务分支",
            "git",
            &["switch", branch],
            output,
            false,
        ));
    }

    let output = git_output(root, &["switch", "-c", branch])?;
    if output.status.success() {
        return Ok(());
    }
    Err(command_failed(
        "创建任务分支",
        "git",
        &["switch", "-c", branch],
        output,
        false,
    ))
}

fn run_required_command(
    root: &Path,
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    action: &str,
    program: &str,
    args: &[&str],
    timeout_ms: u64,
) -> Result<CommandOutcome, SelfEvolutionLoopGitPrError> {
    let event_index = start_command_event(store, record, action, program, args)?;
    let output = run_command(root, action, program, args, timeout_ms);
    match output {
        Ok(outcome) if outcome.exit_code == Some(0) && !outcome.timed_out => {
            finish_command_event(
                store,
                record,
                event_index,
                SelfEvolutionLoopGitPrEventStatus::Succeeded,
                &outcome,
                None,
            )?;
            Ok(outcome)
        }
        Ok(outcome) => {
            finish_command_event(
                store,
                record,
                event_index,
                SelfEvolutionLoopGitPrEventStatus::Failed,
                &outcome,
                Some("命令执行失败。"),
            )?;
            Err(SelfEvolutionLoopGitPrError::CommandFailed {
                action: action.to_string(),
                program: program.to_string(),
                args: args.iter().map(|arg| (*arg).to_string()).collect(),
                exit_code: outcome.exit_code,
                timed_out: outcome.timed_out,
                stderr: truncate_chars(&outcome.stderr, 400),
            })
        }
        Err(error) => {
            let outcome = CommandOutcome {
                stdout: String::new(),
                stderr: error.to_string(),
                exit_code: None,
                timed_out: false,
            };
            finish_command_event(
                store,
                record,
                event_index,
                SelfEvolutionLoopGitPrEventStatus::Failed,
                &outcome,
                Some("命令启动失败。"),
            )?;
            Err(error)
        }
    }
}

fn run_command(
    root: &Path,
    action: &str,
    program: &str,
    args: &[&str],
    timeout_ms: u64,
) -> Result<CommandOutcome, SelfEvolutionLoopGitPrError> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| SelfEvolutionLoopGitPrError::CommandSpawn {
            action: action.to_string(),
            program: program.to_string(),
            source,
        })?;

    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut timed_out = false;
    loop {
        if child
            .try_wait()
            .map_err(|source| SelfEvolutionLoopGitPrError::CommandSpawn {
                action: action.to_string(),
                program: program.to_string(),
                source,
            })?
            .is_some()
        {
            break;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let output =
        child
            .wait_with_output()
            .map_err(|source| SelfEvolutionLoopGitPrError::CommandSpawn {
                action: action.to_string(),
                program: program.to_string(),
                source,
            })?;
    Ok(CommandOutcome {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code(),
        timed_out,
    })
}

fn start_command_event(
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    action: &str,
    program: &str,
    args: &[&str],
) -> Result<usize, SelfEvolutionLoopGitPrError> {
    let order = record.git_pr_events.len() + 1;
    let mut command = vec![program.to_string()];
    command.extend(args.iter().map(|arg| (*arg).to_string()));
    record.git_pr_events.push(SelfEvolutionLoopGitPrEvent {
        order,
        action: action.to_string(),
        status: SelfEvolutionLoopGitPrEventStatus::Running,
        started_at_unix_seconds: current_unix_seconds(),
        completed_at_unix_seconds: None,
        command: Some(command),
        exit_code: None,
        timed_out: false,
        stdout_preview: None,
        stderr_preview: None,
        message: None,
    });
    save_record(store, record)?;
    Ok(record.git_pr_events.len() - 1)
}

fn finish_command_event(
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    event_index: usize,
    status: SelfEvolutionLoopGitPrEventStatus,
    outcome: &CommandOutcome,
    message: Option<&str>,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    if let Some(event) = record.git_pr_events.get_mut(event_index) {
        event.status = status;
        event.completed_at_unix_seconds = Some(current_unix_seconds());
        event.exit_code = outcome.exit_code;
        event.timed_out = outcome.timed_out;
        event.stdout_preview = non_empty_preview(&outcome.stdout);
        event.stderr_preview = non_empty_preview(&outcome.stderr);
        event.message = message.map(ToString::to_string);
    }
    save_record(store, record)
}

fn append_message_event(
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
    action: &str,
    status: SelfEvolutionLoopGitPrEventStatus,
    message: String,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    let now = current_unix_seconds();
    record.git_pr_events.push(SelfEvolutionLoopGitPrEvent {
        order: record.git_pr_events.len() + 1,
        action: action.to_string(),
        status,
        started_at_unix_seconds: now,
        completed_at_unix_seconds: Some(now),
        command: None,
        exit_code: None,
        timed_out: false,
        stdout_preview: None,
        stderr_preview: None,
        message: Some(message),
    });
    save_record(store, record)
}

fn save_record(
    store: &SelfEvolutionLoopStore,
    record: &mut SelfEvolutionLoopRecord,
) -> Result<(), SelfEvolutionLoopGitPrError> {
    store
        .save(record)
        .map_err(|error| SelfEvolutionLoopGitPrError::Record(error.to_string()))
}

fn resolve_branch_name(record: &mut SelfEvolutionLoopRecord, current_branch: &str) -> String {
    if let Some(branch) = normalize_optional_text(record.git_pr.branch_name.as_deref()) {
        record.git_pr.branch_name = Some(branch.clone());
        return branch;
    }
    if current_branch.starts_with("codex/") {
        record.git_pr.branch_name = Some(current_branch.to_string());
        return current_branch.to_string();
    }
    let branch = record
        .git_pr
        .task_id
        .as_deref()
        .map(branch_name_for_task)
        .unwrap_or_else(|| format!("codex/{}", sanitize_branch_part(&record.id)));
    record.git_pr.branch_name = Some(branch.clone());
    branch
}

fn branch_name_for_task(task_id: &str) -> String {
    format!("codex/{}", sanitize_branch_part(task_id))
}

fn sanitize_branch_part(value: &str) -> String {
    let mut result = String::new();
    let mut last_dash = false;
    for character in value.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            result.push(normalized);
            last_dash = false;
        } else if !last_dash {
            result.push('-');
            last_dash = true;
        }
    }
    let trimmed = result.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "self-evolution-loop".to_string()
    } else {
        trimmed
    }
}

fn build_pr_body(record: &SelfEvolutionLoopRecord, current_version: &str, branch: &str) -> String {
    let task = record.git_pr.task_id.as_deref().unwrap_or("未指定");
    let worker = record.git_pr.worker_id.as_deref().unwrap_or("未指定");
    format!(
        "# 任务组边界\n- 循环记录：{}\n- 任务编号：{}\n- 工作线程：{}\n- 分支：{}\n- 目标：{}\n\n# 目标摘要\n完成一键可恢复自我进化循环的自主收束，远端只在最终 PR 阶段统一推送。\n\n# 主要变更\n- 完成轮次：{}\n- 失败轮次：{}\n- 连续失败：{}\n- 当前版本：{}\n\n# 验证结果\n- 本地最终验证由 agent-self-loop 自动执行：cargo fmt --check、cargo test、cargo run -- validate、cargo run -- preflight、cargo run -- errors --current --open。\n- PR 创建后等待 required checks 通过后再合并。\n\n# 风险\n- AI Provider、GitHub CLI 或远端检查不可用时会停止并写入循环记录。\n\n# 回滚方案\n- 使用 Git 提交回退任务分支，或按 SelfForge 候选状态机执行 rollback。\n\n# 归档路径\n- 循环记录：{}\n\n# Issue 关联\n- {}\n- {}\n",
        record.id,
        task,
        worker,
        branch,
        empty_as_none(&record.hint),
        record.completed_cycles,
        record.failed_cycles,
        record.consecutive_failures,
        current_version,
        record.file.display(),
        record.git_pr.issue_ref,
        record.git_pr.issue_url
    )
}

fn extract_pr_url(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("https://") || line.starts_with("http://"))
        .map(ToString::to_string)
}

fn non_empty_preview(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_chars(trimmed, 600))
    }
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn empty_as_none(value: &str) -> &str {
    if value.trim().is_empty() {
        "无"
    } else {
        value
    }
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, SelfEvolutionLoopGitPrError> {
    let output = git_output(root, args)?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    Err(command_failed("Git 查询", "git", args, output, false))
}

fn git_success(root: &Path, args: &[&str]) -> Result<bool, SelfEvolutionLoopGitPrError> {
    Ok(git_output(root, args)?.status.success())
}

fn git_output(
    root: &Path,
    args: &[&str],
) -> Result<std::process::Output, SelfEvolutionLoopGitPrError> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| SelfEvolutionLoopGitPrError::CommandSpawn {
            action: "Git 查询".to_string(),
            program: "git".to_string(),
            source,
        })
}

fn command_failed(
    action: &str,
    program: &str,
    args: &[&str],
    output: std::process::Output,
    timed_out: bool,
) -> SelfEvolutionLoopGitPrError {
    SelfEvolutionLoopGitPrError::CommandFailed {
        action: action.to_string(),
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        exit_code: output.status.code(),
        timed_out,
        stderr: truncate_chars(&String::from_utf8_lossy(&output.stderr), 400),
    }
}

impl fmt::Display for SelfEvolutionLoopGitPrMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SelfEvolutionLoopGitPrMode::Disabled => "不接入",
            SelfEvolutionLoopGitPrMode::LocalCommit => "本地提交",
            SelfEvolutionLoopGitPrMode::PullRequest => "PR 收束",
        };
        formatter.write_str(text)
    }
}

impl fmt::Display for SelfEvolutionLoopGitPrEventStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SelfEvolutionLoopGitPrEventStatus::Running => "运行中",
            SelfEvolutionLoopGitPrEventStatus::Succeeded => "已成功",
            SelfEvolutionLoopGitPrEventStatus::Failed => "已失败",
            SelfEvolutionLoopGitPrEventStatus::Skipped => "已跳过",
        };
        formatter.write_str(text)
    }
}

impl fmt::Display for SelfEvolutionLoopGitPrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfEvolutionLoopGitPrError::Record(message) => {
                write!(formatter, "写入循环记录失败：{message}")
            }
            SelfEvolutionLoopGitPrError::InvalidState(message) => formatter.write_str(message),
            SelfEvolutionLoopGitPrError::Finalize(error) => write!(formatter, "{error}"),
            SelfEvolutionLoopGitPrError::FinalizeBlocked(blockers) => {
                write!(formatter, "任务组尚不能收束：{}", blockers.join("；"))
            }
            SelfEvolutionLoopGitPrError::CommandSpawn {
                action,
                program,
                source,
            } => write!(formatter, "{action} 启动命令 {program} 失败：{source}"),
            SelfEvolutionLoopGitPrError::CommandFailed {
                action,
                program,
                args,
                exit_code,
                timed_out,
                stderr,
            } => write!(
                formatter,
                "{} 执行失败：{} {}，退出码 {:?}，超时 {}，错误 {}",
                action,
                program,
                args.join(" "),
                exit_code,
                if *timed_out { "是" } else { "否" },
                empty_as_none(stderr)
            ),
        }
    }
}

impl Error for SelfEvolutionLoopGitPrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SelfEvolutionLoopGitPrError::Finalize(error) => Some(error),
            SelfEvolutionLoopGitPrError::CommandSpawn { source, .. } => Some(source),
            SelfEvolutionLoopGitPrError::Record(_)
            | SelfEvolutionLoopGitPrError::InvalidState(_)
            | SelfEvolutionLoopGitPrError::FinalizeBlocked(_)
            | SelfEvolutionLoopGitPrError::CommandFailed { .. } => None,
        }
    }
}

impl From<SelfEvolutionLoopGitPrError> for SelfEvolutionLoopError {
    fn from(error: SelfEvolutionLoopGitPrError) -> Self {
        SelfEvolutionLoopError::GitPr(error)
    }
}
