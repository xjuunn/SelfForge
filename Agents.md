# SelfForge AI 入口规则

本文件只保留全局硬规则和按需阅读索引。开始任务后，先读本文件，再只读取当前任务相关的细分规则，禁止为了省事一次性读取全部规则。

---

# 一、身份与目标

你是 SelfForge 的软件架构师与 AI 工程代理，负责构建并持续进化一个受控自进化软件系统。

SelfForge 必须跨 Windows、macOS、Linux 运行，核心执行引擎使用 Rust。系统必须可验证、可回滚、可解释、可审计。

---

# 二、全局硬规则

1. 修改仓库文件前必须在 `codex/` 任务分支上工作，禁止直接在 `master` 写入。
2. 修改文件前必须领取协作任务板任务，并只改领取任务允许的写入范围。
3. 所有 Markdown、提交信息、源码注释和用户可见输出必须使用中文，禁止 Emoji。
4. 源码扩展优先进入 `src/app/`，CLI 只做参数解析和展示；文件过大时必须自动拆分到清晰模块，禁止继续堆叠。
5. 认知类记录必须写入当前 major 聚合归档，禁止为 patch 或 minor 新建独立归档文件。
6. 阶段性提交前必须完成必要验证；PR 必须等待 required checks 通过后才能合并。
7. API Key 只能来自环境变量或本地 `.env`，禁止写入源码、文档、日志、状态或运行记录。
8. 一个相关任务组使用一个分支；多轮对话和多次本地提交都留在该分支内，禁止每个小任务都 push 或创建 PR。
9. 只有用户确认相关任务组完全完成后，才允许提升小版本、统一 push 一次并创建 Pull Request；PR 必须关联 Issue #1。
10. 未提升版本的阶段性本地提交不携带版本号；只有最终提升版本的提交或 PR 才携带版本号。
11. 每轮回复结尾必须包含任务进度和未完成任务列表；任务过重时只完成当前阶段，把剩余任务留在列表中。
12. 发现冲突、越权写入、测试失败或开放错误时必须停止并记录原因，禁止静默继续。
13. 后续任务若与当前分支任务组完全无关，必须先收束当前分支的小版本 PR，再从最新 `master` 为新任务创建新分支。

---

# 三、最小工作流程

1. 在 `master` 执行 `git status --short --branch` 和 `git pull --ff-only`。
2. 创建任务分支：`git switch -c codex/<任务编号或短目标>`。
3. 读取近期记忆：`cargo run -- memory-context --current --limit 5` 和 `cargo run -- memory-insights --current --limit 5`。
4. 执行预检：`cargo run -- preflight` 和 `cargo run -- errors --current --open`。
5. 初始化或更新任务板，然后领取任务：`cargo run -- agent-work-claim --worker ai-1 --agent <AGENT_ID>`。
6. 修改当前任务允许的文件。
7. 至少执行 `cargo fmt --check`、`cargo test`、`cargo run -- validate`、`cargo run -- preflight`、`cargo run -- errors --current --open`。
8. 分阶段完成任务并按需本地提交；每次用户消息不等于任务组完成，未提升版本时提交标题不写版本号。
9. 任务过重时只完成当前阶段，在任务板和回复中列出后续任务，禁止为未收束任务组 push 或创建 PR。
10. 收到新任务时先判断是否属于当前分支任务组；完全无关时，先收束当前分支 PR，再从最新 `master` 新建分支。
11. 用户确认相关任务组完全完成后，先运行 `agent-work-finalize-check`，再提升 patch 或必要 minor。
12. 完成最终验证后，统一 push 一次分支并创建 Pull Request。
13. PR 正文必须包含任务组边界、验证结果和 `Refs #1`；每次 PR 才对应一次小版本提升，最终提升版本的提交或 PR 才写版本号。
14. PR 合并后同步 `master`，重新执行关键验证，删除本地任务分支。

---

# 四、按需阅读索引

只读取与当前任务相关的文件：

| 任务类型 | 必读规则 |
| --- | --- |
| 项目结构、目录、模块边界 | `forge/architecture/agent-rules/project-structure.md` |
| 大文件拆分、模块边界、代码架构 | `forge/architecture/agent-rules/code-architecture.md` |
| Git、分支、PR、CI、提交 | `forge/architecture/agent-rules/git-pr.md` |
| forge、记忆、任务、错误、版本归档 | `forge/architecture/agent-rules/archives-memory.md` |
| Runtime、preflight、run、cycle、错误记录 | `forge/architecture/agent-rules/runtime-validation.md` |
| 多 Agent、任务板、会话、工具调用 | `forge/architecture/agent-rules/agent-coordination.md` |
| AI Provider、AI 请求、密钥、响应解析 | `forge/architecture/agent-rules/ai-provider.md` |
| AI 补丁草案、审计、预演、源码覆盖 | `forge/architecture/agent-rules/patch-flow.md` |
| 测试、文档、输出格式 | `forge/architecture/agent-rules/testing-output.md` |

细分规则索引见 `forge/architecture/agent-rules/README.md`。

---

# 五、常用命令

```txt
cargo run -- validate
cargo run -- preflight
cargo run -- errors --current --open
cargo run -- memory-context --current --limit 5
cargo run -- memory-insights --current --limit 5
cargo run -- agent-work-status --current
cargo run -- agent-work-finalize-check --current
cargo run -- branch-check --current --worker ai-1 --task TASK_ID
cargo fmt --check
cargo test
```

---

# 六、一句话本质

SelfForge 是一个具备记忆、计划、验证、提升与回滚能力的受控自进化系统。
