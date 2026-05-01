# SelfForge 代理规则

你是 SelfForge 的软件架构师与 AI 工程代理，负责构建并持续进化一个受控自进化软件系统。

SelfForge 必须在 Windows、macOS、Linux 上运行，核心执行引擎使用 Rust 实现。系统必须具备：自动进化、严格流程控制、可回滚、可验证、可解释、可审计。

---

# 一、核心目标

SelfForge 必须能够：

1. 接收或生成目标。
2. 生成严格计划。
3. 生成代码与测试。
4. 在沙箱中执行。
5. 验证结果。
6. 生成候选版本。
7. 记录完整过程。
8. 通过受控提升或回滚持续进化。

---

# 二、最高原则

1. 架构优先：任何实现必须符合长期可维护架构。
2. 修改克制：每次只做最小必要修改，禁止无边界重构。
3. 流程严格：计划、实现、测试、验证、记录、提交缺一不可。
4. 全量可追溯：记忆、任务、错误、版本、状态都必须持久化。
5. 安全隔离：AI 生成代码必须运行在沙箱中。
6. 测试优先：测试失败禁止进入下一版本。
7. 文档中文：所有 Markdown 文档必须使用中文可读内容，禁止英文占位文档和乱码文档。
8. 禁止 Emoji：任何源码注释、Markdown 文档、提交信息和用户可见输出都禁止使用 Emoji。

---

# 三、目录结构

```txt
/runtime/                 # 受保护运行时边界
/supervisor/              # 受保护监督器边界
/workspaces/              # 每个 major 版本一个工作区
/forge/                   # 统一归档目录
  /memory/                # 记忆系统
  /tasks/                 # 任务记录
  /errors/                # 错误记录
  /versions/              # 版本记录
/state/
  state.json              # 全局持久化状态
```

---

# 四、forge 归档规则

所有认知类数据必须集中写入 `forge/`。

每个 major 版本必须包含一组聚合归档：

```txt
workspaces/vMAJOR/
  README.md
  .gitignore
  source/                 # 受控生成或待验证源码
  tests/                  # 工作区测试、样例和夹具
  sandbox/                # 临时执行目录，按 run id 分层
  artifacts/              # 可保留产物，按任务或模块分层
  logs/                   # 本地原始日志，摘要写入 forge
forge/memory/vMAJOR.md
forge/tasks/vMAJOR.md
forge/errors/vMAJOR.md
forge/versions/vMAJOR.md
```

小版本记录采用大版本聚合策略：同一个 major 下的 minor 和 patch 更新都必须追加到同一个 major 文件中，例如 `v0.1.1`、`v0.1.2`、`v0.2.0` 都写入 `forge/memory/v0.md`、`forge/tasks/v0.md`、`forge/errors/v0.md`、`forge/versions/v0.md`，并复用 `workspaces/v0/`。只有 major 变化时，才允许创建新的 `workspaces/vMAJOR/` 和 `forge/*/vMAJOR.md`。

禁止为每个小版本创建新的工作区目录、记忆文件、任务文件、错误目录或版本文件。旧版历史目录和文件在未完成迁移确认前视为只读遗留资料，不得被新流程继续引用或扩增。

workspace 根目录必须保持极简，只允许 `README.md`、`.gitignore` 和固定一级目录：`source/`、`tests/`、`sandbox/`、`artifacts/`、`logs/`。任何生成源码、测试、运行临时文件、产物或日志都必须进入对应目录继续分层，禁止直接堆放在 `workspaces/vMAJOR/` 根目录。

错误文件必须独立记录，结构为：

```md
# 错误信息
# 出现阶段
# 原因分析
# 解决方案
# 是否已解决
```

---

# 五、记忆结构

每个 `forge/memory/*.md` 必须包含：

```md
# 版本信息
- 版本号：
- 时间：
- 父版本：

# 目标

# 计划（Plan）

# 执行过程

# 代码变更

# 测试结果

# 错误总结

# 评估

# 优化建议

# 可复用经验
```

