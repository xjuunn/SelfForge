use super::*;

#[test]
fn memory_context_reads_recent_unique_final_sections() {
    let root = temp_root("memory-context");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before memory context test");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            "# v0 记忆记录\n\n## v0.1.31\n\n普通候选记忆。\n\n## v0.1.32\n\n候选阶段记忆。\n\n## v0.1.32 最终记忆\n\n最终经验应优先生效。\n\n## v0.1.30\n\n较早记忆。\n",
        )
        .expect("test should write memory archive");

    let report = app
        .memory_context("v0.1.32", 2)
        .expect("recent memory context should be readable");

    assert_eq!(report.entries.len(), 2);
    assert_eq!(report.entries[0].version, "v0.1.32");
    assert!(report.entries[0].title.contains("最终"));
    assert!(report.entries[0].body.contains("最终经验"));
    assert_eq!(report.entries[1].version, "v0.1.31");

    cleanup(&root);
}

#[test]
fn memory_context_accepts_zero_limit_without_reading_entries() {
    let root = temp_root("memory-context-zero");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before zero limit memory context test");

    let report = app
        .memory_context(CURRENT_VERSION, 0)
        .expect("zero limit memory context should succeed");

    assert!(report.entries.is_empty());
    assert!(report.archive_path.ends_with("v0.md"));

    cleanup(&root);
}

#[test]
fn memory_context_reports_missing_major_archive() {
    let root = temp_root("memory-context-missing");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before missing memory context test");

    let error = app
        .memory_context("v9.0.0", 5)
        .expect_err("missing memory archive should be reported");

    assert!(matches!(error, MemoryContextError::MissingArchive { .. }));

    cleanup(&root);
}

#[test]
fn memory_insights_extracts_structured_experience() {
    let root = temp_root("memory-insights");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before memory insights test");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            "# v0 记忆记录\n\n## v0.1.49 最终记忆\n\n# 错误总结\n\n本轮没有新增未解决错误。\n本轮未发现功能错误。\n\n# 评估\n\nv0.1.49 让进化流程显式读取历史记忆。\n\n# 优化建议\n\n下一步提取结构化经验。\n\n# 可复用经验\n\n记忆读取应放在应用用例层。\n\n## v0.1.32 最终记忆\n\n# 错误总结\n\n并行执行多个 `cargo run` 时出现 Cargo 构建锁等待提示。\n\n# 评估\n\nv0.1.32 将验证动作沉淀为独立 Agent 会话。\n\n# 优化建议\n\n继续让 Agent 自动生成更结构化的验证目标。\n\n# 可复用经验\n\n运行证据应优先复用 Runtime 记录。\n",
        )
        .expect("test should write memory archive");

    let report = app
        .memory_insights("v0.1.49", 2)
        .expect("memory insights should be extracted");

    assert_eq!(report.source_versions, vec!["v0.1.49", "v0.1.32"]);
    assert_eq!(report.success_experiences.len(), 2);
    assert_eq!(report.failure_experiences.len(), 1);
    assert!(
        report.failure_experiences[0]
            .text
            .contains("构建锁等待提示")
    );
    assert_eq!(report.optimization_suggestions.len(), 2);
    assert_eq!(report.reusable_experiences.len(), 2);

    cleanup(&root);
}

#[test]
fn memory_insights_skips_placeholders_and_zero_limit() {
    let root = temp_root("memory-insights-empty");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before empty memory insights test");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            "# v0 记忆记录\n\n## v0.1.49\n\n# 错误总结\n\n待最终验证后补充。\n\n# 评估\n\n暂无。\n\n# 优化建议\n\n无。\n\n# 可复用经验\n\n待最终验证后补充。\n",
        )
        .expect("test should write placeholder memory archive");

    let report = app
        .memory_insights("v0.1.49", 0)
        .expect("zero limit memory insights should succeed");

    assert!(report.source_versions.is_empty());
    assert!(report.success_experiences.is_empty());
    assert!(report.failure_experiences.is_empty());
    assert!(report.optimization_suggestions.is_empty());
    assert!(report.reusable_experiences.is_empty());

    cleanup(&root);
}

