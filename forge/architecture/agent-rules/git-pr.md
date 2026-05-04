# Git、PR 与 CI 规则

# 分支规则

1. 只读检查可在 `master` 执行；首次写入前必须离开 `master`。
2. 分支名默认使用 `codex/<任务编号或短目标>`，小写短横线命名。
3. 多 AI 并行时必须使用不同分支和不同物理工作树。
4. 任务分支必须和一个相关任务组对应；同一任务组可包含多个协作任务板任务。
5. `branch-check --suggest` 只给建议，禁止创建分支或修改任务板。

# 任务组推送规则

1. 一个相关任务组使用一个任务分支。
2. 每次用户消息只代表当前阶段输入，不自动代表任务组已完成。
3. 后续对话中的微调、补充和修正继续在同一分支内完成。
4. 相关任务可在同一分支内连续完成，并按阶段创建多个本地 commit。
5. 禁止每完成一个小任务就 push，禁止每个本地 commit 都创建 PR。
6. 任务过重时，只完成当前可验证阶段，并在任务板和回复中留下后续任务。
7. 只有用户明确确认任务组完全完成后，才允许统一 push 一次分支并创建 Pull Request。
8. 统一 push 前必须完成最终验证、更新任务板状态，并确认未完成任务列表为空或只剩用户接受的后续任务。
9. 远端只接收任务组收束后的统一推送，减少 GitHub Actions 消耗。
10. 任务组边界必须在 PR 正文写清楚，避免把无关目标塞进同一个 PR。

# 版本提升规则

1. 本地阶段性 commit 不提升小版本，不修改稳定版本状态。
2. 每个收束后的 Pull Request 才对应一次版本提升，默认提升 patch。
3. minor 只用于用户确认的兼容阶段扩展；禁止因为本地 commit 数量增加而提升 minor。
4. 任务组未完成且没有提升版本时，本地提交标题不携带版本号。
5. 最终提升版本的收束提交、PR 标题或 PR 正文必须体现新版本号。

# PR 规则

1. 提交前先执行必要验证。
2. 只有任务组被用户确认完成后，才统一推送分支：`git push -u origin codex/<任务编号或短目标>`。
3. PR 目标分支为 `master`。
4. PR 正文必须包含任务编号、任务组范围、分支名、目标摘要、主要变更、验证结果、风险、回滚方案、归档路径和 Issue 关联。
5. PR 保持小而聚焦，一个 PR 只处理一个相关任务组。
6. PR 创建后才消耗远端 CI；PR 前的阶段性验证优先在本地完成。
7. 合并必须通过仓库平台执行，禁止本地 `git merge` 后直接推送 `master`。

# Issue 关联规则

1. 本项目当前统一关联 Issue #1：`https://github.com/xjuunn/SelfForge/issues/1`。
2. 每个任务组的最终 commit 正文或 PR 正文必须包含 `Refs #1`。
3. 只需要关联时使用 `Refs #1`；只有用户明确要求关闭 issue 时，才使用 `Closes #1`。
4. PR 正文应保留完整 issue URL，便于人工审计。

# 当前仓库保护策略

`master` 必须满足：

1. 分支与最新 `master` 同步。
2. required checks 全部通过：`fmt`、`test-linux`、`test-windows`、`test-macos`、`validate`、`preflight`、`open-errors`。
3. 所有对话已解决。
4. 禁止强推和删除 `master`。
5. 不要求人工 Approve；CI 全绿即可由平台合并。
6. 仓库开启 `delete_branch_on_merge`，PR 合并后远端任务分支应自动删除。

# 提交规则

阶段性本地提交在未提升版本时使用：

```txt
type(scope): 中文描述
```

最终提升版本的收束提交使用：

```txt
type(scope): vMAJOR.MINOR.PATCH 中文描述
```

允许的 `type`：`feat`、`fix`、`refactor`、`test`、`docs`、`chore`、`ci`。

提交标题和正文必须中文，禁止 Emoji。未提升版本时禁止为了凑格式写旧版本号。

阶段性本地提交正文建议包含当前阶段摘要和验证结果；最终收束 commit 或 PR 正文必须包含 `Refs #1`。

最终收束提交正文建议包含：

```txt
任务组：...
验证：...
Refs #1
```

# 合并后收尾

1. 切回 `master` 并执行 `git pull --ff-only`。
2. 执行 `cargo run -- validate`、`cargo run -- preflight`、`cargo run -- errors --current --open`。
3. 删除本地任务分支。
4. 确认远端任务分支已自动删除；未删除时手动删除。
