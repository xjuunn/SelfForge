use super::*;

#[test]
fn ai_patch_source_plan_prepares_diff_and_rollback_manifest() {
    let root = temp_root("ai-patch-source-plan-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before source plan");
    let target_path = root.join("src").join("app").join("minimal_loop.rs");
    fs::create_dir_all(target_path.parent().expect("target parent should exist"))
        .expect("test should create target parent");
    fs::write(&target_path, "fn old() {}\n").expect("test should write target file");
    app.init_agent_work_queue(CURRENT_VERSION, "源码覆盖准备测试", 3)
        .expect("work queue should exist before source plan");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before source plan");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before source plan");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before source plan");
    app.ai_patch_verify_with_runner(
        CURRENT_VERSION,
        &application.record.id,
        1_234,
        |spec, timeout_ms| {
            Ok(AiPatchVerificationCommandRecord {
                command: spec.command.clone(),
                program: spec.program.clone(),
                args: spec.args.clone(),
                started_at_unix_seconds: 1,
                duration_ms: 2,
                timeout_ms,
                exit_code: Some(0),
                timed_out: false,
                stdout_bytes: 0,
                stderr_bytes: 0,
                stdout_preview: String::new(),
                stderr_preview: String::new(),
                status: AiPatchVerificationStatus::Passed,
            })
        },
    )
    .expect("verification should pass before source plan");

    let report = app
        .ai_patch_source_plan(CURRENT_VERSION, &application.record.id)
        .expect("verified application should produce source plan");

    assert_eq!(report.record.status, AiPatchSourcePlanStatus::Prepared);
    assert_eq!(report.record.files.len(), 1);
    assert_eq!(
        report.record.files[0].target_file,
        PathBuf::from("src").join("app").join("minimal_loop.rs")
    );
    assert!(report.record.files[0].target_exists);
    assert!(report.record.files[0].diff_summary.contains("将被覆盖"));
    let backup_file = report.record.files[0]
        .rollback_backup_file
        .as_ref()
        .expect("existing target should have rollback backup");
    let backup_contents =
        fs::read_to_string(root.join(backup_file)).expect("rollback backup should be readable");
    assert_eq!(backup_contents, "fn old() {}\n");
    let target_contents =
        fs::read_to_string(&target_path).expect("target should still be readable");
    assert_eq!(target_contents, "fn old() {}\n");
    let records = app
        .ai_patch_source_plan_records(CURRENT_VERSION, 10)
        .expect("source plan records should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_plan_record(CURRENT_VERSION, &report.record.id)
        .expect("source plan record should be readable");
    assert_eq!(loaded.id, report.record.id);
    let report_file = loaded
        .report_file
        .as_ref()
        .expect("source plan should write markdown report");
    let markdown =
        fs::read_to_string(root.join(report_file)).expect("source plan markdown should exist");
    assert!(markdown.contains("# 回滚清单"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_plan_blocks_unverified_application() {
    let root = temp_root("ai-patch-source-plan-unverified");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before unverified source plan");
    app.init_agent_work_queue(CURRENT_VERSION, "源码覆盖准备测试", 3)
        .expect("work queue should exist before unverified source plan");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before unverified source plan");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before unverified source plan");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before unverified source plan");

    let report = app
        .ai_patch_source_plan(CURRENT_VERSION, &application.record.id)
        .expect("unverified application should write blocked source plan");

    assert_eq!(report.record.status, AiPatchSourcePlanStatus::Blocked);
    assert!(report.record.files.is_empty());
    assert!(report.record.plan_dir.is_none());
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("验证")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_plan_blocks_failed_verification() {
    let root = temp_root("ai-patch-source-plan-failed-verification");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before failed verification source plan");
    app.init_agent_work_queue(CURRENT_VERSION, "源码覆盖准备测试", 3)
        .expect("work queue should exist before failed verification source plan");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before failed verification source plan");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before failed verification source plan");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before failed verification source plan");
    app.ai_patch_verify_with_runner(
        CURRENT_VERSION,
        &application.record.id,
        1_234,
        |spec, timeout_ms| {
            Ok(AiPatchVerificationCommandRecord {
                command: spec.command.clone(),
                program: spec.program.clone(),
                args: spec.args.clone(),
                started_at_unix_seconds: 1,
                duration_ms: 2,
                timeout_ms,
                exit_code: if spec.command == "cargo test" {
                    Some(1)
                } else {
                    Some(0)
                },
                timed_out: false,
                stdout_bytes: 0,
                stderr_bytes: 0,
                stdout_preview: String::new(),
                stderr_preview: String::new(),
                status: if spec.command == "cargo test" {
                    AiPatchVerificationStatus::Failed
                } else {
                    AiPatchVerificationStatus::Passed
                },
            })
        },
    )
    .expect("failed verification should still update application record");

    let report = app
        .ai_patch_source_plan(CURRENT_VERSION, &application.record.id)
        .expect("failed verification should write blocked source plan");

    assert_eq!(report.record.status, AiPatchSourcePlanStatus::Blocked);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("验证")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_plan_blocks_missing_mirror_file() {
    let root = temp_root("ai-patch-source-plan-missing-mirror");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing mirror source plan");
    app.init_agent_work_queue(CURRENT_VERSION, "源码覆盖准备测试", 3)
        .expect("work queue should exist before missing mirror source plan");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before missing mirror source plan");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before missing mirror source plan");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before missing mirror source plan");
    app.ai_patch_verify_with_runner(
        CURRENT_VERSION,
        &application.record.id,
        1_234,
        |spec, timeout_ms| {
            Ok(AiPatchVerificationCommandRecord {
                command: spec.command.clone(),
                program: spec.program.clone(),
                args: spec.args.clone(),
                started_at_unix_seconds: 1,
                duration_ms: 2,
                timeout_ms,
                exit_code: Some(0),
                timed_out: false,
                stdout_bytes: 0,
                stderr_bytes: 0,
                stdout_preview: String::new(),
                stderr_preview: String::new(),
                status: AiPatchVerificationStatus::Passed,
            })
        },
    )
    .expect("verification should pass before missing mirror fixture");
    let verified = app
        .ai_patch_application_record(CURRENT_VERSION, &application.record.id)
        .expect("verified application should be readable");
    fs::remove_file(root.join(&verified.files[0].mirror_file))
        .expect("test should remove mirror file");

    let report = app
        .ai_patch_source_plan(CURRENT_VERSION, &application.record.id)
        .expect("missing mirror should write blocked source plan");

    assert_eq!(report.record.status, AiPatchSourcePlanStatus::Blocked);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("候选镜像文件")
    );

    cleanup(&root);
}

fn successful_patch_verification_record(
    spec: &AiPatchVerificationCommandSpec,
    timeout_ms: u64,
) -> AiPatchVerificationCommandRecord {
    AiPatchVerificationCommandRecord {
        command: spec.command.clone(),
        program: spec.program.clone(),
        args: spec.args.clone(),
        started_at_unix_seconds: 1,
        duration_ms: 2,
        timeout_ms,
        exit_code: Some(0),
        timed_out: false,
        stdout_bytes: 0,
        stderr_bytes: 0,
        stdout_preview: String::new(),
        stderr_preview: String::new(),
        status: AiPatchVerificationStatus::Passed,
    }
}

fn create_verified_source_plan_fixture(
    root: &Path,
    app: &SelfForgeApp,
    relative_path: &str,
    existing_contents: Option<&str>,
) -> AiPatchSourcePlanReport {
    if let Some(contents) = existing_contents {
        let target_path = root.join(relative_path);
        fs::create_dir_all(target_path.parent().expect("target parent should exist"))
            .expect("test should create target parent");
        fs::write(&target_path, contents).expect("test should write target file");
    }
    app.init_agent_work_queue(CURRENT_VERSION, "源码覆盖执行测试", 3)
        .expect("work queue should exist before source execution");
    let draft = create_patch_draft_for_audit(root, app, &format!("- {relative_path}"));
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before source execution");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before source execution");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before source execution");
    app.ai_patch_verify_with_runner(
        CURRENT_VERSION,
        &application.record.id,
        1_234,
        |spec, timeout_ms| Ok(successful_patch_verification_record(spec, timeout_ms)),
    )
    .expect("verification should pass before source execution");
    app.ai_patch_source_plan(CURRENT_VERSION, &application.record.id)
        .expect("source plan should be prepared before source execution")
}

