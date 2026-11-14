# M-03 · 审计复验（实现轮 #3，审计 #3）

结论：**通过**。

## 验收命令（自己跑）
- `cargo test -p automed --lib gate_report`：6 个全过（gate.rs 4 + store.rs 2）。
- `cargo test -p automed --test end_to_end gate_steps_`：2 个全过。
- `vitest run test/task-timeline.test.ts`：12 个（基线 10 + 2），全过。
- 宿主快检在 1349131e 通过，仅作引用。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 期望失败 | 结果 |
|---|---|---|
| 快照去掉「合并所依据的最后一次通过门禁」子查询（发布全部历史 passes） | gate_report ③/⑥ | 两个 store 用例红 ✔ |
| `partial` 也记入 passes | gate_report ① | `counts_only_complete_passes` 红 ✔ |
| 时间线把步骤摘要改回「沿用已通过的结果」 | 新增时间线用例 | 1 failed / 11 passed ✔ |

## 自造判别检查
- 读 diff 核对接线：`run_gate_attempt` 先 `gate_passes` 再 `start_with_passes` 写快照并设 `AUTOME_GATE_PASSES/REPORT`；结束后 `read_report` 才写 `steps/passes`，无报告不加字段（e2e ② 覆盖）；`gate_queue` 的 Job 路径含 `.passes/.report`，`clear/discard_stale` 清理有用例。
- 未合并任务、7 天前、快检/失败/sha 不符的 passes 不外发，由 store 用例的 foreign/open/wrong 夹具与上述注入对照共同证明。
- 结果：未发现产品缺陷，无新增临时检查需保留。
