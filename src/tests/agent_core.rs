use super::*;

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
    assert!(
        report
            .queue
            .events
            .iter()
            .any(|event| { event.action == "retarget" && event.message.contains(CURRENT_VERSION) })
    );

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
fn agent_work_compact_trims_completed_prompts_and_old_events() {
    let root = temp_root("agent-work-compact");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before compact test");
    let init = app
        .init_agent_work_queue(CURRENT_VERSION, "验证任务板压缩", 1)
        .expect("work queue should initialize");
    let claim = app
        .claim_agent_work(CURRENT_VERSION, "ai-1", Some("architect"))
        .expect("task should be claimed before completion");
    app.complete_agent_work(CURRENT_VERSION, &claim.task.id, "ai-1", "架构任务完成")
        .expect("claimed task should complete");

    let mut queue: AgentWorkQueue =
        serde_json::from_str(&fs::read_to_string(&init.queue_path).expect("queue readable"))
            .expect("queue json should parse");
    let pending_prompt = queue
        .tasks
        .iter()
        .find(|task| task.id == "coord-002-application")
        .expect("pending task should exist")
        .prompt
        .clone();
    for index in 0..25 {
        queue.events.push(AgentWorkEvent {
            order: queue.events.len() + 1,
            timestamp_unix_seconds: index,
            action: "测试事件".to_string(),
            worker_id: None,
            task_id: None,
            message: format!("压缩前事件 {index}"),
        });
    }
    fs::write(
        &init.queue_path,
        serde_json::to_string_pretty(&queue).expect("queue should serialize"),
    )
    .expect("test should write verbose queue");

    let report = app
        .compact_agent_work_queue(CURRENT_VERSION, Some(5))
        .expect("work queue compaction should succeed");

    assert_eq!(report.compacted_task_prompts, 1);
    assert!(report.removed_events > 0);
    assert_eq!(report.retained_events, 6);
    let completed = report
        .queue
        .tasks
        .iter()
        .find(|task| task.id == claim.task.id)
        .expect("completed task should remain");
    assert_eq!(completed.status, AgentWorkTaskStatus::Completed);
    assert!(completed.prompt.starts_with("已压缩："));
    let pending = report
        .queue
        .tasks
        .iter()
        .find(|task| task.id == "coord-002-application")
        .expect("pending task should remain");
    assert_eq!(pending.status, AgentWorkTaskStatus::Pending);
    assert_eq!(pending.prompt, pending_prompt);
    assert_eq!(
        report
            .queue
            .events
            .last()
            .map(|event| event.action.as_str()),
        Some("compact")
    );

    cleanup(&root);
}

#[test]
fn agent_work_compact_rejects_zero_keep_events() {
    let root = temp_root("agent-work-compact-zero");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before compact zero test");
    app.init_agent_work_queue(CURRENT_VERSION, "验证非法压缩参数", 1)
        .expect("work queue should initialize");

    let error = app
        .compact_agent_work_queue(CURRENT_VERSION, Some(0))
        .expect_err("zero retained events must be rejected");

    assert!(matches!(error, AgentWorkError::InvalidKeepEvents));

    cleanup(&root);
}

#[test]
fn agent_work_block_marks_pending_task_unclaimable() {
    let root = temp_root("agent-work-block");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before block test");
    app.init_agent_work_queue(CURRENT_VERSION, "验证任务阻断", 2)
        .expect("work queue should initialize");

    let report = app
        .block_agent_work(CURRENT_VERSION, "coord-002-application", "旧任务不再适用")
        .expect("pending task should be blockable");
    let blocked = report
        .queue
        .tasks
        .iter()
        .find(|task| task.id == "coord-002-application")
        .expect("blocked task should remain");
    assert_eq!(blocked.status, AgentWorkTaskStatus::Blocked);
    assert_eq!(blocked.result.as_deref(), Some("旧任务不再适用"));
    assert!(blocked.prompt.starts_with("已阻断："));
    assert_eq!(
        report
            .queue
            .events
            .last()
            .map(|event| event.action.as_str()),
        Some("block")
    );

    let claim = app
        .claim_agent_work(CURRENT_VERSION, "ai-1", Some("builder"))
        .expect("builder should claim next non-blocked task");
    assert_eq!(claim.task.id, "coord-003-cli");

    cleanup(&root);
}

