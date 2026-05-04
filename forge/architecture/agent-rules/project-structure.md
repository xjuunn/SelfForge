# 项目结构规则

# 根目录边界

根目录保留工程入口和稳定边界：

```txt
Agents.md
README.md
Cargo.toml
Cargo.lock
src/
runtime/
supervisor/
workspaces/
forge/
state/
```

详细结构参见 `forge/architecture/project-structure.md`。

# Rust 分层

1. `src/app/` 是应用用例层，新功能优先进入这里。
2. `src/main.rs` 只负责 CLI 参数解析和输出，禁止堆叠业务流程。
3. `runtime` 负责受控执行和验证。
4. `supervisor` 负责编排最小闭环。
5. `evolution` 负责候选版本、提升和回滚状态机。
6. `state` 只负责持久化状态读写。

# workspace 规则

1. workspace 按 major 聚合，例如 `v0.1.65` 复用 `workspaces/v0/`。
2. workspace 根目录只允许 `README.md`、`.gitignore`、`source/`、`tests/`、`sandbox/`、`artifacts/`、`logs/`。
3. 生成源码进入 `source/`，测试进入 `tests/`，运行记录进入 `sandbox/`，产物进入 `artifacts/`，本地日志进入 `logs/`。
4. 禁止把生成文件直接堆放在 workspace 根目录。

# README 规则

根目录 `README.md` 只保留项目简介、当前状态、常用命令和关键约束。详细设计写入 `forge/architecture/`。
