# 错误信息

首次执行 `cargo fmt --check` 时，`src/runtime.rs` 和 `src/main.rs` 存在格式化差异。

# 出现阶段

格式化检查阶段。

# 原因分析

新增 Runtime 执行逻辑后，部分长链式调用和函数签名未按 `rustfmt` 默认规则排版。

# 解决方案

执行 `cargo fmt`，统一修正格式。

# 是否已解决

已解决。后续 `cargo fmt --check` 通过。
