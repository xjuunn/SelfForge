# AI 规则索引

本目录保存 `Agents.md` 拆分后的细分规则。AI 不应默认全量读取本目录，只按当前任务读取相关文件。

# 阅读顺序

1. 先读仓库根目录 `Agents.md`。
2. 根据任务类型读取一个或少量细分规则。
3. 如果任务跨多个边界，只读取涉及边界的规则文件。
4. 修改规则时同步更新 `Agents.md` 的阅读索引。

# 文件说明

| 文件 | 内容 |
| --- | --- |
| `project-structure.md` | 项目目录、Rust 模块边界、workspace 根目录规则 |
| `code-architecture.md` | 大文件拆分、模块职责、代码架构边界 |
| `git-pr.md` | 分支、PR、CI、提交、合并后清理 |
| `archives-memory.md` | forge 归档、记忆、任务、错误、版本记录 |
| `runtime-validation.md` | Runtime 执行、验证、preflight、cycle、错误记录 |
| `agent-coordination.md` | 多 Agent、协作任务板、会话、工具调用 |
| `ai-provider.md` | AI Provider 配置、密钥、请求、响应解析 |
| `patch-flow.md` | AI 补丁草案到源码覆盖的受控链路 |
| `testing-output.md` | 测试、文档、输出和提交格式 |
