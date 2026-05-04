use super::*;
use std::process::Command;

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
    assert_eq!(report.starting_version, "v0.1.70");
    assert_eq!(report.stable_version, "v0.1.70");
    assert_eq!(report.candidate_version.as_deref(), Some("v0.1.71"));
    assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.72"));

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
    assert_eq!(report.starting_version, "v0.1.70");
    assert_eq!(report.stable_version, "v0.1.71");
    assert_eq!(report.candidate_version.as_deref(), Some("v0.1.72"));
    assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.73"));

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
    assert_eq!(report.starting_version, "v0.1.70");
    assert_eq!(report.stable_version, "v0.1.70");
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

#[test]
fn branch_check_blocks_writing_on_master() {
    let root = temp_root("branch-check-master");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before branch check");
    init_git_fixture(&root);

    let report = app
        .branch_check(CURRENT_VERSION, None, None, "master", false)
        .expect("branch check should read git state");

    assert_eq!(report.current_branch, "master");
    assert!(report.on_base_branch);
    assert!(!report.can_write);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("禁止直接写入"))
    );

    cleanup(&root);
}

#[test]
fn branch_check_allows_claimed_task_with_only_queue_changes() {
    let root = temp_root("branch-check-claimed");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before branch check");
    init_git_fixture(&root);
    run_git(&root, &["switch", "-c", "codex/coord-002-application"]);
    app.init_agent_work_queue(CURRENT_VERSION, "验证分支检查", 3)
        .expect("work queue should initialize");
    let claim = app
        .claim_agent_work_with_lease(CURRENT_VERSION, "ai-1", Some("builder"), Some(7200))
        .expect("builder should claim application task");

    assert_eq!(claim.task.id, "coord-002-application");

    let report = app
        .branch_check(
            CURRENT_VERSION,
            Some("ai-1"),
            Some("coord-002-application"),
            "master",
            false,
        )
        .expect("branch check should inspect claimed task");

    assert_eq!(report.current_branch, "codex/coord-002-application");
    assert_eq!(report.task_matches_worker, Some(true));
    assert_eq!(report.task_matches_branch, Some(true));
    assert!(report.unexpected_changes.is_empty());
    assert!(report.can_write);

    cleanup(&root);
}

#[test]
fn branch_check_suggests_branch_from_explicit_task() {
    let root = temp_root("branch-check-suggest");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before branch check");
    init_git_fixture(&root);
    app.init_agent_work_queue(CURRENT_VERSION, "验证分支建议", 3)
        .expect("work queue should initialize");

    let report = app
        .branch_check(
            CURRENT_VERSION,
            Some("ai-1"),
            Some("coord-002-application"),
            "master",
            true,
        )
        .expect("branch check should suggest branch");

    assert_eq!(
        report.suggested_branch.as_deref(),
        Some("codex/coord-002-application")
    );
    assert_eq!(
        report.suggested_branch_source.as_deref(),
        Some("显式任务编号")
    );

    cleanup(&root);
}

fn init_git_fixture(root: &Path) {
    run_git(root, &["init", "-b", "master"]);
    run_git(root, &["config", "user.name", "SelfForge Test"]);
    run_git(
        root,
        &["config", "user.email", "self-forge-test@example.invalid"],
    );
    run_git(root, &["add", "."]);
    run_git(root, &["commit", "-m", "test: 初始化测试仓库"]);
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git command should spawn");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}