#[test]
fn memory_compaction_moves_old_sections_to_cold_archive() {
    let root = temp_root("memory-compact");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before memory compaction test");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            "# v0 记忆记录\n\n## v0.1.49 最终记忆\n\n# 评估\n\n近期成功经验。\n\n## v0.1.38 最终记忆\n\n# 评估\n\n第二条近期经验。\n\n## v0.1.37 最终记忆\n\n# 评估\n\n较早经验。\n\n## v0.1.36 最终记忆\n\n# 评估\n\n更早经验。\n",
        )
        .expect("test should write memory archive");

    let report = app
        .compact_memory("v0.1.49", 2)
        .expect("memory compaction should move old sections");

    assert_eq!(report.original_sections, 4);
    assert_eq!(report.kept_sections, 2);
    assert_eq!(report.archived_sections, 2);
    assert_eq!(report.total_archive_sections, 2);
    assert!(report.memory_path.ends_with("v0.md"));
    assert_eq!(
        report
            .archive_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("v0.md")
    );
    assert_eq!(
        report
            .archive_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str()),
        Some("archive")
    );

    let hot = fs::read_to_string(&report.memory_path).expect("hot memory should be readable");
    assert!(hot.contains("压缩记忆索引"));
    assert!(hot.contains("## v0.1.49 最终记忆"));
    assert!(hot.contains("## v0.1.38 最终记忆"));
    assert!(!hot.contains("## v0.1.37 最终记忆"));
    let cold =
        fs::read_to_string(&report.archive_path).expect("cold memory archive should be readable");
    assert!(cold.contains("历史记忆冷归档"));
    assert!(cold.contains("## v0.1.37 最终记忆"));
    assert!(cold.contains("## v0.1.36 最终记忆"));

    let context = app
        .memory_context("v0.1.49", 5)
        .expect("recent memory context should read only hot memory");
    let source_versions: Vec<String> = context
        .entries
        .iter()
        .map(|entry| entry.version.clone())
        .collect();
    assert_eq!(source_versions, vec!["v0.1.49", "v0.1.38"]);

    cleanup(&root);
}

#[test]
fn memory_compaction_is_idempotent_for_existing_cold_archive() {
    let root = temp_root("memory-compact-idempotent");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before idempotent memory compaction test");
    fs::write(
            root.join("forge").join("memory").join("v0.md"),
            "# v0 记忆记录\n\n## v0.1.49 最终记忆\n\n# 评估\n\n近期经验。\n\n## v0.1.38 最终记忆\n\n# 评估\n\n较早经验。\n\n## v0.1.37 最终记忆\n\n# 评估\n\n归档经验。\n",
        )
        .expect("test should write memory archive");

    let first = app
        .compact_memory("v0.1.49", 2)
        .expect("first compaction should succeed");
    let hot_once = fs::read_to_string(&first.memory_path).expect("hot memory should be readable");
    let cold_once =
        fs::read_to_string(&first.archive_path).expect("cold archive should be readable");

    let second = app
        .compact_memory("v0.1.49", 2)
        .expect("second compaction should be idempotent");
    let hot_twice = fs::read_to_string(&second.memory_path).expect("hot memory should be readable");
    let cold_twice =
        fs::read_to_string(&second.archive_path).expect("cold archive should be readable");

    assert_eq!(second.original_sections, 2);
    assert_eq!(second.kept_sections, 2);
    assert_eq!(second.archived_sections, 0);
    assert_eq!(hot_once, hot_twice);
    assert_eq!(cold_once, cold_twice);

    cleanup(&root);
}

#[test]
fn memory_compaction_rejects_zero_keep_count() {
    let root = temp_root("memory-compact-zero");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before zero keep test");

    let error = app
        .compact_memory(CURRENT_VERSION, 0)
        .expect_err("zero keep count must be rejected");

    assert!(matches!(error, MemoryCompactionError::InvalidKeepCount));

    cleanup(&root);
}
