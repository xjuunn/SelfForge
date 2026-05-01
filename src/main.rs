use self_forge::{
    AgentStepExecutionRequest, AgentToolInvocation, AgentToolInvocationInput, AgentWorkQueueReport,
    AgentWorkTaskStatus, CURRENT_VERSION, CycleResult, ErrorArchive, ErrorListQuery, ForgeState,
    MinimalLoopOutcome, RunQuery, SelfForgeApp, Supervisor, VersionBump,
};
use std::env;
use std::error::Error;
use std::process;

const DEFAULT_AI_TIMEOUT_MS: u64 = 60_000;

fn main() {
    let root = match env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("failed to resolve current directory: {error}");
            process::exit(1);
        }
    };
    let supervisor = Supervisor::new(&root);
    let app = SelfForgeApp::new(&root);
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "status".to_string());

    let result: Result<String, Box<dyn Error>> = match command.as_str() {
        "init" => boxed(supervisor.initialize_current_version().map(|report| {
            format!(
                "SelfForge {} initialized: {} created, {} existing",
                report.version,
                report.created_paths.len(),
                report.existing_paths.len()
            )
        })),
        "validate" => boxed(supervisor.verify_current_version().map(|report| {
            format!(
                "SelfForge {} valid: {} paths checked",
                report.version,
                report.checked_paths.len()
            )
        })),
        "status" => boxed(supervisor.verify_current_version().map(|report| {
            format!(
                "SelfForge {} ready: {} paths checked",
                report.version,
                report.checked_paths.len()
            )
        })),
        "preflight" => preflight(&app),
        "memory-context" => memory_context(&app, args.collect()),
        "memory-insights" => memory_insights(&app, args.collect()),
        "memory-compact" => memory_compact(&app, args.collect()),
        "ai-config" => ai_config(&app),
        "ai-request" => ai_request(&app, args.collect()),
        "agents" => agents(&app),
        "agent-tools" => agent_tools(&app, args.collect()),
        "agent-work-init" => agent_work_init(&app, args.collect()),
        "agent-work-status" => agent_work_status(&app, args.collect()),
        "agent-work-claim" => agent_work_claim(&app, args.collect()),
        "agent-work-complete" => agent_work_complete(&app, args.collect()),
        "agent-work-release" => agent_work_release(&app, args.collect()),
        "agent-tool-run" => agent_tool_run(&app, args.collect()),
        "agent-step" => agent_step(&app, args.collect()),
        "agent-plan" => agent_plan(&app, args.collect()),
        "agent-start" => agent_start(&app, args.collect()),
        "agent-sessions" => agent_sessions(&app, args.collect()),
        "agent-session" => agent_session(&app, args.collect()),
        "agent-run" => agent_run(&app, args.collect()),
        "agent-verify" => agent_verify(&app, args.collect()),
        "agent-advance" => agent_advance(&app, args.collect()),
        "agent-evolve" => agent_evolve(&app, args.collect()),
        "evolve" => evolve(&supervisor, args.collect()),
        "advance" => advance(&app, args.collect()),
        "promote" => boxed(supervisor.promote_candidate().map(|report| {
            format!(
                "SelfForge promoted {} from {}, current workspace {}",
                report.promoted_version, report.previous_version, report.state.workspace
            )
        })),
        "rollback" => {
            let reason = args.collect::<Vec<String>>().join(" ");
            let reason = if reason.trim().is_empty() {
                "manual rollback"
            } else {
                reason.trim()
            };
            boxed(supervisor.rollback_candidate(reason).map(|report| {
                format!(
                    "SelfForge rolled back {} and kept current {}, status {}",
                    report.rolled_back_version, report.current_version, report.state.status
                )
            }))
        }
        "cycle" => boxed(
            supervisor
                .run_candidate_cycle()
                .map(|report| match report.result {
                    CycleResult::Promoted => format!(
                        "SelfForge cycle promoted {} from {}, current workspace {}",
                        report.candidate_version, report.previous_version, report.state.workspace
                    ),
                    CycleResult::RolledBack => format!(
                        "SelfForge cycle rolled back {} and kept {}, reason {}",
                        report.candidate_version,
                        report.previous_version,
                        report.failure.unwrap_or_else(|| "未记录原因".to_string())
                    ),
                }),
        ),
        "run" => {
            let run = match parse_run_args(args.collect()) {
                Ok(run) => run,
                Err(error) => exit_with_error(error),
            };
            boxed(supervisor.execute_in_workspace(
                &run.version,
                &run.program,
                &run.args,
                run.timeout_ms,
            )
            .map(|report| {
                format!(
                    "SelfForge run {} in {}: exit {:?}, timed_out {}, stdout {} bytes, stderr {} bytes, record {}",
                    report.program,
                    report.workspace.display(),
                    report.exit_code,
                    report.timed_out,
                    report.stdout.len(),
                    report.stderr.len(),
                    report.run_dir.display()
                )
            }))
        }
        "runs" => {
            let runs = match parse_runs_args(args.collect()) {
                Ok(runs) => runs,
                Err(error) => exit_with_error(error),
            };
            boxed(
                supervisor
                    .query_runs(
                        &runs.version,
                        RunQuery {
                            limit: runs.limit,
                            failed_only: runs.failed_only,
                            timed_out_only: runs.timed_out_only,
                        },
                    )
                    .map(|entries| {
                        if entries.is_empty() {
                            return format!("SelfForge runs {}: no records", runs.version);
                        }

                        let mut lines = vec![format!(
                            "SelfForge runs {}: {} record(s)",
                            runs.version,
                            entries.len()
                        )];
                        for entry in entries {
                            lines.push(format!(
                                "{} exit {:?} timed_out {} stdout {} bytes stderr {} bytes report {}",
                                entry.run_id,
                                entry.exit_code,
                                entry.timed_out,
                                entry.stdout_bytes,
                                entry.stderr_bytes,
                                entry.report_file
                            ));
                        }
                        lines.join("\n")
                    }),
            )
        }
        "errors" => {
            let command = match parse_errors_args(args.collect()) {
                Ok(command) => command,
                Err(error) => exit_with_error(error),
            };
            let archive = ErrorArchive::new(&root);
            boxed(
                archive
                    .list_run_errors(
                        &command.version,
                        ErrorListQuery {
                            limit: command.limit,
                            open_only: command.open_only,
                            resolved_only: command.resolved_only,
                        },
                    )
                    .map(|entries| {
                        if entries.is_empty() {
                            return format!("SelfForge errors {}: no records", command.version);
                        }

                        let mut lines = vec![format!(
                            "SelfForge errors {}: {} record(s)",
                            command.version,
                            entries.len()
                        )];
                        for entry in entries {
                            lines.push(format!(
                                "{} resolved {} archive {}",
                                entry.run_id,
                                entry.resolved,
                                entry.archive_path.display()
                            ));
                        }
                        lines.join("\n")
                    }),
            )
        }
        "record-error" => {
            let command = match parse_record_error_args(args.collect()) {
                Ok(command) => command,
                Err(error) => exit_with_error(error),
            };
            let archive = ErrorArchive::new(&root);
            boxed(
                archive
                    .record_failed_run(
                        &command.version,
                        command.run_id.as_deref(),
                        &command.stage,
                        &command.solution,
                    )
                    .map(|report| {
                        if report.appended {
                            format!(
                                "SelfForge recorded error {} for {} in {}",
                                report.run_id,
                                report.version,
                                report.archive_path.display()
                            )
                        } else {
                            format!(
                                "SelfForge error {} for {} already recorded in {}",
                                report.run_id,
                                report.version,
                                report.archive_path.display()
                            )
                        }
                    }),
            )
        }
        "resolve-error" => {
            let command = match parse_resolve_error_args(args.collect()) {
                Ok(command) => command,
                Err(error) => exit_with_error(error),
            };
            let archive = ErrorArchive::new(&root);
            boxed(
                archive
                    .resolve_run_error(&command.version, &command.run_id, &command.verification)
                    .map(|report| {
                        if report.updated {
                            format!(
                                "SelfForge resolved error {} for {} in {}",
                                report.run_id,
                                report.version,
                                report.archive_path.display()
                            )
                        } else {
                            format!(
                                "SelfForge error {} for {} already resolved in {}",
                                report.run_id,
                                report.version,
                                report.archive_path.display()
                            )
                        }
                    }),
            )
        }
        "help" | "-h" | "--help" => {
            println!("{}", help_text());
            return;
        }
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("{}", help_text());
            process::exit(2);
        }
    };

    match result {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}

