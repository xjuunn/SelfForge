# 归档与记忆规则

# 聚合策略

所有认知类数据写入 `forge/`，并按 major 聚合：

```txt
forge/memory/vMAJOR.md
forge/tasks/vMAJOR.md
forge/errors/vMAJOR.md
forge/versions/vMAJOR.md
```

patch 和 minor 只追加到当前 major 文件，禁止为小版本创建独立归档文件或目录。

# 记忆结构

每条记忆至少包含：

```md
# 版本信息
- 版本号：
- 时间：
- 父版本：

# 目标
# 计划
# 执行过程
# 代码变更
# 测试结果
# 错误总结
# 评估
# 优化建议
# 可复用经验
```

# 记忆读取

1. 新任务优先读取最近 3 到 5 条热记忆。
2. 使用 `memory-context --current --limit 5` 读取上下文。
3. 使用 `memory-insights --current --limit 5` 提取经验。
4. 默认禁止读取冷归档，除非审计、复盘或用户明确要求。

# 记忆压缩

1. 使用 `memory-compact --current --keep N` 压缩热记忆。
2. 热记忆保留近期完整小节和压缩索引。
3. 久远完整记忆迁移到 `forge/memory/archive/vMAJOR.md`。

# 错误归档

错误记录必须包含：

```md
# 错误信息
# 出现阶段
# 原因分析
# 解决方案
# 是否已解决
```

记录错误使用 `record-error`，解决错误使用 `resolve-error`，查询开放错误使用 `errors --current --open`。
