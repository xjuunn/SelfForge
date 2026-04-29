pub mod documentation;
pub mod evolution;
pub mod layout;
pub mod runtime;
pub mod state;
pub mod supervisor;
pub mod version;

pub use documentation::{
    DocumentationError, DocumentationReport, DocumentationViolation, validate_chinese_markdown,
};
pub use evolution::{
    CycleReport, CycleResult, EvolutionEngine, EvolutionError, EvolutionReport, PromotionReport,
    RollbackReport,
};
pub use layout::{BootstrapReport, ForgeError, SelfForge, ValidationReport};
pub use runtime::Runtime;
pub use state::{ForgeState, StateError};
pub use supervisor::Supervisor;
pub use version::{
    ForgeVersion, VersionBump, VersionError, next_version_after, next_version_after_with_bump,
    version_series_file_name,
};

pub const CURRENT_VERSION: &str = "v0.1.3";

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
        assert!(root.join("workspaces").join("v0.1.3").is_dir());
        assert!(
            root.join("forge")
                .join("memory")
                .join("v0.1.3.md")
                .is_file()
        );
        assert!(root.join("forge").join("tasks").join("v0.1.3.md").is_file());
        assert!(root.join("forge").join("errors").join("v0.1.3").is_dir());
        assert!(
            root.join("forge")
                .join("versions")
                .join("v0.1.md")
                .is_file()
        );
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

        assert_eq!(report.current_version, "v0.1.3");
        assert_eq!(report.next_version, "v0.1.4");
        assert!(root.join("workspaces").join("v0.1.4").is_dir());
        assert!(
            root.join("forge")
                .join("memory")
                .join("v0.1.4.md")
                .is_file()
        );
        assert!(root.join("forge").join("tasks").join("v0.1.4.md").is_file());
        assert!(root.join("forge").join("errors").join("v0.1.4").is_dir());
        assert!(
            root.join("forge")
                .join("versions")
                .join("v0.1.md")
                .is_file()
        );
        assert!(
            !root
                .join("forge")
                .join("versions")
                .join("v0.1.4.md")
                .exists()
        );
        let version_record =
            fs::read_to_string(root.join("forge").join("versions").join("v0.1.md"))
                .expect("series version record should be readable");
        assert!(version_record.contains("## v0.1.4"));
        assert_eq!(report.state.current_version, "v0.1.3");
        assert_eq!(report.state.status, "candidate_prepared");
        assert_eq!(
            report.state.version_scheme.as_deref(),
            Some("semantic:vMAJOR.MINOR.PATCH")
        );
        assert_eq!(report.state.candidate_version.as_deref(), Some("v0.1.4"));
        assert_eq!(
            report.state.candidate_workspace.as_deref(),
            Some("workspaces/v0.1.4")
        );

        supervisor
            .verify_version("v0.1.4")
            .expect("candidate layout should validate");

        cleanup(&root);
    }

    #[test]
    fn evolution_preserves_existing_candidate_task_document() {
        let root = temp_root("preserve-task");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before evolution");
        fs::create_dir_all(root.join("forge").join("tasks"))
            .expect("test should create task directory");
        fs::write(
            root.join("forge").join("tasks").join("v0.1.4.md"),
            "人工任务计划",
        )
        .expect("test should write existing candidate task");

        supervisor
            .prepare_next_version("prepare the next controlled candidate")
            .expect("evolution should prepare a candidate version");

        let task = fs::read_to_string(root.join("forge").join("tasks").join("v0.1.4.md"))
            .expect("task should remain readable");
        assert_eq!(task, "人工任务计划");

        cleanup(&root);
    }

    #[test]
    fn semantic_version_patch_bump_is_default() {
        let next = next_version_after("v0.1.0").expect("patch version should advance");

        assert_eq!(next, "v0.1.1");
    }

    #[test]
    fn semantic_version_patch_records_share_minor_series_file() {
        let file = version_series_file_name("v0.1.9")
            .expect("semantic version should resolve a series file");

        assert_eq!(file, "v0.1.md");
    }

    #[test]
    fn promotion_moves_candidate_to_current_version() {
        let root = temp_root("promote");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before evolution");
        supervisor
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared");

        let report = supervisor
            .promote_candidate()
            .expect("candidate should promote after validation");

        assert_eq!(report.previous_version, "v0.1.3");
        assert_eq!(report.promoted_version, "v0.1.4");
        assert_eq!(report.state.current_version, "v0.1.4");
        assert_eq!(report.state.parent_version.as_deref(), Some("v0.1.3"));
        assert_eq!(report.state.candidate_version, None);
        assert_eq!(report.state.status, "active");

        cleanup(&root);
    }

    #[test]
    fn cycle_promotes_valid_candidate_version() {
        let root = temp_root("cycle-promote");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before cycle");
        supervisor
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared");

        let report = supervisor
            .run_candidate_cycle()
            .expect("valid candidate should complete the cycle");

        assert_eq!(report.previous_version, "v0.1.3");
        assert_eq!(report.candidate_version, "v0.1.4");
        assert_eq!(report.result, CycleResult::Promoted);
        assert!(report.candidate_validation.is_some());
        assert_eq!(report.failure, None);
        assert_eq!(report.state.current_version, "v0.1.4");
        assert_eq!(report.state.candidate_version, None);
        assert_eq!(report.state.status, "active");

        cleanup(&root);
    }

    #[test]
    fn rollback_clears_candidate_without_deleting_workspace() {
        let root = temp_root("rollback");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before rollback");
        supervisor
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared");

        let report = supervisor
            .rollback_candidate("测试回滚")
            .expect("rollback should clear candidate state");

        assert_eq!(report.current_version, "v0.1.3");
        assert_eq!(report.rolled_back_version, "v0.1.4");
        assert_eq!(report.state.status, "rolled_back");
        assert_eq!(report.state.current_version, "v0.1.3");
        assert_eq!(report.state.candidate_version, None);
        assert!(root.join("workspaces").join("v0.1.4").is_dir());

        cleanup(&root);
    }

    #[test]
    fn cycle_rolls_back_invalid_candidate_version() {
        let root = temp_root("cycle-rollback");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before cycle");
        supervisor
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared");
        fs::remove_file(root.join("workspaces").join("v0.1.4").join("README.md"))
            .expect("test should be able to break candidate workspace");

        let report = supervisor
            .run_candidate_cycle()
            .expect("invalid candidate should roll back without promoting");

        assert_eq!(report.previous_version, "v0.1.3");
        assert_eq!(report.candidate_version, "v0.1.4");
        assert_eq!(report.result, CycleResult::RolledBack);
        assert!(report.candidate_validation.is_none());
        assert!(report.failure.is_some());
        assert_eq!(report.state.current_version, "v0.1.3");
        assert_eq!(report.state.candidate_version, None);
        assert_eq!(report.state.status, "rolled_back");

        cleanup(&root);
    }

    #[test]
    fn validation_rejects_non_chinese_markdown_document() {
        let root = temp_root("docs");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before doc audit");
        fs::write(
            root.join("workspaces")
                .join(CURRENT_VERSION)
                .join("english.md"),
            "# English\n\nOnly ASCII text.\n",
        )
        .expect("test should write non-Chinese document");

        let error = supervisor
            .verify_current_version()
            .expect_err("validation must reject non-Chinese markdown");

        assert!(
            error
                .to_string()
                .contains("documentation policy violations")
        );

        cleanup(&root);
    }

    #[test]
    fn semantic_version_supports_explicit_minor_and_major_bumps() {
        let minor = next_version_after_with_bump("v0.1.9", VersionBump::Minor)
            .expect("minor bump should reset patch");
        let major = next_version_after_with_bump("v0.9.9", VersionBump::Major)
            .expect("major bump should reset minor and patch");

        assert_eq!(minor, "v0.2.0");
        assert_eq!(major, "v1.0.0");
    }

    #[test]
    fn evolution_rejects_invalid_current_version() {
        let root = temp_root("invalid-version");

        fs::create_dir_all(root.join("state")).expect("test should create state directory");
        fs::write(
            root.join("state").join("state.json"),
            "{\n  \"current_version\": \"v1\",\n  \"parent_version\": null,\n  \"status\": \"initialized\",\n  \"workspace\": \"workspaces/v1\",\n  \"last_verified\": null\n}\n",
        )
        .expect("test should write invalid state");

        let error = EvolutionEngine::new(&root)
            .prepare_next_version("goal")
            .expect_err("invalid current version must stop evolution");

        assert!(error.to_string().contains("invalid version"));

        cleanup(&root);
    }
}
