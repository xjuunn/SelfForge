use self_forge::{SelfEvolutionLoopReport, SelfEvolutionLoopRequest};
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSelfLoopArgs {
    pub dry_run: bool,
    pub request: SelfEvolutionLoopRequest,
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
        },
    })
}

pub fn format_agent_self_loop_preview(args: &AgentSelfLoopArgs) -> String {
    format!(
        "SelfForge 自我进化循环预览 最大轮次 {} 最大连续失败 {} 超时 {} 恢复 {} 提示 {}",
        args.request.max_cycles,
        args.request.max_failures,
        args.request.timeout_ms,
        yes_no(args.request.resume),
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
}
