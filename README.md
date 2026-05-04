# SelfForge

SelfForge 是一个受控自进化软件系统，目标是让版本生成、验证、记录、提升和回滚形成可审计闭环。

# 当前状态

- 当前版本：`v0.1.71`
- 核心语言：Rust
- 状态文件：`state/state.json`
- 归档目录：`forge/`
- 工作区：`workspaces/v0/`

# 常用命令

下面代码块中的命令均可在项目根目录直接复制执行，并应以退出码 0 完成。需要真实记录编号、会改写任务板、会创建候选版本、会发起真实 AI 请求、会提交或创建 PR 的命令不放入本清单；这些流程先通过 `cargo run -- help` 查看参数，再按当前任务板、记录编号和收束确认执行。

```txt
cargo run -- help
cargo run -- validate
cargo test
cargo run -- preflight
cargo run -- errors --current --open
cargo run -- branch-check --suggest
cargo run -- memory-context --current --limit 5
cargo run -- memory-insights --current --limit 5
cargo run -- ai-config
cargo run -- ai-request --dry-run "提示词"
cargo run -- agents
cargo run -- agent-tools
cargo run -- agent-skills --current
cargo run -- agent-skill-select --current --limit 3 --token-budget 800 "README 命令"
cargo run -- agent-work-status --current
cargo run -- agent-work-status --current --active-only
cargo run -- agent-work-finalize-check --current
cargo run -- agent-self-loops --current --limit 10
```

# 需要上下文的流程

协作任务、Agent 会话、补丁草案、源码覆盖、自我升级、自我进化循环、版本提升和回滚都依赖当前任务板状态、真实记录编号或最终收束确认。执行这些流程时先运行 `cargo run -- help` 查看完整命令，再用 `agent-work-status`、`agent-sessions`、`agent-patch-drafts`、`agent-patch-audits`、`agent-patch-applications`、`agent-self-upgrades`、`agent-self-loops` 等查询命令取得真实编号。真实 AI 请求使用 `ai-request` 时必须确认环境变量中的密钥可用；普通巡检使用 `ai-request --dry-run`。

`agent-self-loop --commit-each-cycle` 只允许创建本地阶段提交；`agent-self-loop --finalize-pr --confirm-finalize` 才允许统一 push、创建 PR、等待 required checks、合并并删除远程任务分支。最终收束前必须先通过 `agent-work-finalize-check`。

# AI 技能加载

Agent 技能索引写入 `workspaces/v0/artifacts/agents/skills/skill-index.json`。`agent-skills` 只读取技能元数据，不读取技能正文；`agent-skill-select` 根据目标、标签、触发词和能力召回少量技能，并受 `--limit` 与 `--token-budget` 限制。这样即使技能数量达到几百个，默认上下文也只携带轻量索引和少量相关技能正文。`agent-self-upgrade --dry-run` 会展示技能索引、候选、选择、正文和 token 统计；实际自我升级提示词会包含按需技能上下文，无索引时保持只使用当前状态和近期记忆。

# AI 配置

SelfForge 会读取当前进程环境变量和项目根目录 `.env`。真实环境变量优先于 `.env`，`.env` 禁止提交。

```txt
SELFFORGE_AI_PROVIDER=deepseek
DEEPSEEK_API_KEY=你的密钥
```

# 关键约束

所有 Markdown 文档必须使用中文，禁止 Emoji。小版本记录追加到 `forge/*/v0.md`，工作区复用 `workspaces/v0/`。久远记忆归档到 `forge/memory/archive/v0.md`，默认 Agent 计划只读取近期记忆。
