# 错误信息

修复 `run` 分支后，`cargo check` 仍提示 `return exit_with_error(error)` 产生 unreachable warning。

# 出现阶段

严格编译检查阶段。

# 原因分析

`exit_with_error` 返回 never 类型 `!`，在 match 分支中再写 `return` 会让编译器认为表达式不可达。

# 解决方案

去掉多余的 `return`，直接调用 `exit_with_error(error)`。

# 是否已解决

已解决。后续 `cargo check` 无 warning，`cargo test` 全部通过。
