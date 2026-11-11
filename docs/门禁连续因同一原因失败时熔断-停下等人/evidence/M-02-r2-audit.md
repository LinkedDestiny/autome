# M-02 · 审计 #2 独立复验

- 负向对照（均注入后还原，工作区干净）：
  1. 熔断分支先查实现预算（budget_n==0 时报 ImplBudget）→ `same_failure_takes_priority_over_exhausted_budget` 变红，其余 12 过。
  2. 重跑门禁给 `RunCoreStep` 而非 `DiscardGateRuns` → `same_failure_rerun_discards_gate_runs` 变红。
  3. extend 忽略追加数（`checked_add(0)`）→ `same_failure_extend_adds_budget_and_discards_runs` 变红。
- 验收命令：`cargo test -p autome-domain same_failure` 13/13；`cargo test -p autome-domain` 库 374 全过。
- 已有测试：相对 main 在 crates 下仅删 1 行（`task.rs` 非测试代码里 ExtendBudget 的匹配臂拆分），无测试被改。
- Bug Loop 路径代码走读：FixGate/MergeGate 熔断、同节点 rerun、extend 均给 DiscardGateRuns，与设计一致。
- `cargo test --workspace` 全量通过（domain 374、automed 606、end_to_end 80、cli 19+11、prompt_rendering 16、real_cli 7）。
- 结论：通过，无实现缺陷、无验证缺口。