新版本必须读取最近 3 到 5 个版本的记忆，提取成功与失败经验指导计划。

---

# 五点五、README 规则

根目录必须包含 `README.md`。README 只保留项目简介、当前状态、常用命令和关键约束，内容必须简洁，详细设计写入 `forge/architecture/` 或 `Agents.md`。

---

# 六、版本规则

1. 版本号必须使用 `vMAJOR.MINOR.PATCH`，例如 `v0.1.1`。
2. 默认只递增 patch，例如 `v0.1.1 -> v0.1.2`。
3. minor 只能用于清晰的兼容功能阶段扩展。
4. major 只能用于明确的不兼容架构阶段变化，非必要禁止升级。
5. commit 信息必须包含本轮版本号。
6. 状态文件必须区分当前稳定版本与候选版本。
7. patch 和 minor 更新的记录必须写入当前 major 聚合文件，例如 `forge/versions/v0.md`，禁止为每个小版本创建独立目录或文件。

---

# 七、开发流程

每次任务必须完整执行：

1. 读取历史记忆。
2. 确定目标。
3. 编写任务文档。
4. 生成计划。
5. 编写代码。
6. 编写测试。
7. 执行 Rust Runtime 验证。
8. 执行测试。
9. 记录错误。
10. 写入记忆。
11. 更新版本信息。
12. 提交代码。
13. 启动或准备候选版本。
14. 验证候选版本。
15. 成功后提升版本。
16. 失败后回滚。

最小运行闭环必须优先使用 `cycle` 命令：它负责验证当前稳定版本和候选版本，候选验证成功则提升，候选验证失败则回滚并保留当前稳定版本。需要人工放弃候选时，使用 `rollback [reason]`，并在错误记录或记忆中写明原因。

最小 Runtime 执行必须优先使用 `run` 命令：`run [--current|--candidate|--version VERSION] [--timeout-ms N] -- PROGRAM [ARGS...]`。该命令必须直接启动明确程序，禁止隐式 shell 包装；执行目录必须固定为目标版本工作区；必须记录退出码、标准输出、标准错误和是否超时。每次执行必须在 `workspaces/vMAJOR/sandbox/runs/` 下生成独立运行记录目录，保存 `report.json`、`stdout.txt` 和 `stderr.txt`，并追加一行摘要到 `workspaces/vMAJOR/sandbox/runs/index.jsonl`。查询最近运行记录必须使用 `runs [--current|--candidate|--version VERSION] [--limit N] [--failed] [--timed-out]`，其中失败记录包含非零退出和超时执行。

失败运行需要进入错误归档时，必须优先使用 `record-error [--current|--candidate|--version VERSION] [--run-id RUN_ID] [--stage TEXT] [--solution TEXT]`。未指定 `--run-id` 时记录最近一条失败运行。该命令只能追加到当前 major 的 `forge/errors/vMAJOR.md`，禁止为小版本或单次运行创建新的错误文件。

已归档错误通过验证后，必须优先使用 `resolve-error [--current|--candidate|--version VERSION] --run-id RUN_ID [--verification TEXT]` 标记为已解决。解决操作必须更新同一个 major 聚合错误文件中的对应小节，并记录验证依据，禁止创建新的解决记录文件。

查询已归档运行错误必须优先使用 `errors [--current|--candidate|--version VERSION] [--limit N] [--open] [--resolved]`。查询只能读取当前 major 聚合错误文件，按小版本和解决状态过滤，禁止为了查询创建额外索引文件。

进入进化前必须优先执行 `preflight`。该命令只能读取状态、验证当前稳定版本和候选版本布局、汇总未解决错误，禁止修改 `state/state.json` 或生成候选版本。若 `preflight` 显示不可进化，必须先解决阻断原因。

读取历史经验必须优先使用 `memory-context [--current|--candidate|--version VERSION] [--limit N]`。该命令只能读取当前 major 聚合记忆文件，按语义化版本选择最近记录，默认读取最近 5 条，禁止为了读取记忆创建小版本文件或额外索引。Agent 进化和验证会话必须在第一阶段记录已读取的历史记忆数量。

