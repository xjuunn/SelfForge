use self_forge::{
    SelfEvolutionLoopGitPrMode, SelfEvolutionLoopGitPrRequest, SelfEvolutionLoopRecord,
    SelfEvolutionLoopReport, SelfEvolutionLoopRequest, SelfEvolutionLoopSummary,
};
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSelfLoopArgs {
    pub dry_run: bool,
    pub request: SelfEvolutionLoopRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSelfLoopsArgs {
    pub version: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSelfLoopRecordArgs {
    pub version: String,
    pub id: String,
}

pub fn parse_agent_self_loop_args(
    arguments: Vec<String>,
    default_timeout_ms: u64,
) -> Result<AgentSelfLoopArgs, Box<dyn Error>> {
    let mut dry_run = false;
    let mut resume = false;
    let mut max_cycles = 1usize;
    let mut max_failures = 1usize;
    let mut timeout_ms = default_timeout_ms;
    let mut git_pr = SelfEvolutionLoopGitPrRequest::default();
    let mut hint_parts = Vec::new();
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--dry-run" => {
                dry_run = true;
                index += 1;
            }
            "--resume" => {
                resume = true;
                index += 1;
            }
            "--max-cycles" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--max-cycles 需要轮次数量".into());
                };
                max_cycles = value.parse::<usize>()?;
                index += 2;
            }
            "--max-failures" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--max-failures 需要失败次数".into());
                };
                max_failures = value.parse::<usize>()?;
                index += 2;
            }
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            "--commit-each-cycle" => {
                if git_pr.mode != SelfEvolutionLoopGitPrMode::PullRequest {
                    git_pr.mode = SelfEvolutionLoopGitPrMode::LocalCommit;
                }
                index += 1;
            }
            "--finalize-pr" => {
                git_pr.mode = SelfEvolutionLoopGitPrMode::PullRequest;
                index += 1;
            }
            "--confirm-finalize" => {
                git_pr.confirmed = true;
                index += 1;
            }
            "--branch" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--branch 需要分支名".into());
                };
                git_pr.branch_name = Some(value.clone());
                index += 2;
            }
            "--task" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--task 需要任务编号".into());
                };
                git_pr.task_id = Some(value.clone());
                index += 2;
            }
            "--worker" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--worker 需要工作线程编号".into());
                };
                git_pr.worker_id = Some(value.clone());
                index += 2;
            }
            "--base" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--base 需要基础分支名".into());
                };
                git_pr.base_branch = value.clone();
                index += 2;
            }
            "--remote" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--remote 需要远端名称".into());
                };
                git_pr.remote = value.clone();
                index += 2;
            }
            "--git-timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--git-timeout-ms 需要毫秒数".into());
                };
                git_pr.command_timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            "--check-timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--check-timeout-ms 需要毫秒数".into());
                };
                git_pr.check_timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            "--check-interval-seconds" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--check-interval-seconds 需要秒数".into());
                };
                git_pr.check_interval_seconds = value.parse::<u64>()?;
                index += 2;
            }
            "--" => {
                hint_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-self-loop 参数: {other}").into());
            }
            _ => {
                hint_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    Ok(AgentSelfLoopArgs {
        dry_run,
        request: SelfEvolutionLoopRequest {
            hint: hint_parts.join(" "),
            max_cycles,
            max_failures,
            timeout_ms,
            resume,
            git_pr,
        },
    })
}

pub fn parse_agent_self_loops_args(
    arguments: Vec<String>,
    default_version: &str,
) -> Result<AgentSelfLoopsArgs, Box<dyn Error>> {
    let mut version = default_version.to_string();
    let mut limit = 10usize;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--current" => {
                version = default_version.to_string();
                index += 1;
            }
            "--version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--version 需要版本号".into());
                };
                version = value.clone();
                index += 2;
            }
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            other => return Err(format!("未知 agent-self-loops 参数: {other}").into()),
        }
    }

    Ok(AgentSelfLoopsArgs { version, limit })
}

pub fn parse_agent_self_loop_record_args(
    arguments: Vec<String>,
    default_version: &str,
) -> Result<AgentSelfLoopRecordArgs, Box<dyn Error>> {
    let mut version = default_version.to_string();
    let mut id = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--current" => {
                version = default_version.to_string();
                index += 1;
            }
            "--version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--version 需要版本号".into());
                };
                version = value.clone();
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-self-loop-record 参数: {other}").into());
            }
            value => {
                if id.is_some() {
                    return Err("agent-self-loop-record 只允许一个记录编号".into());
                }
                id = Some(value.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentSelfLoopRecordArgs {
        version,
        id: id.ok_or("agent-self-loop-record 需要记录编号")?,
    })
}

