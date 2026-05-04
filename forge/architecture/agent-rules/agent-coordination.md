# 多 Agent 与协作规则

# 协作任务板

任务板是多 AI 协作唯一事实源：

```txt
workspaces/vMAJOR/artifacts/agents/coordination/work-queue.json
```

常用命令：

```txt
agent-work-init
agent-work-status
agent-work-claim --worker ID --agent AGENT_ID
agent-work-complete TASK_ID --worker ID --summary TEXT
agent-work-release TASK_ID --worker ID --reason TEXT
agent-work-reap
```

# 领取边界

1. 修改文件前必须先创建任务分支，再领取任务。
2. 只处理已领取任务的写入范围和验收标准。
3. 不得主动实现其他未领取任务。
4. 发现写入范围冲突、依赖缺失或职责不清时必须释放任务并写明原因。
5. 任务领取必须带租约；过期任务由 `agent-work-reap` 释放。

# Agent 目录与计划

1. 多 Agent 能力通过 `src/app/agent/` 扩展。
2. 新 Agent 应表现为 `AgentDefinition`、能力集合和计划步骤。
3. 查询 Agent 使用 `agents`。
4. 生成计划使用 `agent-plan [goal]`。
5. `agent-plan` 必须通过应用层读取 `memory-insights`。

# Agent Tool

1. 工具定义使用结构化 `AgentToolDefinition`。
2. 工具配置只写入 `workspaces/vMAJOR/artifacts/agents/tool-config.json`。
3. 调用工具使用 `agent-tool-run TOOL_ID --agent AGENT_ID`。
4. 未绑定工具必须拒绝调用。
5. Runtime 类工具必须复用 Runtime 运行记录。

# Agent 会话

1. 创建会话使用 `agent-start [goal]`。
2. 会话文件写入 `workspaces/vMAJOR/artifacts/agents/sessions/`。
3. 会话摘要追加到 `workspaces/vMAJOR/artifacts/agents/index.jsonl`。
4. 会话必须保存计划上下文和事件时间线。
5. 步进执行使用 `agent-step SESSION_ID`，多步执行使用 `agent-steps SESSION_ID`。
6. 真实程序验证优先使用 `agent-verify` 或 `agent-run`。
