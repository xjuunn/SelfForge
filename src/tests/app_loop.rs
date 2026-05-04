use super::*;

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
    assert_eq!(report.starting_version, "v0.1.65");
    assert_eq!(report.stable_version, "v0.1.65");
    assert_eq!(report.candidate_version.as_deref(), Some("v0.1.66"));
    assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.67"));

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
    assert_eq!(report.starting_version, "v0.1.65");
    assert_eq!(report.stable_version, "v0.1.66");
    assert_eq!(report.candidate_version.as_deref(), Some("v0.1.67"));
    assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.68"));

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
    assert_eq!(report.starting_version, "v0.1.65");
    assert_eq!(report.stable_version, "v0.1.65");
    assert_eq!(report.candidate_version.as_deref(), Some("v9.0.0"));
    assert_eq!(report.next_expected_version, None);
    assert!(report.failure.is_some());

    cleanup(&root);
}

#[test]
fn app_advance_stops_when_current_version_has_open_errors() {
    let root = temp_root("app-open-errors");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before open error guard test");
    let program = std::env::current_exe()
        .expect("test executable path should be available")
        .to_string_lossy()
        .into_owned();
    let failed = app
        .supervisor()
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
    ErrorArchive::new(&root)
        .record_failed_run(CURRENT_VERSION, Some(&run_id), "", "")
        .expect("failed run should be archived before advance");

    let error = app
        .advance("继续推进")
        .expect_err("advance must stop when current version has open errors");

    assert!(matches!(
        error,
        MinimalLoopError::OpenErrors { ref version, run_id: ref error_run_id }
            if version == CURRENT_VERSION && error_run_id == &run_id
    ));
    let state = ForgeState::load(&root).expect("state should remain readable");
    assert_eq!(state.current_version, CURRENT_VERSION);
    assert_eq!(state.candidate_version, None);

    cleanup(&root);
}

#[test]
fn app_preflight_allows_clean_current_version() {
    let root = temp_root("app-preflight-clean");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before preflight");

    let report = app
        .preflight()
        .expect("preflight should allow a clean current version");

    assert_eq!(report.current_version, CURRENT_VERSION);
    assert_eq!(report.current_workspace, "workspaces/v0");
    assert_eq!(report.status, "initialized");
    assert_eq!(report.candidate_version, None);
    assert!(report.checked_paths.len() >= 20);
    assert!(report.candidate_checked_paths.is_empty());
    assert!(report.open_errors.is_empty());
    assert!(report.can_advance);

    cleanup(&root);
}

#[test]
fn app_preflight_reports_candidate_validation() {
    let root = temp_root("app-preflight-candidate");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before preflight");
    app.supervisor()
        .prepare_next_version("prepare candidate")
        .expect("candidate should be prepared before preflight");

    let report = app
        .preflight()
        .expect("preflight should include candidate validation");
    let expected_candidate =
        next_version_after(CURRENT_VERSION).expect("current version should advance");

    assert_eq!(
        report.candidate_version.as_deref(),
        Some(expected_candidate.as_str())
    );
    assert_eq!(report.candidate_workspace.as_deref(), Some("workspaces/v0"));
    assert!(!report.candidate_checked_paths.is_empty());
    assert!(report.open_errors.is_empty());
    assert!(report.can_advance);

    cleanup(&root);
}

#[test]
fn app_preflight_reports_open_errors() {
    let root = temp_root("app-preflight-open-errors");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before preflight open error test");
    let program = std::env::current_exe()
        .expect("test executable path should be available")
        .to_string_lossy()
        .into_owned();
    let failed = app
        .supervisor()
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
    ErrorArchive::new(&root)
        .record_failed_run(CURRENT_VERSION, Some(&run_id), "", "")
        .expect("failed run should be archived before preflight");

    let report = app
        .preflight()
        .expect("preflight should report open errors without changing state");

    assert_eq!(report.open_errors.len(), 1);
    assert_eq!(report.open_errors[0].run_id, run_id);
    assert!(!report.open_errors[0].resolved);
    assert!(!report.can_advance);
    let state = ForgeState::load(&root).expect("state should remain readable");
    assert_eq!(state.current_version, CURRENT_VERSION);
    assert_eq!(state.candidate_version, None);

    cleanup(&root);
}
