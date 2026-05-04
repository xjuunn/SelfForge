use super::*;

#[test]
fn ai_patch_draft_preview_builds_controlled_prompt_from_memory() {
    let root = temp_root("ai-patch-draft-preview");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before AI patch draft preview");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-draft-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控进化基础。\n\n# 优化建议\n\n实现 AI 生成补丁的沙箱化草案流程。\n\n# 可复用经验\n\nAI 写代码前必须先生成计划和测试。\n"
            ),
        )
        .expect("test should write memory archive");

    let preview = app
        .ai_patch_draft_preview_with_lookup("生成补丁草案", |_| None)
        .expect("preview should build controlled AI patch draft request");

    assert_eq!(preview.current_version, CURRENT_VERSION);
    assert_eq!(
        preview.target_version,
        next_version_after(CURRENT_VERSION).expect("next version should parse")
    );
    assert_eq!(preview.request.provider_id, "deepseek");
    assert_eq!(preview.insights.source_versions, vec![CURRENT_VERSION]);
    assert!(preview.prompt.contains("中文 Markdown"));
    assert!(preview.prompt.contains("# 计划"));
    assert!(preview.prompt.contains("# 测试草案"));
    assert!(preview.prompt.contains("patch-drafts"));
    assert!(preview.prompt.contains("禁止修改 runtime 和 supervisor"));
    assert!(
        !preview
            .request
            .body
            .to_string()
            .contains("test-patch-draft-key")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_draft_success_writes_record_and_markdown_without_key_or_prompt() {
    let root = temp_root("ai-patch-draft-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before AI patch draft success");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-draft-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控进化基础。\n\n# 优化建议\n\n实现 AI 生成补丁的沙箱化草案流程。\n\n# 可复用经验\n\nAI 写代码前必须先生成计划和测试。\n"
            ),
        )
        .expect("test should write memory archive");
    let preview = app
        .ai_patch_draft_preview_with_lookup("生成补丁草案", |_| None)
        .expect("preview should build before finishing patch draft");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
            request,
            response: AiTextResponse {
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                protocol: "openai-chat-completions".to_string(),
                text: "# 补丁目标\n生成受控补丁草案。\n\n# 计划\n1. 先记录计划。\n2. 再生成测试。\n\n# 允许写入范围\n只写入 patch-drafts 目录。\n\n# 代码草案\n```rust\nfn example() {}\n```\n\n# 测试草案\n```rust\n#[test]\nfn example_test() {}\n```\n\n# 验证命令\ncargo test\n\n# 风险与回滚\n失败时删除草案记录。\n"
                    .to_string(),
                raw_bytes: 256,
            },
            status_code: 200,
        };

    let report = app
        .finish_ai_patch_draft(preview, ai)
        .expect("successful AI patch draft should write record");

    assert_eq!(report.record.status, AiPatchDraftStatus::Succeeded);
    assert_eq!(report.record.goal, "生成补丁草案");
    let draft_file = report
        .record
        .draft_file
        .as_ref()
        .expect("successful draft should have markdown file");
    let draft_file_text = draft_file.to_string_lossy();
    assert!(
        draft_file_text.contains("workspaces\\v0\\artifacts\\agents\\patch-drafts")
            || draft_file_text.contains("workspaces/v0/artifacts/agents/patch-drafts")
    );
    let draft_contents =
        fs::read_to_string(root.join(draft_file)).expect("draft markdown should be readable");
    assert!(draft_contents.contains("# 计划"));
    assert!(draft_contents.contains("# 测试草案"));
    assert!(!draft_contents.contains("test-patch-draft-key"));
    assert!(!draft_contents.contains("你是 SelfForge 的 AI 补丁草案 Agent"));

    let records = app
        .ai_patch_draft_records(CURRENT_VERSION, 10)
        .expect("patch draft records should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.record.id);
    let loaded = app
        .ai_patch_draft_record(CURRENT_VERSION, &report.record.id)
        .expect("patch draft record should be readable");
    assert_eq!(loaded.id, report.record.id);
    assert_eq!(loaded.draft_file, report.record.draft_file);

    app.supervisor()
        .verify_current_version()
        .expect("generated Chinese markdown draft should pass validation");

    cleanup(&root);
}

