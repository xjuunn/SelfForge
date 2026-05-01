# SelfForge

SelfForge 是一个受控自进化软件系统，目标是让版本生成、验证、记录、提升和回滚形成可审计闭环。

# 当前状态

- 当前版本：`v0.1.65`
- 核心语言：Rust
- 状态文件：`state/state.json`
- 归档目录：`forge/`
- 工作区：`workspaces/v0/`

# 常用命令

```txt
cargo run -- validate
cargo test
cargo run -- preflight
cargo run -- memory-insights
cargo run -- memory-compact --keep 5
cargo run -- ai-config
cargo run -- ai-request "提示词"
cargo run -- agents
cargo run -- agent-tools
cargo run -- agent-work-status
cargo run -- agent-plan "目标"
cargo run -- agent-start "目标"
cargo run -- agent-steps SESSION_ID
cargo run -- agent-patch-draft "目标"
cargo run -- agent-patch-draft --from-task-audit TASK_AUDIT_ID
cargo run -- agent-patch-audit DRAFT_RECORD_ID
cargo run -- agent-patch-preview AUDIT_RECORD_ID
cargo run -- agent-patch-apply PREVIEW_RECORD_ID
cargo run -- agent-patch-verify APPLICATION_RECORD_ID
cargo run -- agent-patch-source-plan APPLICATION_RECORD_ID
cargo run -- agent-patch-source-execute SOURCE_PLAN_ID
cargo run -- agent-patch-source-promotion SOURCE_EXECUTION_ID
cargo run -- agent-patch-source-candidate PROMOTION_ID
cargo run -- agent-patch-source-cycle CANDIDATE_RECORD_ID
cargo run -- agent-patch-source-cycle-summary CYCLE_RECORD_ID
cargo run -- agent-patch-source-task-draft SUMMARY_RECORD_ID
cargo run -- agent-patch-source-task-audit TASK_DRAFT_ID
cargo run -- agent-self-upgrade "目标提示"
cargo run -- advance "目标"
cargo run -- cycle
cargo run -- errors --current --open
```

# AI 配置

SelfForge 会读取当前进程环境变量和项目根目录 `.env`。真实环境变量优先于 `.env`，`.env` 禁止提交。

```txt
SELFFORGE_AI_PROVIDER=deepseek
DEEPSEEK_API_KEY=你的密钥
```

# 关键约束

所有 Markdown 文档必须使用中文，禁止 Emoji。小版本记录追加到 `forge/*/v0.md`，工作区复用 `workspaces/v0/`。久远记忆归档到 `forge/memory/archive/v0.md`，默认 Agent 计划只读取近期记忆。
