use self_forge::Supervisor;
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
            let goal = args.collect::<Vec<_>>().join(" ");
            let goal = if goal.trim().is_empty() {
                "prepare next controlled self-evolution candidate"
            } else {
                goal.trim()
            };
            boxed(supervisor.prepare_next_version(goal).map(|report| {
                format!(
                    "SelfForge prepared {} from {}: {} paths checked, workspace {}",
                    report.next_version,
                    report.current_version,
                    report.candidate_validation.checked_paths.len(),
                    report.workspace.display()
                )
            }))
        }
        "help" | "-h" | "--help" => {
            println!("SelfForge commands: init, validate, status, evolve [goal]");
            return;
        }
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("SelfForge commands: init, validate, status, evolve [goal]");
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
