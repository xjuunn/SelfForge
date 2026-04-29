use self_forge::Supervisor;
use std::env;
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
    let command = env::args().nth(1).unwrap_or_else(|| "status".to_string());

    let result = match command.as_str() {
        "init" => supervisor.initialize_current_version().map(|report| {
            format!(
                "SelfForge {} initialized: {} created, {} existing",
                report.version,
                report.created_paths.len(),
                report.existing_paths.len()
            )
        }),
        "validate" => supervisor.verify_current_version().map(|report| {
            format!(
                "SelfForge {} valid: {} paths checked",
                report.version,
                report.checked_paths.len()
            )
        }),
        "status" => supervisor.verify_current_version().map(|report| {
            format!(
                "SelfForge {} ready: {} paths checked",
                report.version,
                report.checked_paths.len()
            )
        }),
        "help" | "-h" | "--help" => {
            println!("SelfForge commands: init, validate, status");
            return;
        }
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("SelfForge commands: init, validate, status");
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
