pub mod evolution;
pub mod layout;
pub mod runtime;
pub mod state;
pub mod supervisor;

pub use evolution::{EvolutionEngine, EvolutionError, EvolutionReport, next_version_after};
pub use layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport};
pub use runtime::Runtime;
pub use state::{ForgeState, StateError};
pub use supervisor::Supervisor;

pub const CURRENT_VERSION: &str = "v1";

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("selfforge-{name}-{stamp}"))
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            let _ = fs::remove_dir_all(path);
        }
    }

    #[test]
    fn bootstrap_creates_required_architecture() {
        let root = temp_root("bootstrap");
        let supervisor = Supervisor::new(&root);

        let report = supervisor
            .initialize_current_version()
            .expect("bootstrap should create the base architecture");

        assert!(report.created_paths.len() >= 10);
        assert!(root.join("runtime").is_dir());
        assert!(root.join("supervisor").is_dir());
        assert!(root.join("workspaces").join("v1").is_dir());
        assert!(root.join("forge").join("memory").join("v1.md").is_file());
        assert!(root.join("forge").join("tasks").join("v1.md").is_file());
        assert!(root.join("forge").join("errors").join("v1").is_dir());
        assert!(root.join("forge").join("versions").join("v1.md").is_file());
        assert!(root.join("state").join("state.json").is_file());

        cleanup(&root);
    }

    #[test]
    fn bootstrap_is_idempotent_for_existing_architecture() {
        let root = temp_root("idempotent");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("first bootstrap should succeed");
        let second = supervisor
            .initialize_current_version()
            .expect("second bootstrap should not rewrite existing files");

        assert!(second.created_paths.is_empty());
        assert!(!second.existing_paths.is_empty());

        cleanup(&root);
    }

    #[test]
    fn validate_reports_missing_state_as_error() {
        let root = temp_root("missing-state");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before removing state");
        fs::remove_file(root.join("state").join("state.json"))
            .expect("test should be able to remove generated state");

        let error = supervisor
            .verify_current_version()
            .expect_err("validation must fail when persistent state is missing");

        assert!(error.to_string().contains("state/state.json"));

        cleanup(&root);
    }

    #[test]
    fn evolution_prepares_next_candidate_version() {
        let root = temp_root("evolve");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before evolution");
        let report = supervisor
            .prepare_next_version("prepare the next controlled candidate")
            .expect("evolution should prepare a candidate version");

        assert_eq!(report.current_version, "v1");
        assert_eq!(report.next_version, "v2");
        assert!(root.join("workspaces").join("v2").is_dir());
        assert!(root.join("forge").join("memory").join("v2.md").is_file());
        assert!(root.join("forge").join("tasks").join("v2.md").is_file());
        assert!(root.join("forge").join("errors").join("v2").is_dir());
        assert!(root.join("forge").join("versions").join("v2.md").is_file());
        assert_eq!(report.state.current_version, "v1");
        assert_eq!(report.state.status, "candidate_prepared");
        assert_eq!(report.state.candidate_version.as_deref(), Some("v2"));
        assert_eq!(
            report.state.candidate_workspace.as_deref(),
            Some("workspaces/v2")
        );

        supervisor
            .verify_version("v2")
            .expect("candidate layout should validate");

        cleanup(&root);
    }

    #[test]
    fn evolution_version_boundary_advances_large_version() {
        let next = next_version_after("v999").expect("large version should advance");

        assert_eq!(next, "v1000");
    }

    #[test]
    fn evolution_rejects_invalid_current_version() {
        let root = temp_root("invalid-version");

        fs::create_dir_all(root.join("state")).expect("test should create state directory");
        fs::write(
            root.join("state").join("state.json"),
            "{\n  \"current_version\": \"latest\",\n  \"parent_version\": null,\n  \"status\": \"initialized\",\n  \"workspace\": \"workspaces/latest\",\n  \"last_verified\": null\n}\n",
        )
        .expect("test should write invalid state");

        let error = EvolutionEngine::new(&root)
            .prepare_next_version("goal")
            .expect_err("invalid current version must stop evolution");

        assert!(error.to_string().contains("invalid version"));

        cleanup(&root);
    }
}
