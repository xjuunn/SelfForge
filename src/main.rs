use self_forge::{
    AgentStepExecutionRequest, AgentToolInvocation, AgentToolInvocationInput, AgentWorkQueueReport,
    AgentWorkTaskStatus, CURRENT_VERSION, CycleResult, ErrorArchive, ErrorListQuery, ForgeState,
    MinimalLoopOutcome, RunQuery, SelfForgeApp, Supervisor, VersionBump,
};
use std::env;
use std::error::Error;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_AI_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_PATCH_VERIFICATION_TIMEOUT_MS: u64 = 120_000;

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
        "agent-work-reap" => agent_work_reap(&app, args.collect()),
        "agent-tool-run" => agent_tool_run(&app, args.collect()),
        "agent-step" => agent_step(&app, args.collect()),
        "agent-steps" => agent_steps(&app, args.collect()),
        "agent-plan" => agent_plan(&app, args.collect()),
        "agent-start" => agent_start(&app, args.collect()),
        "agent-sessions" => agent_sessions(&app, args.collect()),
        "agent-session" => agent_session(&app, args.collect()),
        "agent-run" => agent_run(&app, args.collect()),
        "agent-verify" => agent_verify(&app, args.collect()),
        "agent-advance" => agent_advance(&app, args.collect()),
        "agent-evolve" => agent_evolve(&app, args.collect()),
        "agent-patch-draft" => agent_patch_draft(&app, args.collect()),
        "agent-patch-drafts" => agent_patch_drafts(&app, args.collect()),
        "agent-patch-draft-record" => agent_patch_draft_record(&app, args.collect()),
        "agent-patch-audit" => agent_patch_audit(&app, args.collect()),
        "agent-patch-audits" => agent_patch_audits(&app, args.collect()),
        "agent-patch-audit-record" => agent_patch_audit_record(&app, args.collect()),
        "agent-patch-preview" => agent_patch_preview(&app, args.collect()),
        "agent-patch-previews" => agent_patch_previews(&app, args.collect()),
        "agent-patch-preview-record" => agent_patch_preview_record(&app, args.collect()),
        "agent-patch-apply" => agent_patch_apply(&app, args.collect()),
        "agent-patch-verify" => agent_patch_verify(&app, args.collect()),
        "agent-patch-source-plan" => agent_patch_source_plan(&app, args.collect()),
        "agent-patch-source-plans" => agent_patch_source_plans(&app, args.collect()),
        "agent-patch-source-plan-record" => agent_patch_source_plan_record(&app, args.collect()),
        "agent-patch-source-execute" => agent_patch_source_execute(&app, args.collect()),
        "agent-patch-source-executions" => agent_patch_source_executions(&app, args.collect()),
        "agent-patch-source-execution-record" => {
            agent_patch_source_execution_record(&app, args.collect())
        }
        "agent-patch-source-promotion" => agent_patch_source_promotion(&app, args.collect()),
        "agent-patch-source-promotions" => agent_patch_source_promotions(&app, args.collect()),
        "agent-patch-source-promotion-record" => {
            agent_patch_source_promotion_record(&app, args.collect())
        }
        "agent-patch-source-candidate" => agent_patch_source_candidate(&app, args.collect()),
        "agent-patch-source-candidates" => agent_patch_source_candidates(&app, args.collect()),
        "agent-patch-source-candidate-record" => {
            agent_patch_source_candidate_record(&app, args.collect())
        }
        "agent-patch-applications" => agent_patch_applications(&app, args.collect()),
        "agent-patch-application-record" => agent_patch_application_record(&app, args.collect()),
        "agent-self-upgrade" => agent_self_upgrade(&app, args.collect()),
        "agent-self-upgrades" => agent_self_upgrades(&app, args.collect()),
        "agent-self-upgrade-record" => agent_self_upgrade_record(&app, args.collect()),
        "agent-self-upgrade-report" => agent_self_upgrade_report(&app, args.collect()),
        "agent-self-upgrade-reports" => agent_self_upgrade_reports(&app, args.collect()),
        "agent-self-upgrade-report-record" => {
            agent_self_upgrade_report_record(&app, args.collect())
        }
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
        app.claim_agent_work_with_lease(
            &command.version,
            &command.worker_id,
            command.preferred_agent_id.as_deref(),
            command.lease_seconds,
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
            lines.push(format!("租约 {}", format_agent_work_lease(&report.task)));
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

fn agent_work_reap(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_work_reap_args(arguments)?;
    boxed(
        app.reap_expired_agent_work(&command.version, &command.text)
            .map(|report| {
                let mut lines = vec![format!(
                    "SelfForge 协作任务过期清理 版本 {} 释放 {} 文件 {}",
                    report.version,
                    report.released_tasks.len(),
                    report.queue_path.display()
                )];
                for task in &report.released_tasks {
                    lines.push(format!("已释放任务 {} {}", task.id, task.title));
                }
                let queue_report = AgentWorkQueueReport {
                    version: report.version,
                    queue_path: report.queue_path,
                    created: false,
                    queue: report.queue,
                };
                append_agent_work_queue_lines(&mut lines, &queue_report);
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
            join_or_none(&task.write_scope),
        ));
        lines.push(format!(
            "任务 {} 租约 {}",
            task.id,
            format_agent_work_lease(task)
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

fn format_agent_work_lease(task: &self_forge::AgentWorkTask) -> String {
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

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
        if let Some(task_id) = report.work_task_id.as_deref() {
            let worker_id = report.work_worker_id.as_deref().unwrap_or("未知");
            lines.push(format!("协作任务 {task_id} 工作线程 {worker_id}"));
        }
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

fn agent_steps(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_steps_args(arguments)?;

    boxed(
        app.execute_agent_steps(command.request, command.max_steps)
            .map(|report| {
                let mut lines = vec![format!(
                    "SelfForge Agent 多步运行 会话 {} 版本 {} 目标版本 {} 执行 {}/{} 停止 {}",
                    report.session_id,
                    report.session_version,
                    report.target_version,
                    report.executed_steps.len(),
                    report.max_steps,
                    report.stop
                )];
                for step in report.executed_steps {
                    lines.push(format!(
                        "- 步骤 {} Agent {} 工具 {} 摘要 {}",
                        step.step_order, step.agent_id, step.tool.tool_id, step.tool.summary
                    ));
                    if let Some(task_id) = step.work_task_id.as_deref() {
                        let worker_id = step.work_worker_id.as_deref().unwrap_or("未知");
                        lines.push(format!("  协作任务 {task_id} 工作线程 {worker_id}"));
                    }
                    if let Some(run) = step.tool.run {
                        lines.push(format!(
                            "  运行记录 {} 退出码 {:?} 超时 {} 报告 {}",
                            run.run_id, run.exit_code, run.timed_out, run.report_file
                        ));
                    }
                }
                lines.join("\n")
            }),
    )
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
                let work_queue = session
                    .plan_context
                    .as_ref()
                    .and_then(|context| context.work_queue.as_ref())
                    .map(format_work_queue_context_summary)
                    .unwrap_or_else(|| "协作任务板 无".to_string());
                format!(
                    "SelfForge Agent 会话已创建 {} 版本 {} 状态 {} 步骤 {} {} {} 文件 {}",
                    session.id,
                    session.version,
                    session.status,
                    session.steps.len(),
                    context,
                    work_queue,
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
    let session = app.agent_session(&command.version, &command.id)?;
    let audit = app.ai_self_upgrade_record_for_session(&command.version, &session.id)?;
    let mut lines = vec![format!(
        "SelfForge Agent 会话 {} 版本 {} 状态 {} 目标 {}",
        session.id, session.version, session.status, session.goal
    )];
    if let Some(audit) = audit {
        lines.push(format!(
            "自我升级审计 {} 状态 {} 目标 {} 候选 {} 当前稳定 {} 文件 {}",
            audit.id,
            audit.status,
            audit.proposed_goal.as_deref().unwrap_or("无"),
            audit.candidate_version.as_deref().unwrap_or("无"),
            audit.stable_version_after.as_deref().unwrap_or("无"),
            audit.file.display()
        ));
    } else {
        lines.push("自我升级审计 无".to_string());
    }
    if let Some(context) = session.plan_context.as_ref() {
        lines.push(format_plan_context_summary(context));
        if let Some(queue) = context.work_queue.as_ref() {
            lines.push(format_work_queue_context_summary(queue));
        }
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
            step.order, step.agent_id, step.title, step.status, tools, step.verification
        ));
        if let Some(task_id) = step.work_task_id.as_deref() {
            let worker_id = step.work_worker_id.as_deref().unwrap_or("未知");
            lines.push(format!("   协作任务 {task_id} 工作线程 {worker_id}"));
        }
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
                    run.run_id, run.version, run.exit_code, run.timed_out, run.report_file
                )
            })
            .unwrap_or_default();
        lines.push(format!(
            "事件 {} 时间 {} 类型 {}{}{} 内容 {}",
            event.order, event.timestamp_unix_seconds, event.kind, step, run, event.message
        ));
    }
    lines.push(format!("文件 {}", session.file.display()));
    Ok(lines.join("\n"))
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

fn format_work_queue_context_summary(queue: &self_forge::AgentSessionWorkQueueContext) -> String {
    let created = if queue.created {
        "已创建"
    } else {
        "已复用"
    };
    format!(
        "协作任务板 {} 版本 {} 任务 {} 线程 {} 租约 {} 秒 文件 {}",
        created,
        queue.version,
        queue.task_count,
        queue.thread_count,
        queue.lease_duration_seconds,
        queue.queue_file
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

fn agent_self_upgrade(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_self_upgrade_args(arguments)?;
    if command.dry_run {
        return boxed(app.ai_self_upgrade_preview(&command.hint).map(|preview| {
            format!(
                "SelfForge AI 自我升级预览 当前版本 {} 提供商 {} 模型 {} 协议 {} 记忆来源 {} 优化建议 {} 用户提示 {} 提示词字节 {}",
                preview.current_version,
                preview.request.provider_id,
                preview.request.model,
                preview.request.protocol,
                preview.insights.source_versions.len(),
                preview.insights.optimization_suggestions.len(),
                preview.hint.as_deref().unwrap_or("无"),
                preview.prompt.len()
            )
        }));
    }

    boxed(app.ai_self_upgrade(&command.hint, command.timeout_ms).map(|report| {
        let prepared = report
            .evolution
            .prepared_candidate_version
            .as_deref()
            .unwrap_or("复用已有候选");
        format!(
            "SelfForge AI 自我升级完成 当前版本 {} 提供商 {} 模型 {} 目标 {} 会话 {} 准备 {} 候选版本 {} 结果 {:?} 当前稳定版本 {} 未解决错误 {} 审计记录 {} 审计文件 {} 总结报告 {} 报告文件 {}",
            report.preview.current_version,
            report.ai.response.provider_id,
            report.ai.response.model,
            report.proposed_goal,
            report.evolution.session.id,
            prepared,
            report.evolution.cycle.candidate_version,
            report.evolution.cycle.result,
            report.evolution.cycle.state.current_version,
            report.evolution.preflight.open_errors.len(),
            report.audit.id,
            report.audit.file.display(),
            report.summary.id,
            report.summary.markdown_file.display()
        )
    }))
}

fn agent_patch_draft(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_draft_args(arguments)?;
    if command.dry_run {
        return boxed(app.ai_patch_draft_preview(&command.goal).map(|preview| {
            format!(
                "SelfForge AI 补丁草案预览 当前版本 {} 目标版本 {} 提供商 {} 模型 {} 协议 {} 记忆来源 {} 允许写入 {} 必要章节 {} 用户目标 {} 提示词字节 {}",
                preview.current_version,
                preview.target_version,
                preview.request.provider_id,
                preview.request.model,
                preview.request.protocol,
                preview.insights.source_versions.len(),
                preview.allowed_write_roots.join("、"),
                preview.required_sections.join("、"),
                preview.goal,
                preview.prompt.len()
            )
        }));
    }

    boxed(
        app.ai_patch_draft(&command.goal, command.timeout_ms)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁草案完成 当前版本 {} 目标版本 {} 提供商 {} 模型 {} 目标 {} 记录 {} 记录文件 {} 草案文件 {} 响应字节 {}",
                    report.preview.current_version,
                    report.preview.target_version,
                    report.ai.response.provider_id,
                    report.ai.response.model,
                    report.preview.goal,
                    report.record.id,
                    report.record.file.display(),
                    report
                        .record
                        .draft_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.ai.response.raw_bytes
                )
            }),
    )
}

fn agent_patch_drafts(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_drafts_args(arguments)?;
    boxed(
        app.ai_patch_draft_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!("SelfForge AI 补丁草案记录 {}: no records", command.version);
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁草案记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 目标版本 {} 目标 {} 草案 {} 错误 {} 文件 {}",
                        record.id,
                        record.status,
                        record.target_version,
                        record.goal,
                        record
                            .draft_file
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "无".to_string()),
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_draft_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_draft_record_args(arguments)?;
    boxed(
        app.ai_patch_draft_record(&command.version, &command.id)
            .map(|record| {
                format!(
                    "SelfForge AI 补丁草案记录 {} 版本 {} 目标版本 {} 状态 {} 提供商 {} 模型 {} 协议 {} 目标 {} 允许写入 {} 必要章节 {} 草案 {} 错误 {} 文件 {}\nAI 响应摘要 {}",
                    record.id,
                    record.version,
                    record.target_version,
                    record.status,
                    record.provider_id,
                    record.model,
                    record.protocol,
                    record.goal,
                    record.allowed_write_roots.join("、"),
                    record.required_sections.join("、"),
                    record
                        .draft_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.error.as_deref().unwrap_or("无"),
                    record.file.display(),
                    record.ai_response_preview.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_audit(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_audit_args(arguments)?;
    boxed(
        app.ai_patch_audit(&command.version, &command.draft_id)
            .map(|report| {
                let queue_status = report
                    .queue
                    .as_ref()
                    .map(|queue| format!("{} 个任务", queue.queue.tasks.len()))
                    .unwrap_or_else(|| "无协作队列".to_string());
                let mut lines = vec![format!(
                    "SelfForge AI 补丁审计完成 版本 {} 草案 {} 目标版本 {} 状态 {} 写入范围 {} 冲突 {} 发现 {} 协作队列 {} 审计记录 {} 文件 {}",
                    report.record.version,
                    report.record.draft_id,
                    report.record.target_version,
                    report.record.status,
                    report.record.normalized_write_scope.len(),
                    report.record.active_conflict_count,
                    report.record.finding_count,
                    queue_status,
                    report.record.id,
                    report.record.file.display()
                )];
                for finding in &report.record.findings {
                    lines.push(format!(
                        "发现 {} {} 路径 {} 任务 {} 线程 {} 说明 {}",
                        finding.severity,
                        finding.kind,
                        finding.path.as_deref().unwrap_or("无"),
                        finding.task_id.as_deref().unwrap_or("无"),
                        finding.worker_id.as_deref().unwrap_or("无"),
                        finding.message
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_audits(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_audits_args(arguments)?;
    boxed(
        app.ai_patch_audit_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!("SelfForge AI 补丁审计记录 {}: no records", command.version);
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁审计记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 草案 {} 目标版本 {} 冲突 {} 发现 {} 文件 {}",
                        record.id,
                        record.status,
                        record.draft_id,
                        record.target_version,
                        record.active_conflict_count,
                        record.finding_count,
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_audit_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_audit_record_args(arguments)?;
    boxed(
        app.ai_patch_audit_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁审计记录 {} 版本 {} 草案 {} 目标版本 {} 状态 {} 写入范围 {} 受保护根 {} 冲突 {} 发现 {} 文件 {}",
                    record.id,
                    record.version,
                    record.draft_id,
                    record.target_version,
                    record.status,
                    record.normalized_write_scope.join("、"),
                    record.protected_roots.join("、"),
                    record.active_conflict_count,
                    record.finding_count,
                    record.file.display()
                )];
                for finding in &record.findings {
                    lines.push(format!(
                        "发现 {} {} 路径 {} 任务 {} 线程 {} 说明 {}",
                        finding.severity,
                        finding.kind,
                        finding.path.as_deref().unwrap_or("无"),
                        finding.task_id.as_deref().unwrap_or("无"),
                        finding.worker_id.as_deref().unwrap_or("无"),
                        finding.message
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_preview(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_preview_args(arguments)?;
    boxed(
        app.ai_patch_preview(&command.version, &command.audit_id)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁应用预演完成 版本 {} 审计 {} 草案 {} 目标版本 {} 状态 {} 变更 {} 代码块 {} 预演 {} JSON {} 错误 {}",
                    report.record.version,
                    report.record.audit_id,
                    report.record.draft_id,
                    report.record.target_version,
                    report.record.status,
                    report.record.change_count,
                    report.record.code_block_count,
                    report
                        .record
                        .preview_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.file.display(),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_previews(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_previews_args(arguments)?;
    boxed(
        app.ai_patch_preview_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 补丁应用预演记录 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁应用预演记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 审计 {} 草案 {} 目标版本 {} 变更 {} 预演 {} 错误 {} 文件 {}",
                        record.id,
                        record.status,
                        record.audit_id,
                        record.draft_id,
                        record.target_version,
                        record.change_count,
                        record
                            .preview_file
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "无".to_string()),
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_preview_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_preview_record_args(arguments)?;
    boxed(
        app.ai_patch_preview_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁应用预演 {} 版本 {} 审计 {} 草案 {} 目标版本 {} 状态 {} 写入范围 {} 变更 {} 代码块 {} 预演 {} JSON {} 错误 {}",
                    record.id,
                    record.version,
                    record.audit_id,
                    record.draft_id,
                    record.target_version,
                    record.status,
                    record.normalized_write_scope.join("、"),
                    record.change_count,
                    record.code_block_count,
                    record
                        .preview_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.file.display(),
                    record.error.as_deref().unwrap_or("无")
                )];
                for change in &record.changes {
                    lines.push(format!(
                        "预演变更 路径 {} 代码块 {} 语言 {} 字节 {} 摘要 {}",
                        change.path,
                        change.code_block_index,
                        change.language.as_deref().unwrap_or("未标注"),
                        change.content_bytes,
                        change.content_preview
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_apply(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_apply_args(arguments)?;
    boxed(
        app.ai_patch_apply(&command.version, &command.preview_id)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁候选应用完成 版本 {} 候选版本 {} 预演 {} 状态 {} 准备候选 {} 应用文件 {} 应用目录 {} 记录 {} 报告 {} 错误 {}",
                    report.record.version,
                    report.record.candidate_version,
                    report.record.preview_id,
                    report.record.status,
                    report
                        .prepared_candidate_version
                        .as_deref()
                        .unwrap_or("复用已有候选"),
                    report.record.applied_file_count,
                    report
                        .record
                        .application_dir
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.file.display(),
                    report
                        .record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_verify(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_verify_args(arguments)?;
    boxed(
        app.ai_patch_verify(&command.version, &command.id, command.timeout_ms)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁候选应用验证完成 版本 {} 记录 {} 状态 {} 执行命令 {} 验证状态 {} 报告 {} 错误 {}",
                    report.record.version,
                    report.record.id,
                    report.record.status,
                    report.executed_count,
                    report.status,
                    report
                        .record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_source_plan(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_plan_args(arguments)?;
    boxed(
        app.ai_patch_source_plan(&command.version, &command.application_id)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁源码覆盖准备完成 版本 {} 应用 {} 状态 {} 文件 {} 记录 {} 报告 {} 错误 {}",
                    report.record.version,
                    report.record.application_id,
                    report.record.status,
                    report.record.files.len(),
                    report.record.file.display(),
                    report
                        .record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_source_plans(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_plans_args(arguments)?;
    boxed(
        app.ai_patch_source_plan_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 补丁源码覆盖准备记录 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖准备记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 应用 {} 文件 {} 错误 {} JSON {}",
                        record.id,
                        record.status,
                        record.application_id,
                        record.file_count,
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_plan_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_plan_record_args(arguments)?;
    boxed(
        app.ai_patch_source_plan_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖准备 {} 版本 {} 应用 {} 状态 {} 文件 {} 报告 {} JSON {} 错误 {}",
                    record.id,
                    record.version,
                    record.application_id,
                    record.status,
                    record.files.len(),
                    record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.file.display(),
                    record.error.as_deref().unwrap_or("无")
                )];
                for file in &record.files {
                    lines.push(format!(
                        "覆盖文件 来源 {} 镜像 {} 目标 {} 原始字节 {} 新字节 {} 回滚 {}",
                        file.source_path,
                        file.mirror_file.display(),
                        file.target_file.display(),
                        file.original_bytes,
                        file.new_bytes,
                        file.rollback_action
                    ));
                }
                for step in &record.rollback_steps {
                    lines.push(format!("回滚步骤 {step}"));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_execute(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_execute_args(arguments)?;
    boxed(
        app.ai_patch_source_execute(&command.version, &command.source_plan_id, command.timeout_ms)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁源码覆盖执行完成 版本 {} 准备 {} 状态 {} 文件 {} 验证 {} 回滚 {} 记录 {} 报告 {} 错误 {}",
                    report.record.version,
                    report.record.source_plan_id,
                    report.record.status,
                    report.record.files.len(),
                    report.record.verification_status,
                    if report.record.rollback_performed { "是" } else { "否" },
                    report.record.file.display(),
                    report
                        .record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_source_executions(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_executions_args(arguments)?;
    boxed(
        app.ai_patch_source_execution_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 补丁源码覆盖执行记录 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖执行记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 准备 {} 文件 {} 验证 {} 回滚 {} 错误 {} JSON {}",
                        record.id,
                        record.status,
                        record.source_plan_id,
                        record.file_count,
                        record.verification_status,
                        if record.rollback_performed {
                            "是"
                        } else {
                            "否"
                        },
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_execution_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_execution_record_args(arguments)?;
    boxed(
        app.ai_patch_source_execution_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖执行 {} 版本 {} 准备 {} 状态 {} 文件 {} 验证 {} 回滚 {} 报告 {} JSON {} 错误 {}",
                    record.id,
                    record.version,
                    record.source_plan_id,
                    record.status,
                    record.files.len(),
                    record.verification_status,
                    if record.rollback_performed { "是" } else { "否" },
                    record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.file.display(),
                    record.error.as_deref().unwrap_or("无")
                )];
                for file in &record.files {
                    lines.push(format!(
                        "覆盖文件 来源 {} 镜像 {} 目标 {} 动作 {} 覆盖前字节 {} 覆盖后字节 {} 备份 {} 回滚 {}",
                        file.source_path,
                        file.mirror_file.display(),
                        file.target_file.display(),
                        file.action,
                        file.before_bytes,
                        file.after_bytes,
                        file.execution_backup_file
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "无".to_string()),
                        file.rollback_action
                    ));
                }
                for run in &record.verification_runs {
                    lines.push(format!(
                        "验证命令 {} 状态 {} 退出码 {:?} 超时 {} 耗时毫秒 {}",
                        run.command, run.status, run.exit_code, run.timed_out, run.duration_ms
                    ));
                }
                for step in &record.rollback_steps {
                    lines.push(format!("回滚记录 {step}"));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_promotion(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_promotion_args(arguments)?;
    boxed(
        app.ai_patch_source_promotion(&command.version, &command.source_execution_id)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁源码覆盖提升衔接完成 版本 {} 执行 {} 状态 {} 下一候选 {} 验证 {} 提交 {} 记录 {} 报告 {} 错误 {}",
                    report.record.version,
                    report.record.source_execution_id,
                    report.record.status,
                    report.record.next_candidate_version,
                    report.record.verification_status,
                    report
                        .record
                        .suggested_commit_title
                        .as_deref()
                        .unwrap_or("无"),
                    report.record.file.display(),
                    report
                        .record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_source_promotions(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_promotions_args(arguments)?;
    boxed(
        app.ai_patch_source_promotion_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 补丁源码覆盖提升衔接记录 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖提升衔接记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 执行 {} 下一候选 {} 验证 {} 验证运行 {} 文件 {} 错误 {} JSON {}",
                        record.id,
                        record.status,
                        record.source_execution_id,
                        record.next_candidate_version,
                        record.verification_status,
                        record.verification_run_count,
                        record.file_count,
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_promotion_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_promotion_record_args(arguments)?;
    boxed(
        app.ai_patch_source_promotion_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖提升衔接 {} 版本 {} 执行 {} 状态 {} 下一候选 {} 验证 {} 验证运行 {} 文件 {} 提交 {} 报告 {} JSON {} 错误 {}",
                    record.id,
                    record.version,
                    record.source_execution_id,
                    record.status,
                    record.next_candidate_version,
                    record.verification_status,
                    record.verification_run_count,
                    record.file_count,
                    record
                        .suggested_commit_title
                        .as_deref()
                        .unwrap_or("无"),
                    record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.file.display(),
                    record.error.as_deref().unwrap_or("无")
                )];
                lines.push(format!("下一候选目标 {}", record.next_candidate_goal));
                for check in &record.readiness_checks {
                    lines.push(format!("就绪检查 {check}"));
                }
                for command in &record.verification_commands {
                    lines.push(format!("验证命令 {command}"));
                }
                for file in &record.changed_files {
                    lines.push(format!("变更文件 {file}"));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_candidate(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_candidate_args(arguments)?;
    boxed(
        app.ai_patch_source_candidate(&command.version, &command.promotion_id)
            .map(|report| {
                format!(
                    "SelfForge AI 补丁源码覆盖候选准备完成 版本 {} 衔接 {} 状态 {} 候选 {} 状态变化 {} -> {} 验证路径 {} 记录 {} 报告 {} 错误 {}",
                    report.record.version,
                    report.record.promotion_id,
                    report.record.status,
                    report.record.candidate_version,
                    report.record.state_status_before,
                    report.record.state_status_after,
                    report.record.candidate_checked_path_count,
                    report.record.file.display(),
                    report
                        .record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    report.record.error.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_patch_source_candidates(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_candidates_args(arguments)?;
    boxed(
        app.ai_patch_source_candidate_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 补丁源码覆盖候选准备记录 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖候选准备记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 衔接 {} 候选 {} 准备后候选 {} 错误 {} JSON {}",
                        record.id,
                        record.status,
                        record.promotion_id,
                        record.candidate_version,
                        record.candidate_version_after.as_deref().unwrap_or("无"),
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_source_candidate_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_source_candidate_record_args(arguments)?;
    boxed(
        app.ai_patch_source_candidate_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁源码覆盖候选准备 {} 版本 {} 衔接 {} 状态 {} 候选 {} 工作区 {} 验证路径 {} 报告 {} JSON {} 错误 {}",
                    record.id,
                    record.version,
                    record.promotion_id,
                    record.status,
                    record.candidate_version,
                    record
                        .candidate_workspace
                        .as_deref()
                        .unwrap_or("无"),
                    record.candidate_checked_path_count,
                    record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.file.display(),
                    record.error.as_deref().unwrap_or("无")
                )];
                lines.push(format!(
                    "状态变化 {}:{} -> {}:{}",
                    record.stable_version_before,
                    record.state_status_before,
                    record.stable_version_after,
                    record.state_status_after
                ));
                for check in &record.readiness_checks {
                    lines.push(format!("就绪检查 {check}"));
                }
                for command in &record.follow_up_commands {
                    lines.push(format!("后续命令 {command}"));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_applications(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_applications_args(arguments)?;
    boxed(
        app.ai_patch_application_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 补丁候选应用记录 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 补丁候选应用记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 验证 {} 候选版本 {} 预演 {} 应用文件 {} 应用目录 {} 错误 {} 文件 {}",
                        record.id,
                        record.status,
                        record.verification_status,
                        record.candidate_version,
                        record.preview_id,
                        record.applied_file_count,
                        record
                            .application_dir
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "无".to_string()),
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_patch_application_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_patch_application_record_args(arguments)?;
    boxed(
        app.ai_patch_application_record(&command.version, &command.id)
            .map(|record| {
                let mut lines = vec![format!(
                    "SelfForge AI 补丁候选应用 {} 版本 {} 候选版本 {} 预演 {} 审计 {} 草案 {} 状态 {} 应用文件 {} 应用目录 {} 报告 {} JSON {} 错误 {}",
                    record.id,
                    record.version,
                    record.candidate_version,
                    record.preview_id,
                    record.audit_id,
                    record.draft_id,
                    record.status,
                    record.applied_file_count,
                    record
                        .application_dir
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record
                        .report_file
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "无".to_string()),
                    record.file.display(),
                    record.error.as_deref().unwrap_or("无")
                )];
                for file in &record.files {
                    lines.push(format!(
                        "应用文件 来源 {} 镜像 {} 字节 {}",
                        file.source_path,
                        file.mirror_file.display(),
                        file.content_bytes
                    ));
                }
                for command in &record.verification_commands {
                    lines.push(format!("验证命令 {command}"));
                }
                for run in &record.verification_runs {
                    lines.push(format!(
                        "验证结果 命令 {} 状态 {} 退出码 {} 超时 {} 耗时毫秒 {}",
                        run.command,
                        run.status,
                        run.exit_code
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "无".to_string()),
                        if run.timed_out { "是" } else { "否" },
                        run.duration_ms
                    ));
                }
                lines.push(format!("验证状态 {}", record.verification_status));
                lines.push(format!("回滚提示 {}", record.rollback_hint));
                lines.join("\n")
            }),
    )
}

fn agent_self_upgrades(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_self_upgrades_args(arguments)?;
    boxed(
        app.ai_self_upgrade_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!("SelfForge AI 自我升级记录 {}: no records", command.version);
                }

                let mut lines = vec![format!(
                    "SelfForge AI 自我升级记录 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 目标 {} 会话 {} 候选 {} 当前稳定 {} 错误 {} 文件 {}",
                        record.id,
                        record.status,
                        record.proposed_goal.as_deref().unwrap_or("无"),
                        record.session_id.as_deref().unwrap_or("无"),
                        record.candidate_version.as_deref().unwrap_or("无"),
                        record.stable_version_after.as_deref().unwrap_or("无"),
                        record.error.as_deref().unwrap_or("无"),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_self_upgrade_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_self_upgrade_record_args(arguments)?;
    boxed(
        app.ai_self_upgrade_record(&command.version, &command.id)
            .map(|record| {
                format!(
                    "SelfForge AI 自我升级记录 {} 版本 {} 状态 {} 提供商 {} 模型 {} 协议 {} 目标 {} 会话 {} 候选 {} 当前稳定 {} 记忆来源 {} 错误 {} 文件 {}\nAI 响应摘要 {}",
                    record.id,
                    record.version,
                    record.status,
                    record.provider_id,
                    record.model,
                    record.protocol,
                    record.proposed_goal.as_deref().unwrap_or("无"),
                    record.session_id.as_deref().unwrap_or("无"),
                    record.candidate_version.as_deref().unwrap_or("无"),
                    record.stable_version_after.as_deref().unwrap_or("无"),
                    record.memory_source_versions.join("、"),
                    record.error.as_deref().unwrap_or("无"),
                    record.file.display(),
                    record.ai_response_preview.as_deref().unwrap_or("无")
                )
            }),
    )
}

fn agent_self_upgrade_report(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_self_upgrade_report_args(arguments)?;
    boxed(
        app.ai_self_upgrade_summary(&command.version, &command.audit_id)
            .map(|report| {
                format!(
                    "SelfForge AI 自我升级总结报告完成 版本 {} 审计 {} 状态 {} 目标 {} 会话 {} 当前稳定 {} 报告 {} Markdown {} JSON {}",
                    report.record.version,
                    report.record.audit_id,
                    report.record.status,
                    report.record.proposed_goal.as_deref().unwrap_or("无"),
                    report.record.session_id.as_deref().unwrap_or("无"),
                    report.record.stable_version_after.as_deref().unwrap_or("无"),
                    report.record.id,
                    report.record.markdown_file.display(),
                    report.record.file.display()
                )
            }),
    )
}

fn agent_self_upgrade_reports(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_self_upgrade_reports_args(arguments)?;
    boxed(
        app.ai_self_upgrade_summary_records(&command.version, command.limit)
            .map(|records| {
                if records.is_empty() {
                    return format!(
                        "SelfForge AI 自我升级总结报告 {}: no records",
                        command.version
                    );
                }

                let mut lines = vec![format!(
                    "SelfForge AI 自我升级总结报告 {}: {} record(s)",
                    command.version,
                    records.len()
                )];
                for record in records {
                    lines.push(format!(
                        "{} 状态 {} 审计 {} 目标 {} 会话 {} 当前稳定 {} Markdown {} JSON {}",
                        record.id,
                        record.status,
                        record.audit_id,
                        record.proposed_goal.as_deref().unwrap_or("无"),
                        record.session_id.as_deref().unwrap_or("无"),
                        record.stable_version_after.as_deref().unwrap_or("无"),
                        record.markdown_file.display(),
                        record.file.display()
                    ));
                }
                lines.join("\n")
            }),
    )
}

fn agent_self_upgrade_report_record(
    app: &SelfForgeApp,
    arguments: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    let command = parse_agent_self_upgrade_report_record_args(arguments)?;
    boxed(
        app.ai_self_upgrade_summary_record(&command.version, &command.id)
            .map(|record| {
                format!(
                    "SelfForge AI 自我升级总结报告 {} 版本 {} 审计 {} 状态 {} 目标 {} 会话 {} 候选 {} 当前稳定 {} Markdown {} JSON {}",
                    record.id,
                    record.version,
                    record.audit_id,
                    record.status,
                    record.proposed_goal.as_deref().unwrap_or("无"),
                    record.session_id.as_deref().unwrap_or("无"),
                    record.candidate_version.as_deref().unwrap_or("无"),
                    record.stable_version_after.as_deref().unwrap_or("无"),
                    record.markdown_file.display(),
                    record.file.display()
                )
            }),
    )
}

struct AiRequestArgs {
    dry_run: bool,
    timeout_ms: u64,
    prompt: String,
}

struct AgentSelfUpgradeArgs {
    dry_run: bool,
    timeout_ms: u64,
    hint: String,
}

struct AgentPatchDraftArgs {
    dry_run: bool,
    timeout_ms: u64,
    goal: String,
}

struct AgentPatchDraftsArgs {
    version: String,
    limit: usize,
}

struct AgentPatchDraftRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchAuditArgs {
    version: String,
    draft_id: String,
}

struct AgentPatchAuditsArgs {
    version: String,
    limit: usize,
}

struct AgentPatchAuditRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchPreviewArgs {
    version: String,
    audit_id: String,
}

struct AgentPatchPreviewsArgs {
    version: String,
    limit: usize,
}

struct AgentPatchPreviewRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchApplyArgs {
    version: String,
    preview_id: String,
}

struct AgentPatchVerifyArgs {
    version: String,
    id: String,
    timeout_ms: u64,
}

struct AgentPatchSourcePlanArgs {
    version: String,
    application_id: String,
}

struct AgentPatchSourcePlansArgs {
    version: String,
    limit: usize,
}

struct AgentPatchSourcePlanRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchSourceExecuteArgs {
    version: String,
    timeout_ms: u64,
    source_plan_id: String,
}

struct AgentPatchSourceExecutionsArgs {
    version: String,
    limit: usize,
}

struct AgentPatchSourceExecutionRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchSourcePromotionArgs {
    version: String,
    source_execution_id: String,
}

struct AgentPatchSourcePromotionsArgs {
    version: String,
    limit: usize,
}

struct AgentPatchSourcePromotionRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchSourceCandidateArgs {
    version: String,
    promotion_id: String,
}

struct AgentPatchSourceCandidatesArgs {
    version: String,
    limit: usize,
}

struct AgentPatchSourceCandidateRecordArgs {
    version: String,
    id: String,
}

struct AgentPatchApplicationsArgs {
    version: String,
    limit: usize,
}

struct AgentPatchApplicationRecordArgs {
    version: String,
    id: String,
}

struct AgentSelfUpgradesArgs {
    version: String,
    limit: usize,
}

struct AgentSelfUpgradeRecordArgs {
    version: String,
    id: String,
}

struct AgentSelfUpgradeReportArgs {
    version: String,
    audit_id: String,
}

struct AgentSelfUpgradeReportsArgs {
    version: String,
    limit: usize,
}

struct AgentSelfUpgradeReportRecordArgs {
    version: String,
    id: String,
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

fn parse_agent_self_upgrade_args(
    arguments: Vec<String>,
) -> Result<AgentSelfUpgradeArgs, Box<dyn Error>> {
    let mut dry_run = false;
    let mut timeout_ms = DEFAULT_AI_TIMEOUT_MS;
    let mut hint_parts = Vec::new();
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
                hint_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-self-upgrade 参数: {other}").into());
            }
            _ => {
                hint_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    Ok(AgentSelfUpgradeArgs {
        dry_run,
        timeout_ms,
        hint: hint_parts.join(" "),
    })
}

fn parse_agent_patch_draft_args(
    arguments: Vec<String>,
) -> Result<AgentPatchDraftArgs, Box<dyn Error>> {
    let mut dry_run = false;
    let mut timeout_ms = DEFAULT_AI_TIMEOUT_MS;
    let mut goal_parts = Vec::new();
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
                goal_parts.extend(arguments[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-patch-draft 参数: {other}").into());
            }
            _ => {
                goal_parts.extend(arguments[index..].iter().cloned());
                break;
            }
        }
    }

    let goal = if goal_parts.is_empty() {
        "生成下一轮 AI 补丁草案".to_string()
    } else {
        goal_parts.join(" ")
    };

    Ok(AgentPatchDraftArgs {
        dry_run,
        timeout_ms,
        goal,
    })
}

fn parse_agent_patch_drafts_args(
    arguments: Vec<String>,
) -> Result<AgentPatchDraftsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-patch-drafts 参数: {other}").into());
            }
            other => {
                return Err(format!("未知 agent-patch-drafts 参数: {other}").into());
            }
        }
    }

    Ok(AgentPatchDraftsArgs { version, limit })
}

fn parse_agent_patch_draft_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchDraftRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-patch-draft-record 参数: {other}").into());
            }
            other => {
                if id.is_none() {
                    id = Some(other.to_string());
                    index += 1;
                } else {
                    return Err("agent-patch-draft-record 只允许一个记录编号".into());
                }
            }
        }
    }

    Ok(AgentPatchDraftRecordArgs {
        version,
        id: id.ok_or("agent-patch-draft-record 需要记录编号")?,
    })
}

fn parse_agent_patch_audit_args(
    arguments: Vec<String>,
) -> Result<AgentPatchAuditArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut draft_id = None;
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
                return Err(format!("未知 agent-patch-audit 参数: {other}").into());
            }
            other => {
                if draft_id.is_some() {
                    return Err("agent-patch-audit 只允许一个草案记录编号".into());
                }
                draft_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchAuditArgs {
        version,
        draft_id: draft_id.ok_or("agent-patch-audit 需要草案记录编号")?,
    })
}

fn parse_agent_patch_audits_args(
    arguments: Vec<String>,
) -> Result<AgentPatchAuditsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-audits 参数: {other}").into()),
        }
    }

    Ok(AgentPatchAuditsArgs { version, limit })
}

fn parse_agent_patch_audit_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchAuditRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-patch-audit-record 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-audit-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchAuditRecordArgs {
        version,
        id: id.ok_or("agent-patch-audit-record 需要记录编号")?,
    })
}

fn parse_agent_patch_preview_args(
    arguments: Vec<String>,
) -> Result<AgentPatchPreviewArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut audit_id = None;
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
                return Err(format!("未知 agent-patch-preview 参数: {other}").into());
            }
            other => {
                if audit_id.is_some() {
                    return Err("agent-patch-preview 只允许一个审计记录编号".into());
                }
                audit_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchPreviewArgs {
        version,
        audit_id: audit_id.ok_or("agent-patch-preview 需要审计记录编号")?,
    })
}

fn parse_agent_patch_previews_args(
    arguments: Vec<String>,
) -> Result<AgentPatchPreviewsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-previews 参数: {other}").into()),
        }
    }

    Ok(AgentPatchPreviewsArgs { version, limit })
}

fn parse_agent_patch_preview_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchPreviewRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-patch-preview-record 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-preview-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchPreviewRecordArgs {
        version,
        id: id.ok_or("agent-patch-preview-record 需要记录编号")?,
    })
}

fn parse_agent_patch_apply_args(
    arguments: Vec<String>,
) -> Result<AgentPatchApplyArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut preview_id = None;
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
                return Err(format!("未知 agent-patch-apply 参数: {other}").into());
            }
            other => {
                if preview_id.is_some() {
                    return Err("agent-patch-apply 只允许一个预演记录编号".into());
                }
                preview_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchApplyArgs {
        version,
        preview_id: preview_id.ok_or("agent-patch-apply 需要预演记录编号")?,
    })
}

fn parse_agent_patch_verify_args(
    arguments: Vec<String>,
) -> Result<AgentPatchVerifyArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut timeout_ms = DEFAULT_PATCH_VERIFICATION_TIMEOUT_MS;
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
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-patch-verify 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-verify 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchVerifyArgs {
        version,
        id: id.ok_or("agent-patch-verify 需要候选应用记录编号")?,
        timeout_ms,
    })
}

fn parse_agent_patch_source_plan_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourcePlanArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut application_id = None;
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
                return Err(format!("未知 agent-patch-source-plan 参数: {other}").into());
            }
            other => {
                if application_id.is_some() {
                    return Err("agent-patch-source-plan 只允许一个候选应用记录编号".into());
                }
                application_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourcePlanArgs {
        version,
        application_id: application_id.ok_or("agent-patch-source-plan 需要候选应用记录编号")?,
    })
}

fn parse_agent_patch_source_plans_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourcePlansArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-source-plans 参数: {other}").into()),
        }
    }

    Ok(AgentPatchSourcePlansArgs { version, limit })
}

fn parse_agent_patch_source_plan_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourcePlanRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-patch-source-plan-record 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-source-plan-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourcePlanRecordArgs {
        version,
        id: id.ok_or("agent-patch-source-plan-record 需要记录编号")?,
    })
}

fn parse_agent_patch_source_execute_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourceExecuteArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut timeout_ms = DEFAULT_PATCH_VERIFICATION_TIMEOUT_MS;
    let mut source_plan_id = None;
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
            "--timeout-ms" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--timeout-ms 需要毫秒数".into());
                };
                timeout_ms = value.parse::<u64>()?;
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-patch-source-execute 参数: {other}").into());
            }
            other => {
                if source_plan_id.is_some() {
                    return Err("agent-patch-source-execute 只允许一个源码覆盖准备记录编号".into());
                }
                source_plan_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourceExecuteArgs {
        version,
        timeout_ms,
        source_plan_id: source_plan_id
            .ok_or("agent-patch-source-execute 需要源码覆盖准备记录编号")?,
    })
}

fn parse_agent_patch_source_executions_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourceExecutionsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-source-executions 参数: {other}").into()),
        }
    }

    Ok(AgentPatchSourceExecutionsArgs { version, limit })
}

fn parse_agent_patch_source_execution_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourceExecutionRecordArgs, Box<dyn Error>> {
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
                return Err(
                    format!("未知 agent-patch-source-execution-record 参数: {other}").into(),
                );
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-source-execution-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourceExecutionRecordArgs {
        version,
        id: id.ok_or("agent-patch-source-execution-record 需要记录编号")?,
    })
}

fn parse_agent_patch_source_promotion_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourcePromotionArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut source_execution_id = None;
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
                return Err(format!("未知 agent-patch-source-promotion 参数: {other}").into());
            }
            other => {
                if source_execution_id.is_some() {
                    return Err(
                        "agent-patch-source-promotion 只允许一个源码覆盖执行记录编号".into(),
                    );
                }
                source_execution_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourcePromotionArgs {
        version,
        source_execution_id: source_execution_id
            .ok_or("agent-patch-source-promotion 需要源码覆盖执行记录编号")?,
    })
}

fn parse_agent_patch_source_promotions_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourcePromotionsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-source-promotions 参数: {other}").into()),
        }
    }

    Ok(AgentPatchSourcePromotionsArgs { version, limit })
}