pub fn format_agent_self_loop_preview(args: &AgentSelfLoopArgs) -> String {
    format!(
        "SelfForge 自我进化循环预览 最大轮次 {} 最大连续失败 {} 超时 {} 恢复 {} Git/PR {} 分支 {} 任务 {} 确认收束 {} 提示 {}",
        args.request.max_cycles,
        args.request.max_failures,
        args.request.timeout_ms,
        yes_no(args.request.resume),
        args.request.git_pr.mode,
        args.request.git_pr.branch_name.as_deref().unwrap_or("自动"),
        args.request.git_pr.task_id.as_deref().unwrap_or("无"),
        yes_no(args.request.git_pr.confirmed),
        empty_as_none(&args.request.hint)
    )
}

pub fn format_agent_self_loop_report(report: SelfEvolutionLoopReport) -> String {
    let mut lines = vec![format!(
        "SelfForge 自我进化循环 状态 {} 记录 {} 版本 {} 恢复 {} 完成轮次 {} 失败轮次 {} 连续失败 {} 文件 {} 索引 {}",
        report.record.status,
        report.record.id,
        report.record.version,
        yes_no(report.resumed),
        report.record.completed_cycles,
        report.record.failed_cycles,
        report.record.consecutive_failures,
        report.record.file.display(),
        report.index_file.display()
    )];
    lines.push(format!(
        "Git/PR 模式 {} 分支 {} 任务 {} PR {} 事件 {}",
        report.record.git_pr.mode,
        report.record.git_pr.branch_name.as_deref().unwrap_or("无"),
        report.record.git_pr.task_id.as_deref().unwrap_or("无"),
        report.record.pr_url.as_deref().unwrap_or("无"),
        report.record.git_pr_events.len()
    ));
    for step in &report.record.steps {
        lines.push(format!(
            "轮次 {} 状态 {} 稳定版本 {} -> {} 审计 {} 总结 {} 错误 {}",
            step.cycle,
            step.status,
            step.stable_version_before,
            step.stable_version_after.as_deref().unwrap_or("无"),
            step.audit_id.as_deref().unwrap_or("无"),
            step.summary_id.as_deref().unwrap_or("无"),
            step.error.as_deref().unwrap_or("无")
        ));
    }
    for event in &report.record.git_pr_events {
        let command = event
            .command
            .as_ref()
            .map(|parts| parts.join(" "))
            .unwrap_or_else(|| "无".to_string());
        let detail = event
            .message
            .as_deref()
            .or(event.stderr_preview.as_deref())
            .or(event.stdout_preview.as_deref())
            .unwrap_or("无");
        lines.push(format!(
            "Git/PR 事件 {} {} 状态 {} 命令 {} 详情 {}",
            event.order, event.action, event.status, command, detail
        ));
    }
    lines.join("\n")
}

pub fn format_agent_self_loop_records(
    version: &str,
    records: Vec<SelfEvolutionLoopSummary>,
) -> String {
    if records.is_empty() {
        return format!("SelfForge 自我进化循环记录 {version}: no records");
    }

    let mut lines = vec![format!(
        "SelfForge 自我进化循环记录 {version}: {} record(s)",
        records.len()
    )];
    for record in records {
        lines.push(format!(
            "{} 状态 {} Git/PR {} 完成 {} 失败 {} 连续失败 {} PR {} 文件 {}",
            record.id,
            record.status,
            record.git_pr_mode,
            record.completed_cycles,
            record.failed_cycles,
            record.consecutive_failures,
            record.pr_url.as_deref().unwrap_or("无"),
            record.file.display()
        ));
    }
    lines.join("\n")
}