fn evolve(supervisor: &Supervisor, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let mut bump = VersionBump::Patch;
    let mut goal_parts = Vec::new();
    for argument in arguments {
        match argument.as_str() {
            "--patch" => bump = VersionBump::Patch,
            "--minor" => bump = VersionBump::Minor,
            "--major" => bump = VersionBump::Major,
            _ => goal_parts.push(argument),
        }
    }
    let goal = goal_parts.join(" ");
    let goal = if goal.trim().is_empty() {
        "prepare next controlled self-evolution candidate"
    } else {
        goal.trim()
    };
    boxed(
        supervisor
            .prepare_next_version_with_bump(goal, bump)
            .map(|report| {
                format!(
                    "SelfForge prepared {} from {}: {} paths checked, workspace {}, commit version {}",
                    report.next_version,
                    report.current_version,
                    report.candidate_validation.checked_paths.len(),
                    report.workspace.display(),
                    report.next_version
                )
            }),
    )
}

fn advance(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let goal = arguments.join(" ");
    let goal = if goal.trim().is_empty() {
        "推进 SelfForge 最小可运行闭环"
    } else {
        goal.trim()
    };

    boxed(app.advance(goal).map(|report| match report.outcome {
        MinimalLoopOutcome::Prepared => format!(
            "SelfForge advance prepared candidate {} from {}, next expected {:?}",
            report.candidate_version.unwrap_or_else(|| "无".to_string()),
            report.stable_version,
            report.next_expected_version
        ),
        MinimalLoopOutcome::PromotedAndPrepared => format!(
            "SelfForge advance promoted from {} and prepared candidate {} from {}",
            report.starting_version,
            report.candidate_version.unwrap_or_else(|| "无".to_string()),
            report.stable_version
        ),
        MinimalLoopOutcome::RolledBack => format!(
            "SelfForge advance rolled back candidate {} and kept {}, reason {}",
            report.candidate_version.unwrap_or_else(|| "无".to_string()),
            report.stable_version,
            report.failure.unwrap_or_else(|| "未记录原因".to_string())
        ),
    }))
}

fn preflight(app: &SelfForgeApp) -> Result<String, Box<dyn Error>> {
    boxed(app.preflight().map(|report| {
        let candidate = report.candidate_version.as_deref().unwrap_or("无");
        let candidate_workspace = report.candidate_workspace.as_deref().unwrap_or("无");
        let can_advance = if report.can_advance { "是" } else { "否" };

        format!(
            "SelfForge 前置检查 当前版本 {} 状态 {} 工作区 {} 候选版本 {} 候选工作区 {} 当前检查路径 {} 候选检查路径 {} 未解决错误 {} 可进化 {}",
            report.current_version,
            report.status,
            report.current_workspace,
            candidate,
            candidate_workspace,
            report.checked_paths.len(),
            report.candidate_checked_paths.len(),
            report.open_errors.len(),
            can_advance
        )
    }))
}

