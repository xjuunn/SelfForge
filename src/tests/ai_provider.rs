use super::*;

#[test]
fn ai_config_auto_selects_openai_when_key_exists() {
    let values = HashMap::from([("OPENAI_API_KEY", "test-openai-key")]);

    let report = AiProviderRegistry::inspect_with(env_lookup(&values))
        .expect("openai key should produce a valid AI config report");
    let selected = report
        .selected()
        .expect("configured provider should be selected");

    assert!(report.ready);
    assert_eq!(report.selected_provider.as_deref(), Some("openai"));
    assert_eq!(selected.id, "openai");
    assert_eq!(selected.api_key_env_var.as_deref(), Some("OPENAI_API_KEY"));
    assert_eq!(selected.protocol, "openai-responses");
    assert_eq!(selected.request_path, "/responses");
}

#[test]
fn ai_config_prefers_requested_deepseek_provider() {
    let values = HashMap::from([
        ("OPENAI_API_KEY", "test-openai-key"),
        ("DEEPSEEK_API_KEY", "test-deepseek-key"),
        ("DEEPSEEK_MODEL", "deepseek-v4-pro"),
        ("SELFFORGE_AI_PROVIDER", "deepseek"),
    ]);

    let report = AiProviderRegistry::inspect_with(env_lookup(&values))
        .expect("requested provider should be accepted");
    let selected = report
        .selected()
        .expect("requested configured provider should be selected");

    assert!(report.ready);
    assert_eq!(selected.id, "deepseek");
    assert_eq!(
        selected.api_key_env_var.as_deref(),
        Some("DEEPSEEK_API_KEY")
    );
    assert_eq!(selected.model, "deepseek-v4-pro");
    assert_eq!(selected.model_source, "DEEPSEEK_MODEL");
    assert_eq!(selected.protocol, "openai-chat-completions");
}

#[test]
fn ai_config_uses_google_api_key_before_gemini_api_key() {
    let values = HashMap::from([
        ("GEMINI_API_KEY", "test-gemini-key"),
        ("GOOGLE_API_KEY", "test-google-key"),
        ("SELFFORGE_AI_PROVIDER", "gemini"),
    ]);

    let report = AiProviderRegistry::inspect_with(env_lookup(&values))
        .expect("gemini provider should accept either key variable");
    let selected = report
        .selected()
        .expect("configured gemini provider should be selected");

    assert!(report.ready);
    assert_eq!(selected.id, "gemini");
    assert_eq!(selected.api_key_env_var.as_deref(), Some("GOOGLE_API_KEY"));
    assert_eq!(selected.protocol, "gemini-generate-content");
    assert!(selected.request_path.contains(&selected.model));
}

#[test]
fn ai_config_reports_unknown_provider() {
    let values = HashMap::from([("SELFFORGE_AI_PROVIDER", "unknown-ai")]);

    let error = AiProviderRegistry::inspect_with(env_lookup(&values))
        .expect_err("unknown provider must be rejected");

    assert!(matches!(
        error,
        AiConfigError::UnknownProvider { ref requested, .. } if requested == "unknown-ai"
    ));
    assert!(error.to_string().contains("openai"));
    assert!(error.to_string().contains("deepseek"));
    assert!(error.to_string().contains("gemini"));
}

#[test]
fn ai_config_loads_project_dotenv_when_process_env_is_empty() {
    let root = temp_root("ai-dotenv-config");
    fs::create_dir_all(&root).expect("test should create project root");
    fs::write(
            root.join(".env"),
            "# AI 配置\nexport SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=\"test-dotenv-key\"\nDEEPSEEK_MODEL=deepseek-dotenv # 本地模型\n",
        )
        .expect("test should write dotenv file");

    let report = AiProviderRegistry::inspect_project_with(&root, |_| None)
        .expect("dotenv provider should produce a valid AI config report");
    let selected = report
        .selected()
        .expect("dotenv configured provider should be selected");

    assert!(report.ready);
    assert_eq!(report.selected_provider.as_deref(), Some("deepseek"));
    assert_eq!(selected.id, "deepseek");
    assert_eq!(
        selected.api_key_env_var.as_deref(),
        Some("DEEPSEEK_API_KEY")
    );
    assert_eq!(selected.model, "deepseek-dotenv");
    assert!(!format!("{report:?}").contains("test-dotenv-key"));

    cleanup(&root);
}