#[test]
fn ai_patch_draft_missing_test_section_writes_failed_record() {
    let root = temp_root("ai-patch-draft-invalid");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before failed patch draft");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-draft-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控进化基础。\n\n# 优化建议\n\n实现 AI 生成补丁的沙箱化草案流程。\n\n# 可复用经验\n\nAI 写代码前必须先生成计划和测试。\n"
            ),
        )
        .expect("test should write memory archive");
    let preview = app
        .ai_patch_draft_preview_with_lookup("生成补丁草案", |_| None)
        .expect("preview should build before invalid patch draft");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
        request,
        response: AiTextResponse {
            provider_id: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            protocol: "openai-chat-completions".to_string(),
            text: "# 补丁目标\n生成受控补丁草案。\n\n# 计划\n只有计划，没有测试章节。\n"
                .to_string(),
            raw_bytes: 80,
        },
        status_code: 200,
    };

    let error = app
        .finish_ai_patch_draft(preview, ai)
        .expect_err("draft without test section should fail");

    assert!(matches!(
        error,
        AiPatchDraftError::InvalidDraft { ref reason, .. } if reason.contains("测试")
    ));
    let records = app
        .ai_patch_draft_records(CURRENT_VERSION, 10)
        .expect("failed patch draft record should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, AiPatchDraftStatus::Failed);
    assert!(records[0].draft_file.is_none());
    assert!(
        records[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("测试")
    );
    let contents = fs::read_to_string(root.join(&records[0].file))
        .expect("failed record file should be readable");
    assert!(!contents.contains("test-patch-draft-key"));
    assert!(!contents.contains("你是 SelfForge 的 AI 补丁草案 Agent"));

    cleanup(&root);
}

fn create_task_audit_record_for_patch_draft(
    root: &Path,
    status: AiPatchSourceTaskAuditStatus,
    approved_goal: &str,
) -> AiPatchSourceTaskAuditRecord {
    AiPatchSourceTaskAuditStore::new(root)
        .create(
            AiPatchSourceTaskAuditRecord {
                id: String::new(),
                version: CURRENT_VERSION.to_string(),
                task_draft_id: "patch-source-task-draft-test".to_string(),
                summary_id: "patch-source-cycle-summary-test".to_string(),
                cycle_id: "patch-source-cycle-test".to_string(),
                created_at_unix_seconds: 0,
                status,
                source_task_status: AiPatchSourceTaskDraftStatus::Drafted,
                proposed_task_title: "生成已审计补丁草案".to_string(),
                proposed_task_description: "用于测试已审计任务草案进入补丁草案流程。".to_string(),
                suggested_target_version: next_version_after(CURRENT_VERSION)
                    .expect("next version should parse"),
                approved_goal: approved_goal.to_string(),
                findings: vec![AiPatchSourceTaskAuditFinding {
                    check: "测试审计".to_string(),
                    passed: status == AiPatchSourceTaskAuditStatus::Approved,
                    message: "测试审计记录。".to_string(),
                }],
                follow_up_commands: vec![
                    "cargo run -- preflight".to_string(),
                    format!("cargo run -- agent-patch-draft \"{approved_goal}\""),
                ],
                blocked_reason: if status == AiPatchSourceTaskAuditStatus::Approved {
                    None
                } else {
                    Some("测试阻断审计".to_string())
                },
                markdown_file: PathBuf::new(),
                file: PathBuf::new(),
            },
            "# 测试任务草案审计\n",
        )
        .expect("test task audit should be stored")
}

#[test]
fn ai_patch_draft_preview_from_task_audit_uses_approved_goal() {
    let root = temp_root("ai-patch-draft-from-task-audit-preview");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before task audit patch draft preview");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-draft-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备任务草案审计闸门。\n\n# 优化建议\n\n让补丁草案读取已批准审计记录。\n\n# 可复用经验\n\n补丁草案必须使用已批准目标。\n"
            ),
        )
        .expect("test should write memory archive");
    let audit = create_task_audit_record_for_patch_draft(
        &root,
        AiPatchSourceTaskAuditStatus::Approved,
        "根据已批准任务草案审计生成补丁草案",
    );

    let preview = app
        .ai_patch_draft_preview_from_task_audit_with_lookup(&audit.id, |_| None)
        .expect("approved task audit should build patch draft preview");

    assert_eq!(preview.goal, "根据已批准任务草案审计生成补丁草案");
    assert!(
        preview
            .prompt
            .contains("根据已批准任务草案审计生成补丁草案")
    );
    assert_eq!(preview.request.provider_id, "deepseek");
    assert_eq!(
        preview.source_task_audit_id.as_deref(),
        Some(audit.id.as_str())
    );

    cleanup(&root);
}

