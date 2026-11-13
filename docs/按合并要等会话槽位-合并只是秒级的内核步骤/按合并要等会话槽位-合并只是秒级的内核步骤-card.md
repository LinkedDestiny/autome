# 按合并要等会话槽位：合并只是秒级的内核步骤，却排在别的任务的会话后面，可能等几十分钟

## 现象

T-51 合并后，`autome merge <任务>` 在并行槽位占满时把任务放进排队（`排队（会从 合并中 继续）`），要等别的任务让出槽位才真正合并。2026-11-13 07:39 的现场：

- `[loop] parallel = 3`，T-64（修复）、T-71（定位）、T-72（定位）三个会话占满槽位；
- T-63 停在「等你合并」，合并前门禁已通过、分支就在最新 main 之上；
- `autome merge T-63` → `T-63  排队（会从 合并中 继续）`。main 没有动。三个会话各自还要跑十几到几十分钟，T-63 的合并要等其中一个任务整个结束或停下。
- 同时排队的 T-67（`排队（会从 rebase 继续）`）排队序号更靠前，槽位一空先轮到它 rebase 并跑合并前门禁，T-63 继续等；等 T-63 终于合并，T-67 的门禁结果又过期了。

操作者只能手工改账本的排队序号再暂停一个在跑的任务来腾槽位。

## 复现步骤

1. 项目配 `parallel = 1`；任务 A 处于实现会话中（占 1 个槽）；任务 B 停在 `AwaitMerge`，分支在默认分支之上，（配了门禁则）合并前门禁已通过。
2. 对 B 执行 `autome merge`（或 RPC `task.merge`）。
3. 看 B 的状态与默认分支。

## 期望行为

- B 立即进入合并并完成（`Merging` →（`MainCheck` 沿用或无门禁）→ `Cleanup` → 已完成），默认分支上出现合并提交；A 的会话不受影响。合并这几步不启动模型会话，执行时间是秒级，不应受 `[loop] parallel`（它限制的是同时推进的会话负载）约束，也不应把别的任务挤出槽位。
- 合并发现分支落后（`Behind`）需要回去 rebase 并再跑合并前门禁时：rebase 本身同样不等会话槽位；之后的合并前门禁是否占槽位，保持 T-51 现在的口径（门禁是重负载，另有 `[gate] concurrency` 排队），不在本任务里改，但要在 Bug 文档里写明现状。
- `MainCheck` 需要真正跑门禁（内容不同、不能沿用）时的口径同上：按现状处理并写明。
- T-51 修好的行为不回退：approve、rerun（进入会话节点的）、以及会启动新实现会话的 merge（有「纳入」表态时 `press_merge` 回实现轮）在满槽时仍然排队；`slots_in_use` 的不变量测试继续通过。
- `autome status` / `task.get` 对「已按合并」的任务不再出现「排队（会从 合并中 继续）」这种状态；如果因为别的原因确实要等（例如将来 T-70 的合并临界区），显示的原因要说清楚等的是什么。

## 实际行为

见现象。`crates/automed/src/scheduler.rs` 里 T-51 加的用户动作准入按「目标状态是否占槽」判断，`Active { Merging }` 被算作占槽，于是满槽时 merge 被改成 `Requeued { resume: Merging }`。

## 环境

main `db99b44`；`crates/automed/src/scheduler.rs`（T-51 的准入与 `slots_in_use`）、`crates/autome-domain/src/task.rs`（`press_merge`、`Node::is_core_step`）、`crates/automed/src/scheduler_gate_queue_tests.rs`（T-51 的回归测试 `user_action_slot_*`）。

## 线索（可选）

- T-51 的任务卡是按「所有用户动作满槽都排队」写的，其中把 merge 也算进去是任务卡的疏漏：merge 的常见路径不启动会话。修的时候以「是否会启动会话 / 重负载」为准，而不是「是不是用户动作」。
- 与 T-70（合并队列，设计待批准）的关系：T-70 要加「项目合并临界区」，届时合并要等的是锁而不是会话槽位。两者改同一段准入代码，后合并的一方 rebase 时要保留对方的行为；本任务只处理「合并尾段不等会话槽位」。
- 回归测试可以加在 `crates/automed/src/scheduler_gate_queue_tests.rs` 旁边或 `crates/automed/tests/end_to_end.rs`；复现命令用仓库根目录可直接执行的形式（如 `cargo test --offline -p automed --lib merge_does_not_wait_for_a_session_slot -- --test-threads=1`）。