fn parse_agent_patch_source_promotion_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourcePromotionRecordArgs, Box<dyn Error>> {
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
                return Err(
                    format!("未知 agent-patch-source-promotion-record 参数: {other}").into(),
                );
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-source-promotion-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourcePromotionRecordArgs {
        version,
        id: id.ok_or("agent-patch-source-promotion-record 需要记录编号")?,
    })
}

fn parse_agent_patch_source_candidate_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourceCandidateArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut promotion_id = None;
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
                return Err(format!("未知 agent-patch-source-candidate 参数: {other}").into());
            }
            other => {
                if promotion_id.is_some() {
                    return Err("agent-patch-source-candidate 只允许一个提升衔接记录编号".into());
                }
                promotion_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourceCandidateArgs {
        version,
        promotion_id: promotion_id.ok_or("agent-patch-source-candidate 需要提升衔接记录编号")?,
    })
}

fn parse_agent_patch_source_candidates_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourceCandidatesArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-source-candidates 参数: {other}").into()),
        }
    }

    Ok(AgentPatchSourceCandidatesArgs { version, limit })
}

fn parse_agent_patch_source_candidate_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchSourceCandidateRecordArgs, Box<dyn Error>> {
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
                return Err(
                    format!("未知 agent-patch-source-candidate-record 参数: {other}").into(),
                );
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-source-candidate-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchSourceCandidateRecordArgs {
        version,
        id: id.ok_or("agent-patch-source-candidate-record 需要记录编号")?,
    })
}