#[test]
fn ai_patch_draft_from_task_audit_persists_source_audit_id() {
    let root = temp_root("ai-patch-draft-from-task-audit-source-link");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before task audit source link test");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-draft-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备任务草案审计闸门。\n\n# 优化建议\n\n让补丁草案保存来源审计编号。\n\n# 可复用经验\n\n补丁草案必须能反查来源审计。\n"
            ),
        )
        .expect("test should write memory archive");
    let audit = create_task_audit_record_for_patch_draft(
        &root,
        AiPatchSourceTaskAuditStatus::Approved,
        "保存来源任务草案审计编号",
    );
    let preview = app
        .ai_patch_draft_preview_from_task_audit_with_lookup(&audit.id, |_| None)
        .expect("approved task audit should build linked preview");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
            request,
            response: AiTextResponse {
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                protocol: "openai-chat-completions".to_string(),
                text: "# 补丁目标\n保存来源任务草案审计编号。\n\n# 计划\n1. 扩展记录结构。\n2. 补充测试。\n\n# 允许写入范围\n只写入 patch-drafts 目录。\n\n# 代码草案\n```rust\nfn source_link() {}\n```\n\n# 测试草案\n```rust\n#[test]\nfn source_link_test() {}\n```\n\n# 验证命令\ncargo test\n\n# 风险与回滚\n失败时保留当前稳定版本。\n"
                    .to_string(),
                raw_bytes: 256,
            },
            status_code: 200,
        };

    let report = app
        .finish_ai_patch_draft(preview, ai)
        .expect("successful linked draft should be stored");

    assert_eq!(
        report.record.source_task_audit_id.as_deref(),
        Some(audit.id.as_str())
    );
    let records = app
        .ai_patch_draft_records(CURRENT_VERSION, 10)
        .expect("patch draft summaries should be queryable");
    assert_eq!(
        records[0].source_task_audit_id.as_deref(),
        Some(audit.id.as_str())
    );
    let loaded = app
        .ai_patch_draft_record(CURRENT_VERSION, &report.record.id)
        .expect("linked patch draft record should be readable");
    assert_eq!(
        loaded.source_task_audit_id.as_deref(),
        Some(audit.id.as_str())
    );

    cleanup(&root);
}

#[test]
fn ai_patch_draft_preview_from_task_audit_rejects_blocked_audit() {
    let root = temp_root("ai-patch-draft-from-task-audit-blocked");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked task audit patch draft preview");
    let audit = create_task_audit_record_for_patch_draft(
        &root,
        AiPatchSourceTaskAuditStatus::Blocked,
        "被阻断的补丁草案目标",
    );

    let error = app
        .ai_patch_draft_preview_from_task_audit_with_lookup(&audit.id, |_| None)
        .expect_err("blocked task audit should not build patch draft preview");

    assert!(matches!(
        error,
        AiPatchDraftError::TaskAuditNotApproved {
            status: AiPatchSourceTaskAuditStatus::Blocked,
            ..
        }
    ));

    cleanup(&root);
}

#[test]
fn ai_patch_draft_preview_from_task_audit_reports_missing_audit() {
    let root = temp_root("ai-patch-draft-from-task-audit-missing");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing task audit patch draft preview");

    let error = app
        .ai_patch_draft_preview_from_task_audit_with_lookup(
            "patch-source-task-audit-missing",
            |_| None,
        )
        .expect_err("missing task audit should be reported");

    assert!(matches!(
        error,
        AiPatchDraftError::TaskAudit(AiPatchSourceTaskAuditStoreError::NotFound { .. })
    ));

    cleanup(&root);
}

