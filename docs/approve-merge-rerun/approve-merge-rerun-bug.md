# approve / merge / rerun 绕过并行容量检查

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --lib user_action_slot_ -- --test-threads=1
根因: 用户触发经 apply_trigger/advance 直接将不占槽任务迁移到 Active 并执行动作，未经过 fill_slots 的容量准入；随后 tick 不能撤回已产生的超额占槽。
复核: 待复核
repos: autome-v2

## 现象

基线 `c5f4cb448a7c54c66859ee89fa37160b8f017cc3`。配置 `parallel=1`，一个实现任务已占槽时，对另一个任务执行 approve、rerun 或 merge，实际占槽数均由 1 变为 2。

approve / rerun 已登记新的实现会话；merge 先进入占槽的 `Active { Merging }`，内核动作在后续 tick 执行。不能把 merge 描述成直接启动模型会话。resume 对照保持占槽数 1，目标先进入 `Requeued { Implement }`，释放槽位后才启动。

任务卡中的 11-12 生产现场 `parallel=3 / slots_in_use=6` 未访问历史运行现场验证；本轮稳定复现了同一容量绕过机制，不把测试当作历史现场证据。

## 复现

在本 worktree 根目录执行状态块命令。本仓库本身是 Rust workspace，实际代码仓为 `autome-v2`，成员 crate 为 `automed`；无需切换到外部成员目录，Cargo 通过 `-p automed` 选择成员。

回归测试追加在 `crates/automed/src/scheduler_gate_queue_tests.rs`，由既有 `scheduler::tests` 引入，未修改产品逻辑。三个缺陷用例及一个 resume 对照共用相同夹具：

1. 真实临时 Git 仓库、worktree、内存 Store、项目配置；设 `parallel=1`，固定一个 `Active Implement` 任务占槽。先确认统计为 1 且 `fill_slots` 不放行。
2. 目标分别处于 AwaitDesignApproval、Stopped Implement、AwaitMerge；调用生产 IPC 使用的 `scheduler::apply_trigger`，在后续 tick 前检查容量不变量。
3. 期望仍为 1、目标进入 Requeued、没有新会话、满槽时不可放行。
4. 释放原任务槽位，期望目标被 `fill_slots` 唯一放行，恢复正确节点；实现节点恰有一个会话。

测试使用既有 dry launcher，不启动真实模型或门禁进程；不依赖网络、sleep、退出时机。占槽任务用状态固定，符合 `slots_in_use` 按任务状态计算的生产契约。Git 缺失会明确失败，不会静默跳过。这是调度器集成回归，不声称完成真实 CLI / 模型 / 合并门禁 e2e 验收。

两次实跑均退出 101：**1 passed / 3 failed / 0 ignored**。失败均为业务断言 `left: 2, right: 1`，分别对应 approve、merge、rerun；resume 完整通过排队与释放后继续的断言。首次耗时 7.79 秒，重复运行 7.94 秒（不含编译）。

原始日志：`.autome/output/approve-merge-rerun-regression.log`、`.autome/output/approve-merge-rerun-regression-repeat.log`。`git diff --check` 通过；未跑全量门禁，本轮要求保留失败回归供宿主复现。

## 根因分析

- `crates/automed/src/dispatch.rs:1892`：`task_trigger` 先调用 `apply_trigger`，再调用 `tick`。
- `crates/automed/src/scheduler.rs:1618`：`apply_trigger` 直接进入 `advance`。后者读取配置、计算状态迁移、持久化新状态并执行动作，没有统一的重新占槽准入检查。
- `crates/autome-domain/src/task.rs:1200`：Approve 直接返回 Active Implement + StartRole；`:1027` 的 RerunFrom 同样直接进入指定角色节点；`:835` 的 press_merge 在无新纳入项时返回 Active Merging + RunCoreStep。
- `crates/automed/src/scheduler.rs:1573`：容量限制仅在 `fill_slots_reporting_errors` 遍历 Queued/Requeued 时检查。用户触发已把任务移出该队列，因此之后的 tick 无法阻止先前发生的超额占槽。
- `crates/automed/src/store.rs:1386` 按 `occupies_slot()` 统计任务状态；测试中迁移前为 1、迁移后为 2，排除了单纯 UI 统计错误。无需恢复流程或并发竞态即可复现，排除“必须重复恢复才超额”的解释。
- `crates/autome-domain/src/task.rs:933` 的 Resume 已返回 Requeued，本轮对照实跑通过，不能笼统归为所有 resume 都绕过容量。

相邻路径静态检查：feature ExtendBudget（`task.rs:990`）、Bug ExtendBudget / RerunFrom / Reply（`bug_loop.rs:108-172`）也存在直接进入活动节点的分支，修复轮应按任务卡核对；本轮未给这些分支声明运行验收通过。

K0b 文档核对：`docs/用户按合并时如果默认分支已经前进-任务自动/用户按合并时如果默认分支已经前进-任务自动.md:21` 明确记载当时按合并“不检查槽位”。已有 `a_merge_behind_main_is_requeued_for_a_rebase_and_main_is_untouched` 覆盖的是进入 Merging 后发现主分支前进，再转 Requeued Rebase；不证明首次按合并已受容量保护。任务卡所称 K0b 的普遍“按合并无空槽即排队”不能当作已实现事实；本任务卡明确要求所有重新占槽动作遵守上限，当前实现确实不满足。

## 修复说明

第 1/3 轮在 `scheduler::advance` 统一检查从不占槽到占槽的迁移。项目已满槽时保存原动作并转为 `Requeued { resume }`，不执行启动动作；`SlotAvailable` 放行时恢复原动作。已有占槽任务内部流转不重复申请槽位，有空槽的直接启动行为保持不变。

原动作通过任务事件保存，保留新会话启动、决策注入及门禁记录清理语义，避免统一改成 Reenter 后误恢复旧会话。预算仍在用户触发时更新；排队不提前消费决策，完成放行或退出排队后清除待执行动作。该准入按状态迁移判断，覆盖 approve、merge、rerun 以及 extend/reply 的重新占槽分支。

定位轮回归文件未修改。新增 `deferred_merge_keeps_decisions_for_the_new_session`，验证满槽合并先排队、决策未提前消费，释放槽位后由新实现会话携带决策。

验证结果：修复前同一复现命令为 1 passed / 3 failed；最终产品代码修复后为 **4 passed / 0 failed / 0 ignored**。新增决策测试 **1 passed / 0 failed**。原始日志分别为 `.autome/output/fix-1-before.log`、`.autome/output/fix-1-after-final.log`、`.autome/output/fix-1-decisions.log`。

相关子集 `cargo test --offline -p automed --lib scheduler::tests -- --test-threads=1`：**118 passed / 0 failed / 0 ignored**，日志 `.autome/output/fix-1-scheduler.log`。该子集启动后追加的上述决策测试另行实跑通过；子集已使用最终产品逻辑。`cargo fmt --all -- --check`、`git diff --check` 通过。未运行 `sh scripts/ci-local.sh`，由宿主合并前执行。真实 CLI/模型 e2e、parallel=3 累计场景和 extend/reply 满槽专项尚未实跑，不以统一准入的静态覆盖替代验收。

## 复核意见

待独立复核。真实 HTTP/CLI e2e、parallel=3 的多动作累计、extend/reply 及带 rebase/门禁的 merge 验收仍需后续补充。
