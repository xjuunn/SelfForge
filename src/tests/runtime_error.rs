use super::*;

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
    let after_duplicate =
        fs::read_to_string(&resolved.archive_path).expect("error archive should remain readable");
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
fn error_archive_lists_open_and_resolved_errors() {
    let root = temp_root("error-list");
    let supervisor = Supervisor::new(&root);

    supervisor
        .initialize_current_version()
        .expect("bootstrap should succeed before error list test");
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
    let other = supervisor
        .execute_in_workspace(
            "v0.9.0",
            &program,
            &["--self-forge-invalid-test-flag".to_string()],
            5_000,
        )
        .expect("another small version failed run should be recorded");
    let other_run_id = other
        .run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("run directory should have a valid file name")
        .to_string();

    let archive = ErrorArchive::new(&root);
    archive
        .record_failed_run(CURRENT_VERSION, Some(&run_id), "", "")
        .expect("current failed run should be archived");
    archive
        .record_failed_run("v0.9.0", Some(&other_run_id), "", "")
        .expect("other version failed run should be archived");

    let open_errors = archive
        .list_run_errors(CURRENT_VERSION, ErrorListQuery::open(10))
        .expect("open archived errors should be listed");
    assert_eq!(open_errors.len(), 1);
    assert_eq!(open_errors[0].run_id, run_id);
    assert_eq!(open_errors[0].version, CURRENT_VERSION);
    assert!(!open_errors[0].resolved);

    let no_errors = archive
        .list_run_errors(CURRENT_VERSION, ErrorListQuery::open(0))
        .expect("zero limit should be accepted");
    assert!(no_errors.is_empty());

    archive
        .resolve_run_error(CURRENT_VERSION, &run_id, "cargo test 通过")
        .expect("archived error should be resolvable");

    let resolved_errors = archive
        .list_run_errors(CURRENT_VERSION, ErrorListQuery::resolved(10))
        .expect("resolved archived errors should be listed");
    assert_eq!(resolved_errors.len(), 1);
    assert_eq!(resolved_errors[0].run_id, run_id);
    assert!(resolved_errors[0].resolved);

    let other_version_errors = archive
        .list_run_errors("v0.9.0", ErrorListQuery::recent(10))
        .expect("other version archived errors should remain queryable");
    assert_eq!(other_version_errors.len(), 1);
    assert_eq!(other_version_errors[0].run_id, other_run_id);

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