#[test]
fn ai_patch_audit_passes_clean_scope_and_writes_record() {
    let root = temp_root("ai-patch-audit-pass");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before patch audit");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁审计测试", 3)
        .expect("work queue should exist for conflict audit");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");

    let report = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean patch draft should be audited");

    assert_eq!(report.record.status, AiPatchAuditStatus::Passed);
    assert_eq!(report.record.draft_id, draft.id);
    assert_eq!(
        report.record.normalized_write_scope,
        vec!["src/app/minimal_loop.rs"]
    );
    assert_eq!(report.record.active_conflict_count, 0);
    assert_eq!(report.record.finding_count, 0);
    assert!(
        report
            .record
            .file
            .to_string_lossy()
            .contains("patch-audits")
    );
    let records = app
        .ai_patch_audit_records(CURRENT_VERSION, 10)
        .expect("patch audit records should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.record.id);
    let loaded = app
        .ai_patch_audit_record(CURRENT_VERSION, &report.record.id)
        .expect("patch audit record should be readable");
    assert_eq!(loaded.id, report.record.id);

    cleanup(&root);
}

#[test]
fn ai_patch_audit_persists_source_task_audit_id() {
    let root = temp_root("ai-patch-audit-source-task-audit-id");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before linked patch audit");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁审计来源追踪测试", 3)
        .expect("work queue should exist for linked patch audit");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-audit-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备补丁草案来源追踪。\n\n# 优化建议\n\n把来源审计编号贯通到补丁审计记录。\n\n# 可复用经验\n\n追踪链路必须使用结构化字段。\n"
            ),
        )
        .expect("test should write memory archive");
    let task_audit = create_task_audit_record_for_patch_draft(
        &root,
        AiPatchSourceTaskAuditStatus::Approved,
        "生成带来源追踪的补丁审计记录",
    );
    let preview = app
        .ai_patch_draft_preview_from_task_audit_with_lookup(&task_audit.id, |_| None)
        .expect("approved task audit should build linked patch draft preview");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
            request,
            response: AiTextResponse {
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                protocol: "openai-chat-completions".to_string(),
                text: "# 补丁目标\n生成带来源追踪的补丁审计记录。\n\n# 计划\n1. 扩展审计记录。\n2. 补充追踪测试。\n\n# 允许写入范围\n- src/app/agent/patch_audit.rs\n\n# 代码草案\n```rust\nfn audit_source_link() {}\n```\n\n# 测试草案\n```rust\n#[test]\nfn audit_source_link_test() {}\n```\n\n# 验证命令\ncargo test\n\n# 风险与回滚\n失败时保留当前稳定版本。\n"
                    .to_string(),
                raw_bytes: 256,
            },
            status_code: 200,
        };
    let draft = app
        .finish_ai_patch_draft(preview, ai)
        .expect("linked patch draft should be stored")
        .record;

    let report = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("linked patch draft should be audited");

    assert_eq!(report.record.status, AiPatchAuditStatus::Passed);
    assert_eq!(
        report.record.source_task_audit_id.as_deref(),
        Some(task_audit.id.as_str())
    );
    let records = app
        .ai_patch_audit_records(CURRENT_VERSION, 10)
        .expect("patch audit summaries should be queryable");
    assert_eq!(
        records[0].source_task_audit_id.as_deref(),
        Some(task_audit.id.as_str())
    );
    let loaded = app
        .ai_patch_audit_record(CURRENT_VERSION, &report.record.id)
        .expect("patch audit record should be readable");
    assert_eq!(
        loaded.source_task_audit_id.as_deref(),
        Some(task_audit.id.as_str())
    );

    cleanup(&root);
}

#[test]
fn ai_patch_audit_rejects_protected_scope() {
    let root = temp_root("ai-patch-audit-protected");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before protected audit");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁审计测试", 3)
        .expect("work queue should exist for protected audit");
    let draft = create_patch_draft_for_audit(&root, &app, "- runtime/README.md");

    let report = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("protected patch draft should still write an audit record");

    assert_eq!(report.record.status, AiPatchAuditStatus::Failed);
    assert!(report.record.findings.iter().any(|finding| {
        finding.kind == AiPatchAuditFindingKind::ProtectedPath
            && finding.path.as_deref() == Some("runtime/README.md")
    }));

    cleanup(&root);
}