#[test]
fn agent_work_block_rejects_completed_task() {
    let root = temp_root("agent-work-block-completed");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before completed block test");
    app.init_agent_work_queue(CURRENT_VERSION, "验证已完成任务不可阻断", 1)
        .expect("work queue should initialize");
    let claim = app
        .claim_agent_work(CURRENT_VERSION, "ai-1", Some("architect"))
        .expect("task should be claimed");
    app.complete_agent_work(CURRENT_VERSION, &claim.task.id, "ai-1", "任务完成")
        .expect("task should complete");

    let error = app
        .block_agent_work(CURRENT_VERSION, &claim.task.id, "不应阻断已完成任务")
        .expect_err("completed task must not be blocked");

    assert!(matches!(error, AgentWorkError::TaskAlreadyCompleted { .. }));

    cleanup(&root);
}

#[test]
fn agent_work_reset_completed_allows_blocked_terminal_tasks() {
    let root = temp_root("agent-work-reset-terminal");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before terminal reset test");
    let init = app
        .init_agent_work_queue(CURRENT_VERSION, "旧协作目标", 2)
        .expect("work queue should initialize");
    let mut queue: AgentWorkQueue =
        serde_json::from_str(&fs::read_to_string(&init.queue_path).expect("queue readable"))
            .expect("queue json should parse");
    for (index, task) in queue.tasks.iter_mut().enumerate() {
        if index == 0 {
            task.status = AgentWorkTaskStatus::Blocked;
            task.result = Some("旧任务不再适用".to_string());
            task.claimed_by = None;
            task.claimed_at_unix_seconds = None;
            task.lease_expires_at_unix_seconds = None;
            task.completed_at_unix_seconds = None;
        } else {
            task.status = AgentWorkTaskStatus::Completed;
            task.result = Some("已完成".to_string());
            task.claimed_by = Some("ai-1".to_string());
            task.claimed_at_unix_seconds = Some(1);
            task.lease_expires_at_unix_seconds = None;
            task.completed_at_unix_seconds = Some(2);
        }
    }
    fs::write(
        &init.queue_path,
        serde_json::to_string_pretty(&queue).expect("queue should serialize"),
    )
    .expect("test should write terminal queue");

    let report = app
        .init_agent_work_queue_with_reset_completed(CURRENT_VERSION, "下一轮协作目标", 4, true)
        .expect("terminal completed and blocked queue should restart");

    assert!(report.created);
    assert_eq!(report.queue.goal, "下一轮协作目标");
    assert_eq!(report.queue.thread_count, 4);
    assert!(
        report
            .queue
            .tasks
            .iter()
            .all(|task| task.status == AgentWorkTaskStatus::Pending)
    );
    assert!(
        report
            .queue
            .events
            .iter()
            .any(|event| event.action == "init")
    );
    assert_eq!(
        report
            .queue
            .events
            .last()
            .map(|event| event.action.as_str()),
        Some("restart")
    );

    cleanup(&root);
}

#[test]
fn self_evolution_loop_records_failed_cycle_without_crashing() {
    let root = temp_root("self-loop-failed-cycle");
    let app = SelfForgeApp::new(&root);
    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before self loop");

    let report = app
        .run_self_evolution_loop_with_executor(
            SelfEvolutionLoopRequest {
                hint: "保持小步进化".to_string(),
                max_cycles: 2,
                max_failures: 1,
                timeout_ms: 1000,
                resume: false,
                git_pr: SelfEvolutionLoopGitPrRequest::default(),
            },
            |_app, _hint, _timeout_ms| {
                Err(AiSelfUpgradeError::EmptyGoal {
                    response_preview: "空响应".to_string(),
                })
            },
        )
        .expect("self loop should record controlled failures instead of crashing");

    assert_eq!(report.record.status, SelfEvolutionLoopStatus::Stopped);
    assert_eq!(report.record.completed_cycles, 0);
    assert_eq!(report.record.failed_cycles, 1);
    assert_eq!(report.record.consecutive_failures, 1);
    assert_eq!(report.record.steps.len(), 1);
    assert_eq!(
        report.record.steps[0].status,
        SelfEvolutionLoopStepStatus::Failed
    );
    assert!(report.record.file.exists());
    assert!(report.index_file.exists());

    cleanup(&root);
}

