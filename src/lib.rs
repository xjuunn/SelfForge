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

pub const CURRENT_VERSION: &str = "v0.1.61";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
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

    fn env_lookup<'a>(values: &'a HashMap<&str, &str>) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| values.get(key).map(|value| (*value).to_string())
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

        assert_eq!(report.current_version, "v0.1.61");
        assert_eq!(report.next_version, "v0.1.62");
        assert!(root.join("workspaces").join("v0").is_dir());
        assert_workspace_structure(&root);
        assert!(!root.join("workspaces").join("v0.1.62").exists());
        assert!(root.join("forge").join("memory").join("v0.md").is_file());
        assert!(root.join("forge").join("tasks").join("v0.md").is_file());
        assert!(root.join("forge").join("errors").join("v0.md").is_file());
        assert!(root.join("forge").join("versions").join("v0.md").is_file());
        assert!(
            !root
                .join("forge")
                .join("versions")
                .join("v0.1.62.md")
                .exists()
        );
        let version_record = fs::read_to_string(root.join("forge").join("versions").join("v0.md"))
            .expect("major version record should be readable");
        assert!(version_record.contains("## v0.1.62"));
        assert_eq!(report.state.current_version, "v0.1.61");
        assert_eq!(report.state.status, "candidate_prepared");
        assert_eq!(
            report.state.version_scheme.as_deref(),
            Some("semantic:vMAJOR.MINOR.PATCH")
        );
        assert_eq!(report.state.candidate_version.as_deref(), Some("v0.1.62"));
        assert_eq!(
            report.state.candidate_workspace.as_deref(),
            Some("workspaces/v0")
        );

        supervisor
            .verify_version("v0.1.62")
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
        state.workspace = "workspaces/v0.1.61".to_string();
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
        assert!(task.contains("## v0.1.62"));

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

        assert_eq!(report.previous_version, "v0.1.61");
        assert_eq!(report.promoted_version, "v0.1.62");
        assert_eq!(report.state.current_version, "v0.1.62");
        assert_eq!(report.state.parent_version.as_deref(), Some("v0.1.61"));
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

        assert_eq!(report.previous_version, "v0.1.61");
        assert_eq!(report.candidate_version, "v0.1.62");
        assert_eq!(report.result, CycleResult::Promoted);
        assert!(report.candidate_validation.is_some());
        assert_eq!(report.failure, None);
        assert_eq!(report.state.current_version, "v0.1.62");
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

        assert_eq!(report.current_version, "v0.1.61");
        assert_eq!(report.rolled_back_version, "v0.1.62");
        assert_eq!(report.state.status, "rolled_back");
        assert_eq!(report.state.current_version, "v0.1.61");
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

        assert_eq!(report.previous_version, "v0.1.61");
        assert_eq!(report.candidate_version, "v9.0.0");
        assert_eq!(report.result, CycleResult::RolledBack);
        assert!(report.candidate_validation.is_none());
        assert!(report.failure.is_some());
        assert_eq!(report.state.current_version, "v0.1.61");
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
        assert_eq!(report.starting_version, "v0.1.61");
        assert_eq!(report.stable_version, "v0.1.61");
        assert_eq!(report.candidate_version.as_deref(), Some("v0.1.62"));
        assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.63"));

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
        assert_eq!(report.starting_version, "v0.1.61");
        assert_eq!(report.stable_version, "v0.1.62");
        assert_eq!(report.candidate_version.as_deref(), Some("v0.1.63"));
        assert_eq!(report.next_expected_version.as_deref(), Some("v0.1.64"));

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
        assert_eq!(report.starting_version, "v0.1.61");
        assert_eq!(report.stable_version, "v0.1.61");
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
        let cold = fs::read_to_string(&report.archive_path)
            .expect("cold memory archive should be readable");
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
        let hot_once =
            fs::read_to_string(&first.memory_path).expect("hot memory should be readable");
        let cold_once =
            fs::read_to_string(&first.archive_path).expect("cold archive should be readable");

        let second = app
            .compact_memory("v0.1.49", 2)
            .expect("second compaction should be idempotent");
        let hot_twice =
            fs::read_to_string(&second.memory_path).expect("hot memory should be readable");
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

    #[test]
    fn agent_registry_standard_contains_core_agents() {
        let registry = AgentRegistry::standard();

        assert!(registry.find("architect").is_some());
        assert!(registry.find("builder").is_some());
        assert!(registry.find("verifier").is_some());
        assert!(registry.find("reviewer").is_some());
        assert!(registry.find("archivist").is_some());
        assert!(
            registry
                .agent_for(AgentCapability::Implementation)
                .expect("implementation agent should exist")
                .has_capability(AgentCapability::Runtime)
        );
    }

    #[test]
    fn agent_plan_for_goal_has_ordered_steps_and_archival_tail() {
        let registry = AgentRegistry::standard();

        let plan = registry
            .plan_for_goal("实现多 Agent 协作")
            .expect("agent plan should be generated");

        assert_eq!(plan.goal, "实现多 Agent 协作");
        assert_eq!(plan.steps.len(), 6);
        assert_eq!(plan.steps[0].order, 1);
        assert_eq!(plan.steps[0].capability, AgentCapability::Planning);
        assert_eq!(plan.steps[5].agent_id, "archivist");
        assert_eq!(plan.steps[5].capability, AgentCapability::Documentation);
        assert!(
            plan.steps
                .iter()
                .all(|step| !step.verification.trim().is_empty())
        );
    }

    #[test]
    fn app_agent_plan_rejects_empty_goal() {
        let root = temp_root("agent-empty-goal");
        let app = SelfForgeApp::new(&root);

        let error = app
            .agent_plan("   ")
            .expect_err("empty goal must be rejected");

        assert!(matches!(error, AgentError::EmptyGoal));
    }

    #[test]
    fn app_agent_plan_with_memory_includes_recent_insights() {
        let root = temp_root("agent-plan-memory");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent plan memory test");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            "# v0 记忆记录\n\n## v0.1.49 最终记忆\n\n# 错误总结\n\n本轮没有新增未解决错误。\n\n# 评估\n\nv0.1.49 已完成结构化经验提取。\n\n# 优化建议\n\n计划阶段应该直接展示历史经验摘要。\n\n# 可复用经验\n\n应用层报告应复用统一记忆经验结构。\n\n## v0.1.33 最终记忆\n\n# 错误总结\n\n旧风险记录。\n\n# 评估\n\n旧评估记录。\n\n# 优化建议\n\n旧建议记录。\n\n# 可复用经验\n\n旧经验记录。\n",
        )
        .expect("test should write memory archive");

        let report = app
            .agent_plan_with_memory("生成带记忆经验的计划", CURRENT_VERSION, 1)
            .expect("agent plan should include memory insights");

        assert_eq!(report.plan.goal, "生成带记忆经验的计划");
        assert_eq!(report.insights.source_versions, vec!["v0.1.49"]);
        assert_eq!(report.insights.success_experiences.len(), 1);
        assert_eq!(report.insights.failure_experiences.len(), 0);
        assert!(
            report.insights.optimization_suggestions[0]
                .text
                .contains("计划阶段")
        );
        assert!(
            report.insights.reusable_experiences[0]
                .text
                .contains("应用层报告")
        );

        cleanup(&root);
    }

    #[test]
    fn app_agent_plan_with_memory_accepts_zero_limit() {
        let root = temp_root("agent-plan-memory-zero");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before zero limit agent plan test");

        let report = app
            .agent_plan_with_memory("零条记忆也应能生成计划", CURRENT_VERSION, 0)
            .expect("zero limit should still produce an agent plan");

        assert_eq!(report.plan.steps.len(), 6);
        assert!(report.insights.source_versions.is_empty());
        assert!(report.insights.reusable_experiences.is_empty());

        cleanup(&root);
    }

    #[test]
    fn app_agent_plan_with_memory_reports_missing_archive() {
        let root = temp_root("agent-plan-memory-missing");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before missing archive agent plan test");

        let error = app
            .agent_plan_with_memory("读取不存在的记忆", "v9.0.0", 5)
            .expect_err("missing memory archive should stop agent plan report");

        assert!(matches!(
            error,
            AgentPlanReportError::Memory(MemoryContextError::MissingArchive {
                ref version,
                ..
            }) if version == "v9.0.0"
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_tools_load_builtin_assignments_without_config() {
        let root = temp_root("agent-tools-builtin");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before builtin tool test");

        let report = app
            .agent_tools(CURRENT_VERSION)
            .expect("builtin tools should load without dynamic config");

        assert_eq!(report.version, CURRENT_VERSION);
        assert!(!report.config_exists);
        assert!(report.tools.iter().any(|tool| tool.id == "runtime.run"));
        let builder = report
            .assignments
            .iter()
            .find(|assignment| assignment.agent_id == "builder")
            .expect("builder assignment should exist");
        assert!(builder.tool_ids.iter().any(|tool| tool == "runtime.run"));
        let architect = report
            .assignments
            .iter()
            .find(|assignment| assignment.agent_id == "architect")
            .expect("architect assignment should exist");
        assert!(
            architect
                .tool_ids
                .iter()
                .any(|tool| tool == "memory.insights")
        );

        cleanup(&root);
    }

    #[test]
    fn agent_tools_can_initialize_dynamic_config_file() {
        let root = temp_root("agent-tools-init");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before tool config init test");

        let created = app
            .init_agent_tool_config(CURRENT_VERSION)
            .expect("tool config should initialize");
        assert!(created.created);
        assert!(created.config_path.is_file());

        let second = app
            .init_agent_tool_config(CURRENT_VERSION)
            .expect("tool config init should be idempotent");
        assert!(!second.created);

        let report = app
            .agent_tools(CURRENT_VERSION)
            .expect("initialized tool config should load");
        assert!(report.config_exists);

        cleanup(&root);
    }

    #[test]
    fn agent_work_queue_initializes_major_scoped_file() {
        let root = temp_root("agent-work-init");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before work queue init test");

        let report = app
            .init_agent_work_queue(CURRENT_VERSION, "多 AI 并行修改代码", 3)
            .expect("work queue should initialize");

        assert!(report.created);
        assert_eq!(report.version, CURRENT_VERSION);
        assert_eq!(report.queue.thread_count, 3);
        assert_eq!(report.queue.tasks.len(), 5);
        assert!(report.queue_path.is_file());
        assert!(
            report
                .queue_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with("workspaces/v0/artifacts/agents/coordination/work-queue.json")
        );
        assert!(
            report
                .queue
                .tasks
                .iter()
                .all(|task| task.prompt.contains("只处理领取到的任务"))
        );

        let status = app
            .agent_work_status(CURRENT_VERSION)
            .expect("work queue status should be readable");
        assert_eq!(status.queue.tasks.len(), 5);

        let second = app
            .init_agent_work_queue(CURRENT_VERSION, "不会覆盖已有队列", 9)
            .expect("work queue init should be idempotent");
        assert!(!second.created);
        assert_eq!(second.queue.thread_count, 3);

        cleanup(&root);
    }

    #[test]
    fn agent_work_queue_retargets_existing_major_queue_version() {
        let root = temp_root("agent-work-retarget");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before work queue retarget test");
        app.init_agent_work_queue("v0.9.0", "旧版本协作队列", 2)
            .expect("old version work queue should initialize");

        let report = app
            .init_agent_work_queue(CURRENT_VERSION, "当前版本协作队列", 5)
            .expect("existing work queue should retarget to current version");

        assert!(!report.created);
        assert_eq!(report.queue.version, CURRENT_VERSION);
        assert_eq!(report.queue.thread_count, 2);
        assert!(report.queue.events.iter().any(|event| {
            event.action == "retarget" && event.message.contains(CURRENT_VERSION)
        }));

        cleanup(&root);
    }

    #[test]
    fn agent_work_claims_unclaimed_tasks_without_duplication() {
        let root = temp_root("agent-work-claim");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before work claim test");
        app.init_agent_work_queue(CURRENT_VERSION, "多线程领取任务", 3)
            .expect("work queue should initialize");

        let first = app
            .claim_agent_work(CURRENT_VERSION, "ai-1", Some("builder"))
            .expect("first builder should claim an application task");
        let second = app
            .claim_agent_work(CURRENT_VERSION, "ai-2", Some("builder"))
            .expect("second builder should claim another builder task");

        assert_eq!(first.task.id, "coord-002-application");
        assert_eq!(second.task.id, "coord-003-cli");
        assert_ne!(first.task.id, second.task.id);
        assert_eq!(first.task.claimed_by.as_deref(), Some("ai-1"));
        assert_eq!(second.task.claimed_by.as_deref(), Some("ai-2"));
        assert!(first.prompt.contains("只完成当前已领取任务"));
        let available_line = first
            .prompt
            .lines()
            .find(|line| line.starts_with("当前仍可领取任务："))
            .expect("claim prompt should list remaining tasks");
        assert!(!available_line.contains(&first.task.id));

        let status = app
            .agent_work_status(CURRENT_VERSION)
            .expect("work queue status should remain readable");
        let claimed_count = status
            .queue
            .tasks
            .iter()
            .filter(|task| task.status == AgentWorkTaskStatus::Claimed)
            .count();
        assert_eq!(claimed_count, 2);

        cleanup(&root);
    }

    #[test]
    fn agent_work_claim_skips_overlapping_write_scope() {
        let root = temp_root("agent-work-conflict");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before scope conflict test");
        let init = app
            .init_agent_work_queue(CURRENT_VERSION, "验证写入范围冲突", 3)
            .expect("work queue should initialize");
        app.claim_agent_work(CURRENT_VERSION, "ai-1", Some("builder"))
            .expect("application task should be claimed first");

        let mut queue: AgentWorkQueue =
            serde_json::from_str(&fs::read_to_string(&init.queue_path).expect("queue readable"))
                .expect("queue json should parse");
        queue.tasks.push(AgentWorkTask {
            id: "coord-conflict".to_string(),
            title: "冲突写入范围任务".to_string(),
            description: "该任务故意覆盖已领取任务的子路径。".to_string(),
            preferred_agent_id: "reviewer".to_string(),
            priority: 1,
            depends_on: Vec::new(),
            write_scope: vec!["src/app/agent/coordination.rs".to_string()],
            acceptance: vec!["冲突任务不得被第二个线程领取。".to_string()],
            status: AgentWorkTaskStatus::Pending,
            claimed_by: None,
            claimed_at_unix_seconds: None,
            lease_expires_at_unix_seconds: None,
            completed_at_unix_seconds: None,
            result: None,
            prompt: "只处理领取到的任务。".to_string(),
        });
        fs::write(
            &init.queue_path,
            serde_json::to_string_pretty(&queue).expect("queue should serialize"),
        )
        .expect("test should write modified queue");

        let second = app
            .claim_agent_work(CURRENT_VERSION, "ai-2", None)
            .expect("second worker should claim a non-conflicting task");

        assert_ne!(second.task.id, "coord-conflict");
        assert_eq!(second.task.id, "coord-001-architecture");
        let status = app
            .agent_work_status(CURRENT_VERSION)
            .expect("work queue status should be readable");
        let conflict = status
            .queue
            .tasks
            .iter()
            .find(|task| task.id == "coord-conflict")
            .expect("conflict task should remain in queue");
        assert_eq!(conflict.status, AgentWorkTaskStatus::Pending);

        cleanup(&root);
    }

    #[test]
    fn agent_work_complete_requires_claiming_worker() {
        let root = temp_root("agent-work-wrong-worker");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before wrong worker test");
        app.init_agent_work_queue(CURRENT_VERSION, "验证完成权限", 1)
            .expect("work queue should initialize");
        let claim = app
            .claim_agent_work(CURRENT_VERSION, "ai-1", None)
            .expect("task should be claimed");

        let error = app
            .complete_agent_work(CURRENT_VERSION, &claim.task.id, "ai-2", "错误完成")
            .expect_err("unclaimed worker must not complete a task");

        assert!(matches!(
            error,
            AgentWorkError::TaskNotClaimedByWorker { .. }
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_work_release_returns_task_to_pending() {
        let root = temp_root("agent-work-release");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before release test");
        app.init_agent_work_queue(CURRENT_VERSION, "验证释放任务", 1)
            .expect("work queue should initialize");
        let claim = app
            .claim_agent_work(CURRENT_VERSION, "ai-1", None)
            .expect("task should be claimed");

        let report = app
            .release_agent_work(CURRENT_VERSION, &claim.task.id, "ai-1", "发现范围冲突")
            .expect("claiming worker should release task");
        let task = report
            .queue
            .tasks
            .iter()
            .find(|task| task.id == claim.task.id)
            .expect("released task should remain in queue");

        assert_eq!(task.status, AgentWorkTaskStatus::Pending);
        assert_eq!(task.claimed_by, None);
        assert_eq!(task.claimed_at_unix_seconds, None);
        assert_eq!(task.lease_expires_at_unix_seconds, None);
        assert_eq!(task.result.as_deref(), Some("发现范围冲突"));
        assert!(task.prompt.contains("候选执行者"));
        assert!(!task.prompt.contains("ai-1"));

        cleanup(&root);
    }

    #[test]
    fn agent_work_release_clears_completed_timestamp() {
        let root = temp_root("agent-work-release-completed");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before completed release test");
        app.init_agent_work_queue(CURRENT_VERSION, "验证完成后释放", 1)
            .expect("work queue should initialize");
        let claim = app
            .claim_agent_work(CURRENT_VERSION, "ai-1", None)
            .expect("task should be claimed");
        let completed = app
            .complete_agent_work(CURRENT_VERSION, &claim.task.id, "ai-1", "验证完成")
            .expect("claiming worker should complete task");
        let completed_task = completed
            .queue
            .tasks
            .iter()
            .find(|task| task.id == claim.task.id)
            .expect("completed task should remain in queue");
        assert!(completed_task.completed_at_unix_seconds.is_some());

        let released = app
            .release_agent_work(CURRENT_VERSION, &claim.task.id, "ai-1", "恢复待领取")
            .expect("claiming worker should release completed task");
        let released_task = released
            .queue
            .tasks
            .iter()
            .find(|task| task.id == claim.task.id)
            .expect("released task should remain in queue");

        assert_eq!(released_task.status, AgentWorkTaskStatus::Pending);
        assert_eq!(released_task.completed_at_unix_seconds, None);
        assert_eq!(released_task.result.as_deref(), Some("恢复待领取"));

        cleanup(&root);
    }

    #[test]
    fn agent_work_claim_records_lease_expiration() {
        let root = temp_root("agent-work-lease");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before lease test");
        let init = app
            .init_agent_work_queue(CURRENT_VERSION, "验证任务租约", 1)
            .expect("work queue should initialize");

        let claim = app
            .claim_agent_work_with_lease(CURRENT_VERSION, "ai-1", Some("builder"), Some(120))
            .expect("task should be claimed with a lease");

        assert_eq!(init.queue.lease_duration_seconds, 3_600);
        assert_eq!(claim.task.claimed_by.as_deref(), Some("ai-1"));
        let claimed_at = claim
            .task
            .claimed_at_unix_seconds
            .expect("claimed task should record claim time");
        let lease_expires_at = claim
            .task
            .lease_expires_at_unix_seconds
            .expect("claimed task should record lease expiration");
        assert_eq!(lease_expires_at, claimed_at + 120);
        assert!(claim.prompt.contains("租约到期时间"));

        cleanup(&root);
    }

    #[test]
    fn agent_work_claim_rejects_zero_lease() {
        let root = temp_root("agent-work-zero-lease");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before zero lease test");
        app.init_agent_work_queue(CURRENT_VERSION, "验证非法租约", 1)
            .expect("work queue should initialize");

        let error = app
            .claim_agent_work_with_lease(CURRENT_VERSION, "ai-1", None, Some(0))
            .expect_err("zero lease must be rejected");

        assert!(matches!(error, AgentWorkError::InvalidLeaseSeconds));

        cleanup(&root);
    }

    #[test]
    fn agent_work_reap_releases_expired_claims() {
        let root = temp_root("agent-work-reap-expired");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before expired lease test");
        let init = app
            .init_agent_work_queue(CURRENT_VERSION, "验证租约清理", 1)
            .expect("work queue should initialize");
        let claim = app
            .claim_agent_work_with_lease(CURRENT_VERSION, "ai-1", Some("builder"), Some(120))
            .expect("task should be claimed before forcing expiration");

        let mut queue: AgentWorkQueue =
            serde_json::from_str(&fs::read_to_string(&init.queue_path).expect("queue readable"))
                .expect("queue json should parse");
        let task = queue
            .tasks
            .iter_mut()
            .find(|task| task.id == claim.task.id)
            .expect("claimed task should be present");
        task.lease_expires_at_unix_seconds = Some(1);
        fs::write(
            &init.queue_path,
            serde_json::to_string_pretty(&queue).expect("queue should serialize"),
        )
        .expect("test should write expired queue");

        let report = app
            .reap_expired_agent_work(CURRENT_VERSION, "测试租约过期")
            .expect("expired task should be reaped");

        assert_eq!(report.released_tasks.len(), 1);
        assert_eq!(report.released_tasks[0].id, claim.task.id);
        let task = report
            .queue
            .tasks
            .iter()
            .find(|task| task.id == claim.task.id)
            .expect("released task should remain in queue");
        assert_eq!(task.status, AgentWorkTaskStatus::Pending);
        assert_eq!(task.claimed_by, None);
        assert_eq!(task.claimed_at_unix_seconds, None);
        assert_eq!(task.lease_expires_at_unix_seconds, None);
        assert_eq!(task.result.as_deref(), Some("测试租约过期"));
        assert!(task.prompt.contains("候选执行者"));
        assert!(
            report
                .queue
                .events
                .iter()
                .any(|event| event.action == "reap")
        );

        cleanup(&root);
    }

    #[test]
    fn agent_work_reap_keeps_active_claims() {
        let root = temp_root("agent-work-reap-active");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before active lease test");
        app.init_agent_work_queue(CURRENT_VERSION, "验证活跃租约", 1)
            .expect("work queue should initialize");
        let claim = app
            .claim_agent_work_with_lease(CURRENT_VERSION, "ai-1", Some("builder"), Some(86_400))
            .expect("task should be claimed with a long lease");

        let report = app
            .reap_expired_agent_work(CURRENT_VERSION, "活跃租约不应释放")
            .expect("active lease reap should succeed without changes");

        assert!(report.released_tasks.is_empty());
        let task = report
            .queue
            .tasks
            .iter()
            .find(|task| task.id == claim.task.id)
            .expect("claimed task should remain in queue");
        assert_eq!(task.status, AgentWorkTaskStatus::Claimed);
        assert_eq!(task.claimed_by.as_deref(), Some("ai-1"));
        assert!(task.lease_expires_at_unix_seconds.is_some());

        cleanup(&root);
    }

    #[test]
    fn agent_work_queue_rejects_zero_threads() {
        let root = temp_root("agent-work-zero-thread");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before zero thread test");

        let error = app
            .init_agent_work_queue(CURRENT_VERSION, "非法线程数量", 0)
            .expect_err("zero thread count must be rejected");

        assert!(matches!(error, AgentWorkError::InvalidThreadCount));

        cleanup(&root);
    }

    #[test]
    fn agent_tools_load_dynamic_tool_and_plan_assignments() {
        let root = temp_root("agent-tools-dynamic");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before dynamic tool test");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION} 最终记忆\n\n# 错误总结\n\n本轮没有新增未解决错误。\n\n# 评估\n\n动态工具应能进入计划。\n\n# 优化建议\n\n工具配置需要可组合。\n\n# 可复用经验\n\n工具绑定应由应用层统一解析。\n"
            ),
        )
        .expect("test should write memory archive");
        let tool_config = root
            .join("workspaces")
            .join("v0")
            .join("artifacts")
            .join("agents")
            .join("tool-config.json");
        fs::create_dir_all(
            tool_config
                .parent()
                .expect("tool config should have parent"),
        )
        .expect("test should create agent artifact directory");
        fs::write(
            &tool_config,
            r#"{
  "tools": [
    {
      "id": "custom.audit",
      "name": "自定义审计工具",
      "description": "用于验证动态工具配置是否进入计划。",
      "kind": "custom",
      "capabilities": ["Review"],
      "agent_ids": [],
      "enabled": true
    }
  ],
  "agent_bindings": [
    {
      "agent_id": "builder",
      "tool_ids": ["custom.audit"]
    }
  ]
}
"#,
        )
        .expect("test should write dynamic tool config");

        let report = app
            .agent_tools(CURRENT_VERSION)
            .expect("dynamic tool config should load");
        assert!(report.config_exists);
        assert!(report.tools.iter().any(|tool| tool.id == "custom.audit"));
        let builder = report
            .assignments
            .iter()
            .find(|assignment| assignment.agent_id == "builder")
            .expect("builder assignment should exist");
        assert!(builder.tool_ids.iter().any(|tool| tool == "custom.audit"));

        let plan = app
            .agent_plan_with_memory("验证动态工具计划", CURRENT_VERSION, 1)
            .expect("agent plan should include dynamic tool assignments");
        let builder_step = plan
            .plan
            .steps
            .iter()
            .find(|step| step.agent_id == "builder")
            .expect("builder step should exist");
        assert!(
            builder_step
                .tool_ids
                .iter()
                .any(|tool| tool == "custom.audit")
        );
        assert!(
            plan.tools
                .tools
                .iter()
                .any(|tool| tool.id == "custom.audit")
        );

        cleanup(&root);
    }

    #[test]
    fn agent_tools_report_unknown_binding_tool() {
        let root = temp_root("agent-tools-unknown");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before unknown tool test");
        let tool_config = root
            .join("workspaces")
            .join("v0")
            .join("artifacts")
            .join("agents")
            .join("tool-config.json");
        fs::create_dir_all(
            tool_config
                .parent()
                .expect("tool config should have parent"),
        )
        .expect("test should create agent artifact directory");
        fs::write(
            &tool_config,
            r#"{
  "agent_bindings": [
    {
      "agent_id": "builder",
      "tool_ids": ["missing.tool"]
    }
  ]
}
"#,
        )
        .expect("test should write invalid tool config");

        let error = app
            .agent_tools(CURRENT_VERSION)
            .expect_err("unknown tool binding should fail");

        assert!(matches!(
            error,
            AgentToolError::UnknownTool { ref tool_id } if tool_id == "missing.tool"
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_tool_invocation_reads_memory_insights() {
        let root = temp_root("agent-tool-invoke-memory");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before tool invocation");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION} 最终记忆\n\n# 错误总结\n\n本轮没有新增未解决错误。\n\n# 评估\n\n工具调用应读取结构化记忆。\n\n# 优化建议\n\n工具执行入口应位于应用层。\n\n# 可复用经验\n\n工具调用需要先验证 Agent 绑定。\n"
            ),
        )
        .expect("test should write memory archive");

        let report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "architect".to_string(),
                tool_id: "memory.insights".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::MemoryInsights { limit: 1 },
            })
            .expect("architect should invoke memory insights");

        assert_eq!(report.agent_id, "architect");
        assert_eq!(report.tool_id, "memory.insights");
        assert!(report.summary.contains("成功 1"));
        assert!(
            report
                .details
                .iter()
                .any(|detail| detail.contains(CURRENT_VERSION))
        );

        cleanup(&root);
    }

    #[test]
    fn agent_tool_invocation_rejects_unassigned_tool() {
        let root = temp_root("agent-tool-unassigned");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before unassigned tool test");

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "memory.context".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::MemoryContext { limit: 1 },
            })
            .expect_err("builder should not invoke an unassigned memory tool");

        assert!(matches!(
            error,
            AgentToolInvocationError::ToolNotAssigned {
                ref agent_id,
                ref tool_id
            } if agent_id == "builder" && tool_id == "memory.context"
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_tool_invocation_rejects_wrong_input_kind() {
        let root = temp_root("agent-tool-wrong-input");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before wrong input test");

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "architect".to_string(),
                tool_id: "memory.insights".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::MemoryContext { limit: 1 },
            })
            .expect_err("tool should reject mismatched input");

        assert!(matches!(
            error,
            AgentToolInvocationError::UnsupportedInput {
                ref tool_id,
                ref expected
            } if tool_id == "memory.insights" && expected == "MemoryInsights"
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_tool_invocation_runtime_run_updates_session() {
        let root = temp_root("agent-tool-runtime");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before runtime tool test");
        let session = app
            .start_agent_session(CURRENT_VERSION, "通过工具执行 Runtime")
            .expect("session should start before runtime tool invocation");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "runtime.run".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::RuntimeRun {
                    session_version: CURRENT_VERSION.to_string(),
                    session_id: session.id.clone(),
                    target_version: CURRENT_VERSION.to_string(),
                    step_order: 3,
                    program,
                    args: vec!["--help".to_string()],
                    timeout_ms: 5_000,
                },
            })
            .expect("builder should invoke runtime tool");

        assert!(report.run.is_some());
        assert!(report.summary.contains("退出码 Some(0)"));
        let updated = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("session should remain readable after runtime tool");
        let step = updated
            .steps
            .iter()
            .find(|step| step.order == 3)
            .expect("builder step should exist");
        assert_eq!(step.status, AgentStepStatus::Completed);
        assert!(
            updated
                .events
                .iter()
                .any(|event| event.kind == AgentSessionEventKind::RuntimeRun)
        );

        cleanup(&root);
    }

    #[test]
    fn agent_step_executes_next_memory_tool_and_persists_result() {
        let root = temp_root("agent-step-memory");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent step test");
        let session = app
            .start_agent_session(CURRENT_VERSION, "自动执行下一步")
            .expect("session should start before step execution");

        let report = app
            .execute_next_agent_step(AgentStepExecutionRequest {
                session_version: CURRENT_VERSION.to_string(),
                session_id: session.id.clone(),
                target_version: CURRENT_VERSION.to_string(),
                tool_id: None,
                limit: 2,
                program: None,
                args: Vec::new(),
                timeout_ms: 5_000,
                prompt: None,
            })
            .expect("first step should execute a memory tool");

        assert_eq!(report.step_order, 1);
        assert_eq!(report.tool.tool_id, "memory.context");
        assert_eq!(
            report.work_task_id.as_deref(),
            Some("coord-001-architecture")
        );
        assert!(
            report
                .work_worker_id
                .as_deref()
                .unwrap_or_default()
                .contains("-step-1")
        );
        assert!(!report.session_completed);
        let updated = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("session should remain readable");
        assert_eq!(updated.status, AgentSessionStatus::Running);
        assert_eq!(updated.steps[0].status, AgentStepStatus::Completed);
        assert_eq!(
            updated.steps[0].work_task_id.as_deref(),
            Some("coord-001-architecture")
        );
        assert!(
            updated.steps[0]
                .result
                .as_deref()
                .unwrap_or_default()
                .contains("memory.context")
        );
        let queue = app
            .agent_work_status(CURRENT_VERSION)
            .expect("work queue should remain readable");
        let task = queue
            .queue
            .tasks
            .iter()
            .find(|task| task.id == "coord-001-architecture")
            .expect("claimed task should remain in queue");
        assert_eq!(task.status, AgentWorkTaskStatus::Completed);
        assert!(
            task.result
                .as_deref()
                .unwrap_or_default()
                .contains("memory.context")
        );

        cleanup(&root);
    }

    #[test]
    fn agent_step_requires_runtime_command_without_mutating_step() {
        let root = temp_root("agent-step-runtime-input");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before runtime input test");
        let session = app
            .start_agent_session(CURRENT_VERSION, "推进到 Runtime 步骤")
            .expect("session should start before step execution");
        for _ in 0..2 {
            app.execute_next_agent_step(AgentStepExecutionRequest {
                session_version: CURRENT_VERSION.to_string(),
                session_id: session.id.clone(),
                target_version: CURRENT_VERSION.to_string(),
                tool_id: None,
                limit: 1,
                program: None,
                args: Vec::new(),
                timeout_ms: 5_000,
                prompt: None,
            })
            .expect("memory step should execute");
        }

        let error = app
            .execute_next_agent_step(AgentStepExecutionRequest {
                session_version: CURRENT_VERSION.to_string(),
                session_id: session.id.clone(),
                target_version: CURRENT_VERSION.to_string(),
                tool_id: None,
                limit: 1,
                program: None,
                args: Vec::new(),
                timeout_ms: 5_000,
                prompt: None,
            })
            .expect_err("runtime step should require a command");

        assert!(matches!(
            error,
            AgentStepExecutionError::InputRequired {
                step_order: 3,
                ref tool_id,
                ref input
            } if tool_id == "runtime.run" && input == "PROGRAM"
        ));
        let updated = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("session should remain readable");
        assert_eq!(updated.steps[2].status, AgentStepStatus::Pending);
        assert_eq!(updated.steps[2].work_task_id, None);
        let queue = app
            .agent_work_status(CURRENT_VERSION)
            .expect("work queue should remain readable");
        let step_worker = format!("{}-step-3", session.id);
        assert!(
            queue
                .queue
                .tasks
                .iter()
                .all(|task| task.claimed_by.as_deref() != Some(step_worker.as_str()))
        );

        cleanup(&root);
    }

    #[test]
    fn agent_step_runtime_command_updates_session_with_run() {
        let root = temp_root("agent-step-runtime-run");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before runtime step test");
        let session = app
            .start_agent_session(CURRENT_VERSION, "执行 Runtime 步进")
            .expect("session should start before step execution");
        for _ in 0..2 {
            app.execute_next_agent_step(AgentStepExecutionRequest {
                session_version: CURRENT_VERSION.to_string(),
                session_id: session.id.clone(),
                target_version: CURRENT_VERSION.to_string(),
                tool_id: None,
                limit: 1,
                program: None,
                args: Vec::new(),
                timeout_ms: 5_000,
                prompt: None,
            })
            .expect("memory step should execute");
        }
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let report = app
            .execute_next_agent_step(AgentStepExecutionRequest {
                session_version: CURRENT_VERSION.to_string(),
                session_id: session.id.clone(),
                target_version: CURRENT_VERSION.to_string(),
                tool_id: None,
                limit: 1,
                program: Some(program),
                args: vec!["--help".to_string()],
                timeout_ms: 5_000,
                prompt: None,
            })
            .expect("runtime step should execute with command");

        assert_eq!(report.step_order, 3);
        assert_eq!(report.tool.tool_id, "runtime.run");
        assert!(report.tool.run.is_some());
        assert_eq!(report.work_task_id.as_deref(), Some("coord-003-cli"));
        let updated = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("session should remain readable");
        assert_eq!(updated.steps[2].status, AgentStepStatus::Completed);
        assert_eq!(
            updated.steps[2].work_task_id.as_deref(),
            Some("coord-003-cli")
        );
        assert!(
            updated
                .events
                .iter()
                .any(|event| event.kind == AgentSessionEventKind::RuntimeRun)
        );
        let queue = app
            .agent_work_status(CURRENT_VERSION)
            .expect("work queue should remain readable");
        let task = queue
            .queue
            .tasks
            .iter()
            .find(|task| task.id == "coord-003-cli")
            .expect("runtime work task should remain in queue");
        assert_eq!(task.status, AgentWorkTaskStatus::Completed);

        cleanup(&root);
    }

    #[test]
    fn agent_step_reports_when_no_pending_step_exists() {
        let root = temp_root("agent-step-no-pending");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before no pending test");
        let mut session = app
            .start_agent_session(CURRENT_VERSION, "没有待执行步骤")
            .expect("session should start before manual completion");
        for order in 1..=session.steps.len() {
            session
                .update_step(order, AgentStepStatus::Completed, "测试完成")
                .expect("test should complete every step");
        }
        session.mark_completed("测试完成。");
        AgentSessionStore::new(&root)
            .save(&session)
            .expect("completed session should save");

        let error = app
            .execute_next_agent_step(AgentStepExecutionRequest {
                session_version: CURRENT_VERSION.to_string(),
                session_id: session.id.clone(),
                target_version: CURRENT_VERSION.to_string(),
                tool_id: None,
                limit: 1,
                program: None,
                args: Vec::new(),
                timeout_ms: 5_000,
                prompt: None,
            })
            .expect_err("completed session should not have pending steps");

        assert!(matches!(
            error,
            AgentStepExecutionError::NoPendingStep { ref session_id }
                if session_id == &session.id
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_steps_runs_until_step_limit_without_external_input() {
        let root = temp_root("agent-steps-limit");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before multi step test");
        let session = app
            .start_agent_session(CURRENT_VERSION, "受控多步运行")
            .expect("session should start before multi step execution");

        let report = app
            .execute_agent_steps(
                AgentStepExecutionRequest {
                    session_version: CURRENT_VERSION.to_string(),
                    session_id: session.id.clone(),
                    target_version: CURRENT_VERSION.to_string(),
                    tool_id: None,
                    limit: 2,
                    program: None,
                    args: Vec::new(),
                    timeout_ms: 5_000,
                    prompt: None,
                },
                2,
            )
            .expect("multi step execution should stop at limit");

        assert_eq!(report.executed_steps.len(), 2);
        assert_eq!(report.stop, AgentStepRunStop::StepLimitReached);
        let updated = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("session should remain readable");
        assert_eq!(updated.steps[0].status, AgentStepStatus::Completed);
        assert_eq!(updated.steps[1].status, AgentStepStatus::Completed);
        assert_eq!(updated.steps[2].status, AgentStepStatus::Pending);

        cleanup(&root);
    }

    #[test]
    fn agent_steps_stops_when_next_step_requires_external_input() {
        let root = temp_root("agent-steps-input");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before multi step input test");
        let session = app
            .start_agent_session(CURRENT_VERSION, "受控停止等待输入")
            .expect("session should start before multi step input test");

        let report = app
            .execute_agent_steps(
                AgentStepExecutionRequest {
                    session_version: CURRENT_VERSION.to_string(),
                    session_id: session.id.clone(),
                    target_version: CURRENT_VERSION.to_string(),
                    tool_id: None,
                    limit: 2,
                    program: None,
                    args: Vec::new(),
                    timeout_ms: 5_000,
                    prompt: None,
                },
                10,
            )
            .expect("multi step execution should stop for missing input");

        assert_eq!(report.executed_steps.len(), 2);
        assert!(matches!(
            report.stop,
            AgentStepRunStop::InputRequired {
                step_order: 3,
                ref tool_id,
                ref input
            } if tool_id == "runtime.run" && input == "PROGRAM"
        ));
        let updated = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("session should remain readable");
        assert_eq!(updated.steps[2].status, AgentStepStatus::Pending);
        assert_eq!(updated.steps[2].work_task_id, None);

        cleanup(&root);
    }

    #[test]
    fn agent_steps_rejects_zero_max_steps() {
        let root = temp_root("agent-steps-zero");
        let app = SelfForgeApp::new(&root);

        let error = app
            .execute_agent_steps(
                AgentStepExecutionRequest {
                    session_version: CURRENT_VERSION.to_string(),
                    session_id: "session-for-zero-test".to_string(),
                    target_version: CURRENT_VERSION.to_string(),
                    tool_id: None,
                    limit: 1,
                    program: None,
                    args: Vec::new(),
                    timeout_ms: 5_000,
                    prompt: None,
                },
                0,
            )
            .expect_err("zero max steps must be rejected before session loading");

        assert!(matches!(error, AgentStepRunError::InvalidStepLimit));

        cleanup(&root);
    }

    #[test]
    fn agent_tool_invocation_reports_missing_custom_runner() {
        let root = temp_root("agent-tool-custom-runner");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before custom runner test");
        let tool_config = root
            .join("workspaces")
            .join("v0")
            .join("artifacts")
            .join("agents")
            .join("tool-config.json");
        fs::create_dir_all(
            tool_config
                .parent()
                .expect("tool config should have parent"),
        )
        .expect("test should create agent artifact directory");
        fs::write(
            &tool_config,
            r#"{
  "tools": [
    {
      "id": "custom.audit",
      "name": "自定义审计工具",
      "description": "用于验证缺失执行器的错误路径。",
      "kind": "custom",
      "capabilities": ["Review"],
      "agent_ids": ["builder"],
      "enabled": true
    }
  ]
}
"#,
        )
        .expect("test should write custom tool config");

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "custom.audit".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::Empty,
            })
            .expect_err("custom tool without runner should fail explicitly");

        assert!(matches!(
            error,
            AgentToolInvocationError::ToolRunnerMissing { ref tool_id }
                if tool_id == "custom.audit"
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_registry_rejects_duplicate_agent_ids() {
        let agents = vec![
            AgentDefinition::new(
                "builder",
                "实现 Agent",
                "负责实现",
                vec![AgentCapability::Implementation],
                vec!["计划"],
                vec!["代码"],
            ),
            AgentDefinition::new(
                "builder",
                "重复 Agent",
                "重复标识",
                vec![AgentCapability::Testing],
                vec!["代码"],
                vec!["测试"],
            ),
        ];

        let error = AgentRegistry::new(agents).expect_err("duplicate id must be rejected");

        assert!(matches!(
            error,
            AgentError::DuplicateAgent { ref id } if id == "builder"
        ));
    }

    #[test]
    fn agent_plan_reports_missing_capability() {
        let registry = AgentRegistry::new(vec![AgentDefinition::new(
            "architect",
            "架构 Agent",
            "只负责架构",
            vec![AgentCapability::Planning],
            vec!["目标"],
            vec!["计划"],
        )])
        .expect("single agent registry should be valid");

        let error = registry
            .plan_for_goal("实现功能")
            .expect_err("missing architecture capability should be reported");

        assert!(matches!(
            error,
            AgentError::MissingCapability {
                capability: AgentCapability::Architecture
            }
        ));
    }

    #[test]
    fn agent_session_start_persists_session_and_index() {
        let root = temp_root("agent-session-start");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent session test");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION} 最终记忆\n\n# 错误总结\n\n本轮没有新增未解决错误。\n\n# 评估\n\n会话应保存计划上下文。\n\n# 优化建议\n\n计划依据需要可审计。\n\n# 可复用经验\n\n会话上下文应复用记忆经验结构。\n"
            ),
        )
        .expect("test should write memory archive");

        let session = app
            .start_agent_session(CURRENT_VERSION, "持久化 Agent 会话")
            .expect("agent session should be persisted");

        assert_eq!(session.version, CURRENT_VERSION);
        assert_eq!(session.status, AgentSessionStatus::Planned);
        assert_eq!(session.steps.len(), 6);
        assert_eq!(session.events.len(), 2);
        let context = session
            .plan_context
            .as_ref()
            .expect("agent session should persist plan context");
        assert_eq!(context.memory_version, CURRENT_VERSION);
        assert!(context.memory_archive_file.contains("forge"));
        assert!(context.memory_archive_file.contains("v0.md"));
        let work_queue = context
            .work_queue
            .as_ref()
            .expect("agent session should persist work queue context");
        assert_eq!(work_queue.version, CURRENT_VERSION);
        assert_eq!(work_queue.task_count, 5);
        assert_eq!(work_queue.thread_count, 5);
        assert_eq!(work_queue.lease_duration_seconds, 3_600);
        assert!(work_queue.created);
        assert!(work_queue.queue_file.contains("work-queue.json"));
        assert!(root.join(&work_queue.queue_file).is_file());
        assert!(!context.source_versions.is_empty());
        assert_eq!(
            session.events[0].kind,
            AgentSessionEventKind::SessionCreated
        );
        assert_eq!(
            session.events[1].kind,
            AgentSessionEventKind::WorkQueuePrepared
        );
        assert!(
            session
                .steps
                .iter()
                .all(|step| step.status == AgentStepStatus::Pending)
        );
        assert!(root.join(&session.file).is_file());
        assert!(
            root.join("workspaces")
                .join("v0")
                .join("artifacts")
                .join("agents")
                .join("index.jsonl")
                .is_file()
        );

        let listed = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("agent session index should be readable");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, session.id);
        assert_eq!(listed[0].step_count, 6);
        assert_eq!(listed[0].event_count, 2);

        let loaded = app
            .agent_session(CURRENT_VERSION, &session.id)
            .expect("agent session should be loadable");
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.goal, "持久化 Agent 会话");
        assert_eq!(loaded.plan_context, session.plan_context);
        assert_eq!(loaded.events.len(), 2);

        cleanup(&root);
    }

    #[test]
    fn agent_session_start_reuses_existing_work_queue_context() {
        let root = temp_root("agent-session-work-queue-reuse");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before work queue context test");
        app.init_agent_work_queue(CURRENT_VERSION, "预置协作队列", 3)
            .expect("preexisting work queue should initialize");

        let session = app
            .start_agent_session(CURRENT_VERSION, "复用协作任务板")
            .expect("agent session should reuse work queue");

        let context = session
            .plan_context
            .as_ref()
            .and_then(|context| context.work_queue.as_ref())
            .expect("work queue context should be persisted");
        assert_eq!(context.thread_count, 3);
        assert!(!context.created);
        assert!(session.events.iter().any(|event| {
            event.kind == AgentSessionEventKind::WorkQueuePrepared
                && event.message.contains("已复用")
        }));

        cleanup(&root);
    }

    #[test]
    fn agent_session_list_respects_limit_and_version() {
        let root = temp_root("agent-session-list");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent session list test");

        let first = app
            .start_agent_session(CURRENT_VERSION, "第一条会话")
            .expect("first current session should persist");
        let second = app
            .start_agent_session(CURRENT_VERSION, "第二条会话")
            .expect("second current session should persist");
        let other = app
            .start_agent_session("v0.9.0", "其他小版本会话")
            .expect("same major workspace should accept another small version session");

        let limited = app
            .agent_sessions(CURRENT_VERSION, 1)
            .expect("agent session list should support limit");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].id, second.id);

        let current = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("agent session list should filter by version");
        assert_eq!(current.len(), 2);
        assert!(current.iter().any(|session| session.id == first.id));
        assert!(current.iter().any(|session| session.id == second.id));
        assert!(current.iter().all(|session| session.id != other.id));

        let none = app
            .agent_sessions(CURRENT_VERSION, 0)
            .expect("zero limit should be accepted");
        assert!(none.is_empty());

        cleanup(&root);
    }

    #[test]
    fn agent_session_list_all_major_reads_latest_summary_across_versions() {
        let root = temp_root("agent-session-list-all");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before all session list test");

        let current = app
            .start_agent_session(CURRENT_VERSION, "当前小版本会话")
            .expect("current session should persist");
        let mut other = app
            .start_agent_session("v0.9.0", "同 major 其他小版本会话")
            .expect("same major workspace should accept another version session");
        other.mark_completed("其他小版本会话已完成");
        AgentSessionStore::new(&root)
            .save(&other)
            .expect("updated session should append a latest summary");

        let current_only = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("version scoped session list should remain filtered");
        assert_eq!(current_only.len(), 1);
        assert_eq!(current_only[0].id, current.id);

        let all = app
            .agent_sessions_all(CURRENT_VERSION, 10)
            .expect("all major session list should be readable");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, other.id);
        assert_eq!(all[0].status, AgentSessionStatus::Completed);
        assert_eq!(all[0].outcome.as_deref(), Some("其他小版本会话已完成"));
        assert!(all.iter().any(|session| session.id == current.id));

        let limited = app
            .agent_sessions_all(CURRENT_VERSION, 1)
            .expect("all major session list should support limit");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].id, other.id);

        let none = app
            .agent_sessions_all(CURRENT_VERSION, 0)
            .expect("zero limit should be accepted");
        assert!(none.is_empty());

        cleanup(&root);
    }

    #[test]
    fn agent_session_rejects_empty_goal() {
        let root = temp_root("agent-session-empty");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before empty session goal test");

        let error = app
            .start_agent_session(CURRENT_VERSION, "   ")
            .expect_err("empty goal must be rejected");

        assert!(matches!(
            error,
            AgentSessionError::Agent(AgentError::EmptyGoal)
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_session_records_failed_context_when_memory_archive_is_missing() {
        let root = temp_root("agent-session-missing-memory");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before missing memory session test");
        fs::remove_file(root.join("forge").join("memory").join("v0.md"))
            .expect("test should remove memory archive");

        let error = app
            .start_agent_session(CURRENT_VERSION, "缺失记忆时停止会话")
            .expect_err("missing memory archive should fail session start");

        assert!(matches!(error, AgentSessionError::PlanContext { .. }));
        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("failed session should remain indexed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Failed);

        let loaded = app
            .agent_session(CURRENT_VERSION, &sessions[0].id)
            .expect("failed session should remain readable");
        assert_eq!(loaded.plan_context, None);
        assert!(loaded.error.unwrap_or_default().contains("记忆归档不存在"));

        cleanup(&root);
    }

    #[test]
    fn agent_session_reports_missing_and_invalid_session_id() {
        let root = temp_root("agent-session-missing");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before missing session test");

        let missing = app
            .agent_session(CURRENT_VERSION, "agent-session-missing")
            .expect_err("missing session should be reported");
        assert!(matches!(missing, AgentSessionError::NotFound { .. }));

        let invalid = app
            .agent_session(CURRENT_VERSION, "../agent-session-missing")
            .expect_err("invalid session id must be rejected");
        assert!(matches!(
            invalid,
            AgentSessionError::InvalidSessionId { .. }
        ));

        cleanup(&root);
    }

    #[test]
    fn agent_session_save_updates_latest_summary_without_duplicate_listing() {
        let root = temp_root("agent-session-save");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before session save test");

        let store = AgentSessionStore::new(&root);
        let mut session = store
            .start(CURRENT_VERSION, "更新会话状态")
            .expect("session should be created");
        session.mark_running();
        session
            .update_step(1, AgentStepStatus::Completed, "计划已生成")
            .expect("session step should update");
        session.mark_completed("会话状态已更新");
        store.save(&session).expect("session should be saved");

        let listed = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("session index should be readable");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, session.id);
        assert_eq!(listed[0].status, AgentSessionStatus::Completed);
        assert_eq!(listed[0].outcome.as_deref(), Some("会话状态已更新"));
        assert_eq!(listed[0].event_count, 4);

        let loaded = store
            .load(CURRENT_VERSION, &session.id)
            .expect("updated session should remain loadable");
        assert_eq!(loaded.events.len(), 4);
        assert_eq!(loaded.events[0].order, 1);
        assert_eq!(loaded.events[0].kind, AgentSessionEventKind::SessionCreated);
        assert_eq!(loaded.events[2].kind, AgentSessionEventKind::StepUpdated);
        assert_eq!(loaded.events[2].step_order, Some(1));
        assert!(loaded.events[3].message.contains("会话已完成"));

        cleanup(&root);
    }

    #[test]
    fn agent_session_summary_accepts_legacy_index_without_event_count() {
        let legacy_summary = r#"{
            "id": "agent-session-legacy",
            "version": "v0.1.1",
            "goal": "旧会话",
            "status": "Completed",
            "created_at_unix_seconds": 1,
            "updated_at_unix_seconds": 2,
            "step_count": 6,
            "file": "workspaces/v0/artifacts/agents/sessions/agent-session-legacy.json"
        }"#;

        let summary = serde_json::from_str::<AgentSessionSummary>(legacy_summary)
            .expect("legacy session summary should remain readable");

        assert_eq!(summary.event_count, 0);
        assert_eq!(summary.status, AgentSessionStatus::Completed);
    }

    #[test]
    fn agent_session_accepts_legacy_file_without_plan_context() {
        let legacy_session = r#"{
            "id": "agent-session-legacy",
            "version": "v0.1.1",
            "goal": "旧会话",
            "status": "Completed",
            "created_at_unix_seconds": 1,
            "updated_at_unix_seconds": 2,
            "plan": {
                "goal": "旧会话",
                "agents": [],
                "steps": []
            },
            "steps": [],
            "outcome": "旧会话已完成",
            "events": [],
            "file": "workspaces/v0/artifacts/agents/sessions/agent-session-legacy.json"
        }"#;

        let session = serde_json::from_str::<AgentSession>(legacy_session)
            .expect("legacy session without plan context should remain readable");

        assert_eq!(session.id, "agent-session-legacy");
        assert_eq!(session.plan_context, None);
        assert_eq!(session.status, AgentSessionStatus::Completed);
    }

    #[test]
    fn agent_session_event_accepts_legacy_event_without_run_reference() {
        let legacy_event = r#"{
            "order": 1,
            "timestamp_unix_seconds": 1,
            "kind": "StepUpdated",
            "step_order": 4,
            "message": "旧事件"
        }"#;

        let event = serde_json::from_str::<AgentSessionEvent>(legacy_event)
            .expect("legacy event should remain readable");

        assert_eq!(event.kind, AgentSessionEventKind::StepUpdated);
        assert_eq!(event.step_order, Some(4));
        assert_eq!(event.run, None);
    }

    #[test]
    fn agent_run_records_successful_runtime_reference_in_session_event() {
        let root = temp_root("agent-run-success");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent run");
        let session = app
            .start_agent_session(CURRENT_VERSION, "执行 Runtime 并关联会话")
            .expect("agent session should be created before agent run");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let report = app
            .agent_run(
                CURRENT_VERSION,
                &session.id,
                CURRENT_VERSION,
                4,
                &program,
                &["--help".to_string()],
                5_000,
            )
            .expect("agent run should execute and persist a run reference");

        assert_eq!(report.step_order, 4);
        assert_eq!(report.execution.exit_code, Some(0));
        assert!(!report.execution.timed_out);
        assert_eq!(report.session.status, AgentSessionStatus::Running);
        assert_eq!(report.session.steps[3].status, AgentStepStatus::Completed);
        let run_event = report
            .session
            .events
            .iter()
            .find(|event| event.kind == AgentSessionEventKind::RuntimeRun)
            .expect("runtime run event should be recorded");
        let reference = run_event
            .run
            .as_ref()
            .expect("runtime run event should contain a run reference");
        assert_eq!(reference.run_id, report.run_id);
        assert_eq!(reference.version, CURRENT_VERSION);
        assert_eq!(reference.exit_code, Some(0));
        assert!(reference.report_file.contains(&report.run_id));

        let runs = app
            .supervisor()
            .list_runs(CURRENT_VERSION, 10)
            .expect("runtime run should be queryable");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, report.run_id);

        cleanup(&root);
    }

    #[test]
    fn agent_run_marks_session_failed_when_runtime_exit_fails() {
        let root = temp_root("agent-run-failed");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before failed agent run");
        let session = app
            .start_agent_session(CURRENT_VERSION, "记录失败 Runtime 运行")
            .expect("agent session should be created before failed run");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let report = app
            .agent_run(
                CURRENT_VERSION,
                &session.id,
                CURRENT_VERSION,
                4,
                &program,
                &["--self-forge-invalid-test-flag".to_string()],
                5_000,
            )
            .expect("non-zero exit should still be persisted as a runtime run");

        assert_ne!(report.execution.exit_code, Some(0));
        assert_eq!(report.session.status, AgentSessionStatus::Failed);
        assert_eq!(report.session.steps[3].status, AgentStepStatus::Failed);
        assert!(report.session.error.is_some());
        let run_event = report
            .session
            .events
            .iter()
            .find(|event| event.kind == AgentSessionEventKind::RuntimeRun)
            .expect("failed runtime run event should be recorded");
        let reference = run_event
            .run
            .as_ref()
            .expect("failed runtime event should contain a run reference");
        assert_eq!(reference.run_id, report.run_id);
        assert_ne!(reference.exit_code, Some(0));

        cleanup(&root);
    }

    #[test]
    fn agent_verify_creates_completed_session_with_runtime_reference() {
        let root = temp_root("agent-verify-success");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent verify");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let report = app
            .agent_verify(
                "执行验证命令",
                CURRENT_VERSION,
                &program,
                &["--help".to_string()],
                5_000,
            )
            .expect("agent verify should create a completed session");

        assert_eq!(report.execution.exit_code, Some(0));
        assert_eq!(report.session.status, AgentSessionStatus::Completed);
        assert!(
            report.session.steps[0]
                .result
                .as_deref()
                .unwrap_or_default()
                .contains("历史记忆")
        );
        assert_eq!(report.session.steps[3].status, AgentStepStatus::Completed);
        assert_eq!(report.session.steps[4].status, AgentStepStatus::Completed);
        assert_eq!(report.session.steps[5].status, AgentStepStatus::Completed);
        let run_event = report
            .session
            .events
            .iter()
            .find(|event| event.kind == AgentSessionEventKind::RuntimeRun)
            .expect("verification session should contain a runtime run event");
        let reference = run_event
            .run
            .as_ref()
            .expect("runtime event should include a run reference");
        assert_eq!(reference.run_id, report.run_id);
        assert_eq!(reference.exit_code, Some(0));

        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("verification session should be indexed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Completed);

        cleanup(&root);
    }

    #[test]
    fn agent_verify_marks_session_failed_when_runtime_command_fails() {
        let root = temp_root("agent-verify-failed");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before failed agent verify");
        let program = std::env::current_exe()
            .expect("test executable path should be available")
            .to_string_lossy()
            .into_owned();

        let report = app
            .agent_verify(
                "执行失败验证命令",
                CURRENT_VERSION,
                &program,
                &["--self-forge-invalid-test-flag".to_string()],
                5_000,
            )
            .expect("non-zero verification run should still be persisted");

        assert_ne!(report.execution.exit_code, Some(0));
        assert_eq!(report.session.status, AgentSessionStatus::Failed);
        assert_eq!(report.session.steps[3].status, AgentStepStatus::Failed);
        assert!(report.session.error.is_some());
        assert!(report.session.events.iter().any(|event| {
            event.kind == AgentSessionEventKind::RuntimeRun && event.run.is_some()
        }));

        cleanup(&root);
    }

    #[test]
    fn agent_verify_reports_empty_program_as_execution_error() {
        let root = temp_root("agent-verify-empty-program");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before empty program agent verify");

        let error = app
            .agent_verify("验证空命令", CURRENT_VERSION, "", &[], 1_000)
            .expect_err("empty program should be reported as an execution error");

        assert!(matches!(error, AgentRunError::Execution { .. }));
        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("failed verification session should be indexed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Failed);

        cleanup(&root);
    }

    #[test]
    fn agent_advance_prepares_candidate_and_completes_session() {
        let root = temp_root("agent-advance-prepare");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent advance");

        let report = app
            .agent_advance("Agent 驱动候选生成")
            .expect("agent advance should prepare a candidate");

        assert_eq!(report.minimal_loop.outcome, MinimalLoopOutcome::Prepared);
        assert_eq!(report.minimal_loop.starting_version, CURRENT_VERSION);
        assert_eq!(report.minimal_loop.stable_version, CURRENT_VERSION);
        assert_eq!(
            report.minimal_loop.candidate_version.as_deref(),
            Some("v0.1.62")
        );
        assert_eq!(report.session.status, AgentSessionStatus::Completed);
        assert!(
            report
                .session
                .steps
                .iter()
                .all(|step| step.status == AgentStepStatus::Completed)
        );

        let state = ForgeState::load(&root).expect("state should remain readable");
        assert_eq!(state.current_version, CURRENT_VERSION);
        assert_eq!(state.candidate_version.as_deref(), Some("v0.1.62"));
        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("completed agent session should be listed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Completed);

        cleanup(&root);
    }

    #[test]
    fn agent_advance_promotes_existing_candidate_and_prepares_next() {
        let root = temp_root("agent-advance-promote");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent advance");
        app.supervisor()
            .prepare_next_version("prepare candidate")
            .expect("candidate should be prepared before agent advance");

        let report = app
            .agent_advance("Agent 推进候选提升")
            .expect("agent advance should promote and prepare");

        assert_eq!(
            report.minimal_loop.outcome,
            MinimalLoopOutcome::PromotedAndPrepared
        );
        assert_eq!(report.minimal_loop.starting_version, CURRENT_VERSION);
        assert_eq!(report.minimal_loop.stable_version, "v0.1.62");
        assert_eq!(
            report.minimal_loop.candidate_version.as_deref(),
            Some("v0.1.63")
        );
        assert_eq!(report.session.status, AgentSessionStatus::Completed);

        let state = ForgeState::load(&root).expect("state should remain readable");
        assert_eq!(state.current_version, "v0.1.62");
        assert_eq!(state.candidate_version.as_deref(), Some("v0.1.63"));

        cleanup(&root);
    }

    #[test]
    fn agent_advance_stops_and_marks_session_failed_when_open_errors_exist() {
        let root = temp_root("agent-advance-open-errors");
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
            .expect("failed run should be archived before agent advance");

        let error = app
            .agent_advance("存在错误时停止")
            .expect_err("agent advance must stop when current version has open errors");

        assert!(matches!(
            error,
            AgentEvolutionError::Blocked {
                ref open_errors,
                ..
            } if open_errors.len() == 1
        ));
        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("failed agent session should be listed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Failed);

        let state = ForgeState::load(&root).expect("state should remain readable");
        assert_eq!(state.current_version, CURRENT_VERSION);
        assert_eq!(state.candidate_version, None);

        cleanup(&root);
    }

    #[test]
    fn agent_evolve_prepares_candidate_runs_cycle_and_completes_session() {
        let root = temp_root("agent-evolve-cycle");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent evolve");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION} 最终记忆\n\n# 错误总结\n\n本轮没有新增未解决错误。\n\n# 评估\n\nAgent 进化会话应保存计划依据。\n\n# 优化建议\n\n执行计划时继续引用计划上下文。\n\n# 可复用经验\n\n计划上下文应持久化到会话文件。\n"
            ),
        )
        .expect("test should write memory archive");

        let report = app
            .agent_evolve("Agent 单轮完整进化")
            .expect("agent evolve should prepare and promote a candidate");

        assert_eq!(
            report.prepared_candidate_version.as_deref(),
            Some("v0.1.62")
        );
        assert_eq!(report.cycle.previous_version, CURRENT_VERSION);
        assert_eq!(report.cycle.candidate_version, "v0.1.62");
        assert_eq!(report.cycle.result, CycleResult::Promoted);
        assert_eq!(report.cycle.state.current_version, "v0.1.62");
        assert_eq!(report.cycle.state.candidate_version, None);
        assert!(report.memory_compaction.is_some());
        assert_eq!(report.session.status, AgentSessionStatus::Completed);
        let context = report
            .session
            .plan_context
            .as_ref()
            .expect("agent evolve session should persist plan context");
        assert_eq!(context.memory_version, CURRENT_VERSION);
        assert!(!context.source_versions.is_empty());
        assert!(
            report.session.steps[0]
                .result
                .as_deref()
                .unwrap_or_default()
                .contains("历史记忆")
        );
        assert!(report.session.events.len() >= 8);
        assert!(report.session.events.iter().any(|event| {
            event.kind == AgentSessionEventKind::StepUpdated && event.step_order == Some(4)
        }));
        assert!(
            report
                .session
                .steps
                .iter()
                .all(|step| step.status == AgentStepStatus::Completed)
        );

        let state = ForgeState::load(&root).expect("state should remain readable");
        assert_eq!(state.current_version, "v0.1.62");
        assert_eq!(state.candidate_version, None);
        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("completed evolve session should be listed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Completed);

        cleanup(&root);
    }

    #[test]
    fn agent_session_list_all_major_finds_evolve_session_after_promotion() {
        let root = temp_root("agent-session-list-after-promotion");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before promotion session query test");

        let report = app
            .agent_evolve("提升后仍可审计 Agent 会话")
            .expect("agent evolve should promote a candidate");

        assert_eq!(report.cycle.state.current_version, "v0.1.62");

        let promoted_version_only = app
            .agent_sessions("v0.1.62", 10)
            .expect("promoted version scoped session list should be readable");
        assert!(
            promoted_version_only.is_empty(),
            "the session belongs to the version that started the evolution"
        );

        let all = app
            .agent_sessions_all("v0.1.62", 10)
            .expect("all major session list should find previous patch session");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, report.session.id);
        assert_eq!(all[0].version, CURRENT_VERSION);
        assert_eq!(all[0].status, AgentSessionStatus::Completed);

        cleanup(&root);
    }

    #[test]
    fn agent_evolve_cycles_existing_candidate_without_preparing_another() {
        let root = temp_root("agent-evolve-existing-candidate");
        let app = SelfForgeApp::new(&root);

        app.supervisor()
            .initialize_current_version()
            .expect("bootstrap should succeed before agent evolve");
        app.supervisor()
            .prepare_next_version("已有候选版本")
            .expect("candidate should be prepared before agent evolve");

        let report = app
            .agent_evolve("验证已有候选版本")
            .expect("agent evolve should cycle the existing candidate");

        assert_eq!(report.prepared_candidate_version, None);
        assert_eq!(report.cycle.previous_version, CURRENT_VERSION);
        assert_eq!(report.cycle.candidate_version, "v0.1.62");
        assert_eq!(report.cycle.result, CycleResult::Promoted);
        assert_eq!(report.cycle.state.current_version, "v0.1.62");
        assert_eq!(report.cycle.state.candidate_version, None);
        assert_eq!(report.session.status, AgentSessionStatus::Completed);

        cleanup(&root);
    }

    #[test]
    fn agent_evolve_stops_before_candidate_when_open_errors_exist() {
        let root = temp_root("agent-evolve-open-errors");
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
            .expect("failed run should be archived before agent evolve");

        let error = app
            .agent_evolve("存在错误时停止单轮进化")
            .expect_err("agent evolve must stop when current version has open errors");

        assert!(matches!(
            error,
            AgentEvolutionError::Blocked {
                ref open_errors,
                ..
            } if open_errors.len() == 1
        ));
        let state = ForgeState::load(&root).expect("state should remain readable");
        assert_eq!(state.current_version, CURRENT_VERSION);
        assert_eq!(state.candidate_version, None);
        let sessions = app
            .agent_sessions(CURRENT_VERSION, 10)
            .expect("failed evolve session should be listed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, AgentSessionStatus::Failed);

        cleanup(&root);
    }

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

    fn create_patch_draft_for_audit(
        root: &Path,
        app: &SelfForgeApp,
        scope_markdown: &str,
    ) -> AiPatchDraftRecord {
        fs::write(
            root.join(".env"),
            "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-audit-key\n",
        )
        .expect("test should write dotenv file");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备受控补丁草案能力。\n\n# 优化建议\n\n实现候选补丁差异审计和冲突检查。\n\n# 可复用经验\n\nAI 生成代码前必须先审计写入范围。\n"
            ),
        )
        .expect("test should write memory archive");
        let preview = app
            .ai_patch_draft_preview_with_lookup("生成补丁草案", |_| None)
            .expect("preview should build before audit fixture");
        let request = preview.request.clone();
        let ai = AiExecutionReport {
            request,
            response: AiTextResponse {
                provider_id: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                protocol: "openai-chat-completions".to_string(),
                text: format!(
                    "# 补丁目标\n生成受控补丁草案。\n\n# 计划\n1. 审计写入范围。\n2. 执行测试。\n\n# 允许写入范围\n{scope_markdown}\n\n# 代码草案\n```rust\nfn example() {{}}\n```\n\n# 测试草案\n```rust\n#[test]\nfn example_test() {{}}\n```\n\n# 验证命令\ncargo test\n\n# 风险与回滚\n失败时保留稳定版本。\n"
                ),
                raw_bytes: 300,
            },
            status_code: 200,
        };

        app.finish_ai_patch_draft(preview, ai)
            .expect("successful patch draft fixture should be written")
            .record
    }

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
        assert_eq!(state.candidate_version.as_deref(), Some("v0.1.62"));
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
            serde_json::to_string_pretty(&preview_record)
                .expect("mutated preview should serialize"),
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
        assert_eq!(report.record.candidate_version, "v0.1.62");
        let state = ForgeState::load(&root).expect("state should remain readable");
        assert_eq!(state.candidate_version.as_deref(), Some("v0.1.62"));
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
                |_spec, _timeout_ms| {
                    unreachable!("unknown command should be rejected before runner")
                },
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
        let markdown = fs::read_to_string(root.join(report_file))
            .expect("source execution markdown should exist");
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
        let markdown =
            fs::read_to_string(root.join(report_file)).expect("cycle markdown should exist");
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
            AiPatchSourceTaskDraftError::Summary(
                AiPatchSourceCycleFollowUpStoreError::NotFound { .. }
            )
        ));

        cleanup(&root);
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

        let preview =
            app.ai_self_upgrade_preview_with_lookup("优先完善 AI 自动自我升级入口", |_| None)
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
    AgentCapability, AgentDefinition, AgentError, AgentEvolutionError, AgentEvolutionReport,
    AgentPlan, AgentPlanReport, AgentPlanReportError, AgentPlanStep, AgentRegistry, AgentRunError,
    AgentRunReference, AgentRunReport, AgentSession, AgentSessionError, AgentSessionEvent,
    AgentSessionEventKind, AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus,
    AgentSessionStep, AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext,
    AgentSingleEvolutionReport, AgentStepExecutionError, AgentStepExecutionReport,
    AgentStepExecutionRequest, AgentStepRunError, AgentStepRunReport, AgentStepRunStop,
    AgentStepStatus, AgentToolAssignment, AgentToolBinding, AgentToolConfig,
    AgentToolConfigInitReport, AgentToolDefinition, AgentToolError, AgentToolInvocation,
    AgentToolInvocationError, AgentToolInvocationInput, AgentToolInvocationReport, AgentToolReport,
    AgentVerificationReport, AgentWorkClaimReport, AgentWorkCoordinator, AgentWorkError,
    AgentWorkEvent, AgentWorkQueue, AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTask,
    AgentWorkTaskStatus, AiConfigError, AiConfigReport, AiExecutionError, AiExecutionReport,
    AiPatchApplicationError, AiPatchApplicationFile, AiPatchApplicationRecord,
    AiPatchApplicationReport, AiPatchApplicationStatus, AiPatchApplicationStore,
    AiPatchApplicationStoreError, AiPatchApplicationSummary, AiPatchAuditError,
    AiPatchAuditFinding, AiPatchAuditFindingKind, AiPatchAuditRecord, AiPatchAuditReport,
    AiPatchAuditSeverity, AiPatchAuditStatus, AiPatchAuditStore, AiPatchAuditStoreError,
    AiPatchAuditSummary, AiPatchDraftError, AiPatchDraftPreview, AiPatchDraftRecord,
    AiPatchDraftReport, AiPatchDraftStatus, AiPatchDraftStore, AiPatchDraftStoreError,
    AiPatchDraftSummary, AiPatchPreviewChange, AiPatchPreviewError, AiPatchPreviewRecord,
    AiPatchPreviewReport, AiPatchPreviewStatus, AiPatchPreviewStore, AiPatchPreviewStoreError,
    AiPatchPreviewSummary, AiPatchSourceCandidateError, AiPatchSourceCandidateRecord,
    AiPatchSourceCandidateReport, AiPatchSourceCandidateStatus, AiPatchSourceCandidateStore,
    AiPatchSourceCandidateStoreError, AiPatchSourceCandidateSummary, AiPatchSourceCycleError,
    AiPatchSourceCycleFollowUpRecord, AiPatchSourceCycleFollowUpStatus,
    AiPatchSourceCycleFollowUpStore, AiPatchSourceCycleFollowUpStoreError,
    AiPatchSourceCycleFollowUpSummary, AiPatchSourceCycleRecord, AiPatchSourceCycleReport,
    AiPatchSourceCycleResult, AiPatchSourceCycleStatus, AiPatchSourceCycleStore,
    AiPatchSourceCycleStoreError, AiPatchSourceCycleSummary, AiPatchSourceCycleSummaryError,
    AiPatchSourceCycleSummaryReport, AiPatchSourceExecutionError, AiPatchSourceExecutionFile,
    AiPatchSourceExecutionRecord, AiPatchSourceExecutionReport, AiPatchSourceExecutionStatus,
    AiPatchSourceExecutionStore, AiPatchSourceExecutionStoreError, AiPatchSourceExecutionSummary,
    AiPatchSourcePlanError, AiPatchSourcePlanFile, AiPatchSourcePlanRecord,
    AiPatchSourcePlanReport, AiPatchSourcePlanStatus, AiPatchSourcePlanStore,
    AiPatchSourcePlanStoreError, AiPatchSourcePlanSummary, AiPatchSourcePromotionError,
    AiPatchSourcePromotionRecord, AiPatchSourcePromotionReport, AiPatchSourcePromotionStatus,
    AiPatchSourcePromotionStore, AiPatchSourcePromotionStoreError, AiPatchSourcePromotionSummary,
    AiPatchSourceTaskDraftError, AiPatchSourceTaskDraftRecord, AiPatchSourceTaskDraftReport,
    AiPatchSourceTaskDraftStatus, AiPatchSourceTaskDraftStore, AiPatchSourceTaskDraftStoreError,
    AiPatchSourceTaskDraftSummary, AiPatchVerificationCommandRecord,
    AiPatchVerificationCommandSpec, AiPatchVerificationError, AiPatchVerificationReport,
    AiPatchVerificationStatus, AiProviderRegistry, AiProviderStatus, AiRawHttpResponse,
    AiRequestError, AiRequestSpec, AiResponseError, AiSelfUpgradeAuditError,
    AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditStatus, AiSelfUpgradeAuditStore,
    AiSelfUpgradeAuditSummary, AiSelfUpgradeError, AiSelfUpgradePreview, AiSelfUpgradeReport,
    AiSelfUpgradeSummaryError, AiSelfUpgradeSummaryIndexEntry, AiSelfUpgradeSummaryRecord,
    AiSelfUpgradeSummaryReport, AiSelfUpgradeSummaryStatus, AiSelfUpgradeSummaryStore,
    AiSelfUpgradeSummaryStoreError, AiTextResponse, ArchivedErrorEntry, ErrorArchive,
    ErrorArchiveError, ErrorArchiveReport, ErrorListQuery, ErrorResolutionReport,
    MemoryCompactionError, MemoryCompactionReport, MemoryContextEntry, MemoryContextError,
    MemoryContextReport, MemoryInsight, MemoryInsightReport, MinimalLoopError, MinimalLoopOutcome,
    MinimalLoopReport, PreflightReport, SelfForgeApp, normalize_ai_self_upgrade_goal,
};
