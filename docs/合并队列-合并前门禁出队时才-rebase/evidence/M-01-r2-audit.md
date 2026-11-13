# M-01 审计复验（实现轮 r2）· 通过

## 验收命令（自己跑）
`cargo test -p autome-domain merge_queue_ && cargo test -p automed --lib merge_queue_ && cargo test -p automed --test end_to_end merge_queue_` → 退出码 0；domain 4、lib 11、e2e 4，共 19，与契约一致。

## 负向对照（注入后已完全还原，`git status` 干净）
- A：`enter` 里让 `waiting_for` 恒为 None（去掉锁检查）→ e2e 3 个失败：`three_auto_merging_tasks_run_the_gate_exactly_three_times`、`the_section_and_the_order_survive_a_core_restart`、`with_two_gate_slots_one_project_still_merges_one_at_a_time`。验收命令会拦住。
- B：`Merging` 等锁分支加 `&& false`（去掉等锁）→ `a_press_during_the_section_waits_and_continues_after_a_gate_failure` 失败（cases.inc:182）。验收命令会拦住。
日志：`.autome/output/negA.log`、`negB.log`（不入库）。

## 结论
验收命令能区分两种典型错误实现，且当前实现通过。未发现产品行为缺陷。快检由宿主报告通过。