fn create_ready_source_promotion_fixture(
    root: &Path,
    app: &SelfForgeApp,
) -> AiPatchSourcePromotionReport {
    let plan = create_verified_source_plan_fixture(
        root,
        app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| Ok(successful_patch_verification_record(spec, timeout_ms)),
        )
        .expect("source execution should pass before candidate fixture");
    app.ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("source promotion should be ready before candidate fixture")
}

fn create_prepared_source_candidate_fixture(
    root: &Path,
    app: &SelfForgeApp,
) -> AiPatchSourceCandidateReport {
    let promotion = create_ready_source_promotion_fixture(root, app);
    let mut state = ForgeState::load(root).expect("state should be readable before reset");
    state.status = "active".to_string();
    state.last_verified = Some(format!("promoted:{CURRENT_VERSION}"));
    state.candidate_version = None;
    state.candidate_workspace = None;
    state
        .save(root)
        .expect("test should reset candidate state before fixture");
    app.ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("ready promotion should prepare candidate fixture")
}

#[test]
fn ai_patch_source_execute_applies_source_and_records_verification() {
    let root = temp_root("ai-patch-source-execute-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before source execution");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );

    let report = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| Ok(successful_patch_verification_record(spec, timeout_ms)),
        )
        .expect("prepared source plan should execute successfully");

    assert_eq!(report.record.status, AiPatchSourceExecutionStatus::Applied);
    assert_eq!(
        report.record.verification_status,
        AiPatchVerificationStatus::Passed
    );
    assert!(!report.record.rollback_performed);
    assert_eq!(report.record.files.len(), 1);
    let target_contents =
        fs::read_to_string(root.join("src/app/minimal_loop.rs")).expect("target should exist");
    assert!(target_contents.contains("fn example()"));
    let backup_file = report.record.files[0]
        .execution_backup_file
        .as_ref()
        .expect("existing target should have execution backup");
    let backup_contents =
        fs::read_to_string(root.join(backup_file)).expect("execution backup should exist");
    assert_eq!(backup_contents, "fn old() {}\n");
    let records = app
        .ai_patch_source_execution_records(CURRENT_VERSION, 10)
        .expect("source execution records should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_execution_record(CURRENT_VERSION, &report.record.id)
        .expect("source execution record should be readable");
    let report_file = loaded
        .report_file
        .as_ref()
        .expect("source execution should write markdown report");
    let markdown =
        fs::read_to_string(root.join(report_file)).expect("source execution markdown should exist");
    assert!(markdown.contains("# 验证结果"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_execute_blocks_unprepared_plan() {
    let root = temp_root("ai-patch-source-execute-blocked-plan");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked source execution");
    let target_path = root.join("src/app/minimal_loop.rs");
    fs::create_dir_all(target_path.parent().expect("target parent should exist"))
        .expect("test should create target parent");
    fs::write(&target_path, "fn old() {}\n").expect("test should write target file");
    app.init_agent_work_queue(CURRENT_VERSION, "源码覆盖执行测试", 3)
        .expect("work queue should exist before blocked source execution");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("audit should pass before blocked source execution");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before blocked source execution");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before blocked source execution");
    let blocked_plan = app
        .ai_patch_source_plan(CURRENT_VERSION, &application.record.id)
        .expect("unverified application should produce blocked source plan");

    let report = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &blocked_plan.record.id,
            1_234,
            |spec, timeout_ms| Ok(successful_patch_verification_record(spec, timeout_ms)),
        )
        .expect("blocked source plan should write blocked execution record");

    assert_eq!(report.record.status, AiPatchSourceExecutionStatus::Blocked);
    assert!(report.record.files.is_empty());
    assert_eq!(
        fs::read_to_string(&target_path).expect("target should remain readable"),
        "fn old() {}\n"
    );
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("准备")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_execute_rolls_back_when_verification_fails() {
    let root = temp_root("ai-patch-source-execute-rollback");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before rollback source execution");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );

    let report = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| {
                let mut record = successful_patch_verification_record(spec, timeout_ms);
                if spec.command == "cargo test" {
                    record.exit_code = Some(1);
                    record.status = AiPatchVerificationStatus::Failed;
                    record.stderr_bytes = 12;
                    record.stderr_preview = "测试失败".to_string();
                }
                Ok(record)
            },
        )
        .expect("failed verification should still write execution record");

    assert_eq!(
        report.record.status,
        AiPatchSourceExecutionStatus::RolledBack
    );
    assert_eq!(
        report.record.verification_status,
        AiPatchVerificationStatus::Failed
    );
    assert!(report.record.rollback_performed);
    assert!(!report.record.rollback_steps.is_empty());
    assert_eq!(
        fs::read_to_string(root.join("src/app/minimal_loop.rs"))
            .expect("target should be restored"),
        "fn old() {}\n"
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_execute_blocks_stale_target_file() {
    let root = temp_root("ai-patch-source-execute-stale-target");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before stale source execution");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let target_path = root.join("src/app/minimal_loop.rs");
    fs::write(&target_path, "fn changed() {}\n").expect("test should make source plan stale");

    let report = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| Ok(successful_patch_verification_record(spec, timeout_ms)),
        )
        .expect("stale source plan should write blocked execution record");

    assert_eq!(report.record.status, AiPatchSourceExecutionStatus::Blocked);
    assert!(!report.record.rollback_performed);
    assert_eq!(
        fs::read_to_string(&target_path).expect("target should remain changed"),
        "fn changed() {}\n"
    );
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("已变化")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_promotion_records_ready_link_for_successful_execution() {
    let root = temp_root("ai-patch-source-promotion-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before source promotion");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| Ok(successful_patch_verification_record(spec, timeout_ms)),
        )
        .expect("source execution should pass before promotion");
    let state_before = ForgeState::load(&root).expect("state should be readable before link");

    let report = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("successful source execution should produce promotion link");

    assert_eq!(report.source_execution.id, execution.record.id);
    assert_eq!(report.record.status, AiPatchSourcePromotionStatus::Ready);
    assert_eq!(
        report.record.verification_status,
        AiPatchVerificationStatus::Passed
    );
    assert_eq!(report.record.verification_run_count, 4);
    assert_eq!(report.record.file_count, 1);
    assert_eq!(
        report.record.next_candidate_version,
        next_version_after(CURRENT_VERSION).expect("current version should advance")
    );
    assert!(
        report
            .record
            .suggested_commit_title
            .as_deref()
            .unwrap_or_default()
            .contains(&report.record.next_candidate_version)
    );
    assert!(
        report
            .record
            .changed_files
            .contains(&"src/app/minimal_loop.rs".to_string())
    );
    assert!(!report.record.readiness_checks.is_empty());
    let records = app
        .ai_patch_source_promotion_records(CURRENT_VERSION, 10)
        .expect("promotion records should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_promotion_record(CURRENT_VERSION, &report.record.id)
        .expect("promotion record should be readable");
    let report_file = loaded
        .report_file
        .as_ref()
        .expect("promotion should write markdown report");
    let markdown =
        fs::read_to_string(root.join(report_file)).expect("promotion markdown should exist");
    assert!(markdown.contains("# 提交信息"));
    assert!(markdown.contains(&execution.record.id));
    let state_after = ForgeState::load(&root).expect("state should remain readable");
    assert_eq!(
        state_after.candidate_version,
        state_before.candidate_version
    );
    assert_eq!(state_after.status, state_before.status);

    cleanup(&root);
}

#[test]
fn ai_patch_source_promotion_blocks_rolled_back_execution() {
    let root = temp_root("ai-patch-source-promotion-rollback");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked promotion");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| {
                let mut record = successful_patch_verification_record(spec, timeout_ms);
                if spec.command == "cargo test" {
                    record.exit_code = Some(1);
                    record.status = AiPatchVerificationStatus::Failed;
                    record.stderr_preview = "测试失败".to_string();
                }
                Ok(record)
            },
        )
        .expect("rolled back source execution should still be recorded");

    let report = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("rolled back execution should produce blocked promotion link");

    assert_eq!(report.record.status, AiPatchSourcePromotionStatus::Blocked);
    assert!(report.record.suggested_commit_title.is_none());
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("未成功")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_promotion_blocks_execution_without_verification_runs() {
    let root = temp_root("ai-patch-source-promotion-no-runs");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing verification runs");
    let execution = AiPatchSourceExecutionStore::new(&root)
        .create(
            AiPatchSourceExecutionRecord {
                id: "patch-source-execution-manual-001".to_string(),
                version: CURRENT_VERSION.to_string(),
                source_plan_id: "patch-source-plan-manual".to_string(),
                application_id: "patch-application-manual".to_string(),
                candidate_version: next_version_after(CURRENT_VERSION)
                    .expect("current version should advance"),
                preview_id: "patch-preview-manual".to_string(),
                audit_id: "patch-audit-manual".to_string(),
                draft_id: "patch-draft-manual".to_string(),
                created_at_unix_seconds: 0,
                status: AiPatchSourceExecutionStatus::Applied,
                execution_dir: None,
                files: vec![AiPatchSourceExecutionFile {
                    source_path: "src/app/minimal_loop.rs".to_string(),
                    mirror_file: PathBuf::from(
                        "workspaces/v0/source/patch-applications/manual/src/app/minimal_loop.rs",
                    ),
                    target_file: PathBuf::from("src/app/minimal_loop.rs"),
                    target_existed_before: true,
                    before_bytes: 12,
                    after_bytes: 24,
                    execution_backup_file: None,
                    action: "覆盖目标文件。".to_string(),
                    rollback_action: "恢复目标文件。".to_string(),
                }],
                verification_commands: vec!["cargo test".to_string()],
                verification_runs: Vec::new(),
                verification_status: AiPatchVerificationStatus::Passed,
                rollback_performed: false,
                rollback_steps: Vec::new(),
                report_file: None,
                error: None,
                file: PathBuf::new(),
            },
            None,
        )
        .expect("manual source execution fixture should be written");

    let report = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.id)
        .expect("missing verification runs should produce blocked promotion link");

    assert_eq!(report.record.status, AiPatchSourcePromotionStatus::Blocked);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("缺少验证运行")
    );
    assert!(report.record.suggested_commit_title.is_none());

    cleanup(&root);
}