#[test]
fn ai_config_process_env_overrides_project_dotenv() {
    let root = temp_root("ai-dotenv-override");
    fs::create_dir_all(&root).expect("test should create project root");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-dotenv-key\n",
    )
    .expect("test should write dotenv file");
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "openai"),
        ("OPENAI_API_KEY", "test-process-key"),
        ("OPENAI_MODEL", "gpt-process"),
    ]);

    let report = AiProviderRegistry::inspect_project_with(&root, env_lookup(&values))
        .expect("process env should override dotenv");
    let selected = report
        .selected()
        .expect("process env provider should be selected");

    assert!(report.ready);
    assert_eq!(selected.id, "openai");
    assert_eq!(selected.model, "gpt-process");
    assert_eq!(selected.api_key_env_var.as_deref(), Some("OPENAI_API_KEY"));

    cleanup(&root);
}

#[test]
fn ai_request_builds_from_project_dotenv_without_leaking_key() {
    let root = temp_root("ai-dotenv-request");
    fs::create_dir_all(&root).expect("test should create project root");
    fs::write(
            root.join(".env"),
            "SELFFORGE_AI_PROVIDER=gemini\nGOOGLE_API_KEY='test-dotenv-google-key'\nGEMINI_MODEL=gemini-dotenv\n",
        )
        .expect("test should write dotenv file");

    let spec = AiProviderRegistry::build_text_request_project_with(&root, "生成计划", |_| None)
        .expect("dotenv request spec should build");

    assert_eq!(spec.provider_id, "gemini");
    assert_eq!(spec.api_key_env_var, "GOOGLE_API_KEY");
    assert!(spec.url.contains("gemini-dotenv"));
    assert_eq!(spec.body["contents"][0]["parts"][0]["text"], "生成计划");
    assert!(!format!("{spec:?}").contains("test-dotenv-google-key"));

    cleanup(&root);
}

#[test]
fn ai_request_executes_http_and_parses_response_text() {
    let root = temp_root("ai-http-success");
    fs::create_dir_all(&root).expect("test should create project root");
    fs::write(
            root.join(".env"),
            "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-http-key\nDEEPSEEK_BASE_URL=http://127.0.0.1:3001\n",
        )
        .expect("test should write dotenv file");

    let report = AiProviderRegistry::execute_text_request_project_with(
        &root,
        "生成计划",
        1_234,
        |_| None,
        |request, api_key, timeout_ms| {
            assert_eq!(api_key, "test-http-key");
            assert_eq!(timeout_ms, 1_234);
            assert_eq!(request.provider_id, "deepseek");
            assert_eq!(request.auth_header_name, "Authorization");
            assert!(!request.body.to_string().contains("test-http-key"));
            Ok(AiRawHttpResponse {
                status_code: 200,
                body: r#"{"choices":[{"message":{"content":"响应文本"}}]}"#.to_string(),
            })
        },
    )
    .expect("http execution should parse text response");

    assert_eq!(report.status_code, 200);
    assert_eq!(report.response.text, "响应文本");
    assert_eq!(report.response.provider_id, "deepseek");
    assert!(!format!("{report:?}").contains("test-http-key"));

    cleanup(&root);
}

