use self_forge::{AgentWorkQueueReport, AgentWorkTask, AgentWorkTaskStatus, ForgeState};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentWorkQueueFilter {
    All,
    ActiveOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkStatusArgs {
    pub version: String,
    pub filter: AgentWorkQueueFilter,
}

pub fn parse_agent_work_status_args(
    arguments: Vec<String>,
    state: &ForgeState,
) -> Result<AgentWorkStatusArgs, Box<dyn Error>> {
    let mut version = state.current_version.clone();
    let mut filter = AgentWorkQueueFilter::All;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--current" => {
                version = state.current_version.clone();
                index += 1;
            }
            "--candidate" => {
                version = state.candidate_version.clone().ok_or("当前没有候选版本")?;
                index += 1;
            }
            "--version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--version 需要版本号".into());
                };
                version = value.clone();
                index += 2;
            }
            "--active-only" => {
                filter = AgentWorkQueueFilter::ActiveOnly;
                index += 1;
            }
            other => return Err(format!("未知 agent-work-status 参数: {other}").into()),
        }
    }

    Ok(AgentWorkStatusArgs { version, filter })
}

pub fn format_agent_work_status_report(
    report: AgentWorkQueueReport,
    filter: AgentWorkQueueFilter,
) -> String {
    let suffix = match filter {
        AgentWorkQueueFilter::All => String::new(),
        AgentWorkQueueFilter::ActiveOnly => " 模式 活跃任务".to_string(),
    };
    let mut lines = vec![format!(
        "SelfForge 多 AI 协作任务板 版本 {} 文件 {}{}",
        report.version,
        report.queue_path.display(),
        suffix
    )];
    append_agent_work_queue_lines_with_filter(&mut lines, &report, filter);
    lines.join("\n")
}

pub fn append_agent_work_queue_lines(lines: &mut Vec<String>, report: &AgentWorkQueueReport) {
    append_agent_work_queue_lines_with_filter(lines, report, AgentWorkQueueFilter::All);
}

pub fn append_agent_work_queue_lines_with_filter(
    lines: &mut Vec<String>,
    report: &AgentWorkQueueReport,
    filter: AgentWorkQueueFilter,
) {
    lines.push(format!(
        "目标 {} 任务 {} 待领取 {} 已领取 {} 已完成 {} 已阻断 {}",
        report.queue.goal,
        report.queue.tasks.len(),
        count_agent_work_status(report, AgentWorkTaskStatus::Pending),
        count_agent_work_status(report, AgentWorkTaskStatus::Claimed),
        count_agent_work_status(report, AgentWorkTaskStatus::Completed),
        count_agent_work_status(report, AgentWorkTaskStatus::Blocked)
    ));
    lines.push(format!("冲突策略 {}", report.queue.conflict_policy));
    let mut included_tasks = 0usize;
    for task in &report.queue.tasks {
        if !filter.includes(task.status) {
            continue;
        }
        included_tasks += 1;
        append_agent_work_task_lines(lines, task);
    }
    if filter == AgentWorkQueueFilter::ActiveOnly && included_tasks == 0 {
        lines.push("活跃任务 无".to_string());
    }
}

pub fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "无".to_string()
    } else {
        values.join("、")
    }
}

pub fn format_agent_work_lease(task: &AgentWorkTask) -> String {
    match task.lease_expires_at_unix_seconds {
        Some(expires_at) => {
            let now = current_unix_seconds();
            if expires_at <= now {
                format!("已过期 unix:{expires_at}")
            } else {
                format!("unix:{expires_at} 剩余 {} 秒", expires_at - now)
            }
        }
        None => "无".to_string(),
    }
}

fn append_agent_work_task_lines(lines: &mut Vec<String>, task: &AgentWorkTask) {
    let claimed_by = task.claimed_by.as_deref().unwrap_or("无");
    lines.push(format!(
        "任务 {} 状态 {} Agent {} 优先级 {} 领取 {} 依赖 {} 写入 {}",
        task.id,
        task.status,
        task.preferred_agent_id,
        task.priority,
        claimed_by,
        join_or_none(&task.depends_on),
        join_or_none(&task.write_scope),
    ));
    lines.push(format!(
        "任务 {} 租约 {}",
        task.id,
        format_agent_work_lease(task)
    ));
    if let Some(branch) = agent_work_status_branch_suggestion(&task.id, task.status) {
        lines.push(format!("任务 {} 建议分支 {}", task.id, branch));
    }
}

fn count_agent_work_status(report: &AgentWorkQueueReport, status: AgentWorkTaskStatus) -> usize {
    report
        .queue
        .tasks
        .iter()
        .filter(|task| task.status == status)
        .count()
}

fn agent_work_status_branch_suggestion(
    task_id: &str,
    status: AgentWorkTaskStatus,
) -> Option<String> {
    if !matches!(
        status,
        AgentWorkTaskStatus::Pending | AgentWorkTaskStatus::Claimed
    ) {
        return None;
    }
    Some(branch_name_for_task_id(task_id))
}

fn branch_name_for_task_id(task_id: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in task_id.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "codex/task".to_string()
    } else {
        format!("codex/{slug}")
    }
}

