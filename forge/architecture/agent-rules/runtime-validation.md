# Runtime 与验证规则

# 预检

进入进化或候选流程前必须执行：

```txt
cargo run -- preflight
cargo run -- errors --current --open
```

存在开放错误时必须先解决，禁止生成或提升候选版本。

# Runtime 执行

运行外部程序必须优先使用：

```txt
cargo run -- run [--current|--candidate|--version VERSION] [--timeout-ms N] -- PROGRAM [ARGS...]
```

要求：

1. 禁止隐式 shell 包装。
2. 执行目录固定为目标版本 workspace。
3. 每次运行写入 `workspaces/vMAJOR/sandbox/runs/`。
4. 必须保存 `report.json`、`stdout.txt`、`stderr.txt`，并追加 `index.jsonl`。

# 查询与错误记录

1. 查询运行：`runs [--limit N] [--failed] [--timed-out]`。
2. 归档失败运行：`record-error [--run-id RUN_ID] [--stage TEXT] [--solution TEXT]`。
3. 标记解决：`resolve-error --run-id RUN_ID [--verification TEXT]`。
4. 错误归档只能追加到当前 major 的 `forge/errors/vMAJOR.md`。

# 版本闭环

1. `advance [goal]` 是简单入口。
2. `cycle` 验证候选；成功提升，失败回滚。
3. 人工放弃候选使用 `rollback [reason]`。
4. `agent-evolve` 只执行一轮完整进化，禁止循环自调用。
5. 候选准备和提升不得绕过 `Supervisor`、`Runtime`、错误归档和状态文件。