#[test]
fn ai_request_reports_http_status_without_leaking_key() {
    let root = temp_root("ai-http-status");
    fs::create_dir_all(&root).expect("test should create project root");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-status-key\n",
    )
    .expect("test should write dotenv file");

    let error = AiProviderRegistry::execute_text_request_project_with(
        &root,
        "生成计划",
        1_000,
        |_| None,
        |_request, api_key, _timeout_ms| {
            Ok(AiRawHttpResponse {
                status_code: 401,
                body: format!("{{\"error\":\"bad key {api_key}\"}}"),
            })
        },
    )
    .expect_err("http status error should stop execution");

    assert!(matches!(
        error,
        AiExecutionError::HttpStatus {
            status_code: 401,
            ..
        }
    ));
    let message = error.to_string();
    assert!(message.contains("[已脱敏]"));
    assert!(!message.contains("test-status-key"));

    cleanup(&root);
}

#[test]
fn ai_request_reports_response_parse_error_after_success_status() {
    let root = temp_root("ai-http-parse");
    fs::create_dir_all(&root).expect("test should create project root");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-parse-key\n",
    )
    .expect("test should write dotenv file");

    let error = AiProviderRegistry::execute_text_request_project_with(
        &root,
        "生成计划",
        1_000,
        |_| None,
        |_request, _api_key, _timeout_ms| {
            Ok(AiRawHttpResponse {
                status_code: 200,
                body: "{}".to_string(),
            })
        },
    )
    .expect_err("missing response text should stop execution");

    assert!(matches!(
        error,
        AiExecutionError::Response(AiResponseError::MissingText { .. })
    ));

    cleanup(&root);
}

#[test]
fn ai_config_missing_project_dotenv_does_not_crash() {
    let root = temp_root("ai-dotenv-missing");

    let report = AiProviderRegistry::inspect_project_with(&root, |_| None)
        .expect("missing dotenv should be treated as empty config");

    assert!(!report.ready);
    assert_eq!(report.selected_provider, None);
}

#[test]
fn ai_request_builds_openai_responses_spec() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "openai"),
        ("OPENAI_API_KEY", "test-openai-key"),
        ("OPENAI_MODEL", "gpt-test"),
    ]);

    let spec = AiProviderRegistry::build_text_request_with("生成一个计划", env_lookup(&values))
        .expect("openai request spec should build");

    assert_eq!(spec.provider_id, "openai");
    assert_eq!(spec.method, "POST");
    assert_eq!(spec.url, "https://api.openai.com/v1/responses");
    assert_eq!(spec.auth_header_name, "Authorization");
    assert_eq!(spec.api_key_env_var, "OPENAI_API_KEY");
    assert_eq!(spec.body["model"], "gpt-test");
    assert_eq!(spec.body["input"], "生成一个计划");
    assert!(!spec.body.to_string().contains("test-openai-key"));
}

#[test]
fn ai_request_builds_deepseek_chat_spec() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "deepseek"),
        ("DEEPSEEK_API_KEY", "test-deepseek-key"),
        ("DEEPSEEK_BASE_URL", "https://api.deepseek.com/"),
    ]);

    let spec = AiProviderRegistry::build_text_request_with("分析错误", env_lookup(&values))
        .expect("deepseek request spec should build");

    assert_eq!(spec.provider_id, "deepseek");
    assert_eq!(spec.url, "https://api.deepseek.com/chat/completions");
    assert_eq!(spec.auth_header_name, "Authorization");
    assert_eq!(spec.api_key_env_var, "DEEPSEEK_API_KEY");
    assert_eq!(spec.body["model"], "deepseek-v4-flash");
    assert_eq!(spec.body["messages"][0]["role"], "user");
    assert_eq!(spec.body["messages"][0]["content"], "分析错误");
    assert_eq!(spec.body["stream"], false);
    assert!(!spec.body.to_string().contains("test-deepseek-key"));
}