#[test]
fn ai_patch_audit_detects_active_write_scope_conflict() {
    let root = temp_root("ai-patch-audit-conflict");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before conflict audit");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁审计测试", 3)
        .expect("work queue should exist before claiming");
    app.claim_agent_work(CURRENT_VERSION, "ai-1", Some("builder"))
        .expect("builder task should be claimed");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/agent/patch_audit.rs");

    let report = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("conflicting patch draft should write an audit record");

    assert_eq!(report.record.status, AiPatchAuditStatus::Failed);
    assert_eq!(report.record.active_conflict_count, 1);
    assert!(report.record.findings.iter().any(|finding| {
        finding.kind == AiPatchAuditFindingKind::ActiveConflict
            && finding.task_id.as_deref() == Some("coord-002-application")
            && finding.worker_id.as_deref() == Some("ai-1")
    }));

    cleanup(&root);
}

#[test]
fn ai_patch_preview_writes_auditable_preview_without_touching_source() {
    let root = temp_root("ai-patch-preview-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before patch preview");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁预演测试", 3)
        .expect("work queue should exist for patch preview");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before patch preview");

    let report = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("passed audit should produce patch preview");

    assert_eq!(report.record.status, AiPatchPreviewStatus::Previewed);
    assert_eq!(report.record.audit_id, audit.record.id);
    assert_eq!(report.record.draft_id, draft.id);
    assert_eq!(report.record.change_count, 1);
    assert_eq!(report.record.code_block_count, 1);
    assert_eq!(
        report.record.changes[0].path,
        "src/app/minimal_loop.rs".to_string()
    );
    assert!(
        report
            .record
            .preview_file
            .as_ref()
            .expect("preview should have markdown file")
            .to_string_lossy()
            .contains("patch-previews")
    );
    let markdown = fs::read_to_string(
        root.join(
            report
                .record
                .preview_file
                .as_ref()
                .expect("preview markdown should exist"),
        ),
    )
    .expect("preview markdown should be readable");
    assert!(markdown.contains("# AI 补丁应用预演"));
    assert!(markdown.contains("fn example()"));
    assert!(
        !root
            .join("src")
            .join("app")
            .join("minimal_loop.rs")
            .exists()
    );

    let records = app
        .ai_patch_preview_records(CURRENT_VERSION, 10)
        .expect("patch preview records should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.record.id);
    let loaded = app
        .ai_patch_preview_record(CURRENT_VERSION, &report.record.id)
        .expect("patch preview record should be readable");
    assert_eq!(loaded.id, report.record.id);
    assert_eq!(loaded.change_count, 1);

    app.supervisor()
        .verify_current_version()
        .expect("generated Chinese preview markdown should pass validation");

    cleanup(&root);
}

#[test]
fn ai_patch_preview_blocks_failed_audit_but_records_reason() {
    let root = temp_root("ai-patch-preview-failed-audit");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked patch preview");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁预演测试", 3)
        .expect("work queue should exist for blocked patch preview");
    let draft = create_patch_draft_for_audit(&root, &app, "- runtime/README.md");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("protected audit should write failed audit record");

    let report = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("failed audit should still produce blocked preview record");

    assert_eq!(audit.record.status, AiPatchAuditStatus::Failed);
    assert_eq!(report.record.status, AiPatchPreviewStatus::Blocked);
    assert_eq!(report.record.change_count, 0);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("审计未通过")
    );
    let records = app
        .ai_patch_preview_records(CURRENT_VERSION, 10)
        .expect("blocked patch preview should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, AiPatchPreviewStatus::Blocked);

    cleanup(&root);
}

