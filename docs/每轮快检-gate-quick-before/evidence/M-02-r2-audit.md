# M-02 r2 审计复验

- 验收命令 1（lib）：19 个通过（新增 10 + 现有 9），与预期一致。
- 验收命令 2（end_to_end）：8 个通过（新增 4 + `feature_before_merge` 4），与预期一致。
- 负向对照（临时注入 `scheduler.rs`，每次跑完用备份还原，`git status` 干净）：
  - 去掉 MergeGate 沿用判断里的 `last["kind"] != "quick"` → `gate_quick_host_merge_gate_never_reuses_a_quick_result` 失败。
  - 收结果不读 `.kind` 标记（`job.launch()` 恒为 None）→ `gate_quick_host_mode_switch_collects_by_launch_kind` 等 3 个失败。
  - 去掉跳过条件里的 `quick_gate().is_none()` → 3 个 `gate_quick_feature_` e2e 失败（快检未运行）。
- 自查：`cargo fmt --all -- --check` 退出码 0；`cargo clippy --all-targets -- -D warnings` 退出码 0。
- 结论：通过。未造新的判别检查（负向对照即判别检查）。