#[test]
fn ai_request_builds_gemini_generate_content_spec() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "gemini"),
        ("GOOGLE_API_KEY", "test-google-key"),
        ("GEMINI_MODEL", "gemini-test"),
    ]);

    let spec = AiProviderRegistry::build_text_request_with("生成测试", env_lookup(&values))
        .expect("gemini request spec should build");

    assert_eq!(spec.provider_id, "gemini");
    assert_eq!(
        spec.url,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-test:generateContent"
    );
    assert_eq!(spec.auth_header_name, "x-goog-api-key");
    assert_eq!(spec.api_key_env_var, "GOOGLE_API_KEY");
    assert_eq!(spec.body["contents"][0]["parts"][0]["text"], "生成测试");
    assert!(!spec.body.to_string().contains("test-google-key"));
}

#[test]
fn ai_request_reports_missing_selected_provider_key() {
    let values = HashMap::from([("SELFFORGE_AI_PROVIDER", "gemini")]);

    let error = AiProviderRegistry::build_text_request_with("生成测试", env_lookup(&values))
        .expect_err("selected provider without key must be rejected");

    assert!(matches!(
        error,
        AiRequestError::MissingApiKey { ref provider } if provider == "gemini"
    ));
    let message = error.to_string();
    assert!(message.contains("GOOGLE_API_KEY"));
    assert!(message.contains("GEMINI_API_KEY"));
    assert!(message.contains("PowerShell"));
}

#[test]
fn ai_request_rejects_empty_prompt() {
    let values = HashMap::from([("OPENAI_API_KEY", "test-openai-key")]);

    let error = AiProviderRegistry::build_text_request_with("   ", env_lookup(&values))
        .expect_err("empty prompt must be rejected");

    assert!(matches!(error, AiRequestError::EmptyPrompt));
}

#[test]
fn ai_self_upgrade_preview_builds_controlled_prompt_from_memory() {
    let root = temp_root("ai-self-upgrade-preview");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before AI self-upgrade preview");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-self-upgrade-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控进化基础。\n\n# 优化建议\n\n继续完善 AI 自动自我升级入口。\n\n# 可复用经验\n\n自我升级必须复用预检和候选验证。\n"
            ),
        )
        .expect("test should write memory archive");

    let preview = app
        .ai_self_upgrade_preview_with_lookup("优先完善 AI 自动自我升级入口", |_| None)
        .expect("preview should build controlled AI self-upgrade request");

    assert_eq!(preview.current_version, CURRENT_VERSION);
    assert_eq!(preview.request.provider_id, "deepseek");
    assert_eq!(preview.insights.source_versions, vec![CURRENT_VERSION]);
    assert!(preview.prompt.contains("只返回一个中文目标句子"));
    assert!(preview.prompt.contains("优先完善 AI 自动自我升级入口"));
    assert!(preview.prompt.contains(CURRENT_VERSION));
    assert!(
        !preview
            .request
            .body
            .to_string()
            .contains("test-self-upgrade-key")
    );

    cleanup(&root);
}

#[test]
fn ai_self_upgrade_normalizes_single_goal_text() {
    let goal =
        normalize_ai_self_upgrade_goal("1. 目标：继续完善 AI 自动自我升级闭环\n\n说明：忽略")
            .expect("goal should normalize");

    assert_eq!(goal, "继续完善 AI 自动自我升级闭环");
}

#[test]
fn ai_self_upgrade_rejects_empty_goal_text() {
    let error = normalize_ai_self_upgrade_goal("```")
        .expect_err("empty AI self-upgrade goal must be rejected");

    assert!(matches!(error, AiSelfUpgradeError::EmptyGoal { .. }));
}

#[test]
fn ai_self_upgrade_preview_stops_when_open_errors_exist() {
    let root = temp_root("ai-self-upgrade-open-errors");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before open error guard test");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-self-upgrade-key\n",
    )
    .expect("test should write dotenv file");
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
        .expect("failed run should be archived before AI self-upgrade");

    let error = app
        .ai_self_upgrade_preview_with_lookup("继续推进", |_| None)
        .expect_err("open errors must block AI self-upgrade");

    assert!(matches!(
        error,
        AiSelfUpgradeError::Blocked {
            ref version,
            ref open_errors,
        } if version == CURRENT_VERSION && open_errors.len() == 1
    ));

    cleanup(&root);
}

