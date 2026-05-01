# SelfForge

SelfForge 是一个受控自进化软件系统，目标是让版本生成、验证、记录、提升和回滚形成可审计闭环。

# 当前状态

- 当前版本：v0.1.32
- 核心语言：Rust
- 状态文件：`state/state.json`
- 归档目录：`forge/`
- 工作区：`workspaces/v0/`

# 常用命令

```txt
cargo run -- validate
cargo test
cargo run -- preflight
cargo run -- ai-config
cargo run -- ai-request "提示词"
cargo run -- ai-request --dry-run "提示词"
cargo run -- agents
cargo run -- agent-plan "目标"
cargo run -- agent-start "目标"
cargo run -- agent-sessions
cargo run -- agent-sessions --all
cargo run -- agent-session SESSION_ID
cargo run -- agent-run SESSION_ID -- PROGRAM
cargo run -- agent-verify "目标" -- PROGRAM
cargo run -- agent-advance "目标"
cargo run -- agent-evolve "目标"
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
