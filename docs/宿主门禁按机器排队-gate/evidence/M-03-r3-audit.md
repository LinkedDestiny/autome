# M-03 独立复验（审计 #3，对应实现轮 r3）

结论：通过。

- 验收命令：`cargo test -p automed --lib` 679 通过（契约 676+3）；`autome-cli` 28+14=42 通过（基线 39+3）；`dispatch::tests::gate_queue_` 3 个、`autome-cli gate_queue_` 3 个单独复跑通过。全量 `cargo test --workspace`、clippy `-D warnings`、`fmt --check` 干净。
- 负向对照（先做，注入后已还原，工作区干净）：
  | 注入 | 结果 |
  |---|---|
  | `task.get` 的 `queue_position` 改写成排队名次（dispatch.rs 的 task_get） | `gate_queue_task_get_reports_live_rank_without_reusing_slot_position` 失败 |
  | `gate_queue_line` 直接用 position 当 k | `gate_queue_line_counts_only_waiters_ahead` 失败 |
  （第一次注入误改到 task_json 的同名行，用例未失败；那不是 task_get，随即改注入正确位置后失败，已确认。）
- 读代码核对：`task.get` / `dashboard.get` 用 `gate_queue::waiters` 现算，按 `gate.queued` 的 seq 排序，名次按事件而非任务创建序；CLI `status`、`status <id>`、`run`（`event_line("gate.queued")`）三处都接了。`gate.queue_dropped` 与未知事件不输出。
- 无产品行为缺陷、无验证缺口。判别检查为一次性变异，不留存。