fn parse_agent_patch_applications_args(
    arguments: Vec<String>,
) -> Result<AgentPatchApplicationsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-patch-applications 参数: {other}").into()),
        }
    }

    Ok(AgentPatchApplicationsArgs { version, limit })
}

fn parse_agent_patch_application_record_args(
    arguments: Vec<String>,
) -> Result<AgentPatchApplicationRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-patch-application-record 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-patch-application-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentPatchApplicationRecordArgs {
        version,
        id: id.ok_or("agent-patch-application-record 需要记录编号")?,
    })
}

fn parse_agent_self_upgrades_args(
    arguments: Vec<String>,
) -> Result<AgentSelfUpgradesArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-self-upgrades 参数: {other}").into()),
        }
    }

    Ok(AgentSelfUpgradesArgs { version, limit })
}

fn parse_agent_self_upgrade_record_args(
    arguments: Vec<String>,
) -> Result<AgentSelfUpgradeRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-self-upgrade-record 参数: {other}").into());
            }
            value => {
                id = Some(value.to_string());
                index += 1;
            }
        }
    }

    let id = id.ok_or("agent-self-upgrade-record 需要记录编号")?;
    Ok(AgentSelfUpgradeRecordArgs { version, id })
}

fn parse_agent_self_upgrade_report_args(
    arguments: Vec<String>,
) -> Result<AgentSelfUpgradeReportArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut audit_id = None;
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
                return Err(format!("未知 agent-self-upgrade-report 参数: {other}").into());
            }
            other => {
                if audit_id.is_some() {
                    return Err("agent-self-upgrade-report 只允许一个审计记录编号".into());
                }
                audit_id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentSelfUpgradeReportArgs {
        version,
        audit_id: audit_id.ok_or("agent-self-upgrade-report 需要审计记录编号")?,
    })
}