#[test]
fn self_evolution_loop_pr_finalize_requires_explicit_confirmation() {
    let root = temp_root("self-loop-pr-confirmation");
    let app = SelfForgeApp::new(&root);
    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before self loop confirmation test");

    let error = app
        .run_self_evolution_loop_with_executor(
            SelfEvolutionLoopRequest {
                hint: "尝试自主 PR 收束".to_string(),
                max_cycles: 1,
                max_failures: 1,
                timeout_ms: 1000,
                resume: false,
                git_pr: SelfEvolutionLoopGitPrRequest {
                    mode: SelfEvolutionLoopGitPrMode::PullRequest,
                    confirmed: false,
                    ..SelfEvolutionLoopGitPrRequest::default()
                },
            },
            |_app, _hint, _timeout_ms| {
                panic!("未确认 PR 收束时不应执行自我升级");
            },
        )
        .expect_err("PR finalize must require explicit confirmation");

    assert!(matches!(
        error,
        SelfEvolutionLoopError::InvalidRequest(ref message)
            if message.contains("--confirm-finalize")
    ));

    cleanup(&root);
}

#[test]
fn self_evolution_loop_records_can_be_listed_and_loaded() {
    let root = temp_root("self-loop-record-query");
    let app = SelfForgeApp::new(&root);
    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before self loop query test");

    let report = app
        .run_self_evolution_loop_with_executor(
            SelfEvolutionLoopRequest {
                hint: "生成可查询记录".to_string(),
                max_cycles: 1,
                max_failures: 1,
                timeout_ms: 1000,
                resume: false,
                git_pr: SelfEvolutionLoopGitPrRequest::default(),
            },
            |_app, _hint, _timeout_ms| {
                Err(AiSelfUpgradeError::EmptyGoal {
                    response_preview: "空响应".to_string(),
                })
            },
        )
        .expect("failed self loop should still persist a record");

    let records = app
        .self_evolution_loop_records(CURRENT_VERSION, 10)
        .expect("self loop records should be listed");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, report.record.id);
    assert_eq!(records[0].status, SelfEvolutionLoopStatus::Stopped);
    assert_eq!(records[0].git_pr_mode, SelfEvolutionLoopGitPrMode::Disabled);

    let loaded = app
        .self_evolution_loop_record(CURRENT_VERSION, &report.record.id)
        .expect("self loop record should load by id");
    assert_eq!(loaded.id, report.record.id);
    assert_eq!(loaded.steps.len(), 1);

    cleanup(&root);
}

