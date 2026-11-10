# M-02 独立复验（审计 #2）

结论：通过。未读实现轮证据，自行复验。

## 验收命令
- `automed --lib requeued` 7 通过；`end_to_end resumes_at` 5 通过；`end_to_end` 全量 43 通过（含未改的 `stopping_a_task_leaves_it_resumable`）。
- `cargo test --workspace` 全绿（autome-domain 288、automed lib 541、e2e 43 等）；clippy `-D warnings`、`fmt --check` 无错误。

## 负向对照（逐个注入 scheduler.rs，复验后 `git checkout` 还原）
| 注入 | 失败的用例 |
|---|---|
| `Reenter` 传 `None` inject | lib 槽满/留存 2 个；e2e `stopped_gate_feedback_resumes_at_implementation_with_original_prompt` |
| `DiscardGateRuns` 空操作 | lib `requeued_stopped_gate_discards_only_its_own_jobs`；e2e `stopped_gate_resumes_at_gate_and_discards_killed_result` |
| `fill_slots` 不检查并行上限 | lib `requeued_waits_for_slot_then_reenters...`；e2e `stopped_task_resumes_at_original_node_after_waiting_for_slot` |
| 不记 `task.resume_input` 事件 | lib `requeued_preserves_pending_input_and_expires...` |
| 额外：`Reenter` 的 kind 恒为 Intake | e2e `stopped_gate_feedback...` 失败 |

五种错误实现都被拦住，验收有判别力。工作区已还原，无产品缺陷、无验证缺口。