fn memory_context(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_memory_context_args(arguments)?;
    boxed(
        app.memory_context(&command.version, command.limit)
            .map(|report| {
                if report.entries.is_empty() {
                    return format!(
                        "SelfForge 最近记忆 {}: 无记录 文件 {}",
                        report.version,
                        report.archive_path.display()
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge 最近记忆 {}: {} 条 文件 {}",
                    report.version,
                    report.entries.len(),
                    report.archive_path.display()
                )];
                for entry in report.entries {
                    lines.push(format!(
                        "- {} 标题 {} 字符 {}",
                        entry.version,
                        entry.title,
                        entry.body.chars().count()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn memory_insights(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_memory_context_args(arguments)?;
    boxed(
        app.memory_insights(&command.version, command.limit)
            .map(|report| {
                let mut lines = vec![format!(
                    "SelfForge 记忆经验 {}: 来源 {} 成功 {} 风险 {} 建议 {} 经验 {} 文件 {}",
                    report.version,
                    report.source_versions.len(),
                    report.success_experiences.len(),
                    report.failure_experiences.len(),
                    report.optimization_suggestions.len(),
                    report.reusable_experiences.len(),
                    report.archive_path.display()
                )];
                append_insight_lines(&mut lines, "成功经验", &report.success_experiences);
                append_insight_lines(&mut lines, "失败风险", &report.failure_experiences);
                append_insight_lines(&mut lines, "优化建议", &report.optimization_suggestions);
                append_insight_lines(&mut lines, "可复用经验", &report.reusable_experiences);
                lines.join("\n")
            }),
    )
}

fn memory_compact(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_memory_compact_args(arguments)?;
    boxed(
        app.compact_memory(&command.version, command.keep)
            .map(|report| {
                format!(
                    "SelfForge 记忆压缩 {}: 原始 {} 条 保留 {} 条 本次归档 {} 条 冷归档总计 {} 条 热文件 {} 冷文件 {}",
                    report.version,
                    report.original_sections,
                    report.kept_sections,
                    report.archived_sections,
                    report.total_archive_sections,
                    report.memory_path.display(),
                    report.archive_path.display()
                )
            }),
    )
}

fn append_insight_lines(
    lines: &mut Vec<String>,
    title: &str,
    insights: &[self_forge::MemoryInsight],
) {
    if insights.is_empty() {
        lines.push(format!("{title}: 无"));
        return;
    }

    lines.push(format!("{title}: {} 条", insights.len()));
    for insight in insights {
        lines.push(format!("- {} {}", insight.version, insight.text));
    }
}

fn ai_config(app: &SelfForgeApp) -> Result<String, Box<dyn Error>> {
    boxed(app.ai_config().map(|report| {
        let ready = if report.ready { "是" } else { "否" };
        let selected = report.selected_provider.as_deref().unwrap_or("无");
        let mut lines = vec![format!(
            "SelfForge AI 配置 就绪 {ready} 选中提供商 {selected} 提供商数量 {}",
            report.providers.len()
        )];
        for provider in report.providers {
            let selected = if provider.selected { "是" } else { "否" };
            let configured = if provider.configured { "是" } else { "否" };
            let key_source = provider.api_key_env_var.as_deref().unwrap_or("未设置");
            lines.push(format!(
                "{} 选中 {} 已配置 {} 密钥变量 {} 模型 {} 基础地址 {} 协议 {} 路径 {}",
                provider.id,
                selected,
                configured,
                key_source,
                provider.model,
                provider.base_url,
                provider.protocol,
                provider.request_path
            ));
        }
        lines.join("\n")
    }))
}

fn ai_request(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_ai_request_args(arguments)?;
    if command.dry_run {
        return boxed(app.ai_request_preview(&command.prompt).map(|spec| {
            let body_size = spec.body.to_string().len();

            format!(
                "SelfForge AI 请求预览 提供商 {} 方法 {} 地址 {} 模型 {} 协议 {} 认证头 {} 密钥变量 {} 内容类型 {} 请求体字节 {}",
                spec.provider_id,
                spec.method,
                spec.url,
                spec.model,
                spec.protocol,
                spec.auth_header_name,
                spec.api_key_env_var,
                spec.content_type,
                body_size
            )
        }));
    }

    boxed(
        app.ai_request(&command.prompt, command.timeout_ms)
            .map(|report| {
                format!(
                    "SelfForge AI 响应 提供商 {} 模型 {} 协议 {} 状态码 {} 响应字节 {}\n{}",
                    report.response.provider_id,
                    report.response.model,
                    report.response.protocol,
                    report.status_code,
                    report.response.raw_bytes,
                    report.response.text
                )
            }),
    )
}

fn agents(app: &SelfForgeApp) -> Result<String, Box<dyn Error>> {
    let agents = app.agents();
    let mut lines = vec![format!("SelfForge Agent 目录 共 {} 个", agents.len())];
    for agent in agents {
        let capabilities = agent
            .capabilities
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("、");
        lines.push(format!(
            "{} {} 能力 {} 输出 {}",
            agent.id,
            agent.name,
            capabilities,
            agent.outputs.join("、")
        ));
    }

    Ok(lines.join("\n"))
}

fn agent_tools(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_tools_args(arguments)?;
    if command.init {
        return boxed(app.init_agent_tool_config(&command.version).map(|report| {
            let created = if report.created {
                "已创建"
            } else {
                "已存在"
            };
            format!(
                "SelfForge Agent 工具配置 {} 版本 {} 文件 {}",
                created,
                report.version,
                report.config_path.display()
            )
        }));
    }

    boxed(app.agent_tools(&command.version).map(|report| {
        let config = if report.config_exists {
            "已配置"
        } else {
            "使用内置默认"
        };
        let mut lines = vec![format!(
            "SelfForge Agent 工具 {} 工具 {} 分配 {} 配置 {} 文件 {}",
            report.version,
            report.tools.len(),
            report.assignments.len(),
            config,
            report.config_path.display()
        )];
        for tool in &report.tools {
            let enabled = if tool.enabled { "是" } else { "否" };
            let capabilities = tool
                .capabilities
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("、");
            let agents = if tool.agent_ids.is_empty() {
                "按能力匹配".to_string()
            } else {
                tool.agent_ids.join("、")
            };
            lines.push(format!(
                "工具 {} 启用 {} 类型 {} 能力 {} Agent {} 名称 {}",
                tool.id, enabled, tool.kind, capabilities, agents, tool.name
            ));
        }
        for assignment in &report.assignments {
            let tools = if assignment.tool_ids.is_empty() {
                "无".to_string()
            } else {
                assignment.tool_ids.join("、")
            };
            lines.push(format!("Agent {} 工具 {}", assignment.agent_id, tools));
        }
        lines.join("\n")
    }))
}

fn agent_work_init(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_work_init_args(arguments)?;
    boxed(
        app.init_agent_work_queue(&command.version, &command.goal, command.thread_count)
            .map(|report| {
                let action = if report.created {
                    "已创建"
                } else {
                    "已存在"
                };
                let mut lines = vec![format!(
                    "SelfForge 多 AI 协作任务板 {} 版本 {} 线程 {} 文件 {}",
                    action,
                    report.version,
                    report.queue.thread_count,
                    report.queue_path.display()
                )];
                append_agent_work_queue_lines(&mut lines, &report);
                lines.join("\n")
            }),
    )
}

fn agent_work_status(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_work_version_args(arguments, "agent-work-status")?;
    boxed(app.agent_work_status(&command.version).map(|report| {
        let mut lines = vec![format!(
            "SelfForge 多 AI 协作任务板 版本 {} 文件 {}",
            report.version,
            report.queue_path.display()
        )];
        append_agent_work_queue_lines(&mut lines, &report);
        lines.join("\n")
    }))
}

fn agent_work_claim(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_work_claim_args(arguments)?;
    boxed(
        app.claim_agent_work(
            &command.version,
            &command.worker_id,
            command.preferred_agent_id.as_deref(),
        )
        .map(|report| {
            let mut lines = vec![format!(
                "SelfForge 协作任务已领取 版本 {} 线程 {} 任务 {} 剩余可领取 {} 文件 {}",
                report.version,
                report.worker_id,
                report.task.id,
                report.remaining_available,
                report.queue_path.display()
            )];
            lines.push(format!(
                "标题 {} Agent {} 写入 {}",
                report.task.title,
                report.task.preferred_agent_id,
                join_or_none(&report.task.write_scope)
            ));
            lines.push("提示词：".to_string());
            lines.push(report.prompt);
            lines.join("\n")
        }),
    )
}

fn agent_work_complete(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_work_update_args(arguments, "agent-work-complete", "--summary")?;
    boxed(
        app.complete_agent_work(
            &command.version,
            &command.task_id,
            &command.worker_id,
            &command.text,
        )
        .map(|report| {
            let mut lines = vec![format!(
                "SelfForge 协作任务已完成 版本 {} 任务 {} 线程 {} 文件 {}",
                report.version,
                command.task_id,
                command.worker_id,
                report.queue_path.display()
            )];
            append_agent_work_queue_lines(&mut lines, &report);
            lines.join("\n")
        }),
    )
}

fn agent_work_release(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_work_update_args(arguments, "agent-work-release", "--reason")?;
    boxed(
        app.release_agent_work(
            &command.version,
            &command.task_id,
            &command.worker_id,
            &command.text,
        )
        .map(|report| {
            let mut lines = vec![format!(
                "SelfForge 协作任务已释放 版本 {} 任务 {} 线程 {} 文件 {}",
                report.version,
                command.task_id,
                command.worker_id,
                report.queue_path.display()
            )];
            append_agent_work_queue_lines(&mut lines, &report);
            lines.join("\n")
        }),
    )
}

fn append_agent_work_queue_lines(lines: &mut Vec<String>, report: &AgentWorkQueueReport) {
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
    for task in &report.queue.tasks {
        let claimed_by = task.claimed_by.as_deref().unwrap_or("无");
        lines.push(format!(
            "任务 {} 状态 {} Agent {} 优先级 {} 领取 {} 依赖 {} 写入 {}",
            task.id,
            task.status,
            task.preferred_agent_id,
            task.priority,
            claimed_by,
            join_or_none(&task.depends_on),
            join_or_none(&task.write_scope)
        ));
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

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "无".to_string()
    } else {
        values.join("、")
    }
}

fn agent_tool_run(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let invocation = parse_agent_tool_run_args(arguments)?;

    boxed(app.invoke_agent_tool(invocation).map(|report| {
        let mut lines = vec![format!(
            "SelfForge Agent 工具调用 Agent {} 工具 {} 版本 {} 摘要 {}",
            report.agent_id, report.tool_id, report.version, report.summary
        )];
        for detail in report.details {
            lines.push(format!("- {detail}"));
        }
        if let Some(run) = report.run {
            lines.push(format!(
                "运行记录 {} 退出码 {:?} 超时 {} 报告 {}",
                run.run_id, run.exit_code, run.timed_out, run.report_file
            ));
        }
        lines.join("\n")
    }))
}

fn agent_step(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let request = parse_agent_step_args(arguments)?;

    boxed(app.execute_next_agent_step(request).map(|report| {
        let completed = if report.session_completed {
            "是"
        } else {
            "否"
        };
        let mut lines = vec![format!(
            "SelfForge Agent 步进 会话 {} 版本 {} 步骤 {} Agent {} 工具 {} 完成会话 {} 摘要 {}",
            report.session_id,
            report.session_version,
            report.step_order,
            report.agent_id,
            report.tool.tool_id,
            completed,
            report.tool.summary
        )];
        for detail in report.tool.details {
            lines.push(format!("- {detail}"));
        }
        if let Some(run) = report.tool.run {
            lines.push(format!(
                "运行记录 {} 退出码 {:?} 超时 {} 报告 {}",
                run.run_id, run.exit_code, run.timed_out, run.report_file
            ));
        }
        lines.join("\n")
    }))
}

fn agent_plan(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_plan_args(arguments)?;
    let report = app.agent_plan_with_memory(&command.goal, &command.version, command.limit);

    boxed(report.map(|report| {
        let plan = report.plan;
        let insights = report.insights;
        let tools = report.tools;
        let mut lines = vec![format!(
            "SelfForge Agent 计划 目标 {} 记忆版本 {} 来源 {} 成功 {} 风险 {} 建议 {} 经验 {} 工具 {} 文件 {}",
            plan.goal,
            insights.version,
            insights.source_versions.len(),
            insights.success_experiences.len(),
            insights.failure_experiences.len(),
            insights.optimization_suggestions.len(),
            insights.reusable_experiences.len(),
            tools.tools.len(),
            insights.archive_path.display()
        )];
        append_insight_lines(&mut lines, "成功经验", &insights.success_experiences);
        append_insight_lines(&mut lines, "失败风险", &insights.failure_experiences);
        append_insight_lines(&mut lines, "优化建议", &insights.optimization_suggestions);
        append_insight_lines(&mut lines, "可复用经验", &insights.reusable_experiences);
        lines.push(format!("参与 Agent {}", plan.agents.len()));
        for step in plan.steps {
            let tools = if step.tool_ids.is_empty() {
                "无".to_string()
            } else {
                step.tool_ids.join("、")
            };
            lines.push(format!(
                "{}. [{}] {} 能力 {} 工具 {} 验证 {}",
                step.order, step.agent_id, step.title, step.capability, tools, step.verification
            ));
        }
        lines.join("\n")
    }))
}

fn agent_start(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_start_args(arguments)?;
    boxed(
        app.start_agent_session(&command.version, &command.goal)
            .map(|session| {
                let context = session
                    .plan_context
                    .as_ref()
                    .map(format_plan_context_summary)
                    .unwrap_or_else(|| "计划依据 无".to_string());
                format!(
                    "SelfForge Agent 会话已创建 {} 版本 {} 状态 {} 步骤 {} {} 文件 {}",
                    session.id,
                    session.version,
                    session.status,
                    session.steps.len(),
                    context,
                    session.file.display()
                )
            }),
    )
}

fn agent_sessions(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_sessions_args(arguments)?;
    let sessions = if command.all_versions {
        app.agent_sessions_all(&command.version, command.limit)
    } else {
        app.agent_sessions(&command.version, command.limit)
    };
    let scope = if command.all_versions {
        format!("{} 所属 major", command.version)
    } else {
        command.version.clone()
    };

    boxed(sessions.map(|sessions| {
        if sessions.is_empty() {
            return format!("SelfForge Agent 会话 {scope}: 无记录");
        }

        let mut lines = vec![format!(
            "SelfForge Agent 会话 {scope}: {} 条记录",
            sessions.len()
        )];
        for session in sessions {
            lines.push(format!(
                "{} 版本 {} 状态 {} 步骤 {} 事件 {} 目标 {} 文件 {}",
                session.id,
                session.version,
                session.status,
                session.step_count,
                session.event_count,
                session.goal,
                session.file.display()
            ));
        }
        lines.join("\n")
    }))
}

fn agent_session(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_session_args(arguments)?;
    boxed(
        app.agent_session(&command.version, &command.id)
            .map(|session| {
                let mut lines = vec![format!(
                    "SelfForge Agent 会话 {} 版本 {} 状态 {} 目标 {}",
                    session.id, session.version, session.status, session.goal
                )];
                if let Some(context) = session.plan_context.as_ref() {
                    lines.push(format_plan_context_summary(context));
                    if !context.source_versions.is_empty() {
                        lines.push(format!(
                            "计划依据来源 {}",
                            context.source_versions.join("、")
                        ));
                    }
                } else {
                    lines.push("计划依据 无".to_string());
                }
                for step in session.steps {
                    let tools = if step.tool_ids.is_empty() {
                        "无".to_string()
                    } else {
                        step.tool_ids.join("、")
                    };
                    lines.push(format!(
                        "{}. [{}] {} 状态 {} 工具 {} 验证 {}",
                        step.order,
                        step.agent_id,
                        step.title,
                        step.status,
                        tools,
                        step.verification
                    ));
                    if let Some(result) = step.result {
                        lines.push(format!("   结果 {}", result));
                    }
                }
                if let Some(outcome) = session.outcome {
                    lines.push(format!("结果 {}", outcome));
                }
                if let Some(error) = session.error {
                    lines.push(format!("错误 {}", error));
                }
                lines.push(format!("事件 {} 条", session.events.len()));
                for event in session.events {
                    let step = event
                        .step_order
                        .map(|order| format!(" 步骤 {order}"))
                        .unwrap_or_default();
                    let run = event
                        .run
                        .as_ref()
                        .map(|run| {
                            format!(
                                " 运行 {} 版本 {} 退出码 {:?} 超时 {} 报告 {}",
                                run.run_id,
                                run.version,
                                run.exit_code,
                                run.timed_out,
                                run.report_file
                            )
                        })
                        .unwrap_or_default();
                    lines.push(format!(
                        "事件 {} 时间 {} 类型 {}{}{} 内容 {}",
                        event.order,
                        event.timestamp_unix_seconds,
                        event.kind,
                        step,
                        run,
                        event.message
                    ));
                }
                lines.push(format!("文件 {}", session.file.display()));
                lines.join("\n")
            }),
    )
}

fn format_plan_context_summary(context: &self_forge::AgentSessionPlanContext) -> String {
    format!(
        "计划依据 记忆版本 {} 来源 {} 成功 {} 风险 {} 建议 {} 经验 {} 文件 {}",
        context.memory_version,
        context.source_versions.len(),
        context.success_experiences.len(),
        context.failure_experiences.len(),
        context.optimization_suggestions.len(),
        context.reusable_experiences.len(),
        context.memory_archive_file
    )
}

fn agent_run(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_run_args(arguments)?;
    boxed(
        app.agent_run(
            &command.session_version,
            &command.session_id,
            &command.target_version,
            command.step_order,
            &command.program,
            &command.args,
            command.timeout_ms,
        )
        .map(|report| {
            format!(
                "SelfForge Agent 运行完成 会话 {} 步骤 {} 运行 {} 版本 {} 退出码 {:?} 超时 {} 状态 {} 记录 {}",
                report.session.id,
                report.step_order,
                report.run_id,
                report.execution.version,
                report.execution.exit_code,
                report.execution.timed_out,
                report.session.status,
                report.execution.run_dir.display()
            )
        }),
    )
}

fn agent_verify(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_verify_args(arguments)?;
    boxed(
        app.agent_verify(
            &command.goal,
            &command.target_version,
            &command.program,
            &command.args,
            command.timeout_ms,
        )
        .map(|report| {
            format!(
                "SelfForge Agent 验证完成 会话 {} 运行 {} 版本 {} 退出码 {:?} 超时 {} 状态 {} 记录 {}",
                report.session.id,
                report.run_id,
                report.execution.version,
                report.execution.exit_code,
                report.execution.timed_out,
                report.session.status,
                report.execution.run_dir.display()
            )
        }),
    )
}

fn agent_advance(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let goal = arguments.join(" ");
    let goal = if goal.trim().is_empty() {
        "推进 SelfForge 自动进化流程"
    } else {
        goal.trim()
    };

    boxed(app.agent_advance(goal).map(|report| {
        format!(
            "SelfForge Agent 自动进化完成 会话 {} 结果 {:?} 起始版本 {} 稳定版本 {} 候选版本 {} 前置检查未解决错误 {}",
            report.session.id,
            report.minimal_loop.outcome,
            report.minimal_loop.starting_version,
            report.minimal_loop.stable_version,
            report
                .minimal_loop
                .candidate_version
                .as_deref()
                .unwrap_or("无"),
            report.preflight.open_errors.len()
        )
    }))
}

fn agent_evolve(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let goal = arguments.join(" ");
    let goal = if goal.trim().is_empty() {
        "执行 SelfForge 单轮完整自动进化"
    } else {
        goal.trim()
    };

    boxed(app.agent_evolve(goal).map(|report| {
        let prepared = report
            .prepared_candidate_version
            .as_deref()
            .unwrap_or("复用已有候选");
        let compaction = report
            .memory_compaction
            .as_ref()
            .map(|report| {
                format!(
                    "已压缩热记忆 保留 {} 归档 {}",
                    report.kept_sections, report.archived_sections
                )
            })
            .unwrap_or_else(|| "未压缩热记忆".to_string());
        format!(
            "SelfForge Agent 单轮进化完成 会话 {} 准备 {} 候选版本 {} 结果 {:?} 当前稳定版本 {} 未解决错误 {} {}",
            report.session.id,
            prepared,
            report.cycle.candidate_version,
            report.cycle.result,
            report.cycle.state.current_version,
            report.preflight.open_errors.len(),
            compaction
        )
    }))
}

struct AiRequestArgs {
    dry_run: bool,
    timeout_ms: u64,
    prompt: String,
}

struct MemoryContextArgs {
    version: String,
    limit: usize,
}

struct MemoryCompactArgs {
    version: String,
    keep: usize,
}

fn parse_ai_request_args(arguments: Vec<String>) -> Result<AiRequestArgs, Box<dyn Error>> {
    let mut dry_run = false;
    let mut timeout_ms = DEFAULT_AI_TIMEOUT_MS;
    let mut prompt_parts = Vec::new();
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--dry-run" => {
                dry_run = true;
                index += 1;
            }
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            "--" => {
                prompt_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 ai-request 参数: {other}").into());
            }
            _ => {
                prompt_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    Ok(AiRequestArgs {
        dry_run,
        timeout_ms,
        prompt: prompt_parts.join(" "),
    })
}

fn parse_memory_context_args(arguments: Vec<String>) -> Result<MemoryContextArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 5;
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
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            other => return Err(format!("未知 memory-context 参数: {other}").into()),
        }
    }

    Ok(MemoryContextArgs { version, limit })
}

fn parse_memory_compact_args(arguments: Vec<String>) -> Result<MemoryCompactArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut keep = 5;
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
            "--keep" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--keep 需要数量".into());
                };
                keep = value.parse::<usize>()?;
                index += 2;
            }
            other => return Err(format!("未知 memory-compact 参数: {other}").into()),
        }
    }

    Ok(MemoryCompactArgs { version, keep })
}