#[test]
fn self_evolution_loop_resume_marks_interrupted_step_failed() {
    let root = temp_root("self-loop-resume-interrupted");
    let app = SelfForgeApp::new(&root);
    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before self loop resume test");
    let records_dir = root
        .join("workspaces")
        .join("v0")
        .join("artifacts")
        .join("agents")
        .join("self-evolution-loops");
    fs::create_dir_all(&records_dir).expect("test should create self loop records dir");
    let record_file = records_dir.join("self-loop-interrupted.json");
    let interrupted = SelfEvolutionLoopRecord {
        id: "self-loop-interrupted".to_string(),
        version: CURRENT_VERSION.to_string(),
        status: SelfEvolutionLoopStatus::Running,
        created_at_unix_seconds: 1,
        updated_at_unix_seconds: 2,
        hint: "恢复中断循环".to_string(),
        max_cycles: 2,
        max_failures: 2,
        timeout_ms: 1000,
        completed_cycles: 0,
        failed_cycles: 0,
        consecutive_failures: 0,
        resumed: false,
        git_pr: SelfEvolutionLoopGitPrRequest::default(),
        git_pr_events: Vec::new(),
        pr_url: None,
        last_error: None,
        steps: vec![SelfEvolutionLoopStepRecord {
            cycle: 1,
            status: SelfEvolutionLoopStepStatus::Running,
            started_at_unix_seconds: 1,
            completed_at_unix_seconds: None,
            stable_version_before: CURRENT_VERSION.to_string(),
            stable_version_after: None,
            audit_id: None,
            summary_id: None,
            error: None,
        }],
        file: record_file.clone(),
    };
    fs::write(
        &record_file,
        serde_json::to_string_pretty(&interrupted).expect("record should serialize"),
    )
    .expect("test should write interrupted record");

    let report = app
        .run_self_evolution_loop_with_executor(
            SelfEvolutionLoopRequest {
                hint: "新的提示会被已有记录覆盖".to_string(),
                max_cycles: 1,
                max_failures: 1,
                timeout_ms: 1000,
                resume: true,
                git_pr: SelfEvolutionLoopGitPrRequest::default(),
            },
            |_app, _hint, _timeout_ms| {
                Err(AiSelfUpgradeError::EmptyGoal {
                    response_preview: "恢复后仍失败".to_string(),
                })
            },
        )
        .expect("resume should persist interrupted step as a failure");

    assert!(report.resumed);
    assert_eq!(report.record.id, "self-loop-interrupted");
    assert_eq!(
        report.record.steps[0].status,
        SelfEvolutionLoopStepStatus::Failed
    );
    assert!(
        report.record.steps[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("中断")
    );
    assert_eq!(report.record.failed_cycles, 2);
    assert_eq!(report.record.status, SelfEvolutionLoopStatus::Stopped);

    cleanup(&root);
}

#[test]
fn agent_work_reset_completed_rejects_active_tasks() {
    let root = temp_root("agent-work-reset-active");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before active reset test");
    app.init_agent_work_queue(CURRENT_VERSION, "仍有活跃任务", 2)
        .expect("work queue should initialize");
    app.claim_agent_work(CURRENT_VERSION, "ai-1", Some("builder"))
        .expect("task should be claimed before reset");

    let error = app
        .init_agent_work_queue_with_reset_completed(CURRENT_VERSION, "不应重开", 3, true)
        .expect_err("active claimed queue must not restart");

    assert!(matches!(error, AgentWorkError::QueueNotCompleted { .. }));

    cleanup(&root);
}

#[test]
fn agent_work_finalize_check_allows_terminal_queue_without_open_errors() {
    let root = temp_root("agent-work-finalize-ready");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before finalize check test");
    let init = app
        .init_agent_work_queue(CURRENT_VERSION, "准备收束任务组", 2)
        .expect("work queue should initialize");
    let mut queue: AgentWorkQueue =
        serde_json::from_str(&fs::read_to_string(&init.queue_path).expect("queue readable"))
            .expect("queue json should parse");
    for (index, task) in queue.tasks.iter_mut().enumerate() {
        task.status = if index == 0 {
            AgentWorkTaskStatus::Blocked
        } else {
            AgentWorkTaskStatus::Completed
        };
        task.result = Some("终态任务".to_string());
        task.claimed_by = if index == 0 {
            None
        } else {
            Some("ai-1".to_string())
        };
        task.claimed_at_unix_seconds = None;
        task.lease_expires_at_unix_seconds = None;
        task.completed_at_unix_seconds = Some(2);
    }
    fs::write(
        &init.queue_path,
        serde_json::to_string_pretty(&queue).expect("queue should serialize"),
    )
    .expect("test should write terminal queue");

    let report = app
        .agent_work_finalize_check(CURRENT_VERSION)
        .expect("terminal queue should be checked");

    assert!(report.can_finalize);
    assert_eq!(report.pending_count, 0);
    assert_eq!(report.claimed_count, 0);
    assert_eq!(report.blocked_count, 1);
    assert_eq!(report.completed_count, report.task_count - 1);
    assert!(report.open_errors.is_empty());
    assert!(report.blockers.is_empty());

    cleanup(&root);
}

#[test]
fn agent_work_finalize_check_blocks_pending_and_claimed_tasks() {
    let root = temp_root("agent-work-finalize-active");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before finalize blocker test");
    app.init_agent_work_queue(CURRENT_VERSION, "仍有活跃任务", 2)
        .expect("work queue should initialize");

    let pending_report = app
        .agent_work_finalize_check(CURRENT_VERSION)
        .expect("pending queue should be checked");
    assert!(!pending_report.can_finalize);
    assert!(pending_report.pending_count > 0);
    assert!(
        pending_report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("待领取"))
    );

    app.claim_agent_work(CURRENT_VERSION, "ai-1", Some("builder"))
        .expect("task should be claimed before finalize check");
    let claimed_report = app
        .agent_work_finalize_check(CURRENT_VERSION)
        .expect("claimed queue should be checked");
    assert!(!claimed_report.can_finalize);
    assert_eq!(claimed_report.claimed_count, 1);
    assert!(
        claimed_report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("已领取"))
    );

    cleanup(&root);
}

