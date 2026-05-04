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

pub const CURRENT_VERSION: &str = "v0.1.73";

pub use app::{
    AgentCapability, AgentCodeDiffReport, AgentCodeListEntry, AgentCodeListReport,
    AgentCodeOutlineItem, AgentCodeOutlineReport, AgentCodeReadReport, AgentCodeSearchMatch,
    AgentCodeSearchReport, AgentCodeToolError, AgentDefinition, AgentError, AgentEvolutionError,
    AgentEvolutionReport, AgentPlan, AgentPlanReport, AgentPlanReportError, AgentPlanStep,
    AgentRegistry, AgentRunError, AgentRunReference, AgentRunReport, AgentSession,
    AgentSessionError, AgentSessionEvent, AgentSessionEventKind, AgentSessionMemoryInsight,
    AgentSessionPlanContext, AgentSessionStatus, AgentSessionStep, AgentSessionStore,
    AgentSessionSummary, AgentSessionWorkQueueContext, AgentSingleEvolutionReport, AgentSkillError,
    AgentSkillIndex, AgentSkillIndexReport, AgentSkillMetadata, AgentSkillSelection,
    AgentSkillSelectionReport, AgentSkillSelectionRequest, AgentStepExecutionError,
    AgentStepExecutionReport, AgentStepExecutionRequest, AgentStepRunError, AgentStepRunReport,
    AgentStepRunStop, AgentStepStatus, AgentToolAssignment, AgentToolBinding, AgentToolConfig,
    AgentToolConfigInitReport, AgentToolDefinition, AgentToolError, AgentToolInvocation,
    AgentToolInvocationError, AgentToolInvocationInput, AgentToolInvocationReport, AgentToolReport,
    AgentVerificationReport, AgentWorkClaimReport, AgentWorkCompactionReport, AgentWorkCoordinator,
    AgentWorkError, AgentWorkEvent, AgentWorkFinalizeCheckError, AgentWorkFinalizeCheckReport,
    AgentWorkQueue, AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTask, AgentWorkTaskStatus,
    AiConfigError, AiConfigReport, AiExecutionError, AiExecutionReport, AiPatchApplicationError,
    AiPatchApplicationFile, AiPatchApplicationRecord, AiPatchApplicationReport,
    AiPatchApplicationStatus, AiPatchApplicationStore, AiPatchApplicationStoreError,
    AiPatchApplicationSummary, AiPatchAuditError, AiPatchAuditFinding, AiPatchAuditFindingKind,
    AiPatchAuditRecord, AiPatchAuditReport, AiPatchAuditSeverity, AiPatchAuditStatus,
    AiPatchAuditStore, AiPatchAuditStoreError, AiPatchAuditSummary, AiPatchDraftError,
    AiPatchDraftPreview, AiPatchDraftRecord, AiPatchDraftReport, AiPatchDraftStatus,
    AiPatchDraftStore, AiPatchDraftStoreError, AiPatchDraftSummary, AiPatchPreviewChange,
    AiPatchPreviewError, AiPatchPreviewRecord, AiPatchPreviewReport, AiPatchPreviewStatus,
    AiPatchPreviewStore, AiPatchPreviewStoreError, AiPatchPreviewSummary,
    AiPatchSourceCandidateError, AiPatchSourceCandidateRecord, AiPatchSourceCandidateReport,
    AiPatchSourceCandidateStatus, AiPatchSourceCandidateStore, AiPatchSourceCandidateStoreError,
    AiPatchSourceCandidateSummary, AiPatchSourceCycleError, AiPatchSourceCycleFollowUpRecord,
    AiPatchSourceCycleFollowUpStatus, AiPatchSourceCycleFollowUpStore,
    AiPatchSourceCycleFollowUpStoreError, AiPatchSourceCycleFollowUpSummary,
    AiPatchSourceCycleRecord, AiPatchSourceCycleReport, AiPatchSourceCycleResult,
    AiPatchSourceCycleStatus, AiPatchSourceCycleStore, AiPatchSourceCycleStoreError,
    AiPatchSourceCycleSummary, AiPatchSourceCycleSummaryError, AiPatchSourceCycleSummaryReport,
    AiPatchSourceExecutionError, AiPatchSourceExecutionFile, AiPatchSourceExecutionRecord,
    AiPatchSourceExecutionReport, AiPatchSourceExecutionStatus, AiPatchSourceExecutionStore,
    AiPatchSourceExecutionStoreError, AiPatchSourceExecutionSummary, AiPatchSourcePlanError,
    AiPatchSourcePlanFile, AiPatchSourcePlanRecord, AiPatchSourcePlanReport,
    AiPatchSourcePlanStatus, AiPatchSourcePlanStore, AiPatchSourcePlanStoreError,
    AiPatchSourcePlanSummary, AiPatchSourcePromotionError, AiPatchSourcePromotionRecord,
    AiPatchSourcePromotionReport, AiPatchSourcePromotionStatus, AiPatchSourcePromotionStore,
    AiPatchSourcePromotionStoreError, AiPatchSourcePromotionSummary, AiPatchSourceTaskAuditError,
    AiPatchSourceTaskAuditFinding, AiPatchSourceTaskAuditRecord, AiPatchSourceTaskAuditReport,
    AiPatchSourceTaskAuditStatus, AiPatchSourceTaskAuditStore, AiPatchSourceTaskAuditStoreError,
    AiPatchSourceTaskAuditSummary, AiPatchSourceTaskDraftError, AiPatchSourceTaskDraftRecord,
    AiPatchSourceTaskDraftReport, AiPatchSourceTaskDraftStatus, AiPatchSourceTaskDraftStore,
    AiPatchSourceTaskDraftStoreError, AiPatchSourceTaskDraftSummary,
    AiPatchVerificationCommandRecord, AiPatchVerificationCommandSpec, AiPatchVerificationError,
    AiPatchVerificationReport, AiPatchVerificationStatus, AiProviderRegistry, AiProviderStatus,
    AiRawHttpResponse, AiRequestError, AiRequestSpec, AiResponseError, AiSelfUpgradeAuditError,
    AiSelfUpgradeAuditRecord, AiSelfUpgradeAuditStatus, AiSelfUpgradeAuditStore,
    AiSelfUpgradeAuditSummary, AiSelfUpgradeError, AiSelfUpgradePreview, AiSelfUpgradeReport,
    AiSelfUpgradeSummaryError, AiSelfUpgradeSummaryIndexEntry, AiSelfUpgradeSummaryRecord,
    AiSelfUpgradeSummaryReport, AiSelfUpgradeSummaryStatus, AiSelfUpgradeSummaryStore,
    AiSelfUpgradeSummaryStoreError, AiTextResponse, ArchivedErrorEntry, BranchCheckError,
    BranchCheckReport, ErrorArchive, ErrorArchiveError, ErrorArchiveReport, ErrorListQuery,
    ErrorResolutionReport, MemoryCompactionError, MemoryCompactionReport, MemoryContextEntry,
    MemoryContextError, MemoryContextReport, MemoryInsight, MemoryInsightReport, MinimalLoopError,
    MinimalLoopOutcome, MinimalLoopReport, PreflightReport, SelfEvolutionLoopError,
    SelfEvolutionLoopGitPrEvent, SelfEvolutionLoopGitPrEventStatus, SelfEvolutionLoopGitPrMode,
    SelfEvolutionLoopGitPrRequest, SelfEvolutionLoopRecord, SelfEvolutionLoopReport,
    SelfEvolutionLoopRequest, SelfEvolutionLoopStatus, SelfEvolutionLoopStepRecord,
    SelfEvolutionLoopStepStatus, SelfEvolutionLoopSummary, SelfForgeApp,
    format_agent_skill_context, inspect_project_code_diff, list_project_code_files,
    normalize_ai_self_upgrade_goal, outline_project_code, read_project_code_file,
    search_project_code,
};

