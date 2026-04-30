use self_forge::{
    CURRENT_VERSION, CycleResult, ErrorArchive, ErrorListQuery, ForgeState, MinimalLoopOutcome,
    RunQuery, SelfForgeApp, Supervisor, VersionBump,
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
        "ai-config" => ai_config(&app),
        "ai-request" => ai_request(&app, args.collect()),
        "agents" => agents(&app),
        "agent-plan" => agent_plan(&app, args.collect()),
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

fn agent_plan(app: &SelfForgeApp, arguments: Vec<String>) -> Result<String, Box<dyn Error>> {
    let goal = arguments.join(" ");
    boxed(app.agent_plan(&goal).map(|plan| {
        let mut lines = vec![format!("SelfForge Agent 计划 目标 {}", plan.goal)];
        lines.push(format!("参与 Agent {}", plan.agents.len()));
        for step in plan.steps {
            lines.push(format!(
                "{}. [{}] {} 能力 {} 验证 {}",
                step.order, step.agent_id, step.title, step.capability, step.verification
            ));
        }
        lines.join("\n")
    }))
}

struct AiRequestArgs {
    dry_run: bool,
    timeout_ms: u64,
    prompt: String,
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

fn help_text() -> &'static str {
    "SelfForge commands: init, validate, status, preflight, ai-config, ai-request [--dry-run] [--timeout-ms N] [prompt], agents, agent-plan [goal], advance [goal], promote, rollback [reason], cycle, run [--current|--candidate|--version VERSION] [--timeout-ms N] -- PROGRAM [ARGS...], runs [--current|--candidate|--version VERSION] [--limit N] [--failed] [--timed-out], errors [--current|--candidate|--version VERSION] [--limit N] [--open] [--resolved], record-error [--current|--candidate|--version VERSION] [--run-id RUN_ID] [--stage TEXT] [--solution TEXT], resolve-error [--current|--candidate|--version VERSION] --run-id RUN_ID [--verification TEXT], evolve [--patch|--minor|--major] [goal]"
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