#[test]
fn ai_self_upgrade_success_writes_audit_record_without_prompt_or_key() {
    let root = temp_root("ai-self-upgrade-audit-success");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before AI self-upgrade audit");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-self-upgrade-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控进化基础。\n\n# 优化建议\n\n继续完善 AI 自我升级审计记录。\n\n# 可复用经验\n\n自我升级必须复用预检和候选验证。\n"
            ),
        )
        .expect("test should write memory archive");
    let preview = app
        .ai_self_upgrade_preview_with_lookup("记录自我升级审计", |_| None)
        .expect("preview should build before finishing self-upgrade");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
        request,
        response: AiTextResponse {
            provider_id: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            protocol: "openai-chat-completions".to_string(),
            text: "继续完善 AI 自我升级审计记录".to_string(),
            raw_bytes: 48,
        },
        status_code: 200,
    };

    let report = app
        .finish_ai_self_upgrade(preview, ai)
        .expect("successful AI self-upgrade should write audit record");

    assert_eq!(report.audit.status, AiSelfUpgradeAuditStatus::Succeeded);
    assert_eq!(
        report.audit.proposed_goal.as_deref(),
        Some("继续完善 AI 自我升级审计记录")
    );
    assert_eq!(
        report.audit.stable_version_after.as_deref(),
        Some(
            next_version_after(CURRENT_VERSION)
                .expect("next version should parse")
                .as_str()
        )
    );
    assert!(report.audit.session_id.is_some());
    let records = app
        .ai_self_upgrade_records(CURRENT_VERSION, 10)
        .expect("audit records should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.audit.id);
    let linked = app
        .ai_self_upgrade_record_for_session(
            CURRENT_VERSION,
            report
                .audit
                .session_id
                .as_deref()
                .expect("audit should link session"),
        )
        .expect("audit record should be queryable by session");
    assert_eq!(
        linked.as_ref().map(|record| record.id.as_str()),
        Some(report.audit.id.as_str())
    );

    let contents = fs::read_to_string(root.join(&report.audit.file))
        .expect("audit record file should be readable");
    assert!(contents.contains("继续完善 AI 自我升级审计记录"));
    assert!(!contents.contains("test-self-upgrade-key"));
    assert!(!contents.contains("你是 SelfForge 的自我升级目标决策 Agent"));
    assert_eq!(report.summary.status, AiSelfUpgradeSummaryStatus::Succeeded);
    assert_eq!(report.summary.audit_id, report.audit.id);
    assert_eq!(report.summary.session_id, report.audit.session_id);
    let summaries = app
        .ai_self_upgrade_summary_records(CURRENT_VERSION, 10)
        .expect("self-upgrade summaries should be queryable");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, report.summary.id);
    let loaded_summary = app
        .ai_self_upgrade_summary_record(CURRENT_VERSION, &report.summary.id)
        .expect("self-upgrade summary record should be readable");
    assert_eq!(loaded_summary.audit_id, report.audit.id);
    let summary_markdown = fs::read_to_string(root.join(&loaded_summary.markdown_file))
        .expect("self-upgrade summary markdown should be readable");
    assert!(summary_markdown.contains("# AI 自我升级总结报告"));
    assert!(summary_markdown.contains("# 目标"));
    assert!(summary_markdown.contains("# 计划"));
    assert!(summary_markdown.contains("# 代码变更"));
    assert!(summary_markdown.contains("# 测试结果"));
    assert!(summary_markdown.contains("# 错误信息"));
    assert!(summary_markdown.contains("# 审计记录"));
    assert!(summary_markdown.contains("# 下一步建议"));
    assert!(!summary_markdown.contains("test-self-upgrade-key"));
    assert!(!summary_markdown.contains("你是 SelfForge 的自我升级目标决策 Agent"));

    cleanup(&root);
}

