# M-04 审计 #4 复验

验收命令（全部通过）：
- `cargo test -p automed queue_move_`：lib 6 + end_to_end 1 = 7，与契约预期一致。
- `cargo test -p automed task_start_queues_a_draft`：1 通过。
- `cargo test -p automed --test end_to_end task_card_a_started_draft_waits_behind_the_parallel_limit`：1 通过。

负向对照（逐条注入，跑 `queue_move_`，随后还原；`git status` 干净）：
| 注入 | 失败用例 |
|---|---|
| task_move 只传被移动任务给 reorder_queue（不重编号） | positions_and_event、draft_keeps_position_on_start、unpositioned_draft_reference；e2e third_task_moved_to_top 也失败 |
| task_start 无条件 mark_queued | draft_keeps_position_on_start、unpositioned_draft_reference |
| reorder_queue 额外重编号其它项目 | other_project_unchanged |
| 参照为 Draft 时拒绝 | unpositioned_draft_reference |

结论：通过。M-04 标 已完成，next 推进 M-05。