fn parse_agent_self_upgrade_reports_args(
    arguments: Vec<String>,
) -> Result<AgentSelfUpgradeReportsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut limit = 10;
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
            other => return Err(format!("未知 agent-self-upgrade-reports 参数: {other}").into()),
        }
    }

    Ok(AgentSelfUpgradeReportsArgs { version, limit })
}

fn parse_agent_self_upgrade_report_record_args(
    arguments: Vec<String>,
) -> Result<AgentSelfUpgradeReportRecordArgs, Box<dyn Error>> {
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
                return Err(format!("未知 agent-self-upgrade-report-record 参数: {other}").into());
            }
            other => {
                if id.is_some() {
                    return Err("agent-self-upgrade-report-record 只允许一个记录编号".into());
                }
                id = Some(other.to_string());
                index += 1;
            }
        }
    }

    Ok(AgentSelfUpgradeReportRecordArgs {
        version,
        id: id.ok_or("agent-self-upgrade-report-record 需要记录编号")?,
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
    lease_seconds: Option<u64>,
}

struct AgentWorkUpdateArgs {
    version: String,
    task_id: String,
    worker_id: String,
    text: String,
}

struct AgentWorkReapArgs {
    version: String,
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

struct AgentStepsArgs {
    request: AgentStepExecutionRequest,
    max_steps: usize,
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
    let mut lease_seconds = None;
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
            "--lease-seconds" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--lease-seconds 需要秒数".into());
                };
                lease_seconds = Some(value.parse::<u64>()?);
                index += 2;
            }
            other => return Err(format!("未知 agent-work-claim 参数: {other}").into()),
        }
    }

    Ok(AgentWorkClaimArgs {
        version,
        worker_id,
        preferred_agent_id,
        lease_seconds,
    })
}

