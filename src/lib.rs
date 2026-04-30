pub mod app;
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
pub use runtime::{ExecutionError, ExecutionReport, RunIndexEntry, RunQuery, Runtime};
pub use state::{ForgeState, StateError};
pub use supervisor::Supervisor;
pub use version::{
    ForgeVersion, VersionBump, VersionError, next_version_after, next_version_after_with_bump,
    version_major_file_name, version_major_key,
};

pub const CURRENT_VERSION: &str = "v0.1.15";

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

    fn assert_workspace_structure(root: &Path) {
        let workspace = root.join("workspaces").join("v0");
        assert!(workspace.join("README.md").is_file());
        assert!(workspace.join(".gitignore").is_file());
        for directory in ["source", "tests", "sandbox", "artifacts", "logs"] {
            assert!(workspace.join(directory).is_dir());
            assert!(workspace.join(directory).join("README.md").is_file());
        }
    }

    #[test]
    fn bootstrap_creates_required_architecture() {
        let root = temp_root("bootstrap");
        let supervisor = Supervisor::new(&root);

        let report = supervisor
            .initialize_current_version()
            .expect("bootstrap should create the base architecture");

        assert!(report.created_paths.len() >= 20);
        assert!(root.join("README.md").is_file());
        assert!(root.join("runtime").is_dir());
        assert!(root.join("supervisor").is_dir());
        assert!(root.join("workspaces").join("v0").is_dir());
        assert_workspace_structure(&root);
        assert!(root.join("forge").join("memory").join("v0.md").is_file());
        assert!(root.join("forge").join("tasks").join("v0.md").is_file());
        assert!(root.join("forge").join("errors").join("v0.md").is_file());
        assert!(root.join("forge").join("versions").join("v0.md").is_file());
        assert!(root.join("state").join("state.json").is_file());

        cleanup(&root);
    }

    #[test]
    fn validation_rejects_unplanned_workspace_root_entries() {
        let root = temp_root("workspace-root-policy");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before root policy test");
        fs::write(
            root.join("workspaces").join("v0").join("loose-file.txt"),
            "临时文件",
        )
        .expect("test should write an unplanned workspace root file");

        let error = supervisor
            .verify_current_version()
            .expect_err("validation must reject loose workspace root files");

        assert!(error.to_string().contains("workspace root"));
        assert!(error.to_string().contains("loose-file.txt"));

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

        assert_eq!(report.current_version, "v0.1.15");
        assert_eq!(report.next_version, "v0.1.16");
        assert!(root.join("workspaces").join("v0").is_dir());
        assert_workspace_structure(&root);
        assert!(!root.join("workspaces").join("v0.1.16").exists());
        assert!(root.join("forge").join("memory").join("v0.md").is_file());
        assert!(root.join("forge").join("tasks").join("v0.md").is_file());
        assert!(root.join("forge").join("errors").join("v0.md").is_file());
        assert!(root.join("forge").join("versions").join("v0.md").is_file());
        assert!(
            !root
                .join("forge")
                .join("versions")
                .join("v0.1.16.md")
                .exists()
        );
        let version_record = fs::read_to_string(root.join("forge").join("versions").join("v0.md"))
            .expect("major version record should be readable");
        assert!(version_record.contains("## v0.1.16"));
        assert_eq!(report.state.current_version, "v0.1.15");
        assert_eq!(report.state.status, "candidate_prepared");
        assert_eq!(
            report.state.version_scheme.as_deref(),
            Some("semantic:vMAJOR.MINOR.PATCH")
        );
        assert_eq!(report.state.candidate_version.as_deref(), Some("v0.1.16"));
        assert_eq!(
            report.state.candidate_workspace.as_deref(),
            Some("workspaces/v0")
        );

        supervisor
            .verify_version("v0.1.16")
            .expect("candidate layout should validate");

        cleanup(&root);
    }

    #[test]
    fn evolution_normalizes_legacy_current_workspace_to_major_workspace() {
        let root = temp_root("normalize-workspace");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before evolution");
        let mut state = ForgeState::load(&root).expect("state should be readable");
        state.workspace = "workspaces/v0.1.15".to_string();
        state.save(&root).expect("state should be writable");

        let report = supervisor
            .prepare_next_version("normalize legacy workspace")
            .expect("evolution should normalize the current workspace");

        assert_eq!(report.state.workspace, "workspaces/v0");

        cleanup(&root);
    }

    #[test]
    fn evolution_appends_candidate_task_document_without_overwriting_existing_content() {
        let root = temp_root("preserve-task");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before evolution");
        fs::create_dir_all(root.join("forge").join("tasks"))
            .expect("test should create task directory");
        fs::write(
            root.join("forge").join("tasks").join("v0.md"),
            "人工任务计划\n",
        )
        .expect("test should write existing candidate task");

        supervisor
            .prepare_next_version("prepare the next controlled candidate")
            .expect("evolution should prepare a candidate version");

        let task = fs::read_to_string(root.join("forge").join("tasks").join("v0.md"))
            .expect("task should remain readable");
        assert!(task.contains("人工任务计划"));
        assert!(task.contains("## v0.1.16"));

        cleanup(&root);
    }

    #[test]
    fn semantic_version_patch_bump_is_default() {
        let next = next_version_after("v0.1.0").expect("patch version should advance");

        assert_eq!(next, "v0.1.1");
    }

    #[test]
    fn semantic_version_small_records_share_major_file() {
        let file = version_major_file_name("v0.1.9")
            .expect("semantic version should resolve a major file");

        assert_eq!(file, "v0.md");
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

        assert_eq!(report.previous_version, "v0.1.15");
        assert_eq!(report.promoted_version, "v0.1.16");
        assert_eq!(report.state.current_version, "v0.1.16");
        assert_eq!(report.state.parent_version.as_deref(), Some("v0.1.15"));
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

        assert_eq!(report.previous_version, "v0.1.15");
        assert_eq!(report.candidate_version, "v0.1.16");
        assert_eq!(report.result, CycleResult::Promoted);
        assert!(report.candidate_validation.is_some());
        assert_eq!(report.failure, None);
        assert_eq!(report.state.current_version, "v0.1.16");
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

        assert_eq!(report.current_version, "v0.1.15");
        assert_eq!(report.rolled_back_version, "v0.1.16");
        assert_eq!(report.state.status, "rolled_back");
        assert_eq!(report.state.current_version, "v0.1.15");
        assert_eq!(report.state.candidate_version, None);
        assert!(root.join("workspaces").join("v0").is_dir());

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
        let mut state = ForgeState::load(&root).expect("state should be readable");
        state.candidate_version = Some("v9.0.0".to_string());
        state.candidate_workspace = Some("workspaces/v9".to_string());
        state.save(&root).expect("state should be writable");

        let report = supervisor
            .run_candidate_cycle()
            .expect("invalid candidate should roll back without promoting");

        assert_eq!(report.previous_version, "v0.1.15");
        assert_eq!(report.candidate_version, "v9.0.0");
        assert_eq!(report.result, CycleResult::RolledBack);
        assert!(report.candidate_validation.is_none());
        assert!(report.failure.is_some());
        assert_eq!(report.state.current_version, "v0.1.15");
        assert_eq!(report.state.candidate_version, None);
        assert_eq!(report.state.status, "rolled_back");

        cleanup(&root);
    }

    #[test]
    fn app_advance_prepares_candidate_when_none_exists() {
        let root = temp_root("app-prepare");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before app advance");

        let report = app
            .advance("推进最小闭环")
            .expect("advance should prepare a candidate when none exists");

        assert_eq!(report.outcome, MinimalLoopOutcome::Prepared);
        assert_eq!(report.starting_version, "v0.1.15");
        assert_eq!(report.stable_version, "v0.1.15");
        assert_eq!(report.candidate_version.as_deref(), Some("v0.1.16"));
        assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.17"));

        cleanup(&root);
    }

    #[test]
    fn app_advance_promotes_candidate_and_prepares_next_candidate() {
        let root = temp_root("app-promote-prepare");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before app advance");
        app.supervisor()
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared before app advance");

        let report = app
            .advance("继续推进")
            .expect("advance should promote valid candidate and prepare the next one");

        assert_eq!(report.outcome, MinimalLoopOutcome::PromotedAndPrepared);
        assert_eq!(report.starting_version, "v0.1.15");
        assert_eq!(report.stable_version, "v0.1.16");
        assert_eq!(report.candidate_version.as_deref(), Some("v0.1.17"));
        assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.18"));

        cleanup(&root);
    }

    #[test]
    fn app_advance_stops_after_candidate_rollback() {
        let root = temp_root("app-rollback");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before app advance");
        app.supervisor()
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared before app advance");
        let mut state = ForgeState::load(&root).expect("state should be readable");
        state.candidate_version = Some("v9.0.0".to_string());
        state.candidate_workspace = Some("workspaces/v9".to_string());
        state.save(&root).expect("state should be writable");

        let report = app
            .advance("继续推进")
            .expect("advance should roll back invalid candidate");

        assert_eq!(report.outcome, MinimalLoopOutcome::RolledBack);
        assert_eq!(report.starting_version, "v0.1.15");
        assert_eq!(report.stable_version, "v0.1.15");
        assert_eq!(report.candidate_version.as_deref(), Some("v9.0.0"));
        assert_eq!(report.next_expected_version, None);
        assert!(report.failure.is_some());

        cleanup(&root);
    }

    #[test]
    fn runtime_executes_command_inside_version_workspace() {
        let root = temp_root("runtime-run");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before runtime execution");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();
        let args = vec!["--help".to_string()];

        let report = supervisor
            .execute_in_workspace(CURRENT_VERSION, &program, &args, 5_000)
            .expect("runtime should execute direct command inside workspace");

        assert_eq!(report.version, CURRENT_VERSION);
        assert!(report.workspace.ends_with("v0"));
        assert_eq!(report.exit_code, Some(0));
        assert!(!report.timed_out);
        assert!(!report.stdout.is_empty() || !report.stderr.is_empty());
        assert!(report.run_dir.starts_with(&report.workspace));
        assert!(
            report
                .run_dir
                .parent()
                .is_some_and(|path| path.ends_with("runs"))
        );
        assert!(report.run_dir.join("report.json").is_file());
        assert!(report.run_dir.join("stdout.txt").is_file());
        assert!(report.run_dir.join("stderr.txt").is_file());
        let run_record = fs::read_to_string(report.run_dir.join("report.json"))
            .expect("runtime run record should be readable");
        assert!(run_record.contains(CURRENT_VERSION));
        assert!(run_record.contains("\"stdout_file\""));
        let run_index = fs::read_to_string(report.run_dir.parent().unwrap().join("index.jsonl"))
            .expect("runtime run index should be readable");
        let run_id = report
            .run_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("run directory should have a valid file name");
        assert!(run_index.contains(run_id));
        assert!(run_index.contains("\"timed_out\":false"));
        let entries = supervisor
            .list_runs(CURRENT_VERSION, 10)
            .expect("runtime run index should be queryable");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].run_id, run_id);
        assert_eq!(entries[0].version, CURRENT_VERSION);
        assert!(!entries[0].timed_out);

        cleanup(&root);
    }

    #[test]
    fn runtime_times_out_command_without_hanging() {
        let root = temp_root("runtime-timeout");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before timeout test");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();
        let args = vec!["--help".to_string()];

        let report = supervisor
            .execute_in_workspace(CURRENT_VERSION, &program, &args, 0)
            .expect("runtime should report immediate timeout");

        assert!(report.timed_out);
        assert!(report.run_dir.join("report.json").is_file());
        let run_record = fs::read_to_string(report.run_dir.join("report.json"))
            .expect("timeout run record should be readable");
        assert!(run_record.contains("\"timed_out\": true"));
        let run_index = fs::read_to_string(report.run_dir.parent().unwrap().join("index.jsonl"))
            .expect("timeout run index should be readable");
        assert!(run_index.contains("\"timed_out\":true"));
        let entries = supervisor
            .list_runs(CURRENT_VERSION, 10)
            .expect("timeout run index should be queryable");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].timed_out);

        cleanup(&root);
    }

    #[test]
    fn runtime_filters_failed_and_timed_out_runs() {
        let root = temp_root("runtime-failed-runs");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before failed run filter test");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        supervisor
            .execute_in_workspace(CURRENT_VERSION, &program, &["--help".to_string()], 5_000)
            .expect("success run should be recorded");
        supervisor
            .execute_in_workspace(
                CURRENT_VERSION,
                &program,
                &["--self-forge-invalid-test-flag".to_string()],
                5_000,
            )
            .expect("failed run should still be recorded");
        supervisor
            .execute_in_workspace(CURRENT_VERSION, &program, &["--help".to_string()], 0)
            .expect("timed out run should still be recorded");
        supervisor
            .execute_in_workspace("v0.9.0", &program, &["--help".to_string()], 5_000)
            .expect("same major workspace may contain another small version run");

        let all_entries = supervisor
            .query_runs(CURRENT_VERSION, RunQuery::recent(10))
            .expect("all runs should be queryable");
        assert_eq!(all_entries.len(), 3);
        let other_version_entries = supervisor
            .query_runs("v0.9.0", RunQuery::recent(10))
            .expect("same major workspace should still filter by requested version");
        assert_eq!(other_version_entries.len(), 1);
        assert_eq!(other_version_entries[0].version, "v0.9.0");

        let failed_entries = supervisor
            .query_runs(CURRENT_VERSION, RunQuery::failed(10))
            .expect("failed runs should be queryable");
        assert_eq!(failed_entries.len(), 2);
        assert!(failed_entries.iter().all(RunIndexEntry::is_failed));
        assert!(failed_entries.iter().any(|entry| entry.timed_out));
        assert!(
            failed_entries
                .iter()
                .any(|entry| entry.exit_code != Some(0))
        );

        let timed_out_entries = supervisor
            .query_runs(CURRENT_VERSION, RunQuery::timed_out(10))
            .expect("timed out runs should be queryable");
        assert_eq!(timed_out_entries.len(), 1);
        assert!(timed_out_entries[0].timed_out);

        let no_entries = supervisor
            .query_runs(CURRENT_VERSION, RunQuery::failed(0))
            .expect("zero limit should be accepted");
        assert!(no_entries.is_empty());

        cleanup(&root);
    }

    #[test]
    fn error_archive_records_latest_failed_run_once() {
        let root = temp_root("error-archive");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before error archive test");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let failed = supervisor
            .execute_in_workspace(
                CURRENT_VERSION,
                &program,
                &["--self-forge-invalid-test-flag".to_string()],
                5_000,
            )
            .expect("failed command should still produce an execution report");
        let run_id = failed
            .run_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("run directory should have a valid file name")
            .to_string();

        let archive = ErrorArchive::new(&root);
        let report = archive
            .record_failed_run(
                CURRENT_VERSION,
                None,
                "Runtime 受控执行",
                "修正命令参数后重新运行验证。",
            )
            .expect("latest failed run should be archived");

        assert!(report.appended);
        assert_eq!(report.run_id, run_id);
        assert!(report.archive_path.ends_with("v0.md"));
        let contents = fs::read_to_string(&report.archive_path)
            .expect("error archive should be readable after append");
        assert!(contents.contains(&format!("## {CURRENT_VERSION} 运行错误 {run_id}")));
        assert!(contents.contains("运行编号"));
        assert!(contents.contains("是否已解决"));
        assert!(contents.contains("否。该记录为失败运行归档草稿"));

        let duplicate = archive
            .record_failed_run(
                CURRENT_VERSION,
                Some(&run_id),
                "Runtime 受控执行",
                "修正命令参数后重新运行验证。",
            )
            .expect("duplicate archive request should be idempotent");
        assert!(!duplicate.appended);
        let after_duplicate =
            fs::read_to_string(&report.archive_path).expect("error archive should remain readable");
        assert_eq!(contents, after_duplicate);

        supervisor
            .verify_current_version()
            .expect("archived error document should pass validation");

        cleanup(&root);
    }

    #[test]
    fn error_archive_reports_when_no_failed_run_exists() {
        let root = temp_root("error-archive-empty");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before empty error archive test");

        let error = ErrorArchive::new(&root)
            .record_failed_run(CURRENT_VERSION, None, "", "")
            .expect_err("empty failed run list should be reported");

        assert!(matches!(error, ErrorArchiveError::NoFailedRun { .. }));

        cleanup(&root);
    }

    #[test]
    fn error_archive_rejects_non_failed_run() {
        let root = temp_root("error-archive-success");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before non-failed error archive test");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();
        let success = supervisor
            .execute_in_workspace(CURRENT_VERSION, &program, &["--help".to_string()], 5_000)
            .expect("success command should be recorded");
        let run_id = success
            .run_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("run directory should have a valid file name")
            .to_string();

        let error = ErrorArchive::new(&root)
            .record_failed_run(CURRENT_VERSION, Some(&run_id), "", "")
            .expect_err("non-failed run must not be archived as an error");

        assert!(matches!(error, ErrorArchiveError::RunNotFailed { .. }));

        cleanup(&root);
    }

    #[test]
    fn error_archive_resolves_archived_run_error_once() {
        let root = temp_root("error-resolve");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before error resolution test");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();
        let failed = supervisor
            .execute_in_workspace(
                CURRENT_VERSION,
                &program,
                &["--self-forge-invalid-test-flag".to_string()],
                5_000,
            )
            .expect("failed command should produce a run record");
        let run_id = failed
            .run_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("run directory should have a valid file name")
            .to_string();
        let archive = ErrorArchive::new(&root);
        archive
            .record_failed_run(CURRENT_VERSION, Some(&run_id), "", "")
            .expect("failed run should be archived before resolution");

        let resolved = archive
            .resolve_run_error(CURRENT_VERSION, &run_id, "cargo test 通过")
            .expect("archived run error should resolve");

        assert!(resolved.updated);
        assert_eq!(resolved.run_id, run_id);
        let contents = fs::read_to_string(&resolved.archive_path)
            .expect("error archive should be readable after resolution");
        assert!(contents.contains("是。验证依据：cargo test 通过"));
        assert!(!contents.contains("否。该记录为失败运行归档草稿"));

        let duplicate = archive
            .resolve_run_error(CURRENT_VERSION, &run_id, "cargo test 通过")
            .expect("repeat resolution should be idempotent");
        assert!(!duplicate.updated);
        let after_duplicate = fs::read_to_string(&resolved.archive_path)
            .expect("error archive should remain readable");
        assert_eq!(contents, after_duplicate);

        supervisor
            .verify_current_version()
            .expect("resolved error document should pass validation");

        cleanup(&root);
    }

    #[test]
    fn error_archive_reports_missing_archived_error() {
        let root = temp_root("error-resolve-missing");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before missing error resolution test");

        let error = ErrorArchive::new(&root)
            .resolve_run_error(CURRENT_VERSION, "run-not-recorded", "cargo test 通过")
            .expect_err("missing archived error should be reported");

        assert!(matches!(
            error,
            ErrorArchiveError::ArchivedErrorNotFound { .. }
        ));

        cleanup(&root);
    }

    #[test]
    fn runtime_rejects_empty_command() {
        let root = temp_root("runtime-empty");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before empty command test");

        let error = supervisor
            .execute_in_workspace(CURRENT_VERSION, "", &[], 1_000)
            .expect_err("empty command must be rejected");

        assert!(matches!(error, ExecutionError::EmptyProgram));

        cleanup(&root);
    }

    #[test]
    fn runtime_rejects_workspace_escape_version() {
        let root = temp_root("runtime-escape");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before escape test");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let error = supervisor
            .execute_in_workspace("..", &program, &[], 1_000)
            .expect_err("workspace escape must be rejected");

        assert!(matches!(error, ExecutionError::WorkspacePath { .. }));

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
                .join("v0")
                .join("source")
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
    fn validation_rejects_markdown_emoji() {
        let root = temp_root("emoji");
        let supervisor = Supervisor::new(&root);

        supervisor
            .initialize_current_version()
            .expect("bootstrap should succeed before emoji audit");
        let emoji = char::from_u32(0x1f600).expect("test emoji code point should be valid");
        fs::write(
            root.join("workspaces")
                .join("v0")
                .join("source")
                .join("emoji.md"),
            format!("# 中文文档\n\n这里包含{emoji}\n"),
        )
        .expect("test should write emoji markdown document");

        let error = supervisor
            .verify_current_version()
            .expect_err("validation must reject markdown emoji");

        assert!(error.to_string().contains("Emoji"));

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
pub use app::{
    ErrorArchive, ErrorArchiveError, ErrorArchiveReport, ErrorResolutionReport, MinimalLoopOutcome,
    MinimalLoopReport, SelfForgeApp,
};