pub fn format_agent_self_loop_record(record: SelfEvolutionLoopRecord) -> String {
    let mut lines = vec![format!(
        "SelfForge 自我进化循环记录 {} 版本 {} 状态 {} Git/PR {} 分支 {} PR {} 完成 {} 失败 {} 连续失败 {} 文件 {}",
        record.id,
        record.version,
        record.status,
        record.git_pr.mode,
        record.git_pr.branch_name.as_deref().unwrap_or("无"),
        record.pr_url.as_deref().unwrap_or("无"),
        record.completed_cycles,
        record.failed_cycles,
        record.consecutive_failures,
        record.file.display()
    )];
    for step in &record.steps {
        lines.push(format!(
            "轮次 {} 状态 {} 稳定版本 {} -> {} 审计 {} 总结 {} 错误 {}",
            step.cycle,
            step.status,
            step.stable_version_before,
            step.stable_version_after.as_deref().unwrap_or("无"),
            step.audit_id.as_deref().unwrap_or("无"),
            step.summary_id.as_deref().unwrap_or("无"),
            step.error.as_deref().unwrap_or("无")
        ));
    }
    for event in &record.git_pr_events {
        lines.push(format!(
            "Git/PR 事件 {} {} 状态 {} 详情 {}",
            event.order,
            event.action,
            event.status,
            event
                .message
                .as_deref()
                .or(event.stderr_preview.as_deref())
                .or(event.stdout_preview.as_deref())
                .unwrap_or("无")
        ));
    }
    lines.join("\n")
}

fn yes_no(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

fn empty_as_none(value: &str) -> &str {
    if value.trim().is_empty() {
        "无"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_self_loop_args_use_bounded_defaults() {
        let args = parse_agent_self_loop_args(Vec::new(), 60_000).unwrap();

        assert!(!args.dry_run);
        assert!(!args.request.resume);
        assert_eq!(args.request.max_cycles, 1);
        assert_eq!(args.request.max_failures, 1);
        assert_eq!(args.request.timeout_ms, 60_000);
        assert_eq!(
            args.request.git_pr.mode,
            SelfEvolutionLoopGitPrMode::Disabled
        );
    }

    #[test]
    fn agent_self_loop_args_parse_resume_limits_and_hint() {
        let args = parse_agent_self_loop_args(
            vec![
                "--resume".to_string(),
                "--max-cycles".to_string(),
                "3".to_string(),
                "--max-failures".to_string(),
                "2".to_string(),
                "--timeout-ms".to_string(),
                "1000".to_string(),
                "继续小步进化".to_string(),
            ],
            60_000,
        )
        .unwrap();

        assert!(args.request.resume);
        assert_eq!(args.request.max_cycles, 3);
        assert_eq!(args.request.max_failures, 2);
        assert_eq!(args.request.timeout_ms, 1000);
        assert_eq!(args.request.hint, "继续小步进化");
    }

    #[test]
    fn agent_self_loop_args_parse_git_pr_finalize_options() {
        let args = parse_agent_self_loop_args(
            vec![
                "--finalize-pr".to_string(),
                "--confirm-finalize".to_string(),
                "--branch".to_string(),
                "codex/coord-024-autonomous-loop-git-pr".to_string(),
                "--task".to_string(),
                "coord-024-autonomous-loop-git-pr".to_string(),
                "--worker".to_string(),
                "ai-1".to_string(),
                "--git-timeout-ms".to_string(),
                "2000".to_string(),
                "--check-timeout-ms".to_string(),
                "3000".to_string(),
                "--check-interval-seconds".to_string(),
                "5".to_string(),
                "自主收束".to_string(),
            ],
            60_000,
        )
        .unwrap();

        assert_eq!(
            args.request.git_pr.mode,
            SelfEvolutionLoopGitPrMode::PullRequest
        );
        assert!(args.request.git_pr.confirmed);
        assert_eq!(
            args.request.git_pr.branch_name.as_deref(),
            Some("codex/coord-024-autonomous-loop-git-pr")
        );
        assert_eq!(
            args.request.git_pr.task_id.as_deref(),
            Some("coord-024-autonomous-loop-git-pr")
        );
        assert_eq!(args.request.git_pr.command_timeout_ms, 2000);
        assert_eq!(args.request.git_pr.check_timeout_ms, 3000);
        assert_eq!(args.request.git_pr.check_interval_seconds, 5);
        assert_eq!(args.request.hint, "自主收束");
    }

    #[test]
    fn agent_self_loops_args_parse_version_and_limit() {
        let args = parse_agent_self_loops_args(
            vec![
                "--version".to_string(),
                "v0.1.69".to_string(),
                "--limit".to_string(),
                "3".to_string(),
            ],
            "v0.1.70",
        )
        .unwrap();

        assert_eq!(args.version, "v0.1.69");
        assert_eq!(args.limit, 3);
    }

    #[test]
    fn agent_self_loop_record_args_parse_id() {
        let args = parse_agent_self_loop_record_args(
            vec!["--current".to_string(), "self-loop-1".to_string()],
            "v0.1.70",
        )
        .unwrap();

        assert_eq!(args.version, "v0.1.70");
        assert_eq!(args.id, "self-loop-1");
    }
}