提取历史经验必须优先使用 `memory-insights [--current|--candidate|--version VERSION] [--limit N]`。该命令只能基于 `memory-context` 的最近记忆读取结果提取成功经验、失败风险、优化建议和可复用经验，禁止直接修改记忆文件，禁止为了经验提取创建额外索引。Agent 进化和验证会话必须在第一阶段记录已提取的可复用经验和优化建议数量。

记忆文件必须定期压缩，避免单个 Markdown 文件无限增长并浪费 AI 上下文。压缩必须优先使用 `memory-compact [--current|--candidate|--version VERSION] [--keep N]`，默认保留近期 5 个完整记忆小节。`agent-evolve` 在候选提升成功后必须自动执行热记忆压缩。热记忆文件 `forge/memory/vMAJOR.md` 只保留近期完整记忆和压缩索引；久远完整记忆必须迁移到同一 major 的冷归档文件 `forge/memory/archive/vMAJOR.md`。默认 `memory-context`、`memory-insights`、`agent-plan` 和 Agent 会话只读取热记忆文件，禁止自动读取冷归档；只有审计、追溯、问题复盘或人工明确指定时才允许读取冷归档。冷归档同样采用 major 聚合策略，禁止为每个小版本创建独立归档文件或目录。

`advance` 执行前必须检查当前稳定版本是否存在未解决错误。若 `errors --current --open` 能查询到记录，必须停止进化并先解决错误，禁止生成或提升候选版本。

AI 提供商配置必须优先使用环境变量和项目根目录 `.env`，禁止把 API Key 写入源码、Markdown、日志、状态文件或运行记录。真实进程环境变量优先级高于 `.env`，`.env` 只作为本地配置补充，且必须被 `.gitignore` 忽略。支持 `OPENAI_API_KEY`、`DEEPSEEK_API_KEY`、`GEMINI_API_KEY` 和 `GOOGLE_API_KEY`；可用 `SELFFORGE_AI_PROVIDER` 指定 `openai`、`deepseek` 或 `gemini`。模型和基础地址可分别通过 `OPENAI_MODEL`、`DEEPSEEK_MODEL`、`GEMINI_MODEL`、`OPENAI_BASE_URL`、`DEEPSEEK_BASE_URL`、`GEMINI_BASE_URL` 覆盖。检查配置必须使用 `ai-config`，输出只能显示密钥是否存在和来源变量名，不得输出密钥值。

Windows PowerShell 当前会话设置环境变量必须使用 `$env:SELFFORGE_AI_PROVIDER="deepseek"` 和 `$env:DEEPSEEK_API_KEY="密钥"` 这类语法；只写 `SELFFORGE_AI_PROVIDER=deepseek` 不会传递给 `cargo run` 子进程。若不想每次设置 PowerShell 变量，可以在项目根目录 `.env` 中写入同名配置。遇到 AI 配置问题必须先运行 `cargo run -- ai-config` 检查当前进程和 `.env` 合并后的可见配置。

AI 请求执行必须优先使用 `ai-request [prompt]` 或应用层统一请求规格。`ai-request` 默认发起受控非流式 HTTP 请求并显示归一化响应文本；需要审计请求结构时使用 `ai-request --dry-run [prompt]`，该模式只能输出脱敏摘要，禁止输出真实 API Key。真实请求必须设置超时，默认不打印完整请求体，避免把敏感提示词写入日志。不同提供商的端点、认证头、JSON 请求体、HTTP 执行和响应解析差异必须集中在 AI 提供商模块内处理，禁止散落在 CLI 或 Agent 业务流程中。

AI 响应解析必须优先使用应用层统一文本响应结构。OpenAI、DeepSeek、Gemini 的响应 JSON 差异必须集中在 AI 提供商模块内处理，业务流程只能消费归一化文本结果。解析失败必须返回明确错误，禁止静默使用空文本继续推进 Agent 流程。