struct RunArgs {
    version: String,
    program: String,
    args: Vec<String>,
    timeout_ms: u64,
}

struct RunsArgs {
    version: String,
    limit: usize,
    failed_only: bool,
    timed_out_only: bool,
}

struct ErrorsArgs {
    version: String,
    limit: usize,
    open_only: bool,
    resolved_only: bool,
}

struct RecordErrorArgs {
    version: String,
    run_id: Option<String>,
    stage: String,
    solution: String,
}

struct ResolveErrorArgs {
    version: String,
    run_id: String,
    verification: String,
}

struct AgentStartArgs {
    version: String,
    goal: String,
}

struct AgentPlanArgs {
    version: String,
    limit: usize,
    goal: String,
}

struct AgentToolsArgs {
    version: String,
    init: bool,
}

struct AgentWorkInitArgs {
    version: String,
    goal: String,
    thread_count: usize,
}

struct AgentWorkVersionArgs {
    version: String,
}

struct AgentWorkClaimArgs {
    version: String,
    worker_id: String,
    preferred_agent_id: Option<String>,
}

struct AgentWorkUpdateArgs {
    version: String,
    task_id: String,
    worker_id: String,
    text: String,
}

struct AgentToolRunArgs {
    agent_id: String,
    tool_id: String,
    version: String,
    limit: usize,
    all_major: bool,
    session_version: String,
    session_id: Option<String>,
    step_order: usize,
    target_version: String,
    timeout_ms: u64,
    prompt: Option<String>,
    command_start: Option<usize>,
    arguments: Vec<String>,
}