#[test]
fn ai_patch_preview_blocks_missing_code_block() {
    let root = temp_root("ai-patch-preview-no-code");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing code preview");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁预演测试", 3)
        .expect("work queue should exist before missing code preview");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-preview-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控补丁草案能力。\n\n# 优化建议\n\n实现补丁应用预演。\n\n# 可复用经验\n\n预演必须先通过审计。\n"
            ),
        )
        .expect("test should write memory archive");
    let preview = app
        .ai_patch_draft_preview_with_lookup("生成缺少代码块的草案", |_| None)
        .expect("preview should build before missing code fixture");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
            request,
            response: AiTextResponse {
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                protocol: "openai-chat-completions".to_string(),
                text: "# 补丁目标\n生成受控补丁草案。\n\n# 计划\n1. 审计写入范围。\n\n# 允许写入范围\n- src/app/minimal_loop.rs\n\n# 代码草案\n这里描述代码，但没有代码块。\n\n# 测试草案\n```rust\n#[test]\nfn example_test() {}\n```\n\n# 验证命令\ncargo test\n\n# 风险与回滚\n失败时保留稳定版本。\n"
                    .to_string(),
                raw_bytes: 260,
            },
            status_code: 200,
        };
    let draft = app
        .finish_ai_patch_draft(preview, ai)
        .expect("draft with test section should be accepted")
        .record;
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("audit should pass before code block preview check");

    let report = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("missing code block should produce blocked preview record");

    assert_eq!(audit.record.status, AiPatchAuditStatus::Passed);
    assert_eq!(report.record.status, AiPatchPreviewStatus::Blocked);
    assert_eq!(report.record.code_block_count, 0);
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("代码块")
    );

    cleanup(&root);
}

#[test]
fn ai_patch_apply_writes_candidate_mirror_and_record() {
    let root = temp_root("ai-patch-apply-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before patch application");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁应用测试", 3)
        .expect("work queue should exist for patch application");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before patch application");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should succeed before patch application");

    let report = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("previewed patch should be applied to candidate mirror");

    let expected_candidate =
        next_version_after(CURRENT_VERSION).expect("next version should parse");
    assert_eq!(report.record.status, AiPatchApplicationStatus::Applied);
    assert_eq!(
        report.prepared_candidate_version.as_deref(),
        Some(expected_candidate.as_str())
    );
    assert_eq!(report.record.candidate_version, expected_candidate);
    assert_eq!(report.record.applied_file_count, 1);
    assert_eq!(
        report.record.files[0].source_path,
        "src/app/minimal_loop.rs"
    );
    assert!(
        report.record.files[0]
            .mirror_file
            .to_string_lossy()
            .contains("source")
    );
    let mirror_contents = fs::read_to_string(root.join(&report.record.files[0].mirror_file))
        .expect("candidate mirror file should be readable");
    assert!(mirror_contents.contains("fn example()"));
    assert!(
        !root
            .join("src")
            .join("app")
            .join("minimal_loop.rs")
            .exists()
    );
    assert!(
        report
            .record
            .verification_commands
            .iter()
            .any(|command| command == "cargo test")
    );
    assert!(!report.record.validation_checked_paths.is_empty());

    let state = ForgeState::load(&root).expect("state should be readable after apply");
    assert_eq!(state.candidate_version.as_deref(), Some("v0.1.66"));
    assert_eq!(state.status, "candidate_prepared");
    let records = app
        .ai_patch_application_records(CURRENT_VERSION, 10)
        .expect("patch application records should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.record.id);
    let loaded = app
        .ai_patch_application_record(CURRENT_VERSION, &report.record.id)
        .expect("patch application record should be readable");
    assert_eq!(loaded.id, report.record.id);
    assert_eq!(loaded.applied_file_count, 1);

    app.supervisor()
        .verify_version(&report.record.candidate_version)
        .expect("candidate layout should remain valid after patch application");

    cleanup(&root);
}

#[test]
fn ai_patch_apply_blocks_failed_preview_without_candidate() {
    let root = temp_root("ai-patch-apply-blocked-preview");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before blocked application");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁应用测试", 3)
        .expect("work queue should exist for blocked application");
    let draft = create_patch_draft_for_audit(&root, &app, "- runtime/README.md");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("protected audit should create failed record");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("failed audit should create blocked preview");

    let report = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("blocked preview should create blocked application record");

    assert_eq!(preview.record.status, AiPatchPreviewStatus::Blocked);
    assert_eq!(report.record.status, AiPatchApplicationStatus::Blocked);
    assert_eq!(report.record.applied_file_count, 0);
    assert!(report.record.application_dir.is_none());
    assert!(report.prepared_candidate_version.is_none());
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("预演")
    );
    let state = ForgeState::load(&root).expect("state should remain readable");
    assert!(state.candidate_version.is_none());

    cleanup(&root);
}

