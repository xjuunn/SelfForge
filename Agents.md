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

`advance` 执行前必须检查当前稳定版本是否存在未解决错误。若 `errors --current --open` 能查询到记录，必须停止进化并先解决错误，禁止生成或提升候选版本。

AI 提供商配置必须优先使用环境变量和项目根目录 `.env`，禁止把 API Key 写入源码、Markdown、日志、状态文件或运行记录。真实进程环境变量优先级高于 `.env`，`.env` 只作为本地配置补充，且必须被 `.gitignore` 忽略。支持 `OPENAI_API_KEY`、`DEEPSEEK_API_KEY`、`GEMINI_API_KEY` 和 `GOOGLE_API_KEY`；可用 `SELFFORGE_AI_PROVIDER` 指定 `openai`、`deepseek` 或 `gemini`。模型和基础地址可分别通过 `OPENAI_MODEL`、`DEEPSEEK_MODEL`、`GEMINI_MODEL`、`OPENAI_BASE_URL`、`DEEPSEEK_BASE_URL`、`GEMINI_BASE_URL` 覆盖。检查配置必须使用 `ai-config`，输出只能显示密钥是否存在和来源变量名，不得输出密钥值。

Windows PowerShell 当前会话设置环境变量必须使用 `$env:SELFFORGE_AI_PROVIDER="deepseek"` 和 `$env:DEEPSEEK_API_KEY="密钥"` 这类语法；只写 `SELFFORGE_AI_PROVIDER=deepseek` 不会传递给 `cargo run` 子进程。若不想每次设置 PowerShell 变量，可以在项目根目录 `.env` 中写入同名配置。遇到 AI 配置问题必须先运行 `cargo run -- ai-config` 检查当前进程和 `.env` 合并后的可见配置。

AI 请求构建必须优先使用 `ai-request [prompt]` 或应用层统一请求规格。请求构建只能输出脱敏摘要，禁止输出真实 API Key；默认不打印完整请求体，避免把敏感提示词写入日志。不同提供商的端点、认证头和 JSON 请求体差异必须集中在 AI 提供商模块内处理，禁止散落在 CLI 或 Agent 业务流程中。

AI 响应解析必须优先使用应用层统一文本响应结构。OpenAI、DeepSeek、Gemini 的响应 JSON 差异必须集中在 AI 提供商模块内处理，业务流程只能消费归一化文本结果。解析失败必须返回明确错误，禁止静默使用空文本继续推进 Agent 流程。

源码扩展必须优先进入 `src/app/` 应用用例层。CLI 只能负责参数解析和输出，不允许堆叠业务流程；`supervisor` 负责编排；`runtime` 负责验证和受控执行；`evolution` 负责版本状态机；`state` 只负责持久化读写。最小闭环的简单入口是 `advance [goal]`。

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
