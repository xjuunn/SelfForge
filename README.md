# SelfForge

SelfForge 是一个受控自进化软件系统，目标是让版本生成、验证、记录、提升和回滚形成可审计闭环。

# 当前状态

- 当前版本：v0.1.58
- 核心语言：Rust
- 状态文件：`state/state.json`
- 归档目录：`forge/`
- 工作区：`workspaces/v0/`

# 常用命令

```txt
cargo run -- validate
cargo test
cargo run -- preflight
cargo run -- memory-context
cargo run -- memory-insights
cargo run -- memory-compact --keep 5
cargo run -- ai-config
cargo run -- ai-request "提示词"
cargo run -- ai-request --dry-run "提示词"
cargo run -- agents
cargo run -- agent-tools
cargo run -- agent-work-init --threads 2 "目标"
cargo run -- agent-work-status
cargo run -- agent-work-claim --worker ai-1 --lease-seconds 3600
cargo run -- agent-work-complete TASK_ID --worker ai-1 --summary "摘要"
cargo run -- agent-work-release TASK_ID --worker ai-1 --reason "原因"
cargo run -- agent-work-reap --reason "租约过期"
cargo run -- agent-tool-run memory.insights --agent architect
cargo run -- agent-step SESSION_ID
cargo run -- agent-steps --max-steps 2 SESSION_ID
cargo run -- agent-plan "目标"
cargo run -- agent-start "目标"
cargo run -- agent-session SESSION_ID
cargo run -- agent-sessions
cargo run -- agent-sessions --all
cargo run -- agent-run SESSION_ID -- PROGRAM
cargo run -- agent-verify "目标" -- PROGRAM
cargo run -- agent-advance "目标"
cargo run -- agent-evolve "目标"
cargo run -- agent-patch-draft --dry-run "目标"
cargo run -- agent-patch-draft "目标"
cargo run -- agent-patch-drafts
cargo run -- agent-patch-draft-record RECORD_ID
cargo run -- agent-patch-audit DRAFT_RECORD_ID
cargo run -- agent-patch-audits
cargo run -- agent-patch-audit-record AUDIT_RECORD_ID
cargo run -- agent-patch-preview AUDIT_RECORD_ID
cargo run -- agent-patch-previews
cargo run -- agent-patch-preview-record PREVIEW_RECORD_ID
cargo run -- agent-patch-apply PREVIEW_RECORD_ID
cargo run -- agent-patch-verify APPLICATION_RECORD_ID
cargo run -- agent-patch-source-plan APPLICATION_RECORD_ID
cargo run -- agent-patch-source-plans
cargo run -- agent-patch-source-plan-record SOURCE_PLAN_ID
cargo run -- agent-patch-source-execute SOURCE_PLAN_ID
cargo run -- agent-patch-source-executions
cargo run -- agent-patch-source-execution-record SOURCE_EXECUTION_ID
cargo run -- agent-patch-source-promotion SOURCE_EXECUTION_ID
cargo run -- agent-patch-source-promotions
cargo run -- agent-patch-source-promotion-record PROMOTION_ID
cargo run -- agent-patch-source-candidate PROMOTION_ID
cargo run -- agent-patch-source-candidates
cargo run -- agent-patch-source-candidate-record CANDIDATE_RECORD_ID
cargo run -- agent-patch-applications
cargo run -- agent-patch-application-record APPLICATION_RECORD_ID
cargo run -- agent-self-upgrade --dry-run "目标提示"
cargo run -- agent-self-upgrade "目标提示"
cargo run -- agent-self-upgrades
cargo run -- agent-self-upgrade-record RECORD_ID
cargo run -- agent-self-upgrade-report AUDIT_RECORD_ID
cargo run -- agent-self-upgrade-reports
cargo run -- agent-self-upgrade-report-record REPORT_ID
cargo run -- advance "目标"
cargo run -- cycle
cargo run -- runs --current
cargo run -- errors --current --open
```

# AI 配置

SelfForge 会自动读取当前进程环境变量和项目根目录 `.env`。真实环境变量优先于 `.env`，`.env` 禁止提交。

```txt
SELFFORGE_AI_PROVIDER=deepseek
DEEPSEEK_API_KEY=你的密钥
```

PowerShell 当前会话也可以直接设置：

```powershell
$env:SELFFORGE_AI_PROVIDER="deepseek"
$env:DEEPSEEK_API_KEY="你的密钥"
cargo run -- ai-config
```

`ai-request` 默认发起真实非流式请求并显示响应文本；`ai-request --dry-run` 只显示脱敏请求摘要。

# 约束

所有文档必须使用中文，禁止使用 Emoji。小版本记录追加到 `forge/*/v0.md`，工作区复用 `workspaces/v0/`。
热记忆只保留近期完整记录，久远完整记忆归档到 `forge/memory/archive/v0.md`，默认 Agent 计划只读取近期记忆。