impl AgentWorkQueueFilter {
    fn includes(self, status: AgentWorkTaskStatus) -> bool {
        match self {
            AgentWorkQueueFilter::All => true,
            AgentWorkQueueFilter::ActiveOnly => {
                matches!(
                    status,
                    AgentWorkTaskStatus::Pending | AgentWorkTaskStatus::Claimed
                )
            }
        }
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use self_forge::{AgentWorkQueue, AgentWorkTask};

    #[test]
    fn agent_work_status_suggests_branches_for_active_tasks_only() {
        let report = test_queue_report();
        let output = format_agent_work_status_report(report, AgentWorkQueueFilter::All);

        assert!(output.contains("任务 coord-020-new-task 建议分支 codex/coord-020-new-task"));
        assert!(
            output.contains("任务 coord-021-claimed-task 建议分支 codex/coord-021-claimed-task")
        );
        assert!(!output.contains("任务 coord-022-done-task 建议分支"));
        assert!(!output.contains("任务 coord-023-blocked-task 建议分支"));
    }

    #[test]
    fn agent_work_status_active_only_filters_terminal_tasks() {
        let report = test_queue_report();
        let output = format_agent_work_status_report(report, AgentWorkQueueFilter::ActiveOnly);

        assert!(output.contains("模式 活跃任务"));
        assert!(output.contains("任务 coord-020-new-task 状态 待领取"));
        assert!(output.contains("任务 coord-021-claimed-task 状态 已领取"));
        assert!(!output.contains("任务 coord-022-done-task 状态 已完成"));
        assert!(!output.contains("任务 coord-023-blocked-task 状态 已阻断"));
        assert!(output.contains("任务 4 待领取 1 已领取 1 已完成 1 已阻断 1"));
    }

    #[test]
    fn agent_work_status_active_only_reports_empty_result() {
        let mut report = test_queue_report();
        for task in &mut report.queue.tasks {
            task.status = AgentWorkTaskStatus::Completed;
            task.claimed_by = None;
        }

        let output = format_agent_work_status_report(report, AgentWorkQueueFilter::ActiveOnly);

        assert!(output.contains("活跃任务 无"));
        assert!(!output.contains("任务 coord-020-new-task 状态"));
    }

    #[test]
    fn agent_work_status_args_accept_active_only() {
        let state = ForgeState {
            current_version: "v0.1.68".to_string(),
            parent_version: Some("v0.1.67".to_string()),
            status: "active".to_string(),
            workspace: "workspaces/v0".to_string(),
            last_verified: Some("promoted:v0.1.68".to_string()),
            version_scheme: Some("semantic:vMAJOR.MINOR.PATCH".to_string()),
            candidate_version: Some("v0.1.69".to_string()),
            candidate_workspace: Some("workspaces/v0".to_string()),
            legacy_versions: Vec::new(),
        };

        let args = parse_agent_work_status_args(
            vec!["--candidate".to_string(), "--active-only".to_string()],
            &state,
        )
        .unwrap();

        assert_eq!(args.version, "v0.1.69");
        assert_eq!(args.filter, AgentWorkQueueFilter::ActiveOnly);
    }

    #[test]
    fn branch_name_for_task_id_sanitizes_non_ascii_text() {
        assert_eq!(
            branch_name_for_task_id("任务/coord 024:Status"),
            "codex/coord-024-status"
        );
    }

    fn test_queue_report() -> AgentWorkQueueReport {
        AgentWorkQueueReport {
            version: "v0.1.68".to_string(),
            queue_path: std::path::PathBuf::from("work-queue.json"),
            created: false,
            queue: AgentWorkQueue {
                version: "v0.1.68".to_string(),
                goal: "验证建议分支".to_string(),
                thread_count: 1,
                lease_duration_seconds: 3600,
                created_at_unix_seconds: 1,
                updated_at_unix_seconds: 1,
                conflict_policy: "无冲突".to_string(),
                prompt_policy: "无".to_string(),
                tasks: vec![
                    test_work_task("coord-020-new-task", AgentWorkTaskStatus::Pending),
                    test_work_task("coord-021-claimed-task", AgentWorkTaskStatus::Claimed),
                    test_work_task("coord-022-done-task", AgentWorkTaskStatus::Completed),
                    test_work_task("coord-023-blocked-task", AgentWorkTaskStatus::Blocked),
                ],
                events: Vec::new(),
            },
        }
    }

    fn test_work_task(id: &str, status: AgentWorkTaskStatus) -> AgentWorkTask {
        AgentWorkTask {
            id: id.to_string(),
            title: id.to_string(),
            description: "测试任务".to_string(),
            preferred_agent_id: "builder".to_string(),
            priority: 1,
            depends_on: Vec::new(),
            write_scope: Vec::new(),
            acceptance: Vec::new(),
            status,
            claimed_by: (status == AgentWorkTaskStatus::Claimed).then(|| "ai-1".to_string()),
            claimed_at_unix_seconds: None,
            lease_expires_at_unix_seconds: None,
            completed_at_unix_seconds: None,
            result: None,
            prompt: "测试提示".to_string(),
        }
    }
}
