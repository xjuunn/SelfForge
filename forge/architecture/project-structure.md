# SelfForge 项目结构规则

# 目标

SelfForge 的目录结构必须让人类和 AI 都能安全扩展：入口清晰、边界稳定、职责单一、状态可追溯、失败可回滚。

# 根目录结构

```txt
/
  Agents.md
  Cargo.toml
  Cargo.lock
  .gitattributes
  .gitignore
  /src/
  /runtime/
  /supervisor/
  /workspaces/
    /v0/                 # v0 大版本聚合工作区，小版本不再建目录
      README.md
      .gitignore
      /source/           # 受控生成或待验证源码
      /tests/            # 工作区测试、样例和夹具
      /sandbox/          # 临时执行目录，按 run id 分层
      /artifacts/        # 可保留产物，按任务或模块分层
      /logs/             # 本地原始日志，摘要写入 forge
  /forge/
    /memory/v0.md        # v0 大版本聚合记忆
    /tasks/v0.md         # v0 大版本聚合任务
    /errors/v0.md        # v0 大版本聚合错误
    /versions/v0.md      # v0 大版本聚合版本记录
  /state/
```

# Rust 源码分层

```txt
/src/
  /app/                  # 应用用例层，放最小闭环和对外业务入口
    mod.rs
    minimal_loop.rs
  main.rs                # CLI 适配层，只解析命令并调用 app 或 supervisor
  lib.rs                 # 公共导出，不堆业务流程
  supervisor.rs          # 编排门面，连接 Runtime 与 Evolution
  runtime.rs             # 受控执行与验证边界
  evolution.rs           # 候选生成、提升、回滚状态机
  layout.rs              # 固定目录与文档结构校验
  state.rs               # 持久化状态读写
  version.rs             # 语义化版本规则
  documentation.rs       # 中文文档审计
```

# 扩展规则

1. 新功能优先进入 `src/app/` 的用例层，再调用底层模块。
2. CLI 只做参数解析和输出格式化，禁止承载业务流程。
3. Runtime 只负责验证和受控执行，禁止写入演进状态。
4. Evolution 只负责版本状态机，禁止直接执行外部命令。
5. State 只负责持久化状态读写，禁止保存进程内临时状态。
6. forge 文档必须记录每次演进的任务、记忆、错误和版本变化。
7. 工作区按 major 聚合：`v0.1.7` 必须复用 `workspaces/v0/`，禁止创建 `workspaces/v0.1.7/`。
8. forge 归档按 major 聚合：patch 和 minor 更新只追加到 `forge/memory/vMAJOR.md`、`forge/tasks/vMAJOR.md`、`forge/errors/vMAJOR.md`、`forge/versions/vMAJOR.md`。
9. workspace 根目录只允许固定入口文件和固定一级目录，禁止直接堆放生成文件。
10. workspace 内部固定目录必须按职责使用：源码进入 `source/`，测试进入 `tests/`，临时执行进入 `sandbox/`，产物进入 `artifacts/`，日志进入 `logs/`。
11. 只有 major 版本变化时，才允许创建新的 major 工作区和 major 归档文件。
12. 大规模目录搬迁必须拆成多个 patch，先建立新入口，再迁移调用，最后清理旧入口。

# 最小可运行闭环

`advance [goal]` 是当前最高层闭环入口：

1. 若没有候选版本，则生成下一候选版本。
2. 若存在候选版本，则运行候选闭环。
3. 候选验证成功后提升为当前版本。
4. 提升成功后生成下一候选版本。
5. 候选验证失败后回滚并停止，不继续生成新候选。

# Git 仓库规则

1. Rust 源码、Markdown、JSON 使用稳定文本换行规则。
2. `target/` 不进入版本库。
3. 每个 patch 必须有清晰中文 commit，标题必须包含版本号。
4. 每个提交必须对应可验证状态，禁止提交测试失败版本。
