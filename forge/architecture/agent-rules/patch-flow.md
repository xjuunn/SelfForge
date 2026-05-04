# AI 补丁链路规则

AI 补丁链路必须分阶段执行，每个阶段只做自己的事，禁止一步跨过审计、预演、验证或回滚边界。

# 草案

1. 生成草案使用 `agent-patch-draft [--dry-run] [goal]`。
2. 草案只能写入 `workspaces/vMAJOR/artifacts/agents/patch-drafts/`。
3. 草案必须包含计划和测试章节。
4. 草案禁止直接修改源码、`runtime`、`supervisor` 或状态文件。
5. 从任务审计继续时使用 `--from-task-audit TASK_AUDIT_ID`，禁止同时提供直接目标文本。

# 审计与预演

1. 审计使用 `agent-patch-audit DRAFT_RECORD_ID`。
2. 审计必须解析允许写入范围，检查非法路径、绝对路径、受保护目录和任务板冲突。
3. 预演使用 `agent-patch-preview AUDIT_RECORD_ID`。
4. 预演只生成报告和结构化记录，禁止直接修改源码。
5. 审计未通过、缺少代码块或路径非法时必须写入阻断记录。

# 候选应用与验证

1. 候选应用使用 `agent-patch-apply PREVIEW_RECORD_ID`。
2. 候选应用只能写入 `workspaces/vMAJOR/source/patch-applications/APPLICATION_ID/` 镜像目录。
3. 验证使用 `agent-patch-verify APPLICATION_RECORD_ID`。
4. 验证命令白名单：`cargo fmt --check`、`cargo test`、`cargo run -- validate`、`cargo run -- preflight`。
5. 任一命令失败或超时，禁止进入源码覆盖。

# 源码覆盖

1. 准备覆盖使用 `agent-patch-source-plan APPLICATION_RECORD_ID`。
2. 执行覆盖使用 `agent-patch-source-execute SOURCE_PLAN_ID`。
3. 执行前必须校验目标路径、候选镜像、字节数和回滚备份。
4. 验证失败或写入失败必须按备份回滚。
5. 覆盖成功后使用 `agent-patch-source-promotion SOURCE_EXECUTION_ID` 生成衔接记录。

# 候选提升

1. 候选准备使用 `agent-patch-source-candidate PROMOTION_ID`。
2. 候选验证与提升回滚使用 `agent-patch-source-cycle CANDIDATE_RECORD_ID`。
3. 后续总结使用 `agent-patch-source-cycle-summary CYCLE_RECORD_ID`。
4. 下一任务草案使用 `agent-patch-source-task-draft SUMMARY_RECORD_ID`。
5. 下一任务审计使用 `agent-patch-source-task-audit TASK_DRAFT_ID`。
