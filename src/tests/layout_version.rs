use super::*;

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

    assert_eq!(report.current_version, "v0.1.66");
    assert_eq!(report.next_version, "v0.1.67");
    assert!(root.join("workspaces").join("v0").is_dir());
    assert_workspace_structure(&root);
    assert!(!root.join("workspaces").join("v0.1.67").exists());
    assert!(root.join("forge").join("memory").join("v0.md").is_file());
    assert!(root.join("forge").join("tasks").join("v0.md").is_file());
    assert!(root.join("forge").join("errors").join("v0.md").is_file());
    assert!(root.join("forge").join("versions").join("v0.md").is_file());
    assert!(
        !root
            .join("forge")
            .join("versions")
            .join("v0.1.67.md")
            .exists()
    );
    let version_record = fs::read_to_string(root.join("forge").join("versions").join("v0.md"))
        .expect("major version record should be readable");
    assert!(version_record.contains("## v0.1.67"));
    assert_eq!(report.state.current_version, "v0.1.66");
    assert_eq!(report.state.status, "candidate_prepared");
    assert_eq!(
        report.state.version_scheme.as_deref(),
        Some("semantic:vMAJOR.MINOR.PATCH")
    );
    assert_eq!(report.state.candidate_version.as_deref(), Some("v0.1.67"));
    assert_eq!(
        report.state.candidate_workspace.as_deref(),
        Some("workspaces/v0")
    );

    supervisor
        .verify_version("v0.1.67")
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
    state.workspace = "workspaces/v0.1.66".to_string();
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
    assert!(task.contains("## v0.1.67"));

    cleanup(&root);
}

#[test]
fn semantic_version_patch_bump_is_default() {
    let next = next_version_after("v0.1.0").expect("patch version should advance");

    assert_eq!(next, "v0.1.1");
}

#[test]
fn semantic_version_small_records_share_major_file() {
    let file =
        version_major_file_name("v0.1.9").expect("semantic version should resolve a major file");

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

    assert_eq!(report.previous_version, "v0.1.66");
    assert_eq!(report.promoted_version, "v0.1.67");
    assert_eq!(report.state.current_version, "v0.1.67");
    assert_eq!(report.state.parent_version.as_deref(), Some("v0.1.66"));
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

    assert_eq!(report.previous_version, "v0.1.66");
    assert_eq!(report.candidate_version, "v0.1.67");
    assert_eq!(report.result, CycleResult::Promoted);
    assert!(report.candidate_validation.is_some());
    assert_eq!(report.failure, None);
    assert_eq!(report.state.current_version, "v0.1.67");
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

    assert_eq!(report.current_version, "v0.1.66");
    assert_eq!(report.rolled_back_version, "v0.1.67");
    assert_eq!(report.state.status, "rolled_back");
    assert_eq!(report.state.current_version, "v0.1.66");
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

    assert_eq!(report.previous_version, "v0.1.66");
    assert_eq!(report.candidate_version, "v9.0.0");
    assert_eq!(report.result, CycleResult::RolledBack);
    assert!(report.candidate_validation.is_none());
    assert!(report.failure.is_some());
    assert_eq!(report.state.current_version, "v0.1.66");
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
