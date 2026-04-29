# 旧版代际记录

# 记录规则

- 本文件集中保留旧式代际命名记录。
- `v1` 与 `v2` 是语义化版本规则启用前的历史线索，不再作为当前版本命名方式。
- 原 `forge/versions/v1.md` 与 `forge/versions/v2.md` 的有效内容已合并到本文件，确认后可删除旧文件。

## v1

# 版本变化

- 建立 SelfForge v1 基础架构。
- 建立 `runtime`、`supervisor`、`workspaces/v1`、`forge`、`state` 目录。
- 建立持久化状态文件 `state/state.json`。

# 新增功能

- Rust CLI：`init`、`validate`、`status`。
- Runtime 验证层。
- Supervisor 编排层。
- forge 文档归档入口。
- v1 测试覆盖初始化、幂等边界和缺失状态错误。

# 修复内容

- 无。

## v2

# 版本变化

- 从 `v1` 生成 `v2` 候选版本。
- 新增 `workspaces/v2` 作为候选版本工作区。
- 新增 v2 的 memory、tasks、errors、versions 文档。
- 更新状态文件，记录候选版本与候选工作区。

# 新增功能

- 候选版本生成引擎。
- 结构化状态读写。
- Supervisor 候选生成入口。
- Runtime 按版本验证布局。
- CLI 命令：`evolve`。

# 修复内容

- 修复新增代码格式差异。
