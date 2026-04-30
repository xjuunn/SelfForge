use self_forge::{
    CURRENT_VERSION, CycleResult, ForgeState, MinimalLoopOutcome, SelfForgeApp, Supervisor,
    VersionBump,
};
use std::env;
use std::error::Error;
use std::process;

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

struct RunArgs {
    version: String,
    program: String,
    args: Vec<String>,
    timeout_ms: u64,
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

fn help_text() -> &'static str {
    "SelfForge commands: init, validate, status, advance [goal], promote, rollback [reason], cycle, run [--current|--candidate|--version VERSION] [--timeout-ms N] -- PROGRAM [ARGS...], evolve [--patch|--minor|--major] [goal]"
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
