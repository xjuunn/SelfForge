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

pub const CURRENT_VERSION: &str = "v0.1.71";

pub use app::{
    AgentCapability, AgentDefinition, AgentError, AgentEvolutionError, AgentEvolutionReport,
    AgentPlan, AgentPlanReport, AgentPlanReportError, AgentPlanStep, AgentRegistry, AgentRunError,
    AgentRunReference, AgentRunReport, AgentSession, AgentSessionError, AgentSessionEvent,
    AgentSessionEventKind, AgentSessionMemoryInsight, AgentSessionPlanContext, AgentSessionStatus,
    AgentSessionStep, AgentSessionStore, AgentSessionSummary, AgentSessionWorkQueueContext,
    AgentSingleEvolutionReport, AgentSkillError, AgentSkillIndex, AgentSkillIndexReport,
    AgentSkillMetadata, AgentSkillSelection, AgentSkillSelectionReport, AgentSkillSelectionRequest,
    AgentStepExecutionError, AgentStepExecutionReport, AgentStepExecutionRequest,
    AgentStepRunError, AgentStepRunReport, AgentStepRunStop, AgentStepStatus, AgentToolAssignment,
    AgentToolBinding, AgentToolConfig, AgentToolConfigInitReport, AgentToolDefinition,
    AgentToolError, AgentToolInvocation, AgentToolInvocationError, AgentToolInvocationInput,
    AgentToolInvocationReport, AgentToolReport, AgentVerificationReport, AgentWorkClaimReport,
    AgentWorkCompactionReport, AgentWorkCoordinator, AgentWorkError, AgentWorkEvent,
    AgentWorkFinalizeCheckError, AgentWorkFinalizeCheckReport, AgentWorkQueue,
    AgentWorkQueueReport, AgentWorkReapReport, AgentWorkTask, AgentWorkTaskStatus, AiConfigError,
    AiConfigReport, AiExecutionError, AiExecutionReport, AiPatchApplicationError,
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
    format_agent_skill_context, normalize_ai_self_upgrade_goal,
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
mod tests;