#[test]
fn ai_patch_apply_blocks_illegal_preview_path_without_candidate() {
    let root = temp_root("ai-patch-apply-illegal-path");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before illegal path application");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁应用测试", 3)
        .expect("work queue should exist for illegal path application");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before illegal path fixture");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should succeed before illegal path fixture");
    let preview_path = root.join(&preview.record.file);
    let mut preview_record: AiPatchPreviewRecord =
        serde_json::from_str(&fs::read_to_string(&preview_path).expect("preview JSON exists"))
            .expect("preview JSON should parse");
    preview_record.changes[0].path = "runtime/README.md".to_string();
    fs::write(
        &preview_path,
        serde_json::to_string_pretty(&preview_record).expect("mutated preview should serialize"),
    )
    .expect("mutated preview should be written");

    let report = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("illegal preview path should create blocked application record");

    assert_eq!(report.record.status, AiPatchApplicationStatus::Blocked);
    assert_eq!(report.record.applied_file_count, 0);
    assert!(report.record.application_dir.is_none());
    assert!(report.prepared_candidate_version.is_none());
    assert!(
        report
            .record
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("受保护路径")
    );
    let state = ForgeState::load(&root).expect("state should remain readable");
    assert!(state.candidate_version.is_none());
    assert!(
        !root
            .join("workspaces")
            .join("v0")
            .join("source")
            .join("patch-applications")
            .exists()
    );

    cleanup(&root);
}

#[test]
fn ai_patch_apply_reuses_existing_candidate() {
    let root = temp_root("ai-patch-apply-existing-candidate");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before existing candidate application");
    app.supervisor()
        .prepare_next_version("已有候选")
        .expect("candidate should be prepared before patch application");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁应用测试", 3)
        .expect("work queue should exist for existing candidate application");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass with existing candidate");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should succeed with existing candidate");

    let report = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should reuse existing candidate");

    assert_eq!(report.record.status, AiPatchApplicationStatus::Applied);
    assert!(report.prepared_candidate_version.is_none());
    assert_eq!(report.record.candidate_version, "v0.1.66");
    let state = ForgeState::load(&root).expect("state should remain readable");
    assert_eq!(state.candidate_version.as_deref(), Some("v0.1.66"));
    assert_eq!(state.status, "candidate_prepared");

    cleanup(&root);
}

#[test]
fn ai_patch_verify_records_successful_command_results() {
    let root = temp_root("ai-patch-verify-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before patch verification");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁验证测试", 3)
        .expect("work queue should exist for patch verification");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before verification");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before verification");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before verification");

    let report = app
        .ai_patch_verify_with_runner(
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
                    stdout_bytes: 2,
                    stderr_bytes: 0,
                    stdout_preview: "通过".to_string(),
                    stderr_preview: String::new(),
                    status: AiPatchVerificationStatus::Passed,
                })
            },
        )
        .expect("verification should record successful command results");

    assert_eq!(report.status, AiPatchVerificationStatus::Passed);
    assert_eq!(report.executed_count, 4);
    assert_eq!(report.record.verification_runs.len(), 4);
    assert!(
        report
            .record
            .verification_runs
            .iter()
            .all(|run| run.timeout_ms == 1_234)
    );
    let loaded = app
        .ai_patch_application_record(CURRENT_VERSION, &application.record.id)
        .expect("verified application record should be readable");
    assert_eq!(
        loaded.verification_status,
        AiPatchVerificationStatus::Passed
    );
    assert_eq!(loaded.verification_runs.len(), 4);
    let report_file = loaded
        .report_file
        .as_ref()
        .expect("verification should keep markdown report");
    let markdown = fs::read_to_string(root.join(report_file))
        .expect("verification markdown should be readable");
    assert!(markdown.contains("# 验证结果"));
    assert!(markdown.contains("cargo test"));

    cleanup(&root);
}