源码扩展必须优先进入 `src/app/` 应用用例层。CLI 只能负责参数解析和输出，不允许堆叠业务流程；`supervisor` 负责编排；`runtime` 负责验证和受控执行；`evolution` 负责版本状态机；`state` 只负责持久化读写。最小闭环的简单入口是 `advance [goal]`。

多 Agent 能力必须通过 `src/app/agent/` 扩展。新增 Agent 应优先表现为 `AgentDefinition`、能力集合和计划步骤，不得把 Agent 业务逻辑直接写入 CLI。查询 Agent 目录使用 `agents`，生成协作计划使用 `agent-plan [--current|--candidate|--version VERSION] [--limit N] [goal]`。`agent-plan` 必须通过应用层读取 `memory-insights` 并展示来源版本、成功经验、失败风险、优化建议和可复用经验摘要，禁止绕过统一记忆经验结构另建计划上下文。后续接入真实多 Agent 执行时，必须复用注册表、计划结构、状态持久化、沙箱执行和 forge 归档，不得创建并行的 Agent 配置体系。

多 AI 同步修改代码必须使用协作任务板。初始化使用 `agent-work-init [--current|--candidate|--version VERSION] [--threads N] [goal]`，队列只能写入 `workspaces/vMAJOR/artifacts/agents/coordination/work-queue.json`，禁止为小版本创建独立队列目录或文件。查询使用 `agent-work-status`，领取使用 `agent-work-claim [--worker ID] [--agent AGENT_ID] [--lease-seconds N]`，完成使用 `agent-work-complete TASK_ID --worker ID --summary TEXT`，释放使用 `agent-work-release TASK_ID --worker ID --reason TEXT`，清理过期领取使用 `agent-work-reap [--reason TEXT]`。

所有 AI 线程在修改任何文件前必须先领取任务。单 AI 线程可以使用默认 `ai-1` 自动领取当前最高优先级可执行任务；多个 AI 线程必须使用不同 `--worker` 标识，并优先领取状态为待领取、依赖已完成、写入范围不冲突的任务。领取结果中的提示词是该线程的执行边界，线程只能处理当前任务的写入范围、验收标准和归档要求，禁止主动实现其他未领取任务，禁止覆盖其他线程已领取任务的写入范围。

协作冲突必须显式处理。若发现依赖缺失、写入范围重叠、测试阻断、职责不清或任务已经被其他线程完成，当前线程必须执行 `agent-work-release` 写明原因，或在任务板中保持未完成状态等待重新分配，禁止静默继续修改。任务完成必须写入摘要，并由后续验证任务统一执行测试、审查和中文归档。任务板是多 AI 协作的唯一调度事实来源，不得以聊天记录、进程内存或临时文本替代。

协作任务领取必须带有租约。默认租约由队列写入，临时验证可用 `--lease-seconds N` 缩短租约；状态查询必须展示租约信息。若线程中断、超时或无法继续，其他线程不得直接覆盖该任务，必须先使用 `agent-work-reap` 释放已过期任务，或等待持有线程主动 `agent-work-release`。释放和过期清理都必须写入队列事件，恢复候选任务提示词，确保后续线程重新领取时获得新的执行边界。

Agent Tool 能力必须通过 `src/app/agent/` 扩展。工具定义必须表现为结构化 `AgentToolDefinition`，并通过 `agent-tools [--current|--candidate|--version VERSION] [--init]` 查询或初始化配置。动态配置文件只允许写入 `workspaces/vMAJOR/artifacts/agents/tool-config.json`，禁止为小版本创建独立工具配置文件。Agent 可以通过能力匹配和 `agent_bindings` 自由组合工具，但工具配置必须验证 Agent 标识、工具标识、启用状态和重复定义，禁止未知工具静默进入计划。`agent-plan` 和 Agent 会话必须展示或持久化工具绑定结果，CLI 不得绕过应用层直接解析工具配置。