#[test]
fn ai_self_upgrade_empty_goal_writes_failed_audit_record() {
    let root = temp_root("ai-self-upgrade-audit-empty-goal");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before failed AI self-upgrade audit");
    fs::write(
        root.join(".env"),
        "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-self-upgrade-key\n",
    )
    .expect("test should write dotenv file");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控进化基础。\n\n# 优化建议\n\n继续完善 AI 自我升级审计记录。\n\n# 可复用经验\n\n自我升级必须复用预检和候选验证。\n"
            ),
        )
        .expect("test should write memory archive");
    let preview = app
        .ai_self_upgrade_preview_with_lookup("记录失败审计", |_| None)
        .expect("preview should build before failed self-upgrade");
    let request = preview.request.clone();
    let ai = AiExecutionReport {
        request,
        response: AiTextResponse {
            provider_id: "deepseek".to_string(),
            model: "deepseek-v4-flash".to_string(),
            protocol: "openai-chat-completions".to_string(),
            text: "```".to_string(),
            raw_bytes: 3,
        },
        status_code: 200,
    };

    let error = app
        .finish_ai_self_upgrade(preview, ai)
        .expect_err("empty goal should fail after writing audit record");

    assert!(matches!(error, AiSelfUpgradeError::EmptyGoal { .. }));
    let records = app
        .ai_self_upgrade_records(CURRENT_VERSION, 10)
        .expect("failed audit record should be queryable");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, AiSelfUpgradeAuditStatus::Failed);
    assert!(records[0].proposed_goal.is_none());
    assert!(records[0].session_id.is_none());
    assert!(
        records[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("未包含可执行目标")
    );
    let summary = app
        .ai_self_upgrade_summary(CURRENT_VERSION, &records[0].id)
        .expect("failed audit should still produce a summary report");
    assert_eq!(summary.record.status, AiSelfUpgradeSummaryStatus::Failed);
    assert!(summary.record.session_id.is_none());
    let markdown = fs::read_to_string(root.join(&summary.record.markdown_file))
        .expect("failed summary markdown should be readable");
    assert!(markdown.contains("先修复审计错误或会话错误"));

    cleanup(&root);
}

#[test]
fn ai_request_missing_provider_message_mentions_current_shell_setup() {
    let values = HashMap::new();

    let error = AiProviderRegistry::build_text_request_with("你好", env_lookup(&values))
        .expect_err("missing provider must explain environment setup");

    assert!(matches!(error, AiRequestError::MissingProvider));
    let message = error.to_string();
    assert!(message.contains("SELFFORGE_AI_PROVIDER"));
    assert!(message.contains("DEEPSEEK_API_KEY"));
    assert!(message.contains("cargo run -- ai-config"));
    assert!(message.contains("PowerShell"));
}

#[test]
fn ai_request_missing_deepseek_key_message_names_deepseek_key() {
    let values = HashMap::from([("SELFFORGE_AI_PROVIDER", "deepseek")]);

    let error = AiProviderRegistry::build_text_request_with("你好", env_lookup(&values))
        .expect_err("missing deepseek key must be explicit");

    assert!(matches!(
        error,
        AiRequestError::MissingApiKey { ref provider } if provider == "deepseek"
    ));
    let message = error.to_string();
    assert!(message.contains("DEEPSEEK_API_KEY"));
    assert!(message.contains("PowerShell"));
    assert!(message.contains("cargo run -- ai-config"));
}

#[test]
fn ai_response_parses_openai_output_text() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "openai"),
        ("OPENAI_API_KEY", "test-openai-key"),
    ]);
    let request = AiProviderRegistry::build_text_request_with("生成计划", env_lookup(&values))
        .expect("openai request should build");

    let response =
        AiProviderRegistry::parse_text_response(&request, r#"{"output_text":"计划已经生成"}"#)
            .expect("openai output_text should parse");

    assert_eq!(response.provider_id, "openai");
    assert_eq!(response.protocol, "openai-responses");
    assert_eq!(response.text, "计划已经生成");
    assert!(response.raw_bytes > 0);
}