#[test]
fn ai_patch_source_candidate_prepares_next_candidate_from_ready_promotion() {
    let root = temp_root("ai-patch-source-candidate-prepare");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before source candidate");
    let promotion = create_ready_source_promotion_fixture(&root, &app);
    let mut state = ForgeState::load(&root).expect("state should be readable before reset");
    state.status = "active".to_string();
    state.last_verified = Some(format!("promoted:{CURRENT_VERSION}"));
    state.candidate_version = None;
    state.candidate_workspace = None;
    state
        .save(&root)
        .expect("test should reset candidate state");

    let report = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("ready promotion should prepare next candidate");

    assert_eq!(report.promotion.id, promotion.record.id);
    assert_eq!(report.record.status, AiPatchSourceCandidateStatus::Prepared);
    assert_eq!(
        report.record.candidate_version,
        next_version_after(CURRENT_VERSION).expect("current version should advance")
    );
    assert_eq!(
        report.record.candidate_version_after.as_deref(),
        Some(report.record.candidate_version.as_str())
    );
    assert_eq!(report.record.state_status_after, "candidate_prepared");
    assert!(report.record.candidate_checked_path_count >= 30);
    assert!(report.record.created_path_count <= report.record.existing_path_count);
    let state = ForgeState::load(&root).expect("state should be readable after candidate");
    assert_eq!(
        state.candidate_version.as_deref(),
        Some(report.record.candidate_version.as_str())
    );
    let records = app
        .ai_patch_source_candidate_records(CURRENT_VERSION, 10)
        .expect("candidate records should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_candidate_record(CURRENT_VERSION, &report.record.id)
        .expect("candidate record should be readable");
    let report_file = loaded
        .report_file
        .as_ref()
        .expect("candidate should write markdown report");
    let markdown =
        fs::read_to_string(root.join(report_file)).expect("candidate markdown should exist");
    assert!(markdown.contains("# 状态变化"));
    assert!(markdown.contains("cargo run -- cycle"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_candidate_reuses_existing_matching_candidate() {
    let root = temp_root("ai-patch-source-candidate-reuse");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before candidate reuse");
    let promotion = create_ready_source_promotion_fixture(&root, &app);
    let state_before = ForgeState::load(&root).expect("state should be readable before reuse");

    let report = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("matching candidate should be reused");

    assert_eq!(report.record.status, AiPatchSourceCandidateStatus::Reused);
    assert_eq!(
        report.record.candidate_version_after,
        state_before.candidate_version
    );
    assert!(report.record.candidate_checked_path_count >= 30);
    let state_after = ForgeState::load(&root).expect("state should be readable after reuse");
    assert_eq!(state_after, state_before);

    cleanup(&root);
}

#[test]
fn ai_patch_source_candidate_blocks_unready_promotion() {
    let root = temp_root("ai-patch-source-candidate-unready");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked candidate");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| {
                let mut record = successful_patch_verification_record(spec, timeout_ms);
                if spec.command == "cargo test" {
                    record.exit_code = Some(1);
                    record.status = AiPatchVerificationStatus::Failed;
                }
                Ok(record)
            },
        )
        .expect("rolled back execution should still be recorded");
    let promotion = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("rolled back execution should produce blocked promotion");
    let state_before =
        ForgeState::load(&root).expect("state should be readable before blocked candidate");

    let report = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("blocked promotion should write blocked candidate record");

    assert_eq!(report.record.status, AiPatchSourceCandidateStatus::Blocked);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("未就绪")
    );
    let state_after =
        ForgeState::load(&root).expect("state should be readable after blocked candidate");
    assert_eq!(state_after, state_before);

    cleanup(&root);
}

#[test]
fn ai_patch_source_candidate_blocks_when_open_errors_exist() {
    let root = temp_root("ai-patch-source-candidate-open-errors");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before open-error candidate");
    let promotion = create_ready_source_promotion_fixture(&root, &app);
    let program = std::env::current_exe()
        .expect("current test executable should be known")
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
        .expect("failed run should be archived before candidate");

    let report = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("open errors should write blocked candidate record");

    assert_eq!(report.record.status, AiPatchSourceCandidateStatus::Blocked);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("未解决错误")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_cycle_promotes_prepared_candidate_and_records_result() {
    let root = temp_root("ai-patch-source-cycle-promote");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before source cycle");
    let candidate = create_prepared_source_candidate_fixture(&root, &app);

    let report = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("prepared candidate should complete source cycle");

    assert_eq!(report.candidate.id, candidate.record.id);
    assert_eq!(report.record.status, AiPatchSourceCycleStatus::Promoted);
    assert_eq!(
        report.record.cycle_result,
        Some(AiPatchSourceCycleResult::Promoted)
    );
    assert_eq!(report.record.stable_version_before, CURRENT_VERSION);
    assert_eq!(
        report.record.stable_version_after,
        candidate.record.candidate_version
    );
    assert_eq!(report.record.state_status_after, "active");
    assert_eq!(report.record.candidate_version_after, None);
    assert!(report.record.preflight_can_advance);
    assert!(report.record.preflight_current_checked_path_count >= 30);
    assert!(report.record.preflight_candidate_checked_path_count >= 30);
    assert!(report.record.cycle_candidate_checked_path_count >= 30);
    let state = ForgeState::load(&root).expect("state should be readable after source cycle");
    assert_eq!(state.current_version, candidate.record.candidate_version);
    assert_eq!(state.candidate_version, None);
    assert_eq!(state.status, "active");
    let records = app
        .ai_patch_source_cycle_records(CURRENT_VERSION, 10)
        .expect("cycle records should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_cycle_record(CURRENT_VERSION, &report.record.id)
        .expect("cycle record should be readable");
    let report_file = loaded
        .report_file
        .as_ref()
        .expect("cycle should write markdown report");
    let markdown = fs::read_to_string(root.join(report_file)).expect("cycle markdown should exist");
    assert!(markdown.contains("# cycle 结果"));
    assert!(markdown.contains("候选版本已提升为稳定版本"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_cycle_blocks_unprepared_candidate_record() {
    let root = temp_root("ai-patch-source-cycle-blocked-candidate");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked source cycle");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| {
                let mut record = successful_patch_verification_record(spec, timeout_ms);
                if spec.command == "cargo test" {
                    record.exit_code = Some(1);
                    record.status = AiPatchVerificationStatus::Failed;
                }
                Ok(record)
            },
        )
        .expect("rolled back execution should still be recorded");
    let promotion = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("rolled back execution should produce blocked promotion");
    let candidate = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("blocked promotion should write blocked candidate record");
    let state_before =
        ForgeState::load(&root).expect("state should be readable before blocked source cycle");

    let report = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("blocked candidate should write blocked cycle record");

    assert_eq!(report.record.status, AiPatchSourceCycleStatus::Blocked);
    assert_eq!(report.record.cycle_result, None);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("未准备完成")
    );
    let state_after =
        ForgeState::load(&root).expect("state should be readable after blocked source cycle");
    assert_eq!(state_after, state_before);

    cleanup(&root);
}

#[test]
fn ai_patch_source_cycle_blocks_when_state_candidate_mismatch() {
    let root = temp_root("ai-patch-source-cycle-state-mismatch");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before mismatched source cycle");
    let candidate = create_prepared_source_candidate_fixture(&root, &app);
    let mut state =
        ForgeState::load(&root).expect("state should be readable before mismatch fixture");
    state.candidate_version = Some("v0.9.9".to_string());
    state
        .save(&root)
        .expect("test should persist mismatched candidate version");
    let state_before =
        ForgeState::load(&root).expect("state should be readable before mismatch cycle");

    let report = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("mismatched candidate should write blocked cycle record");

    assert_eq!(report.record.status, AiPatchSourceCycleStatus::Blocked);
    assert_eq!(report.record.cycle_result, None);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("状态文件候选版本")
    );
    let state_after =
        ForgeState::load(&root).expect("state should be readable after mismatch cycle");
    assert_eq!(state_after, state_before);

    cleanup(&root);
}

#[test]
fn ai_patch_source_cycle_summary_records_promoted_follow_up() {
    let root = temp_root("ai-patch-source-cycle-summary-promoted");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before promoted cycle summary");
    let candidate = create_prepared_source_candidate_fixture(&root, &app);
    let cycle = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("prepared candidate should complete cycle before summary");

    let summary = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, &cycle.record.id)
        .expect("promoted cycle should produce follow-up summary");

    assert_eq!(summary.cycle.id, cycle.record.id);
    assert_eq!(
        summary.record.status,
        AiPatchSourceCycleFollowUpStatus::Promoted
    );
    assert_eq!(
        summary.record.cycle_result,
        Some(AiPatchSourceCycleResult::Promoted)
    );
    assert_eq!(
        summary.record.stable_version_after,
        candidate.record.candidate_version
    );
    assert!(summary.record.memory_compaction_recommended);
    assert!(summary.record.next_goal.contains("记忆压缩"));
    assert!(
        summary
            .record
            .follow_up_commands
            .iter()
            .any(|command| command.contains("memory-compact"))
    );
    let records = app
        .ai_patch_source_cycle_summary_records(CURRENT_VERSION, 10)
        .expect("cycle summaries should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_cycle_summary_record(CURRENT_VERSION, &summary.record.id)
        .expect("cycle summary record should be readable");
    let markdown = fs::read_to_string(root.join(&loaded.markdown_file))
        .expect("cycle summary markdown should be readable");
    assert!(markdown.contains("# AI 补丁源码覆盖 cycle 后续总结"));
    assert!(markdown.contains("# 记忆与任务建议"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_cycle_summary_records_blocked_follow_up() {
    let root = temp_root("ai-patch-source-cycle-summary-blocked");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked cycle summary");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| {
                let mut record = successful_patch_verification_record(spec, timeout_ms);
                if spec.command == "cargo test" {
                    record.exit_code = Some(1);
                    record.status = AiPatchVerificationStatus::Failed;
                }
                Ok(record)
            },
        )
        .expect("rolled back execution should still be recorded");
    let promotion = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("rolled back execution should produce blocked promotion");
    let candidate = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("blocked promotion should write blocked candidate record");
    let cycle = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("blocked candidate should write blocked cycle record");

    let summary = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, &cycle.record.id)
        .expect("blocked cycle should produce follow-up summary");

    assert_eq!(
        summary.record.status,
        AiPatchSourceCycleFollowUpStatus::Blocked
    );
    assert_eq!(summary.record.cycle_result, None);
    assert!(!summary.record.memory_compaction_recommended);
    assert!(summary.record.next_goal.contains("修复"));
    assert!(
        summary
            .record
            .follow_up_commands
            .iter()
            .any(|command| command.contains("agent-patch-source-cycle"))
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_cycle_summary_reports_missing_cycle_record() {
    let root = temp_root("ai-patch-source-cycle-summary-missing");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing cycle summary");

    let error = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, "patch-source-cycle-missing")
        .expect_err("missing cycle record should be reported");

    assert!(matches!(
        error,
        AiPatchSourceCycleSummaryError::Cycle(AiPatchSourceCycleStoreError::NotFound { .. })
    ));

    cleanup(&root);
}

#[test]
fn ai_patch_source_task_draft_records_promoted_summary_task() {
    let root = temp_root("ai-patch-source-task-draft-promoted");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before promoted task draft");
    let candidate = create_prepared_source_candidate_fixture(&root, &app);
    let cycle = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("prepared candidate should complete cycle before task draft");
    let summary = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, &cycle.record.id)
        .expect("promoted cycle should produce summary before task draft");

    let draft = app
        .ai_patch_source_task_draft(CURRENT_VERSION, &summary.record.id)
        .expect("promoted summary should produce task draft");

    assert_eq!(draft.summary.id, summary.record.id);
    assert_eq!(draft.record.status, AiPatchSourceTaskDraftStatus::Drafted);
    assert_eq!(
        draft.record.source_status,
        AiPatchSourceCycleFollowUpStatus::Promoted
    );
    assert_eq!(
        draft.record.suggested_target_version,
        next_version_after(&summary.record.stable_version_after)
            .expect("test should compute next target version")
    );
    assert!(draft.record.required_audit);
    assert!(draft.record.error.is_none());
    assert!(draft.record.proposed_task_title.contains("下一轮 patch"));
    assert!(
        draft
            .record
            .acceptance_checks
            .iter()
            .any(|check| check == "cargo test")
    );
    let records = app
        .ai_patch_source_task_draft_records(CURRENT_VERSION, 10)
        .expect("task drafts should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_task_draft_record(CURRENT_VERSION, &draft.record.id)
        .expect("task draft record should be readable");
    let markdown = fs::read_to_string(root.join(&loaded.markdown_file))
        .expect("task draft markdown should be readable");
    assert!(markdown.contains("# AI 补丁源码覆盖下一任务草案"));
    assert!(markdown.contains("# 任务草案"));
    assert!(markdown.contains("# 验收检查"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_task_draft_records_blocked_summary_repair_task() {
    let root = temp_root("ai-patch-source-task-draft-blocked");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked task draft");
    let plan = create_verified_source_plan_fixture(
        &root,
        &app,
        "src/app/minimal_loop.rs",
        Some("fn old() {}\n"),
    );
    let execution = app
        .ai_patch_source_execute_with_runner(
            CURRENT_VERSION,
            &plan.record.id,
            1_234,
            |spec, timeout_ms| {
                let mut record = successful_patch_verification_record(spec, timeout_ms);
                if spec.command == "cargo test" {
                    record.exit_code = Some(1);
                    record.status = AiPatchVerificationStatus::Failed;
                }
                Ok(record)
            },
        )
        .expect("failed execution should be recorded before blocked task draft");
    let promotion = app
        .ai_patch_source_promotion(CURRENT_VERSION, &execution.record.id)
        .expect("failed execution should produce blocked promotion");
    let candidate = app
        .ai_patch_source_candidate(CURRENT_VERSION, &promotion.record.id)
        .expect("blocked promotion should write blocked candidate record");
    let cycle = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("blocked candidate should write cycle record before task draft");
    let summary = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, &cycle.record.id)
        .expect("blocked cycle should produce summary before task draft");

    let draft = app
        .ai_patch_source_task_draft(CURRENT_VERSION, &summary.record.id)
        .expect("blocked summary should produce repair task draft");

    assert_eq!(draft.record.status, AiPatchSourceTaskDraftStatus::Drafted);
    assert_eq!(
        draft.record.source_status,
        AiPatchSourceCycleFollowUpStatus::Blocked
    );
    assert!(draft.record.proposed_task_title.contains("修复"));
    assert!(draft.record.proposed_task_description.contains("被阻断"));
    assert!(
        draft
            .record
            .follow_up_commands
            .iter()
            .any(|command| command.contains("agent-plan"))
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_task_draft_reports_missing_summary_record() {
    let root = temp_root("ai-patch-source-task-draft-missing");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing task draft");

    let error = app
        .ai_patch_source_task_draft(CURRENT_VERSION, "patch-source-cycle-summary-missing")
        .expect_err("missing summary record should be reported");

    assert!(matches!(
        error,
        AiPatchSourceTaskDraftError::Summary(AiPatchSourceCycleFollowUpStoreError::NotFound { .. })
    ));

    cleanup(&root);
}

#[test]
fn ai_patch_source_task_audit_approves_complete_task_draft() {
    let root = temp_root("ai-patch-source-task-audit-approved");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before approved task audit");
    let candidate = create_prepared_source_candidate_fixture(&root, &app);
    let cycle = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("prepared candidate should complete cycle before task audit");
    let summary = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, &cycle.record.id)
        .expect("promoted cycle should produce summary before task audit");
    let draft = app
        .ai_patch_source_task_draft(CURRENT_VERSION, &summary.record.id)
        .expect("summary should produce task draft before audit");

    let audit = app
        .ai_patch_source_task_audit(CURRENT_VERSION, &draft.record.id)
        .expect("complete task draft should produce approved audit");

    assert_eq!(audit.task_draft.id, draft.record.id);
    assert_eq!(audit.record.status, AiPatchSourceTaskAuditStatus::Approved);
    assert!(audit.record.blocked_reason.is_none());
    assert!(audit.record.findings.iter().all(|finding| finding.passed));
    assert!(
        audit
            .record
            .follow_up_commands
            .iter()
            .any(|command| command.contains("agent-patch-draft"))
    );
    let records = app
        .ai_patch_source_task_audit_records(CURRENT_VERSION, 10)
        .expect("task audits should be queryable");
    assert_eq!(records.len(), 1);
    let loaded = app
        .ai_patch_source_task_audit_record(CURRENT_VERSION, &audit.record.id)
        .expect("task audit record should be readable");
    let markdown = fs::read_to_string(root.join(&loaded.markdown_file))
        .expect("task audit markdown should be readable");
    assert!(markdown.contains("# AI 补丁源码覆盖任务草案审计"));
    assert!(markdown.contains("# 审计发现"));
    assert!(markdown.contains("# 后续命令"));

    cleanup(&root);
}

#[test]
fn ai_patch_source_task_audit_blocks_invalid_task_draft() {
    let root = temp_root("ai-patch-source-task-audit-blocked");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked task audit");
    let candidate = create_prepared_source_candidate_fixture(&root, &app);
    let cycle = app
        .ai_patch_source_cycle(CURRENT_VERSION, &candidate.record.id)
        .expect("prepared candidate should complete cycle before blocked task audit");
    let summary = app
        .ai_patch_source_cycle_summary(CURRENT_VERSION, &cycle.record.id)
        .expect("promoted cycle should produce summary before blocked task audit");
    let draft = app
        .ai_patch_source_task_draft(CURRENT_VERSION, &summary.record.id)
        .expect("summary should produce task draft before blocked audit");
    let mut invalid = draft.record.clone();
    invalid.id = String::new();
    invalid.created_at_unix_seconds = 0;
    invalid.status = AiPatchSourceTaskDraftStatus::Blocked;
    invalid.required_audit = false;
    invalid.acceptance_checks = vec!["cargo test".to_string()];
    invalid.follow_up_commands.clear();
    invalid.error = Some("测试构造的阻断草案".to_string());
    invalid.markdown_file = PathBuf::new();
    invalid.file = PathBuf::new();
    let invalid = AiPatchSourceTaskDraftStore::new(&root)
        .create(invalid, "# 测试阻断任务草案\n")
        .expect("invalid task draft should be stored for audit test");

    let audit = app
        .ai_patch_source_task_audit(CURRENT_VERSION, &invalid.id)
        .expect("invalid task draft should produce blocked audit");

    assert_eq!(audit.record.status, AiPatchSourceTaskAuditStatus::Blocked);
    assert!(
        audit
            .record
            .blocked_reason
            .as_deref()
            .unwrap_or_default()
            .contains("草案状态")
    );
    assert!(audit.record.findings.iter().any(|finding| !finding.passed));
    assert!(
        !audit
            .record
            .follow_up_commands
            .iter()
            .any(|command| command.contains("agent-patch-draft"))
    );

    cleanup(&root);
}

#[test]
fn ai_patch_source_task_audit_reports_missing_task_draft() {
    let root = temp_root("ai-patch-source-task-audit-missing");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing task audit");

    let error = app
        .ai_patch_source_task_audit(CURRENT_VERSION, "patch-source-task-draft-missing")
        .expect_err("missing task draft should be reported");

    assert!(matches!(
        error,
        AiPatchSourceTaskAuditError::TaskDraft(AiPatchSourceTaskDraftStoreError::NotFound { .. })
    ));

    cleanup(&root);
}