struct AgentStepArgs {
    session_version: String,
    session_id: String,
    target_version: String,
    tool_id: Option<String>,
    limit: usize,
    timeout_ms: u64,
    prompt: Option<String>,
    command_start: Option<usize>,
    arguments: Vec<String>,
}

struct AgentSessionsArgs {
    version: String,
    limit: usize,
    all_versions: bool,
}

struct AgentSessionArgs {
    version: String,
    id: String,
}

struct AgentRunArgs {
    session_version: String,
    session_id: String,
    target_version: String,
    step_order: usize,
    timeout_ms: u64,
    program: String,
    args: Vec<String>,
}

struct AgentVerifyArgs {
    goal: String,
    target_version: String,
    timeout_ms: u64,
    program: String,
    args: Vec<String>,
}

fn parse_run_args(arguments: Vec<String>) -> Result<RunArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = CURRENT_VERSION.to_string();
    let mut timeout_ms = 30_000;
    let mut command_start = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--" => {
                command_start = Some(index + 1);
                break;
            }
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
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            _ => {
                command_start = Some(index);
                break;
            }
        }
    }

    let start = command_start.ok_or("run 需要命令")?;
    let program = arguments.get(start).ok_or("run 需要命令")?.clone();
    let args = arguments[start + 1..].to_vec();

    Ok(RunArgs {
        version,
        program,
        args,
        timeout_ms,
    })
}