fn parse_agent_work_reap_args(arguments: Vec<String>) -> Result<AgentWorkReapArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut version = state.current_version.clone();
    let mut text = "租约过期，任务自动释放。".to_string();
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
            "--reason" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--reason 需要说明文本".into());
                };
                text = value.clone();
                index += 2;
            }
            other => return Err(format!("未知 agent-work-reap 参数: {other}").into()),
        }
    }

    Ok(AgentWorkReapArgs { version, text })
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

fn parse_agent_steps_args(arguments: Vec<String>) -> Result<AgentStepsArgs, Box<dyn Error>> {
    let state = ForgeState::load(env::current_dir()?)?;
    let mut session_version = state.current_version.clone();
    let mut target_version = state.current_version.clone();
    let mut session_id = None;
    let mut limit = 5;
    let mut timeout_ms = 30_000;
    let mut max_steps = 10;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--" => {
                return Err("agent-steps 不支持 -- PROGRAM；遇到 Runtime 命令需求时会停止".into());
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
            "--max-steps" => {
                let Some(value) = arguments.get(index + 1) else {
                    return Err("--max-steps 需要数量".into());
                };
                max_steps = value.parse::<usize>()?;
                index += 2;
            }
            "--tool" => {
                return Err("agent-steps 不支持 --tool；多步运行会自动选择无外部输入工具".into());
            }
            "--prompt" => {
                return Err("agent-steps 不支持 --prompt；遇到 AI 提示词需求时会停止".into());
            }
            other if other.starts_with("--") => {
                return Err(format!("未知 agent-steps 参数: {other}").into());
            }
            other => {
                if session_id.is_none() {
                    session_id = Some(other.to_string());
                    index += 1;
                } else {
                    return Err("agent-steps 只允许一个会话标识".into());
                }
            }
        }
    }

    Ok(AgentStepsArgs {
        request: AgentStepExecutionRequest {
            session_version,
            session_id: session_id.ok_or("agent-steps 需要会话标识")?,
            target_version,
            tool_id: None,
            limit,
            program: None,
            args: Vec::new(),
            timeout_ms,
            prompt: None,
        },
        max_steps,
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
    "SelfForge commands:
init, validate, status, preflight
memory-context [--current|--candidate|--version VERSION] [--limit N]
memory-insights [--current|--candidate|--version VERSION] [--limit N]
memory-compact [--current|--candidate|--version VERSION] [--keep N]
ai-config, ai-request [--dry-run] [--timeout-ms N] [prompt]
agents, agent-tools [--current|--candidate|--version VERSION] [--init]
agent-work-init [--current|--candidate|--version VERSION] [--threads N] [goal]
agent-work-status [--current|--candidate|--version VERSION]
agent-work-claim [--current|--candidate|--version VERSION] [--worker ID] [--agent AGENT_ID] [--lease-seconds N]
agent-work-complete [--current|--candidate|--version VERSION] TASK_ID [--worker ID] [--summary TEXT]
agent-work-release [--current|--candidate|--version VERSION] TASK_ID [--worker ID] [--reason TEXT]
agent-work-reap [--current|--candidate|--version VERSION] [--reason TEXT]
agent-tool-run TOOL_ID --agent AGENT_ID [--current|--candidate|--version VERSION] [--limit N] [--all] [--session SESSION_ID] [--session-version VERSION] [--step N] [--target-version VERSION] [--timeout-ms N] [--prompt TEXT] [-- PROGRAM ARGS...]
agent-step [--session-version VERSION] [--target-version VERSION] [--tool TOOL_ID] [--limit N] [--timeout-ms N] [--prompt TEXT] SESSION_ID [-- PROGRAM ARGS...]
agent-steps [--session-version VERSION] [--target-version VERSION] [--limit N] [--timeout-ms N] [--max-steps N] SESSION_ID
agent-plan [--current|--candidate|--version VERSION] [--limit N] [goal]
agent-start [--current|--candidate|--version VERSION] [goal]
agent-sessions [--current|--candidate|--version VERSION] [--limit N] [--all]
agent-session [--current|--candidate|--version VERSION] SESSION_ID
agent-run [--session-version VERSION] [--current|--candidate|--version VERSION] [--step N] [--timeout-ms N] SESSION_ID -- PROGRAM [ARGS...]
agent-verify [--current|--candidate|--version VERSION] [--timeout-ms N] [goal] -- PROGRAM [ARGS...]
agent-advance [goal], agent-evolve [goal]
agent-patch-draft [--dry-run] [--timeout-ms N] [goal]
agent-patch-drafts [--current|--candidate|--version VERSION] [--limit N]
agent-patch-draft-record [--current|--candidate|--version VERSION] RECORD_ID
agent-patch-audit [--current|--candidate|--version VERSION] DRAFT_RECORD_ID
agent-patch-audits [--current|--candidate|--version VERSION] [--limit N]
agent-patch-audit-record [--current|--candidate|--version VERSION] AUDIT_RECORD_ID
agent-patch-preview [--current|--candidate|--version VERSION] AUDIT_RECORD_ID
agent-patch-previews [--current|--candidate|--version VERSION] [--limit N]
agent-patch-preview-record [--current|--candidate|--version VERSION] PREVIEW_RECORD_ID
agent-patch-apply [--current|--candidate|--version VERSION] PREVIEW_RECORD_ID
agent-patch-verify [--current|--candidate|--version VERSION] [--timeout-ms N] APPLICATION_RECORD_ID
agent-patch-source-plan [--current|--candidate|--version VERSION] APPLICATION_RECORD_ID
agent-patch-source-plans [--current|--candidate|--version VERSION] [--limit N]
agent-patch-source-plan-record [--current|--candidate|--version VERSION] SOURCE_PLAN_ID
agent-patch-source-execute [--current|--candidate|--version VERSION] [--timeout-ms N] SOURCE_PLAN_ID
agent-patch-source-executions [--current|--candidate|--version VERSION] [--limit N]
agent-patch-source-execution-record [--current|--candidate|--version VERSION] SOURCE_EXECUTION_ID
agent-patch-source-promotion [--current|--candidate|--version VERSION] SOURCE_EXECUTION_ID
agent-patch-source-promotions [--current|--candidate|--version VERSION] [--limit N]
agent-patch-source-promotion-record [--current|--candidate|--version VERSION] PROMOTION_ID
agent-patch-source-candidate [--current|--candidate|--version VERSION] PROMOTION_ID
agent-patch-source-candidates [--current|--candidate|--version VERSION] [--limit N]
agent-patch-source-candidate-record [--current|--candidate|--version VERSION] CANDIDATE_RECORD_ID
agent-patch-applications [--current|--candidate|--version VERSION] [--limit N]
agent-patch-application-record [--current|--candidate|--version VERSION] APPLICATION_RECORD_ID
agent-self-upgrade [--dry-run] [--timeout-ms N] [hint]
agent-self-upgrades [--current|--candidate|--version VERSION] [--limit N]
agent-self-upgrade-record [--current|--candidate|--version VERSION] RECORD_ID
agent-self-upgrade-report [--current|--candidate|--version VERSION] AUDIT_RECORD_ID
agent-self-upgrade-reports [--current|--candidate|--version VERSION] [--limit N]
agent-self-upgrade-report-record [--current|--candidate|--version VERSION] REPORT_RECORD_ID
advance [goal], promote, rollback [reason], cycle
run [--current|--candidate|--version VERSION] [--timeout-ms N] -- PROGRAM [ARGS...]
runs [--current|--candidate|--version VERSION] [--limit N] [--failed] [--timed-out]
errors [--current|--candidate|--version VERSION] [--limit N] [--open] [--resolved]
record-error [--current|--candidate|--version VERSION] [--run-id RUN_ID] [--stage TEXT] [--solution TEXT]
resolve-error [--current|--candidate|--version VERSION] --run-id RUN_ID [--verification TEXT]
evolve [--patch|--minor|--major] [goal]"
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
