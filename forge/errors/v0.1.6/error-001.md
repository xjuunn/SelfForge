# 错误信息

首次执行 `cargo fmt --check` 时，`src/app/minimal_loop.rs` 的导入列表格式不符合 `rustfmt` 默认规则。

# 出现阶段

格式化检查阶段。

# 原因分析

新增应用层模块时，导入列表采用了多行写法，但 `rustfmt` 判断该导入列表应收敛为单行。

# 解决方案

执行 `cargo fmt`，统一修正格式。

# 是否已解决

已解决。后续 `cargo fmt --check` 通过。
