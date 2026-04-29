# 错误信息

首次执行 `cargo check` 时，`src/main.rs` 的 `run` 分支在 `main` 函数中直接使用 `?`，导致编译失败。

# 出现阶段

编译检查阶段。

# 原因分析

`main` 当前返回 `()`，不能直接使用 `?` 传播 `parse_run_args` 的错误。

# 解决方案

将 `parse_run_args` 的错误显式转为退出流程，通过 `exit_with_error` 输出错误并退出进程。

# 是否已解决

已解决。后续 `cargo check` 通过。
