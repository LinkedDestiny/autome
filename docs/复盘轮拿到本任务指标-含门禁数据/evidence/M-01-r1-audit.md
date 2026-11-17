# M-01 r1 审计复验

结论：通过。

## 验收命令（独立运行）
- `autome-domain metrics::` 18 通过；`automed gate_metrics` 9；`task_metrics` 19；`version_page` 12。数量与验收契约一致。

## 负向对照（均已还原，`git status` 干净）
1. `merge_gate_runs` 的 `value()` 改 `unwrap_or(0)`：`gate_metrics_old_json_preserves_absence`、`gate_metrics_old_json_does_not_dilute_mean` 失败。
2. `gate_counts` 把 `gate.reused` 当结果：`gate_metrics_reused_skipped_are_not_attempts` 失败。
3. 按 `kind` 而非 `node` 分组：group_by_node、failures_include_timeouts、scheduler 的 C1 三条失败。
4. 另造：`write_task_metrics` 取事件时多取 `gate.reused`，并让 `gate_counts` 计它：T3 与 C1（`gate_metrics_measure_without_writing_then_persist`）都失败。只改 `gate_counts` 时 C1 不失败，因为 `timed_events` 已在源头过滤——这是分层过滤，不是缺口。

## 对照代码
- 口径：node ∈ {gate, fix_gate} 为每轮，merge_gate 为合并前，repro_check/main_check 只计耗时；超时 `ok=false` 计失败；未配对 started 不计；耗时按 node+log 配对，倒挂按 0，任一侧 `at_secs` 为 None 不计。符合 A-Q4。
- `measure_task` 只算不写；`write_task_metrics` 经它再 `set_task_metrics` + `task.metrics` 事件，C1 断言 measure 后 `tasks.metrics` 为空且事件数不变。
- 旧 JSON 缺字段读为 None，序列化不出新键；`Some(0)` 读为 `Some(0.0)`。
- `timed_events` 对 ISO 与 `YYYY-MM-DD HH:MM:SS` 都解析，非法时间为 None。

## 范围
未跑全量门禁（宿主合并前负责）。无实现缺陷；无人工验收项。