#[cfg(test)]
mod agent_skill_scaling_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("测试时间应可用")
            .as_nanos();
        std::env::temp_dir().join(format!("selfforge-{name}-{unique}"))
    }

    fn bootstrap_app(root: &PathBuf) -> SelfForgeApp {
        let app = SelfForgeApp::new(root);
        app.supervisor()
            .initialize_current_version()
            .expect("测试应先初始化当前版本");
        app
    }

    fn write_skill_index(
        app: &SelfForgeApp,
        root: &PathBuf,
        skills: Vec<AgentSkillMetadata>,
    ) -> PathBuf {
        let report = app
            .init_agent_skill_index(CURRENT_VERSION)
            .expect("技能索引应可初始化");
        let index = AgentSkillIndex { skills };
        let contents = serde_json::to_string_pretty(&index).expect("测试技能索引应可序列化") + "\n";
        fs::write(&report.index_path, contents).expect("测试应可写入技能索引");
        assert!(root.join("workspaces").join("v0").is_dir());
        report.index_path
    }

    #[test]
    fn agent_skill_index_reads_hundreds_without_loading_content() {
        let root = temp_root("skill-index-hundreds");
        let app = bootstrap_app(&root);
        let skills = (0..500)
            .map(|index| AgentSkillMetadata {
                id: format!("skill-{index}"),
                name: format!("技能 {index}"),
                summary: "只用于索引读取压力测试。".to_string(),
                tags: vec!["索引".to_string()],
                triggers: vec![format!("触发 {index}")],
                capabilities: vec!["planning".to_string()],
                content_path: Some(format!("skills/skill-{index}.md")),
                priority: 0,
                estimated_tokens: 200,
                enabled: true,
            })
            .collect();
        write_skill_index(&app, &root, skills);

        let report = app
            .agent_skill_index(CURRENT_VERSION)
            .expect("读取技能索引应成功");

        assert_eq!(report.skill_count, 500);
        assert_eq!(report.enabled_skill_count, 500);
        assert_eq!(report.loaded_skill_count, 0);
        assert!(report.estimated_index_tokens < 20_000);

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_select_loads_limited_relevant_content() {
        let root = temp_root("skill-select-budget");
        let app = bootstrap_app(&root);
        fs::create_dir_all(root.join("skills")).expect("测试应可创建技能正文目录");
        for index in 0..10 {
            fs::write(
                root.join("skills").join(format!("readme-{index}.md")),
                format!("# README 技能 {index}\n\n用于 README 命令审计。"),
            )
            .expect("测试应可写入技能正文");
        }
        let mut skills = Vec::new();
        for index in 0..500 {
            let relevant = index < 10;
            skills.push(AgentSkillMetadata {
                id: format!("skill-{index}"),
                name: if relevant {
                    format!("README 命令技能 {index}")
                } else {
                    format!("无关技能 {index}")
                },
                summary: if relevant {
                    "帮助审计 README 命令。".to_string()
                } else {
                    "无关能力。".to_string()
                },
                tags: if relevant {
                    vec!["README".to_string(), "命令".to_string()]
                } else {
                    vec!["其他".to_string()]
                },
                triggers: if relevant {
                    vec!["README 命令".to_string()]
                } else {
                    vec![format!("其他 {index}")]
                },
                capabilities: vec!["documentation".to_string()],
                content_path: relevant.then(|| format!("skills/readme-{index}.md")),
                priority: if relevant { 10 } else { 0 },
                estimated_tokens: 100,
                enabled: true,
            });
        }
        write_skill_index(&app, &root, skills);

        let mut request = AgentSkillSelectionRequest::new(CURRENT_VERSION, "需要审计 README 命令");
        request.limit = 3;
        request.token_budget = 250;
        request.required_capabilities = vec!["documentation".to_string()];
        let report = app.select_agent_skills(request).expect("技能召回应成功");

        assert_eq!(report.index_skill_count, 500);
        assert_eq!(report.selected_skill_count, 2);
        assert_eq!(report.loaded_skill_count, 2);
        assert!(report.estimated_context_tokens <= 250);
        assert!(
            report
                .skills
                .iter()
                .all(|skill| skill.metadata.name.contains("README"))
        );

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_select_reads_bounded_content_prefix() {
        let root = temp_root("skill-select-bounded-content");
        let app = bootstrap_app(&root);
        fs::create_dir_all(root.join("skills")).expect("测试应可创建技能正文目录");
        fs::write(
            root.join("skills").join("large.md"),
            format!("# 大技能\n\n{}", "技".repeat(20_000)),
        )
        .expect("测试应可写入大技能正文");
        write_skill_index(
            &app,
            &root,
            vec![AgentSkillMetadata {
                id: "large".to_string(),
                name: "README 命令大技能".to_string(),
                summary: "验证大技能正文只读取受控前缀。".to_string(),
                tags: vec!["README".to_string()],
                triggers: vec!["README 命令".to_string()],
                capabilities: vec!["documentation".to_string()],
                content_path: Some("skills/large.md".to_string()),
                priority: 10,
                estimated_tokens: 50,
                enabled: true,
            }],
        );

        let mut request = AgentSkillSelectionRequest::new(CURRENT_VERSION, "需要 README 命令能力");
        request.limit = 1;
        request.token_budget = 100;
        let report = app.select_agent_skills(request).expect("技能召回应成功");
        let content = report.skills[0]
            .content
            .as_ref()
            .expect("入选技能应加载正文前缀");

        assert_eq!(report.loaded_skill_count, 1);
        assert!(content.contains("# 大技能"));
        assert!(content.len() <= 16 * 1024);
        assert!(content.chars().count() < 20_000);

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_select_normalizes_query_and_capabilities() {
        let root = temp_root("skill-select-query-normalization");
        let app = bootstrap_app(&root);
        write_skill_index(
            &app,
            &root,
            vec![
                AgentSkillMetadata {
                    id: "readme-doc".to_string(),
                    name: "README 文档技能".to_string(),
                    summary: "用于验证查询预处理后仍能召回多词触发技能。".to_string(),
                    tags: vec!["文档".to_string()],
                    triggers: vec!["README 命令".to_string()],
                    capabilities: vec!["Documentation".to_string()],
                    content_path: None,
                    priority: 10,
                    estimated_tokens: 20,
                    enabled: true,
                },
                AgentSkillMetadata {
                    id: "runtime".to_string(),
                    name: "运行时技能".to_string(),
                    summary: "能力不同，不应被文档能力过滤命中。".to_string(),
                    tags: vec!["运行".to_string()],
                    triggers: vec!["README".to_string()],
                    capabilities: vec!["runtime".to_string()],
                    content_path: None,
                    priority: 100,
                    estimated_tokens: 20,
                    enabled: true,
                },
            ],
        );

        let mut request = AgentSkillSelectionRequest::new(CURRENT_VERSION, "需要 readme 巡检");
        request.limit = 5;
        request.token_budget = 100;
        request.required_capabilities = vec![
            " documentation ".to_string(),
            "DOCUMENTATION".to_string(),
            " ".to_string(),
        ];
        let report = app.select_agent_skills(request).expect("技能召回应成功");

        assert_eq!(report.candidate_skill_count, 1);
        assert_eq!(report.selected_skill_count, 1);
        assert_eq!(report.skills[0].metadata.id, "readme-doc");
        assert!(report.skills[0].reason.contains("触发词匹配"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_select_matches_prepared_metadata_fields() {
        let root = temp_root("skill-select-prepared-fields");
        let app = bootstrap_app(&root);
        write_skill_index(
            &app,
            &root,
            vec![AgentSkillMetadata {
                id: "prepared".to_string(),
                name: "架构 技能".to_string(),
                summary: "包含 预算 说明。".to_string(),
                tags: vec!["Token".to_string()],
                triggers: vec!["Lazy Load".to_string()],
                capabilities: vec!["Documentation".to_string()],
                content_path: None,
                priority: 10,
                estimated_tokens: 20,
                enabled: true,
            }],
        );

        let mut request =
            AgentSkillSelectionRequest::new(CURRENT_VERSION, "需要 token documentation 预算 架构");
        request.limit = 1;
        request.token_budget = 100;
        request.required_capabilities = vec!["documentation".to_string()];
        let report = app.select_agent_skills(request).expect("技能召回应成功");

        assert_eq!(report.selected_skill_count, 1);
        assert_eq!(report.skills[0].metadata.id, "prepared");
        assert!(report.skills[0].reason.contains("名称匹配"));
        assert!(report.skills[0].reason.contains("标签匹配"));
        assert!(report.skills[0].reason.contains("能力匹配"));
        assert!(report.skills[0].reason.contains("摘要匹配"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_select_scans_past_over_budget_candidates() {
        let root = temp_root("skill-select-scan-budget");
        let app = bootstrap_app(&root);
        fs::create_dir_all(root.join("skills")).expect("测试应可创建技能正文目录");
        fs::write(
            root.join("skills").join("small.md"),
            "# 小技能\n\n轻量技能正文。",
        )
        .expect("测试应可写入技能正文");

        let mut skills = (0..20)
            .map(|index| AgentSkillMetadata {
                id: format!("large-{index}"),
                name: format!("README 命令大型技能 {index}"),
                summary: "帮助审计 README 命令，但正文很大。".to_string(),
                tags: vec!["README".to_string(), "命令".to_string()],
                triggers: vec!["README 命令".to_string()],
                capabilities: vec!["documentation".to_string()],
                content_path: Some(format!("skills/large-{index}.md")),
                priority: 50,
                estimated_tokens: 500,
                enabled: true,
            })
            .collect::<Vec<_>>();
        skills.push(AgentSkillMetadata {
            id: "small".to_string(),
            name: "README 命令轻量技能".to_string(),
            summary: "帮助审计 README 命令，正文很小。".to_string(),
            tags: vec!["README".to_string(), "命令".to_string()],
            triggers: vec!["README 命令".to_string()],
            capabilities: vec!["documentation".to_string()],
            content_path: Some("skills/small.md".to_string()),
            priority: 0,
            estimated_tokens: 50,
            enabled: true,
        });
        write_skill_index(&app, &root, skills);

        let mut request = AgentSkillSelectionRequest::new(CURRENT_VERSION, "需要审计 README 命令");
        request.limit = 1;
        request.token_budget = 100;
        let report = app.select_agent_skills(request).expect("技能召回应成功");

        assert_eq!(report.selected_skill_count, 1);
        assert_eq!(report.skipped_for_budget, 20);
        assert_eq!(report.skills[0].metadata.id, "small");
        assert_eq!(report.loaded_skill_count, 1);

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_index_rejects_workspace_escape_path() {
        let root = temp_root("skill-index-invalid-path");
        let app = bootstrap_app(&root);
        write_skill_index(
            &app,
            &root,
            vec![AgentSkillMetadata {
                id: "bad-skill".to_string(),
                name: "坏路径技能".to_string(),
                summary: "验证技能正文路径不能越过项目根目录。".to_string(),
                tags: Vec::new(),
                triggers: Vec::new(),
                capabilities: Vec::new(),
                content_path: Some("../secret.md".to_string()),
                priority: 0,
                estimated_tokens: 10,
                enabled: true,
            }],
        );

        let error = app
            .agent_skill_index(CURRENT_VERSION)
            .expect_err("越界技能路径必须被拒绝");

        assert!(error.to_string().contains("不允许越过项目根目录"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn ai_self_upgrade_preview_includes_selected_skill_context() {
        let root = temp_root("skill-self-upgrade-context");
        let app = bootstrap_app(&root);
        fs::write(
            root.join(".env"),
            "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-skill-key\n",
        )
        .expect("测试应可写入 .env");
        fs::create_dir_all(root.join("skills")).expect("测试应可创建技能正文目录");
        fs::write(
            root.join("skills").join("self-upgrade.md"),
            "# 自我升级技能\n\n优先选择最小、可验证、可回滚的自我升级目标。",
        )
        .expect("测试应可写入技能正文");
        write_skill_index(
            &app,
            &root,
            vec![AgentSkillMetadata {
                id: "self-upgrade".to_string(),
                name: "AI 自我升级技能".to_string(),
                summary: "帮助 AI 自我升级目标决策。".to_string(),
                tags: vec!["AI".to_string(), "自我升级".to_string()],
                triggers: vec!["AI 自我升级".to_string()],
                capabilities: vec!["planning".to_string()],
                content_path: Some("skills/self-upgrade.md".to_string()),
                priority: 10,
                estimated_tokens: 80,
                enabled: true,
            }],
        );

        let preview = app
            .ai_self_upgrade_preview_with_lookup("继续优化 AI 自我升级", |_| None)
            .expect("自我升级预览应可召回技能上下文");

        assert_eq!(preview.skills.selected_skill_count, 1);
        assert_eq!(preview.skills.loaded_skill_count, 1);
        assert!(preview.prompt.contains("# 按需技能上下文"));
        assert!(preview.prompt.contains("AI 自我升级技能"));
        assert!(preview.prompt.contains("优先选择最小、可验证、可回滚"));
        assert!(preview.request.body.to_string().contains("AI 自我升级技能"));
        assert!(!preview.request.body.to_string().contains("test-skill-key"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn ai_patch_draft_preview_includes_selected_skill_context() {
        let root = temp_root("skill-patch-draft-context");
        let app = bootstrap_app(&root);
        fs::write(
            root.join(".env"),
            "SELFFORGE_AI_PROVIDER=deepseek\nDEEPSEEK_API_KEY=test-patch-skill-key\n",
        )
        .expect("测试应可写入 .env");
        fs::write(
            root.join("forge").join("memory").join("v0.md"),
            format!(
                "# v0 记忆记录\n\n## {CURRENT_VERSION}\n\n# 错误总结\n\n本轮没有未解决错误。\n\n# 评估\n\n系统已经具备按需技能索引。\n\n# 优化建议\n\n补丁草案应使用相关技能上下文。\n\n# 可复用经验\n\nAI 提示词必须受 token 预算约束。\n"
            ),
        )
        .expect("测试应可写入记忆归档");
        fs::create_dir_all(root.join("skills")).expect("测试应可创建技能正文目录");
        fs::write(
            root.join("skills").join("patch-draft.md"),
            "# 补丁草案技能\n\n生成补丁草案时先列计划，再列测试草案和验证命令。",
        )
        .expect("测试应可写入技能正文");
        write_skill_index(
            &app,
            &root,
            vec![AgentSkillMetadata {
                id: "patch-draft".to_string(),
                name: "AI 补丁草案技能".to_string(),
                summary: "帮助 AI 生成受控补丁草案。".to_string(),
                tags: vec!["补丁草案".to_string(), "AI".to_string()],
                triggers: vec!["补丁草案".to_string()],
                capabilities: vec!["patch".to_string()],
                content_path: Some("skills/patch-draft.md".to_string()),
                priority: 10,
                estimated_tokens: 90,
                enabled: true,
            }],
        );

        let preview = app
            .ai_patch_draft_preview_with_lookup("生成 AI 补丁草案", |_| None)
            .expect("补丁草案预览应可召回技能上下文");

        assert_eq!(preview.skills.selected_skill_count, 1);
        assert_eq!(preview.skills.loaded_skill_count, 1);
        assert!(preview.prompt.contains("# 按需技能上下文"));
        assert!(preview.prompt.contains("AI 补丁草案技能"));
        assert!(preview.prompt.contains("先列计划，再列测试草案"));
        assert!(preview.request.body.to_string().contains("AI 补丁草案技能"));
        assert!(
            !preview
                .request
                .body
                .to_string()
                .contains("test-patch-skill-key")
        );

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_skill_context_formatter_truncates_loaded_content() {
        let metadata = AgentSkillMetadata {
            id: "long-context".to_string(),
            name: "长正文技能".to_string(),
            summary: "验证技能上下文格式化和正文截断。".to_string(),
            tags: vec!["上下文".to_string()],
            triggers: vec!["长正文".to_string()],
            capabilities: vec!["format".to_string()],
            content_path: Some("skills/long.md".to_string()),
            priority: 0,
            estimated_tokens: 1_300,
            enabled: true,
        };
        let report = AgentSkillSelectionReport {
            version: CURRENT_VERSION.to_string(),
            goal: "预览长正文技能".to_string(),
            index_skill_count: 1,
            candidate_skill_count: 1,
            selected_skill_count: 1,
            loaded_skill_count: 1,
            skipped_for_budget: 0,
            estimated_context_tokens: 1_300,
            skills: vec![AgentSkillSelection {
                metadata,
                score: 10,
                reason: "测试匹配".to_string(),
                content: Some("长".repeat(1_500)),
                estimated_tokens: 1_300,
            }],
        };

        let context = format_agent_skill_context(&report);

        assert!(context.contains("技能索引 1 个"));
        assert!(context.contains("长正文技能"));
        assert!(context.contains("摘要：验证技能上下文格式化和正文截断。"));
        assert!(context.contains("正文："));
        assert!(context.contains("..."));
        assert!(context.chars().count() < 1_400);
    }
}

#[cfg(test)]
mod agent_code_tool_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("测试时间应可用")
            .as_nanos();
        std::env::temp_dir().join(format!("selfforge-{name}-{unique}"))
    }

    fn bootstrap_app(root: &PathBuf) -> SelfForgeApp {
        let app = SelfForgeApp::new(root);
        app.supervisor()
            .initialize_current_version()
            .expect("测试应先初始化当前版本");
        app
    }

    fn run_git(root: &PathBuf, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("测试应可执行 Git 命令");
        assert!(
            output.status.success(),
            "Git 命令应成功：{:?}\n{}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn agent_tools_assign_common_code_tools_to_coding_agents() {
        let root = temp_root("code-tool-assignments");
        let app = bootstrap_app(&root);

        let report = app.agent_tools(CURRENT_VERSION).expect("工具报告应可读取");
        let architect = report.tool_ids_for_agent("architect");
        let builder = report.tool_ids_for_agent("builder");
        let verifier = report.tool_ids_for_agent("verifier");
        let reviewer = report.tool_ids_for_agent("reviewer");

        assert!(architect.contains(&"code.outline".to_string()));
        assert!(builder.contains(&"code.search".to_string()));
        assert!(builder.contains(&"code.read".to_string()));
        assert!(builder.contains(&"code.diff".to_string()));
        assert!(builder.contains(&"code.outline".to_string()));
        assert!(builder.contains(&"command.run".to_string()));
        assert!(builder.contains(&"command.history".to_string()));
        assert!(verifier.contains(&"code.outline".to_string()));
        assert!(verifier.contains(&"code.read".to_string()));
        assert!(verifier.contains(&"code.diff".to_string()));
        assert!(verifier.contains(&"command.run".to_string()));
        assert!(verifier.contains(&"command.history".to_string()));
        assert!(reviewer.contains(&"code.outline".to_string()));
        assert!(reviewer.contains(&"code.search".to_string()));
        assert!(reviewer.contains(&"code.read".to_string()));
        assert!(reviewer.contains(&"code.diff".to_string()));
        assert!(!reviewer.contains(&"command.run".to_string()));
        assert!(reviewer.contains(&"command.history".to_string()));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_searches_and_reads_project_code() {
        let root = temp_root("code-tool-search-read");
        let app = bootstrap_app(&root);
        fs::create_dir_all(root.join("src").join("app")).expect("测试应可创建代码目录");
        fs::write(
            root.join("src").join("app").join("demo.rs"),
            "fn main() {\n    let marker = \"SelfForgeNeedle\";\n}\n",
        )
        .expect("测试应可写入代码文件");

        let search = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.search".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeSearch {
                    query: "selfforgeneedle".to_string(),
                    limit: 5,
                },
            })
            .expect("代码搜索工具应可调用");
        assert!(search.summary.contains("匹配 1 条"));
        assert!(search.details[0].contains("src/app/demo.rs:2"));

        let read = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.read".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeRead {
                    path: "src/app/demo.rs".to_string(),
                    max_bytes: 24,
                },
            })
            .expect("代码读取工具应可调用");
        assert!(read.summary.contains("src/app/demo.rs"));
        assert!(read.summary.contains("截断 true"));
        assert!(read.details[0].contains("fn main"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_lists_project_code_files() {
        let root = temp_root("code-tool-list");
        let app = bootstrap_app(&root);
        fs::create_dir_all(root.join("src").join("app")).expect("测试应可创建代码目录");
        fs::create_dir_all(root.join("target")).expect("测试应可创建跳过目录");
        fs::write(root.join("src").join("app").join("a.rs"), "fn a() {}\n")
            .expect("测试应可写入代码文件");
        fs::write(root.join("src").join("app").join("b.rs"), "fn b() {}\n")
            .expect("测试应可写入代码文件");
        fs::write(root.join("target").join("ignored.rs"), "fn ignored() {}\n")
            .expect("测试应可写入跳过目录文件");
        fs::write(root.join(".env"), "OPENAI_API_KEY=secret\n").expect("测试应可写入本地环境文件");
        fs::write(root.join(".env.local"), "DEEPSEEK_API_KEY=secret\n")
            .expect("测试应可写入本地环境文件");

        let report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.list".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeList {
                    path: "src/app".to_string(),
                    limit: 1,
                },
            })
            .expect("代码列表工具应可调用");

        assert!(report.summary.contains("文件 2 个"));
        assert!(report.summary.contains("返回 1 个"));
        assert!(report.summary.contains("截断 true"));
        assert!(report.details[0].contains("src/app/a.rs"));
        assert!(
            !report
                .details
                .iter()
                .any(|detail| detail.contains("target"))
        );

        let root_report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.list".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeList {
                    path: ".".to_string(),
                    limit: 100,
                },
            })
            .expect("代码列表工具应可列出项目根目录");
        assert!(
            !root_report
                .details
                .iter()
                .any(|detail| detail.contains(".env"))
        );

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.read".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeRead {
                    path: ".env".to_string(),
                    max_bytes: 0,
                },
            })
            .expect_err("本地环境文件读取必须被拒绝");
        assert!(error.to_string().contains("不允许越过项目根目录"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_reports_project_code_diff() {
        let root = temp_root("code-tool-diff");
        let app = bootstrap_app(&root);
        run_git(&root, &["init"]);
        run_git(&root, &["config", "user.email", "test@example.com"]);
        run_git(&root, &["config", "user.name", "SelfForge Test"]);
        run_git(&root, &["add", "."]);
        run_git(&root, &["commit", "-m", "初始提交"]);

        fs::write(root.join("README.md"), "# SelfForge\n\n变更标记\n")
            .expect("测试应可修改 README");
        fs::write(root.join(".env"), "OPENAI_API_KEY=secret\n").expect("测试应可写入本地环境文件");

        let report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.diff".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeDiff {
                    path: ".".to_string(),
                    max_bytes: 2_000,
                },
            })
            .expect("差异查看工具应可调用");

        assert!(report.summary.contains("状态 1 条"));
        assert!(report.summary.contains("截断 false"));
        assert!(
            report
                .details
                .iter()
                .any(|detail| detail.contains("README.md"))
        );
        assert!(
            report
                .details
                .iter()
                .any(|detail| detail.contains("变更标记"))
        );
        assert!(!report.details.iter().any(|detail| detail.contains(".env")));
        assert!(
            !report
                .details
                .iter()
                .any(|detail| detail.contains("secret"))
        );

        let truncated = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.diff".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeDiff {
                    path: "README.md".to_string(),
                    max_bytes: 40,
                },
            })
            .expect("差异查看工具应支持截断");
        assert!(truncated.summary.contains("截断 true"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_outlines_project_code_file() {
        let root = temp_root("code-tool-outline");
        let app = bootstrap_app(&root);
        fs::create_dir_all(root.join("src").join("app")).expect("测试应可创建代码目录");
        fs::write(
            root.join("src").join("app").join("outline.rs"),
            "pub struct Demo {\n}\n\nimpl Demo {\n    pub fn run(&self) {}\n}\n\nfn helper() {}\n",
        )
        .expect("测试应可写入代码文件");

        let report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "reviewer".to_string(),
                tool_id: "code.outline".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeOutline {
                    path: "src/app/outline.rs".to_string(),
                    limit: 2,
                },
            })
            .expect("code.outline 应可读取项目内代码结构");

        assert!(report.summary.contains("src/app/outline.rs"));
        assert!(report.summary.contains("符号 4 个"));
        assert!(report.summary.contains("返回 2 个"));
        assert!(report.summary.contains("截断 true"));
        assert!(report.details[0].contains("结构体 Demo"));
        assert!(report.details[1].contains("实现 Demo"));
        assert!(report.details[0].contains("src/app/outline.rs:1"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_rejects_outline_workspace_escape() {
        let root = temp_root("code-tool-outline-escape");
        let app = bootstrap_app(&root);

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "reviewer".to_string(),
                tool_id: "code.outline".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeOutline {
                    path: "../secret.rs".to_string(),
                    limit: 10,
                },
            })
            .expect_err("越界结构提纲读取必须被拒绝");

        assert!(error.to_string().contains("不允许越过项目根目录"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_rejects_outline_sensitive_env_file() {
        let root = temp_root("code-tool-outline-sensitive");
        let app = bootstrap_app(&root);
        fs::write(root.join(".env"), "OPENAI_API_KEY=secret\n").expect("测试应可写入本地环境文件");

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "reviewer".to_string(),
                tool_id: "code.outline".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeOutline {
                    path: ".env".to_string(),
                    limit: 10,
                },
            })
            .expect_err("本地环境文件结构提纲读取必须被拒绝");

        assert!(error.to_string().contains("不允许越过项目根目录"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_rejects_code_read_workspace_escape() {
        let root = temp_root("code-tool-path-escape");
        let app = bootstrap_app(&root);

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "code.read".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CodeRead {
                    path: "../secret.rs".to_string(),
                    max_bytes: 0,
                },
            })
            .expect_err("越界代码读取必须被拒绝");

        assert!(error.to_string().contains("不允许越过项目根目录"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_command_run_records_runtime_without_session() {
        let root = temp_root("command-run-tool");
        let app = bootstrap_app(&root);
        let program = std::env::current_exe()
            .expect("测试可执行文件路径应可读取")
            .to_string_lossy()
            .into_owned();

        let report = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "command.run".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CommandRun {
                    target_version: CURRENT_VERSION.to_string(),
                    program,
                    args: vec!["--help".to_string()],
                    timeout_ms: 5_000,
                },
            })
            .expect("command.run 工具应可无需会话执行命令");

        assert!(report.summary.contains("命令运行"));
        assert!(report.summary.contains("退出码 Some(0)"));
        let run = report.run.expect("command.run 应返回运行记录引用");
        assert_eq!(run.version, CURRENT_VERSION);
        assert_eq!(run.exit_code, Some(0));
        assert!(!run.timed_out);
        assert!(root.join(&run.report_file).is_file());
        let runs = app
            .supervisor()
            .list_runs(CURRENT_VERSION, 10)
            .expect("command.run 记录应可通过 Runtime 查询");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, run.run_id);

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_command_run_rejects_wrong_input() {
        let root = temp_root("command-run-wrong-input");
        let app = bootstrap_app(&root);

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "command.run".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::Empty,
            })
            .expect_err("command.run 必须拒绝错误输入类型");

        assert!(error.to_string().contains("调用输入不匹配"));
        assert!(error.to_string().contains("CommandRun"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_queries_command_history_with_filters() {
        let root = temp_root("command-history-tool");
        let app = bootstrap_app(&root);
        let program = std::env::current_exe()
            .expect("测试可执行文件路径应可读取")
            .to_string_lossy()
            .into_owned();

        for args in [
            vec!["--help".to_string()],
            vec!["--definitely-invalid-self-forge-test-flag".to_string()],
        ] {
            app.invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "command.run".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CommandRun {
                    target_version: CURRENT_VERSION.to_string(),
                    program: program.clone(),
                    args,
                    timeout_ms: 5_000,
                },
            })
            .expect("测试应可准备 Runtime 运行记录");
        }

        let all = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "builder".to_string(),
                tool_id: "command.history".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CommandHistory {
                    target_version: CURRENT_VERSION.to_string(),
                    limit: 10,
                    failed_only: false,
                    timed_out_only: false,
                },
            })
            .expect("command.history 应可读取运行历史");
        assert!(all.summary.contains("返回运行记录 2 条"));
        assert_eq!(all.details.len(), 2);
        assert!(all.details.iter().all(|detail| detail.contains("报告")));

        let failed = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "reviewer".to_string(),
                tool_id: "command.history".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CommandHistory {
                    target_version: CURRENT_VERSION.to_string(),
                    limit: 10,
                    failed_only: true,
                    timed_out_only: false,
                },
            })
            .expect("reviewer 应可只读查询失败运行历史");
        assert!(failed.summary.contains("返回运行记录 1 条"));
        assert_eq!(failed.details.len(), 1);
        assert!(failed.details[0].contains("invalid"));

        let timed_out = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "reviewer".to_string(),
                tool_id: "command.history".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::CommandHistory {
                    target_version: CURRENT_VERSION.to_string(),
                    limit: 10,
                    failed_only: false,
                    timed_out_only: true,
                },
            })
            .expect("command.history 应支持超时过滤");
        assert!(timed_out.summary.contains("返回运行记录 0 条"));
        assert!(timed_out.details.is_empty());

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }

    #[test]
    fn agent_tool_invocation_command_history_rejects_wrong_input() {
        let root = temp_root("command-history-wrong-input");
        let app = bootstrap_app(&root);

        let error = app
            .invoke_agent_tool(AgentToolInvocation {
                agent_id: "reviewer".to_string(),
                tool_id: "command.history".to_string(),
                version: CURRENT_VERSION.to_string(),
                input: AgentToolInvocationInput::Empty,
            })
            .expect_err("command.history 必须拒绝错误输入类型");

        assert!(error.to_string().contains("调用输入不匹配"));
        assert!(error.to_string().contains("CommandHistory"));

        fs::remove_dir_all(root).expect("测试目录应可清理");
    }
}