#[test]
fn ai_response_parses_openai_output_parts() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "openai"),
        ("OPENAI_API_KEY", "test-openai-key"),
    ]);
    let request = AiProviderRegistry::build_text_request_with("生成计划", env_lookup(&values))
        .expect("openai request should build");

    let response = AiProviderRegistry::parse_text_response(
            &request,
            r#"{"output":[{"content":[{"type":"output_text","text":"第一段"},{"type":"output_text","text":"第二段"}]}]}"#,
        )
        .expect("openai output parts should parse");

    assert_eq!(response.text, "第一段\n第二段");
}

#[test]
fn ai_response_parses_deepseek_chat_completion() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "deepseek"),
        ("DEEPSEEK_API_KEY", "test-deepseek-key"),
    ]);
    let request = AiProviderRegistry::build_text_request_with("分析错误", env_lookup(&values))
        .expect("deepseek request should build");

    let response = AiProviderRegistry::parse_text_response(
        &request,
        r#"{"choices":[{"message":{"role":"assistant","content":"错误原因已定位"}}]}"#,
    )
    .expect("deepseek message content should parse");

    assert_eq!(response.provider_id, "deepseek");
    assert_eq!(response.text, "错误原因已定位");
}

#[test]
fn ai_response_parses_gemini_candidates() {
    let values = HashMap::from([
        ("SELFFORGE_AI_PROVIDER", "gemini"),
        ("GOOGLE_API_KEY", "test-google-key"),
    ]);
    let request = AiProviderRegistry::build_text_request_with("生成测试", env_lookup(&values))
        .expect("gemini request should build");

    let response = AiProviderRegistry::parse_text_response(
        &request,
        r#"{"candidates":[{"content":{"parts":[{"text":"测试一"},{"text":"测试二"}]}}]}"#,
    )
    .expect("gemini candidate parts should parse");

    assert_eq!(response.provider_id, "gemini");
    assert_eq!(response.protocol, "gemini-generate-content");
    assert_eq!(response.text, "测试一\n测试二");
}

#[test]
fn ai_response_rejects_invalid_json() {
    let values = HashMap::from([("OPENAI_API_KEY", "test-openai-key")]);
    let request = AiProviderRegistry::build_text_request_with("生成计划", env_lookup(&values))
        .expect("openai request should build");

    let error = AiProviderRegistry::parse_text_response(&request, "not json")
        .expect_err("invalid json must be rejected");

    assert!(matches!(error, AiResponseError::InvalidJson { .. }));
}

#[test]
fn ai_response_reports_missing_text() {
    let values = HashMap::from([("OPENAI_API_KEY", "test-openai-key")]);
    let request = AiProviderRegistry::build_text_request_with("生成计划", env_lookup(&values))
        .expect("openai request should build");

    let error = AiProviderRegistry::parse_text_response(&request, r#"{"output":[]}"#)
        .expect_err("missing text must be reported");

    assert!(matches!(
        error,
        AiResponseError::MissingText { ref protocol } if protocol == "openai-responses"
    ));
}

#[test]
fn ai_response_rejects_empty_text() {
    let values = HashMap::from([("OPENAI_API_KEY", "test-openai-key")]);
    let request = AiProviderRegistry::build_text_request_with("生成计划", env_lookup(&values))
        .expect("openai request should build");

    let error = AiProviderRegistry::parse_text_response(&request, r#"{"output_text":"   "}"#)
        .expect_err("empty text must be rejected");

    assert!(matches!(
        error,
        AiResponseError::EmptyText { ref protocol } if protocol == "openai-responses"
    ));
}
