# 按合并要等会话槽位：定位记录

status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p automed --lib merge_does_not_wait_for_a_session_slot -- --test-threads=1
根因: 用户动作准入以 occupies_slot 判断负载，Merging 被计入会话槽位，满槽时在执行合并前被改成 Requeued；Behind 路径还会显式排队等待 Rebase。
复核: 待复核
repos: autome-v2

## 现象

普通合并不启动模型会话，却被 `[loop] parallel` 限制。在一个实现任务占满 parallel=1 时，对 AwaitMerge 任务调用真实 scheduler::apply_trigger(Merge)，立即得到 `Requeued { resume: Merging }`，Git 合并尚未执行。

任务卡明确修正 T-51 对普通 merge 的旧口径，因此本轮按 Bug 定位，不按“符合旧设计”关闭。

## 复现

当前目录是 autome-v2 单仓库 worktree 根目录（不是包含成员仓库的 workspace），直接执行状态块命令，无须 cd 到成员。测试使用本地临时 Git 仓库和内存账本，不依赖网络、模型或睡眠等待。

回归测试位于 `crates/automed/src/scheduler_gate_queue_tests.rs`：

- `merge_does_not_wait_for_a_session_slot`：建立已提交且基于当前 main 的待合并分支，无门禁、无纳入表态；另一个 Implement 任务占用唯一槽位。调用真实用户动作入口，要求不排队，随后按有限次数驱动真实内核步骤，检查 main 更新、任务完成、没有新增模型会话、原占槽任务状态不变。
- `merge_session_slot_free_capacity_control`：完全相同的场景，只将 parallel 改为 2；已通过，证明测试分支本身可以合并和清理。
- 将旧的 `user_action_slot_merge_waits_for_capacity`（明确要求普通 merge 排队）替换为上述新口径；approve、rerun、resume 的原测试保留。

失败测试两次独立运行均出现断言：`ordinary merge must not wait for a session slot: Requeued { resume: Merging }`，不是超时或编译失败；第二次直接执行 cargo 返回 101。日志为 `.autome/output/locate-repro-1.log` 和 `locate-repro-2.log`。空槽对照日志为 `locate-control.log`。`slots_in_use` 既有不变量测试通过，日志为 `locate-slot-invariant.log`。

测试中的占槽任务用真实账本 Active/Implement 状态表示；没有启动真实模型进程。本轮验证调度及真实本地 Git 合并，不宣称完成 CLI/RPC 展示、真实长会话或门禁复用的端到端验收。

补充验证：`user_action_slot_` 三项通过；`cargo test --offline -p automed --lib merge_ -- --test-threads=1` 共 59 通过、1 失败，唯一失败为本次回归，其中 `deferred_merge_keeps_decisions_for_the_new_session` 验证带纳入表态的 merge 仍排队。日志分别为 `locate-slot-controls.log`、`locate-merge-tests.log`。随后为满槽回归补充“准入后也不能多占槽”的断言，最终版本再次验证回归失败、空槽对照通过（`locate-repro-final.log`、`locate-control-final.log`）。`cargo fmt --all -- --check` 与 `git diff --check` 通过；未运行全仓门禁。

## 根因分析

1. `crates/autome-domain/src/task.rs:835` 的 `press_merge` 在没有纳入表态时返回 Active/Merging + RunCoreStep；有纳入表态时调用 start_impl，仍应遵循会话容量限制。
2. 同文件 `Node::occupies_slot`（117 行）仅排除用户等待节点，因此 Merging、Rebase、Cleanup 和门禁节点都返回 true。`is_core_step`（133 行）虽区分内核节点，却未参与此准入判定。
3. `crates/automed/src/scheduler.rs:1974` 检查旧状态不占槽、新状态占槽且容量已满，便保存 deferred_action，将目标改为 Requeued 并清空执行动作。这里尚未运行 Git 或门禁，排除它们导致本次阻塞的假设。
4. `crates/automed/src/store.rs:1420` 按 TaskState 统计 slots_in_use；`scheduler.rs:1711` 的 fill_slots 在容量满时直接停止取队列，因此合并只能等其他任务释放槽位，且受旧队列顺序影响。
5. 另一个必须保留在修复范围内的入口：`task.rs:1285` 在合并返回 Behind 时显式进入 Requeued/Rebase。即使只放行普通 Merge，落后分支仍会等待会话槽位。Rebase 是无模型的内核步骤；冲突后转 Implement 则需恢复会话准入。本轮未新增 Behind 的动态回归，此项为源码定位。

门禁现状（本任务不改变其负载口径）：Rebase 成功且配置门禁时进入 MergeGate；合并成功且配置门禁时进入 MainCheck，二者当前均占 loop 槽位。实际执行门禁还经过 `scheduler.rs:3686` 的 gate_queue::admit，另受 `[gate] concurrency` 限制。MainCheck 符合复用条件时在 `scheduler.rs:3563` 附近直接记 gate.reused 并推进；无门禁时直接推进；无法复用时创建验证检出并实际执行门禁。修复不能把所有 is_core_step 一概当作无负载，否则会同时改变门禁口径。

后续修复还需保持 approve、rerun、带纳入表态的 merge 的排队语义，以及 slots_in_use 不变量；不能只绕过一次准入，却继续让瞬时内核节点污染占槽统计。T-70 合并临界区是另一个等待原因，本轮不实现该锁。

## 修复说明

定位轮未修改产品代码。回归测试保持失败，交由后续修复轮处理；预算为 0/3。

## 复核意见

待独立复核。
