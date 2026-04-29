# 错误信息

执行真实 `evolve` 后发现 `forge/tasks/v0.1.1.md` 被候选生成流程覆盖。

# 出现阶段

真实候选版本生成与审计记录阶段。

# 原因分析

`write_candidate_documents` 无条件写入候选 memory、tasks、errors、versions 文档。若任务文档已按流程提前写入，自动生成会覆盖已有计划，破坏“计划 → 实现 → 测试 → 验证 → 记录”的审计链。

# 解决方案

将候选文档写入改为 `write_document_if_missing`，已存在则保留；新增 `evolution_preserves_existing_candidate_task_document` 测试验证任务文档不会被覆盖。

# 是否已解决

已解决。`cargo test` 已通过。