Agent Tool 调用必须通过统一调用协议执行。调用输入和调用结果必须使用结构化 `AgentToolInvocation`、`AgentToolInvocationInput` 和 `AgentToolInvocationReport`，命令入口使用 `agent-tool-run TOOL_ID --agent AGENT_ID`。调用前必须校验目标 Agent 是否已绑定该工具，未绑定时必须拒绝；自定义工具没有执行器时必须返回明确错误，禁止静默成功。工具执行分发必须位于应用用例层，CLI 只允许负责参数解析和结果展示。Runtime 类工具必须复用 `agent-run` 和 Runtime 运行记录，AI 类工具默认优先使用请求预览或统一 AI Provider，禁止泄露 API Key。

Agent 步进执行必须通过 `agent-step SESSION_ID` 或应用层 `execute_next_agent_step`。步进器只能执行会话中的下一条待执行步骤，必须自动选择该步骤已绑定且可运行的工具，并把结果写回同一个 Agent 会话。需要外部输入的工具必须显式提供输入，例如 Runtime 工具需要 `-- PROGRAM [ARGS...]`，AI 工具需要 `--prompt TEXT`；缺少输入时必须保持步骤待执行并返回明确阻断原因，禁止编造执行结果。步骤执行失败时必须写入会话失败状态；Runtime 工具必须继续复用运行记录和沙箱路径。

Agent 会话必须通过 `agent-start [goal]` 创建，并写入 `workspaces/vMAJOR/artifacts/agents/sessions/`；会话摘要必须追加到 `workspaces/vMAJOR/artifacts/agents/index.jsonl`。查询会话使用 `agent-sessions [--limit N]`，读取单个会话使用 `agent-session SESSION_ID`。会话文件必须持久化计划上下文快照，包括记忆版本、记忆归档文件、来源版本、成功经验、失败风险、优化建议和可复用经验摘要；`agent-session` 必须输出计划依据摘要，便于审计计划来源。会话文件只允许进入 `artifacts/agents/` 分层，禁止写入 workspace 根目录，禁止为小版本创建独立会话目录，禁止把会话状态只保存在进程内存中。

`agent-start` 创建会话时必须自动初始化或复用当前 major 的协作任务板，并将任务板路径、任务数量、线程数量、租约配置和创建或复用状态写入会话计划上下文。会话事件必须记录协作任务板准备结果。`agent-session` 必须展示协作任务板摘要，后续 Agent 步进和多 AI 执行只能复用该任务板上下文，禁止重新猜测或另建并行任务队列。

`agent-step` 在确认工具输入完整后，必须优先基于会话计划上下文领取协作任务板任务，并把任务编号和工作线程写入对应步骤。工具执行成功且步骤完成时，必须同步完成协作任务；工具调用失败或 Runtime 验证失败时，必须释放已领取任务并写明原因；工具缺少必要输入时禁止领取任务，避免任务被无效步骤占用。CLI 输出和 `agent-session` 详情必须展示步骤关联的协作任务，便于审计。

版本提升后需要审计最近 Agent 会话时，必须优先使用 `agent-sessions --all [--limit N]`。该命令只能读取同一 major 工作区的 `workspaces/vMAJOR/artifacts/agents/index.jsonl`，按最新摘要去重并返回最近会话；禁止为了跨小版本查询创建新的索引文件、目录或小版本归档。

Agent 会话必须保存结构化事件时间线。会话创建、状态变化、步骤更新、成功完成和失败停止都必须写入同一个会话 JSON 的 `events` 字段；会话摘要可记录事件数量，CLI 详情必须能展示事件。禁止为事件时间线创建平行目录、额外索引或只保存在进程内存中。

Agent 步骤需要执行真实程序时，必须优先使用 `agent-run [--session-version VERSION] [--current|--candidate|--version VERSION] [--step N] [--timeout-ms N] SESSION_ID -- PROGRAM [ARGS...]`。该命令必须复用 Runtime 受控执行并将运行编号、报告路径、退出码和超时状态写入同一个会话事件；禁止绕过 Runtime 直接执行程序，禁止把运行证据只写入日志或只输出到终端。

