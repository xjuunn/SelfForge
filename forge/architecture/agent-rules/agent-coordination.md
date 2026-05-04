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
agent-work-status --active-only
agent-work-finalize-check
agent-work-claim --worker ID --agent AGENT_ID
agent-work-complete TASK_ID --worker ID --summary TEXT
agent-work-release TASK_ID --worker ID --reason TEXT
agent-work-reap
agent-work-compact --keep-events N
agent-work-block TASK_ID --reason TEXT
```

# 任务板重开

# 任务板状态过滤

1. 默认 `agent-work-status` 必须保持完整输出，便于审计所有终态和活跃任务。
2. 长队列查看活跃任务时使用 `agent-work-status --active-only`。
3. `--active-only` 只能过滤展示任务明细，任务总数和各状态统计必须保持基于完整队列。
4. 状态查询保持只读，禁止创建分支、领取任务或修改队列。

1. 新一轮相关任务组开始前，优先复用当前 major 的同一个 `work-queue.json`。
2. 当旧队列没有待领取和已领取任务，且任务状态只包含已完成或已阻断时，可以使用 `agent-work-init --reset-completed` 重开下一轮协作。
3. 存在待领取或已领取任务时禁止重开，必须先完成、释放、阻断或清理过期领取。
4. 重开必须保留历史事件并追加 `restart` 事件，便于审计上一轮目标和新目标。

# 任务组收束检查

1. 用户确认相关任务组完成前，禁止 push、创建 PR 或提升版本。
2. 准备最终版本提升前，必须运行 `agent-work-finalize-check`。
3. 检查必须只读，不得修改任务板、状态文件或归档。
4. 存在待领取、已领取任务或开放错误时，禁止收束。
5. 已完成和已阻断任务都视为终态；已阻断任务必须保留阻断原因。

# 任务板压缩

1. 任务板是热调度文件，不是永久归档文件。
2. 当 `work-queue.json` 过大或旧事件影响 AI 读取时，使用 `agent-work-compact` 压缩。
3. 压缩只能写回同一个 `work-queue.json`，禁止创建小版本队列文件或平行索引。
4. 压缩必须保留所有待领取和已领取任务，保留任务状态、写入范围、依赖、验收标准和完成摘要。
5. 已完成任务的长提示词可压缩为摘要，旧事件可折叠为最近事件加 `compact` 事件。
6. 压缩前应确认关键结果已写入任务摘要或 forge 归档，避免把唯一证据只留在旧事件里。

# 任务阻断

1. 过期、重复或不再适用的待领取任务不得直接删除。
2. 使用 `agent-work-block TASK_ID --reason TEXT` 标记为已阻断，并写明中文原因。
3. 已阻断任务必须保留任务编号、写入范围、验收标准和阻断原因，用于审计。
4. 已阻断任务不得再被领取；若后续需要继续，应新建明确的新任务。
5. 已完成任务禁止改为阻断状态，避免破坏完成记录。

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

# 自我进化循环

1. 一键自我进化使用 `agent-self-loop`，默认必须有最大轮次和最大连续失败限制。
2. 循环记录写入 `workspaces/vMAJOR/artifacts/agents/self-evolution-loops/`，禁止只保存在进程内存。
3. 循环必须复用 `agent-self-upgrade`、`preflight`、记忆经验、错误归档、Runtime 和版本状态机。
4. 进程崩溃后使用 `agent-self-loop --resume` 恢复最近未完成循环；恢复时必须把上次运行中的步骤标记为失败。
5. 禁止声称绝对不会崩溃；工程目标是失败可记录、崩溃可恢复、循环可停止。