fn parse_runs_args(arguments: Vec<String>) -> Result<RunsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
    let mut failed_only = false;
    let mut timed_out_only = false;
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
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            "--failed" => {
                failed_only = true;
                index += 1;
            }
            "--timed-out" => {
                timed_out_only = true;
                index += 1;
            }
            other => return Err(format!("未知 runs 参数: {other}").into()),
        }
    }

    Ok(RunsArgs {
        version,
        limit,
        failed_only,
        timed_out_only,
    })
}

fn parse_errors_args(arguments: Vec<String>) -> Result<ErrorsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
    let mut open_only = false;
    let mut resolved_only = false;
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
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            "--open" => {
                open_only = true;
                index += 1;
            }
            "--resolved" => {
                resolved_only = true;
                index += 1;
            }
            other => return Err(format!("未知 errors 参数: {other}").into()),
        }
    }

    Ok(ErrorsArgs {
        version,
        limit,
        open_only,
        resolved_only,
    })
}

fn parse_record_error_args(arguments: Vec<String>) -> Result<RecordErrorArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut run_id = None;
    let mut stage = "Runtime 受控执行".to_string();
    let mut solution = "待分析并修复后重新运行验证。".to_string();
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
            "--run-id" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--run-id 需要运行编号".into());
                };
                run_id = Some(value.clone());
                index += 2;
            }
            "--stage" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--stage 需要阶段说明".into());
                };
                stage = value.clone();
                index += 2;
            }
            "--solution" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--solution 需要解决方案".into());
                };
                solution = value.clone();
                index += 2;
            }
            other => return Err(format!("未知 record-error 参数: {other}").into()),
        }
    }

    Ok(RecordErrorArgs {
        version,
        run_id,
        stage,
        solution,
    })
}

fn parse_resolve_error_args(arguments: Vec<String>) -> Result<ResolveErrorArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut run_id = None;
    let mut verification = "已通过验证命令确认。".to_string();
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
            "--run-id" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--run-id 需要运行编号".into());
                };
                run_id = Some(value.clone());
                index += 2;
            }
            "--verification" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--verification 需要验证依据".into());
                };
                verification = value.clone();
                index += 2;
            }
            other => return Err(format!("未知 resolve-error 参数: {other}").into()),
        }
    }

    let run_id = run_id.ok_or("resolve-error 需要 --run-id")?;

    Ok(ResolveErrorArgs {
        version,
        run_id,
        verification,
    })
}

fn parse_agent_plan_args(arguments: Vec<String>) -> Result<AgentPlanArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 5;
    let mut goal_parts = Vec::new();
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
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            "--" => {
                goal_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-plan 参数: {other}").into());
            }
            _ => {
                goal_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    Ok(AgentPlanArgs {
        version,
        limit,
        goal: goal_parts.join(" "),
    })
}

fn parse_agent_tools_args(arguments: Vec<String>) -> Result<AgentToolsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut init = false;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--init" => {
                init = true;
                index += 1;
            }
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
            other => return Err(format!("未知 agent-tools 参数: {other}").into()),
        }
    }

    Ok(AgentToolsArgs { version, init })
}

fn parse_agent_work_init_args(arguments: Vec<String>) -> Result<AgentWorkInitArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut thread_count = 1;
    let mut goal_parts = Vec::new();
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
            "--threads" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--threads 需要线程数量".into());
                };
                thread_count = value.parse::<usize>()?;
                index += 2;
            }
            "--" => {
                goal_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-work-init 参数: {other}").into());
            }
            _ => {
                goal_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    let goal = if goal_parts.is_empty() {
        "协调多个 AI 线程完成受控代码修改".to_string()
    } else {
        goal_parts.join(" ")
    };

    Ok(AgentWorkInitArgs {
        version,
        goal,
        thread_count,
    })
}

fn parse_agent_work_version_args(
    arguments: Vec<String>,
    command_name: &str,
) -> Result<AgentWorkVersionArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
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
            other => return Err(format!("未知 {command_name} 参数: {other}").into()),
        }
    }

    Ok(AgentWorkVersionArgs { version })
}

fn parse_agent_work_claim_args(
    arguments: Vec<String>,
) -> Result<AgentWorkClaimArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut worker_id = "ai-1".to_string();
    let mut preferred_agent_id = None;
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
            "--worker" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--worker 需要线程标识".into());
                };
                worker_id = value.clone();
                index += 2;
            }
            "--agent" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--agent 需要 Agent 标识".into());
                };
                preferred_agent_id = Some(value.clone());
                index += 2;
            }
            other => return Err(format!("未知 agent-work-claim 参数: {other}").into()),
        }
    }

    Ok(AgentWorkClaimArgs {
        version,
        worker_id,
        preferred_agent_id,
    })
}