#[test]
fn agent_work_finalize_check_blocks_open_errors() {
    let root = temp_root("agent-work-finalize-open-errors");
    let app = SelfForgeApp::new(&root);

    app.supervisor()
        .initialize_current_version()
        .expect("bootstrap should succeed before finalize open error test");
    let init = app
        .init_agent_work_queue(CURRENT_VERSION, "错误阻断收束", 1)
        .expect("work queue should initialize");
    let mut queue: AgentWorkQueue =
        serde_json::from_str(&fs::read_to_string(&init.queue_path).expect("queue readable"))
            .expect("queue json should parse");
    for task in &mut queue.tasks {
        task.status = AgentWorkTaskStatus::Completed;
        task.result = Some("已完成".to_string());
        task.claimed_by = Some("ai-1".to_string());
        task.claimed_at_unix_seconds = None;
        task.lease_expires_at_unix_seconds = None;
        task.completed_at_unix_seconds = Some(2);
    }
    fs::write(
        &init.queue_path,
        serde_json::to_string_pretty(&queue).expect("queue should serialize"),
    )
    .expect("test should write completed queue");
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
        .expect("failed run should be archived before finalize check");

    let report = app
        .agent_work_finalize_check(CURRENT_VERSION)
        .expect("open errors should be checked");

    assert!(!report.can_finalize);
    assert_eq!(report.pending_count, 0);
    assert_eq!(report.claimed_count, 0);
    assert_eq!(report.open_errors.len(), 1);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("开放错误"))
    );

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
        event.kind == AgentSessionEventKind::WorkQueuePrepared && event.message.contains("已复用")
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
    assert!(
        report.session.events.iter().any(|event| {
            event.kind == AgentSessionEventKind::RuntimeRun && event.run.is_some()
        })
    );

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
        Some("v0.1.72")
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
    assert_eq!(state.candidate_version.as_deref(), Some("v0.1.72"));
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
    assert_eq!(report.minimal_loop.stable_version, "v0.1.72");
    assert_eq!(
        report.minimal_loop.candidate_version.as_deref(),
        Some("v0.1.73")
    );
    assert_eq!(report.session.status, AgentSessionStatus::Completed);

    let state = ForgeState::load(&root).expect("state should remain readable");
    assert_eq!(state.current_version, "v0.1.72");
    assert_eq!(state.candidate_version.as_deref(), Some("v0.1.73"));

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
        Some("v0.1.72")
    );
    assert_eq!(report.cycle.previous_version, CURRENT_VERSION);
    assert_eq!(report.cycle.candidate_version, "v0.1.72");
    assert_eq!(report.cycle.result, CycleResult::Promoted);
    assert_eq!(report.cycle.state.current_version, "v0.1.72");
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
    assert_eq!(state.current_version, "v0.1.72");
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

    assert_eq!(report.cycle.state.current_version, "v0.1.72");

    let promoted_version_only = app
        .agent_sessions("v0.1.72", 10)
        .expect("promoted version scoped session list should be readable");
    assert!(
        promoted_version_only.is_empty(),
        "the session belongs to the version that started the evolution"
    );

    let all = app
        .agent_sessions_all("v0.1.72", 10)
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
    assert_eq!(report.cycle.candidate_version, "v0.1.72");
    assert_eq!(report.cycle.result, CycleResult::Promoted);
    assert_eq!(report.cycle.state.current_version, "v0.1.72");
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
