# M-04 独立复验（审计 #4）

结论：通过。

- 验收命令 `cargo test -p autome-cli task_card`：单元 4 passed、`tests/cli.rs` 4 passed，合计 8，0 failed。
- 负向对照 1：`VALUED` 去掉 `--brief` → `args::task_card::brief_and_kind_take_values_and_a_dash_is_a_value` 失败（验收命令会拦住）。
- 负向对照 2：`reply` 改映射到 `task.approve` → `task_card_reply_rejects_the_design_with_the_text_verbatim` 失败。
- 两处均已还原（与备份 cmp 一致，工作区无源码改动）。
- 自查：`autome-cli` 全量 17 + 8 passed；clippy `-D warnings` 与 `fmt --check` 通过；无需求无 `--brief` → 用法错误、退出码 2；
  不带新 flag 时 `task.create` 参数不变（读 main.rs 确认）；非 UTF-8 分支读源码确认（需已注册项目才可达，未单独起沙箱跑，不影响结论）。
- 无人工验收项。
