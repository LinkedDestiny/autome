# M-03 r3 审计复验

- 验收命令 1（lib）：8 个通过（新增 4 + 现有 4），与契约预期一致。
- 验收命令 2（e2e）：4 个通过（新增 2 + `bug_loop_before_merge` 2），与预期一致。
- 负向对照 1：FixGate 的快检分支禁用（`false && bug && FixGate`）→ lib 3 个失败（`runs_repro_and_quick_under_the_full_timeout`、`mode_switch_collects_by_launch_kind`、`result_never_stands_in_for_the_merge_gate`），e2e 2 个全失败。已还原。
- 负向对照 2：去掉 MergeGate 沿用处的 `last["kind"] != "quick"` → `gate_quick_bug_fix_gate_result_never_stands_in_for_the_merge_gate` 失败。已还原（`git status` 干净）。
  （第一次用 BSD sed 注入失败、实际未改动，该次 4 个全过不算对照；改用 python 重做。）
- `cargo fmt --all -- --check` 退出码 0；`cargo clippy --all-targets -- -D warnings` 通过。
- 结论：通过。
