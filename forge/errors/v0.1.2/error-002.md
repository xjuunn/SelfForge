# 错误信息

执行 `cargo test` 时，`evolution_preserves_existing_candidate_task_document` 测试失败。中文文档审计拒绝了测试中写入的英文任务文档 `manual task plan`。

# 出现阶段

测试与验证阶段。

# 原因分析

新增文档审计规则要求所有 Markdown 文档包含中文内容，而测试夹具仍使用英文文本。测试意图是验证“已有候选任务文档不被覆盖”，但夹具内容不符合真实文档规范。

# 解决方案

将测试夹具改为中文 `人工任务计划`，保持测试意图不变，并重新执行 `cargo test`。

# 是否已解决

已解决。后续 `cargo test` 已通过，10 个测试全部通过。
