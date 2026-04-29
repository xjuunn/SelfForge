# 错误信息

首次执行 `cargo fmt --check` 时发现新增文档审计模块、候选提升逻辑和重写后的布局模块存在格式化差异。

# 出现阶段

测试与验证阶段。

# 原因分析

新增和重写的 Rust 代码尚未经过 rustfmt 统一格式化。

# 解决方案

执行 `cargo fmt`，并重新执行 `cargo fmt --check`。

# 是否已解决

已解决。后续 `cargo fmt --check` 已通过。