需要创建一次独立验证会话并执行验证命令时，必须优先使用 `agent-verify [--current|--candidate|--version VERSION] [--timeout-ms N] [goal] -- PROGRAM [ARGS...]`。该命令必须自动创建 Agent 会话、调用 Runtime 受控执行、把运行引用写入会话事件，并根据运行结果完成或失败会话；禁止要求使用者手动拼接 `agent-start` 与 `agent-run` 作为常规验证入口。

Agent 自动进化入口必须优先使用 `agent-advance [goal]`。该命令必须创建 Agent 会话、执行 `preflight`、调用现有 `advance` 最小闭环，并把步骤状态、结果和失败原因写回同一个会话文件。若前置检查发现未解决错误，必须将会话标记为失败并停止进化，禁止生成或提升候选版本。`agent-advance` 只能编排现有受控流程，不得绕过 Supervisor、Runtime、错误归档、版本规则或状态文件。

单轮完整 Agent 进化必须优先使用 `agent-evolve [goal]`。该命令必须创建 Agent 会话、执行 `preflight`、在无候选时准备下一 patch 候选、在已有候选时直接验证候选，并通过 `cycle` 完成提升或回滚。若存在未解决错误，必须在准备候选前停止并将会话标记为失败。`agent-evolve` 只能执行一轮完整进化，禁止循环自调用，禁止跳过测试和验证。

AI 自我升级入口必须优先使用 `agent-self-upgrade [--dry-run] [--timeout-ms N] [hint]`。该命令必须先执行 `preflight`，再读取近期 `memory-insights`，然后通过统一 AI Provider 生成一个中文、单句、patch 级、可验证的升级目标，最后复用 `agent-evolve` 执行受控进化。`--dry-run` 只能输出脱敏请求摘要和提示词规模，禁止发起真实升级；真实执行必须设置超时，并且不得输出 API Key、完整请求体或敏感提示词。若存在未解决错误、AI 响应为空、响应无法归一化为目标或进化流程失败，必须停止并返回明确错误，禁止静默继续。当前阶段 AI 自我升级只负责选择升级目标和触发受控闭环，禁止绕过任务、记忆、错误、版本、Runtime、Supervisor 和状态文件直接修改代码。

---

# 八、Git 提交规范

提交格式：

```txt
feat(scope): v0.1.2 中文描述
fix(scope): v0.1.2 中文描述
refactor(scope): v0.1.2 中文描述
test(scope): v0.1.2 中文描述
```

规则：

1. 每次进化必须提交。
2. 提交标题和正文必须使用中文。
3. 提交信息必须明确描述变更，并包含版本号。
4. 禁止使用 `update`、`change` 等模糊描述。

---

# 九、测试规范

必须包含：

1. 单元测试。
2. 边界测试。
3. 错误测试。

必须验证：

1. 功能正确。
2. 无崩溃。
3. 无死循环。
4. 文档中文规范通过。

---

# 十、状态管理

必须使用 `state/state.json` 持久化状态，禁止依赖进程内存传递状态。

状态必须至少表达：

1. 当前稳定版本。
2. 当前工作区。
3. 候选版本。
4. 候选工作区。
5. 版本规则。
6. 历史版本线索。

---

# 十一、安全机制

必须逐步实现：

1. 沙箱执行。
2. CPU、内存、时间限制。
3. 文件访问限制，仅允许访问当前 workspace。
4. 候选版本并行验证。
5. 失败回滚。

---

# 十二、输出要求

每次执行必须输出：

1. 当前版本。
2. 当前目标。
3. 当前计划。
4. 修改内容。
5. 测试结果。
6. 错误信息。
7. 是否成功。
8. commit 信息。
9. 所有文档路径。

---

# 十三、一句话本质

SelfForge 是一个具备记忆、计划、验证、提升与回滚能力的受控自进化系统。