fn parse_agent_work_update_args(
    arguments: Vec<String>,
    command_name: &str,
    text_flag: &str,
) -> Result<AgentWorkUpdateArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut task_id = None;
    let mut worker_id = "ai-1".to_string();
    let mut text = String::new();
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
            "--worker" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--worker 需要线程标识".into());
                };
                worker_id = value.clone();
                index += 2;
            }
            flag if flag == text_flag => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err(format!("{text_flag} 需要说明文本").into());
                };
                text = value.clone();
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 {command_name} 参数: {other}").into());
            }
            other => {
                if task_id.is_some() {
                    return Err(format!("{command_name} 只能接收一个任务标识").into());
                }
                task_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentWorkUpdateArgs {
        version,
        task_id: task_id.ok_or(format!("{command_name} 需要任务标识"))?,
        worker_id,
        text,
    })
}

fn parse_agent_tool_run_args(
    arguments: Vec<String>,
) -> Result<AgentToolInvocation, Box<dyn Error>> {
    let command = parse_agent_tool_run_command(arguments)?;
    let input = match command.tool_id.as_str() {
        "memory.context" => AgentToolInvocationInput::MemoryContext {
            limit: command.limit,
        },
        "memory.insights" => AgentToolInvocationInput::MemoryInsights {
            limit: command.limit,
        },
        "agent.session" => AgentToolInvocationInput::AgentSessions {
            limit: command.limit,
            all_major: command.all_major,
        },
        "runtime.run" => {
            let session_id = command
                .session_id
                .ok_or("runtime.run 需要 --session 指定 Agent 会话")?;
            let start = command
                .command_start
                .ok_or("runtime.run 需要使用 -- 指定命令")?;
            let program = command
                .arguments
                .get(start)
                .ok_or("runtime.run 需要命令")?
                .clone();
            AgentToolInvocationInput::RuntimeRun {
                session_version: command.session_version,
                session_id,
                target_version: command.target_version,
                step_order: command.step_order,
                program,
                args: command.arguments[start + 1..].to_vec(),
                timeout_ms: command.timeout_ms,
            }
        }
        "ai.request" => {
            let prompt = command.prompt.ok_or("ai.request 需要 --prompt")?;
            AgentToolInvocationInput::AiRequestPreview { prompt }
        }
        "forge.archive" => AgentToolInvocationInput::ForgeArchiveStatus,
        _ => AgentToolInvocationInput::Empty,
    };

    Ok(AgentToolInvocation {
        agent_id: command.agent_id,
        tool_id: command.tool_id,
        version: command.version,
        input,
    })
}

fn parse_agent_tool_run_command(
    arguments: Vec<String>,
) -> Result<AgentToolRunArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut target_version = state.current_version.clone();
    let mut session_version = state.current_version.clone();
    let mut agent_id = None;
    let mut tool_id = None;
    let mut limit = 5;
    let mut all_major = false;
    let mut session_id = None;
    let mut step_order = 4;
    let mut timeout_ms = 30_000;
    let mut prompt = None;
    let mut command_start = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--" => {
                command_start = Some(index + 1);
                break;
            }
            "--agent" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--agent 需要 Agent 标识".into());
                };
                agent_id = Some(value.clone());
                index += 2;
            }
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
            "--target-current" => {
                target_version = state.current_version.clone();
                index += 1;
            }
            "--target-candidate" => {
                target_version = state.candidate_version.clone().ok_or("当前没有候选版本")?;
                index += 1;
            }
            "--target-version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--target-version 需要版本号".into());
                };
                target_version = value.clone();
                index += 2;
            }
            "--session-version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--session-version 需要版本号".into());
                };
                session_version = value.clone();
                index += 2;
            }
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            "--all" => {
                all_major = true;
                index += 1;
            }
            "--session" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--session 需要会话标识".into());
                };
                session_id = Some(value.clone());
                index += 2;
            }
            "--step" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--step 需要步骤序号".into());
                };
                step_order = value.parse::<usize>()?;
                index += 2;
            }
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            "--prompt" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--prompt 需要提示词".into());
                };
                prompt = Some(value.clone());
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-tool-run 参数: {other}").into());
            }
            other => {
                if tool_id.is_none() {
                    tool_id = Some(other.to_string());
                    index += 1;
                } else {
                    command_start = Some(index);
                    break;
                }
            }
        }
    }

    Ok(AgentToolRunArgs {
        agent_id: agent_id.ok_or("agent-tool-run 需要 --agent")?,
        tool_id: tool_id.ok_or("agent-tool-run 需要工具标识")?,
        version,
        limit,
        all_major,
        session_version,
        session_id,
        step_order,
        target_version,
        timeout_ms,
        prompt,
        command_start,
        arguments,
    })
}

fn parse_agent_step_args(
    arguments: Vec<String>,
) -> Result<AgentStepExecutionRequest, Box<dyn Error>> {
    let command = parse_agent_step_command(arguments)?;
    let (program, args) = if let Some(start) = command.command_start {
        let program = command
            .arguments
            .get(start)
            .ok_or("agent-step 的 -- 后需要命令")?
            .clone();
        (Some(program), command.arguments[start + 1..].to_vec())
    } else {
        (None, Vec::new())
    };

    Ok(AgentStepExecutionRequest {
        session_version: command.session_version,
        session_id: command.session_id,
        target_version: command.target_version,
        tool_id: command.tool_id,
        limit: command.limit,
        program,
        args,
        timeout_ms: command.timeout_ms,
        prompt: command.prompt,
    })
}

fn parse_agent_step_command(arguments: Vec<String>) -> Result<AgentStepArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut session_version = state.current_version.clone();
    let mut target_version = state.current_version.clone();
    let mut session_id = None;
    let mut tool_id = None;
    let mut limit = 5;
    let mut timeout_ms = 30_000;
    let mut prompt = None;
    let mut command_start = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--" => {
                command_start = Some(index + 1);
                break;
            }
            "--session-version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--session-version 需要版本号".into());
                };
                session_version = value.clone();
                index += 2;
            }
            "--current" => {
                session_version = state.current_version.clone();
                index += 1;
            }
            "--candidate" => {
                session_version = state.candidate_version.clone().ok_or("当前没有候选版本")?;
                index += 1;
            }
            "--target-current" => {
                target_version = state.current_version.clone();
                index += 1;
            }
            "--target-candidate" => {
                target_version = state.candidate_version.clone().ok_or("当前没有候选版本")?;
                index += 1;
            }
            "--target-version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--target-version 需要版本号".into());
                };
                target_version = value.clone();
                index += 2;
            }
            "--tool" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--tool 需要工具标识".into());
                };
                tool_id = Some(value.clone());
                index += 2;
            }
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            "--prompt" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--prompt 需要提示词".into());
                };
                prompt = Some(value.clone());
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-step 参数: {other}").into());
            }
            other => {
                if session_id.is_none() {
                    session_id = Some(other.to_string());
                    index += 1;
                } else {
                    command_start = Some(index);
                    break;
                }
            }
        }
    }

    Ok(AgentStepArgs {
        session_version,
        session_id: session_id.ok_or("agent-step 需要会话标识")?,
        target_version,
        tool_id,
        limit,
        timeout_ms,
        prompt,
        command_start,
        arguments,
    })
}