#[cfg(test)]
mod self_evolution_loop_record_tests {
    use super::*;

    #[test]
    fn self_evolution_step_record_accepts_old_json_without_ai_process_fields() {
        let json = r#"{
            "cycle": 1,
            "status": "Running",
            "started_at_unix_seconds": 10,
            "completed_at_unix_seconds": null,
            "stable_version_before": "v0.1.72",
            "stable_version_after": null,
            "audit_id": null,
            "summary_id": null,
            "error": null
        }"#;

        let record: SelfEvolutionLoopStepRecord =
            serde_json::from_str(json).expect("旧循环步骤记录应保持兼容");

        assert_eq!(record.phase_events, Vec::<String>::new());
        assert_eq!(record.patch_draft_id, None);
        assert_eq!(record.patch_source_cycle_id, None);
        assert_eq!(record.changed_files, Vec::<String>::new());
    }

    #[test]
    fn self_evolution_step_record_preserves_ai_process_fields() {
        let record = SelfEvolutionLoopStepRecord {
            cycle: 2,
            status: SelfEvolutionLoopStepStatus::Succeeded,
            started_at_unix_seconds: 10,
            completed_at_unix_seconds: Some(20),
            stable_version_before: "v0.1.72".to_string(),
            stable_version_after: Some("v0.1.73".to_string()),
            audit_id: None,
            summary_id: None,
            phase_events: vec!["完成补丁预览。".to_string()],
            patch_draft_id: Some("draft-1".to_string()),
            patch_audit_id: Some("audit-1".to_string()),
            patch_preview_id: Some("preview-1".to_string()),
            patch_application_id: Some("application-1".to_string()),
            patch_source_plan_id: Some("source-plan-1".to_string()),
            patch_source_execution_id: Some("source-execution-1".to_string()),
            patch_source_promotion_id: Some("source-promotion-1".to_string()),
            patch_source_candidate_id: Some("source-candidate-1".to_string()),
            patch_source_cycle_id: Some("source-cycle-1".to_string()),
            patch_source_summary_id: Some("source-summary-1".to_string()),
            changed_files: vec!["src/app/agent/example.rs".to_string()],
            error: None,
        };

        let json = serde_json::to_string(&record).expect("循环步骤记录应可序列化");
        let roundtrip: SelfEvolutionLoopStepRecord =
            serde_json::from_str(&json).expect("循环步骤记录应可反序列化");

        assert_eq!(roundtrip.phase_events, vec!["完成补丁预览。"]);
        assert_eq!(roundtrip.patch_draft_id.as_deref(), Some("draft-1"));
        assert_eq!(
            roundtrip.patch_source_execution_id.as_deref(),
            Some("source-execution-1")
        );
        assert_eq!(
            roundtrip.changed_files,
            vec!["src/app/agent/example.rs".to_string()]
        );
    }
}

#[cfg(test)]
mod tests;
