# M-01 r1 审计复验

- 验收命令 `cargo test -p automed --lib events_since && cargo test -p autome-cli`：automed 4、autome-cli 单元 28、集成 17，全绿，与契约预期数一致。
- fmt、clippy `-D warnings`（autome-cli、automed）通过。
- 负向对照（临时改 main.rs，跑后用备份还原，工作区无残留）：
  - `time` 改用 `clock()` → 单元 `run_event_time_is_stored_at` 红。
  - JSON 分支丢弃 `task.head` → 集成 `run_json_prints_every_event_as_one_json_line` 与 tick 错误用例红。
  - `task_id` 恒取 `subject` → 单元 `run_event_project_preserves_subject` 红。
  - `print_json` 多行：由集成用例逐行解析把关，代码审读确认 JSON 分支只用 `println!("{}", json!(..))` 单行。
- 代码审读：非 `--json` 路径未改；`--json` 下 stderr 三条提示均不打；收尾行仅在 idle 结束时输出。
- 结论：通过，无缺陷、无验证缺口。
