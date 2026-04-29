use self_forge::{Supervisor, VersionBump};
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
        "evolve" => {
            let mut bump = VersionBump::Patch;
            let mut goal_parts = Vec::new();
            for argument in args {
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
            boxed(supervisor.prepare_next_version_with_bump(goal, bump).map(|report| {
                format!(
                    "SelfForge prepared {} from {}: {} paths checked, workspace {}, commit version {}",
                    report.next_version,
                    report.current_version,
                    report.candidate_validation.checked_paths.len(),
                    report.workspace.display(),
                    report.next_version
                )
            }))
        }
        "promote" => boxed(supervisor.promote_candidate().map(|report| {
            format!(
                "SelfForge promoted {} from {}, current workspace {}",
                report.promoted_version, report.previous_version, report.state.workspace
            )
        })),
        "help" | "-h" | "--help" => {
            println!(
                "SelfForge commands: init, validate, status, promote, evolve [--patch|--minor|--major] [goal]"
            );
            return;
        }
        other => {
            eprintln!("unknown command: {other}");
            eprintln!(
                "SelfForge commands: init, validate, status, promote, evolve [--patch|--minor|--major] [goal]"
            );
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

fn boxed<E>(result: Result<String, E>) -> Result<String, Box<dyn Error>>
where
    E: Error + 'static,
{
    result.map_err(|error| Box::new(error) as Box<dyn Error>)
}