fn parse_agent_start_args(arguments: Vec<String>) -> Result<AgentStartArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut goal_parts = Vec::new();
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
            "--" => {
                goal_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-start 参数: {other}").into());
            }
            _ => {
                goal_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    Ok(AgentStartArgs {
        version,
        goal: goal_parts.join(" "),
    })
}

fn parse_agent_sessions_args(arguments: Vec<String>) -> Result<AgentSessionsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
    let mut all_versions = false;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--all" => {
                all_versions = true;
                index += 1;
            }
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
            "--limit" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--limit 需要数量".into());
                };
                limit = value.parse::<usize>()?;
                index += 2;
            }
            other => return Err(format!("未知 agent-sessions 参数: {other}").into()),
        }
    }

    Ok(AgentSessionsArgs {
        version,
        limit,
        all_versions,
    })
}

fn parse_agent_session_args(arguments: Vec<String>) -> Result<AgentSessionArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut id = None;
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
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-session 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-session 只能接收一个会话标识".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentSessionArgs {
        version,
        id: id.ok_or("agent-session 需要会话标识")?,
    })
}

fn parse_agent_run_args(arguments: Vec<String>) -> Result<AgentRunArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut session_version = state.current_version.clone();
    let mut target_version = state.current_version.clone();
    let mut step_order = 4;
    let mut timeout_ms = 30_000;
    let mut session_id = None;
    let mut command_start = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--" => {
                command_start = Some(index + 1);
                break;
            }
            "--session-version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--session-version 需要版本号".into());
                };
                session_version = value.clone();
                index += 2;
            }
            "--current" => {
                target_version = state.current_version.clone();
                index += 1;
            }
            "--candidate" => {
                target_version = state.candidate_version.clone().ok_or("当前没有候选版本")?;
                index += 1;
            }
            "--version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--version 需要版本号".into());
                };
                target_version = value.clone();
                index += 2;
            }
            "--step" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--step 需要步骤序号".into());
                };
                step_order = value.parse::<usize>()?;
                index += 2;
            }
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-run 参数: {other}").into());
            }
            other => {
                if session_id.is_none() {
                    session_id = Some(other.to_string());
                    index += 1;
                } else {
                    command_start = Some(index);
                    break;
                }
            }
        }
    }

    let start = command_start.ok_or("agent-run 需要命令")?;
    let program = arguments.get(start).ok_or("agent-run 需要命令")?.clone();
    let args = arguments[start + 1..].to_vec();

    Ok(AgentRunArgs {
        session_version,
        session_id: session_id.ok_or("agent-run 需要会话标识")?,
        target_version,
        step_order,
        timeout_ms,
        program,
        args,
    })
}

fn parse_agent_verify_args(arguments: Vec<String>) -> Result<AgentVerifyArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut target_version = state.current_version.clone();
    let mut timeout_ms = 30_000;
    let mut goal_parts = Vec::new();
    let mut command_start = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--" => {
                command_start = Some(index + 1);
                break;
            }
            "--current" => {
                target_version = state.current_version.clone();
                index += 1;
            }
            "--candidate" => {
                target_version = state.candidate_version.clone().ok_or("当前没有候选版本")?;
                index += 1;
            }
            "--version" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--version 需要版本号".into());
                };
                target_version = value.clone();
                index += 2;
            }
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-verify 参数: {other}").into());
            }
            other => {
                goal_parts.push(other.to_string());
                index += 1;
            }
        }
    }

    let start = command_start.ok_or("agent-verify 需要使用 -- 指定命令")?;
    let program = arguments.get(start).ok_or("agent-verify 需要命令")?.clone();
    let args = arguments[start + 1..].to_vec();
    let goal = if goal_parts.is_empty() {
        "执行 Agent 验证".to_string()
    } else {
        goal_parts.join(" ")
    };

    Ok(AgentVerifyArgs {
        goal,
        target_version,
        timeout_ms,
        program,
        args,
    })
}

fn help_text() -> &'static str {
    "SelfForge commands: init, validate, status, preflight, memory-context [--current|--candidate|--version VERSION] [--limit N], memory-insights [--current|--candidate|--version VERSION] [--limit N], memory-compact [--current|--candidate|--version VERSION] [--keep N], ai-config, ai-request [--dry-run] [--timeout-ms N] [prompt], agents, agent-tools [--current|--candidate|--version VERSION] [--init], agent-work-init [--current|--candidate|--version VERSION] [--threads N] [goal], agent-work-status [--current|--candidate|--version VERSION], agent-work-claim [--current|--candidate|--version VERSION] [--worker ID] [--agent AGENT_ID], agent-work-complete [--current|--candidate|--version VERSION] TASK_ID [--worker ID] [--summary TEXT], agent-work-release [--current|--candidate|--version VERSION] TASK_ID [--worker ID] [--reason TEXT], agent-tool-run TOOL_ID --agent AGENT_ID [--current|--candidate|--version VERSION] [--limit N] [--all] [--session SESSION_ID] [--session-version VERSION] [--step N] [--target-version VERSION] [--timeout-ms N] [--prompt TEXT] [-- PROGRAM ARGS...], agent-step [--session-version VERSION] [--target-version VERSION] [--tool TOOL_ID] [--limit N] [--timeout-ms N] [--prompt TEXT] SESSION_ID [-- PROGRAM ARGS...], agent-plan [--current|--candidate|--version VERSION] [--limit N] [goal], agent-start [--current|--candidate|--version VERSION] [goal], agent-sessions [--current|--candidate|--version VERSION] [--limit N] [--all], agent-session [--current|--candidate|--version VERSION] SESSION_ID, agent-run [--session-version VERSION] [--current|--candidate|--version VERSION] [--step N] [--timeout-ms N] SESSION_ID -- PROGRAM [ARGS...], agent-verify [--current|--candidate|--version VERSION] [--timeout-ms N] [goal] -- PROGRAM [ARGS...], agent-advance [goal], agent-evolve [goal], advance [goal], promote, rollback [reason], cycle, run [--current|--candidate|--version VERSION] [--timeout-ms N] -- PROGRAM [ARGS...], runs [--current|--candidate|--version VERSION] [--limit N] [--failed] [--timed-out], errors [--current|--candidate|--version VERSION] [--limit N] [--open] [--resolved], record-error [--current|--candidate|--version VERSION] [--run-id RUN_ID] [--stage TEXT] [--solution TEXT], resolve-error [--current|--candidate|--version VERSION] --run-id RUN_ID [--verification TEXT], evolve [--patch|--minor|--major] [goal]"
}

fn exit_with_error(error: Box<dyn Error>) -> ! {
    eprintln!("{error}");
    process::exit(1);
}

fn boxed<E>(result: Result<String, E>) -> Result<String, Box<dyn Error>>
where
    E: Error + 'static,
{
    result.map_err(|error| Box::new(error) as Box<dyn Error>)
}
