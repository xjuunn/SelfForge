# Git、PR 与 CI 规则

# 分支规则

1. 只读检查可在 `master` 执行；首次写入前必须离开 `master`。
2. 分支名默认使用 `codex/<任务编号或短目标>`，小写短横线命名。
3. 多 AI 并行时必须使用不同分支和不同物理工作树。
4. 任务分支必须和协作任务板任务一一对应。
5. `branch-check --suggest` 只给建议，禁止创建分支或修改任务板。

# PR 规则

1. 提交前先执行必要验证。
2. 推送分支：`git push -u origin codex/<任务编号或短目标>`。
3. PR 目标分支为 `master`。
4. PR 正文必须包含任务编号、分支名、目标摘要、主要变更、验证结果、风险、回滚方案和归档路径。
5. PR 保持小而聚焦，一个 PR 只处理一个明确目标。
6. 合并必须通过仓库平台执行，禁止本地 `git merge` 后直接推送 `master`。

# 当前仓库保护策略

`master` 必须满足：

1. 分支与最新 `master` 同步。
2. required checks 全部通过：`fmt`、`test-linux`、`test-windows`、`test-macos`、`validate`、`preflight`、`open-errors`。
3. 所有对话已解决。
4. 禁止强推和删除 `master`。
5. 不要求人工 Approve；CI 全绿即可由平台合并。
6. 仓库开启 `delete_branch_on_merge`，PR 合并后远端任务分支应自动删除。

# 提交规则

提交标题必须使用：

```txt
type(scope): vMAJOR.MINOR.PATCH 中文描述
```

允许的 `type`：`feat`、`fix`、`refactor`、`test`、`docs`、`chore`、`ci`。

提交标题和正文必须中文，必须包含版本号，禁止 Emoji。

# 合并后收尾

1. 切回 `master` 并执行 `git pull --ff-only`。
2. 执行 `cargo run -- validate`、`cargo run -- preflight`、`cargo run -- errors --current --open`。
3. 删除本地任务分支。
4. 确认远端任务分支已自动删除；未删除时手动删除。