#[test]
fn ai_patch_verify_marks_failed_command_result() {
    let root = temp_root("ai-patch-verify-failed");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before failed patch verification");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁验证测试", 3)
        .expect("work queue should exist for failed patch verification");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before failed verification");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before failed verification");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before failed verification");

    let report = app
        .ai_patch_verify_with_runner(
            CURRENT_VERSION,
            &application.record.id,
            1_234,
            |spec, timeout_ms| {
                let failed = spec.command == "cargo test";
                Ok(AiPatchVerificationCommandRecord {
                    command: spec.command.clone(),
                    program: spec.program.clone(),
                    args: spec.args.clone(),
                    started_at_unix_seconds: 1,
                    duration_ms: 2,
                    timeout_ms,
                    exit_code: if failed { Some(1) } else { Some(0) },
                    timed_out: false,
                    stdout_bytes: 0,
                    stderr_bytes: if failed { 6 } else { 0 },
                    stdout_preview: String::new(),
                    stderr_preview: if failed {
                        "测试失败".to_string()
                    } else {
                        String::new()
                    },
                    status: if failed {
                        AiPatchVerificationStatus::Failed
                    } else {
                        AiPatchVerificationStatus::Passed
                    },
                })
            },
        )
        .expect("verification should record failed command results");

    assert_eq!(report.status, AiPatchVerificationStatus::Failed);
    assert_eq!(report.executed_count, 4);
    assert!(
        report
            .record
            .verification_runs
            .iter()
            .any(|run| run.command == "cargo test"
                && run.status == AiPatchVerificationStatus::Failed)
    );

    cleanup(&root);
}

#[test]
fn ai_patch_verify_skips_blocked_application() {
    let root = temp_root("ai-patch-verify-skipped");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before skipped patch verification");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁验证测试", 3)
        .expect("work queue should exist for skipped patch verification");
    let draft = create_patch_draft_for_audit(&root, &app, "- runtime/README.md");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("protected audit should create failed audit");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("failed audit should create blocked preview");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("blocked preview should create blocked application");
    let mut called = 0;

    let report = app
        .ai_patch_verify_with_runner(
            CURRENT_VERSION,
            &application.record.id,
            1_234,
            |_spec, _timeout_ms| {
                called += 1;
                unreachable!("blocked application should not execute verification commands")
            },
        )
        .expect("blocked application should be marked as skipped");

    assert_eq!(called, 0);
    assert_eq!(report.status, AiPatchVerificationStatus::Skipped);
    assert_eq!(report.executed_count, 0);
    assert!(report.record.verification_runs.is_empty());

    cleanup(&root);
}

#[test]
fn ai_patch_verify_rejects_unknown_command_without_running() {
    let root = temp_root("ai-patch-verify-unknown-command");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before unknown verification command");
    app.init_agent_work_queue(CURRENT_VERSION, "补丁验证测试", 3)
        .expect("work queue should exist for unknown verification command");
    let draft = create_patch_draft_for_audit(&root, &app, "- src/app/minimal_loop.rs");
    let audit = app
        .ai_patch_audit(CURRENT_VERSION, &draft.id)
        .expect("clean audit should pass before unknown command fixture");
    let preview = app
        .ai_patch_preview(CURRENT_VERSION, &audit.record.id)
        .expect("preview should pass before unknown command fixture");
    let application = app
        .ai_patch_apply(CURRENT_VERSION, &preview.record.id)
        .expect("application should exist before unknown command fixture");
    let mut record = application.record.clone();
    record.verification_commands = vec!["cargo clippy".to_string()];
    AiPatchApplicationStore::new(&root)
        .update(record, None)
        .expect("test should persist unknown command fixture");

    let error = app
        .ai_patch_verify_with_runner(
            CURRENT_VERSION,
            &application.record.id,
            1_234,
            |_spec, _timeout_ms| unreachable!("unknown command should be rejected before runner"),
        )
        .expect_err("unknown command should fail before execution");

    assert!(error.to_string().contains("不受支持"));
    let loaded = app
        .ai_patch_application_record(CURRENT_VERSION, &application.record.id)
        .expect("application record should remain readable after unknown command");
    assert_eq!(
        loaded.verification_status,
        AiPatchVerificationStatus::Failed
    );
    assert_eq!(loaded.verification_runs.len(), 1);
    assert!(
        loaded.verification_runs[0]
            .stderr_preview
            .contains("不受支持")
    );

    cleanup(&root);
}
