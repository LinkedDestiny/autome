# M-02 · 审计 #2 独立复验

结论：通过。

## 负向对照（先于验收命令；均已还原，工作区无残留）
1. `task.rs:1152` 改回 `gate.is_some()`：`feature_before_merge_a_failing_merge_gate…`、`…two_milestones…` 失败（①②），符合契约。
2. `run_gate` 的 §3 跳过段加 `if false`：`…switching_while_blocked…`、`…switching_while_stopped…` 失败（③④），符合契约。
3. 自造：去掉跳过段里的 `gate_run == BeforeMerge` 条件：`each_round` 下 8 个既有门禁 e2e 加 ①~④ 共 12 个失败——跳过段只在 before_merge 生效被钉住。

## 验收
- `cargo test -p automed --test end_to_end gate`：15 通过（基线 11 + 4 个 `feature_before_merge_*`），与预期一致。
- 全量 `cargo test --workspace`：domain 390、automed lib 655、end_to_end 116、prompt_rendering 16，全绿，无失败。
- 无实现缺陷、无验证缺口；自造检查为一次性变异，不留存。
