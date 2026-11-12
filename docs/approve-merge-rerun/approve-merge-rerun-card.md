## 现象
`[loop] parallel = 3`，但 11-12 下午 `project.list` 报 autome-v2 `slots_in_use = 6`：T-33（合并中）、T-35（实现）、T-37（设计）、T-39（修复门禁）、T-40（设计）、T-41（设计）同时占槽。并行上限只在 `scheduler.rs::fill_slots`（约 1456 行）从队列启动任务时检查；`task.approve`、`task.merge`、`task.rerun_from`、`task.resume`（`dispatch.rs:390-425` → `task_trigger` → `advance`）直接启动会话或内核步骤，不看槽位。操作者一轮 approve/rerun/merge 下来，实际并行远超配置，门禁与会话互相抢 CPU（见 T-47 的超时证据）。
## 复现步骤
1. `parallel = 1`，一个任务在跑实现。
2. 另一个任务停在「等你批准设计」，执行 `autome approve`。
3. 第二个任务立刻启动实现会话，`slots_in_use = 2`。`rerun <t> <node>`、`merge`（rebase/门禁/合并步骤）同理。
## 期望行为
- 所有会让任务重新占槽的用户动作（approve、merge 触发的 rebase/合并前门禁、rerun、resume、extend 后继续、reply 后重定位）在没有空槽时进入 `Requeued { resume }` 排队，由 `fill_slots` 按顺序放行；界面/CLI 显示「排队（会从 <节点> 继续）」。`resume` 已有这条路径（`Requeued`，`dispatch.rs:1550/1889` 与 e2e `requeued_waits_for_slot_then_reenters_and_rejects_stop_without_killing`），其余动作对齐它。
- K0b 已定义「按合并时没有空槽则排队」的语义，核对 `task.merge` 是否真的检查了槽位；若没有，一并修。
- 不占槽的状态（等人节点、失败、暂停）不变。
- e2e：`parallel = 1` 下 approve / rerun / merge 各一条，断言先排队、空槽后继续；`slots_in_use` 永不超过上限（可在 `tick` 末尾加一条 debug 断言或事件）。
## 环境
main，`crates/automed/src/dispatch.rs`（`task_trigger`）、`crates/automed/src/scheduler.rs`（`advance`、`fill_slots`、`redispatch`）、`crates/autome-domain/src/task.rs`（`Requeued`）。
