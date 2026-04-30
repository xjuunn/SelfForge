# SelfForge

SelfForge 是一个受控自进化软件系统，目标是让版本生成、验证、记录、提升和回滚形成可审计闭环。

# 当前状态

- 当前版本：v0.1.14
- 核心语言：Rust
- 状态文件：`state/state.json`
- 归档目录：`forge/`
- 工作区：`workspaces/v0/`

# 常用命令

```txt
cargo run -- validate
cargo test
cargo run -- advance "目标"
cargo run -- cycle
cargo run -- runs --current
cargo run -- runs --current --failed
cargo run -- record-error --current
```

# 约束

所有文档必须使用中文，禁止使用 Emoji。小版本记录追加到 `forge/*/v0.md`，工作区复用 `workspaces/v0/`。
