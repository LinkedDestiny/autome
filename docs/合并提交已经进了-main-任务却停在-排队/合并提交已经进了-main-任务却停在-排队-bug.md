# 合并后的可复用验证被会话槽位阻塞
status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p automed --lib merge_tail_reaches_done_without_a_session_slot -- --test-threads=1
根因: Merging 不占会话槽位而 MainCheck 占槽，合并成功后的统一准入先将 MainCheck 改为 Requeued，尚未执行无负载的门禁结果复用判断。
复核: 待复核
repos: autome-v2

## 现象

parallel=1 且另一个 Implement 任务占满槽位时，目标任务已成功合并到 main，但状态为 `Requeued { resume: MainCheck }`，不能进入 Cleanup 和 Done。任务卡中的现场提交仅作为背景；本轮在本地临时 Git 仓库重建同类场景。

## 复现

当前目录为 autome-v2 单仓库 worktree 根目录，不是包含成员仓库的 workspace，因此状态块命令直接从当前根目录执行，无需 cd。

新增 `crates/automed/src/scheduler_gate_queue_tests.rs` 中两个测试，共用同一场景：

- `merge_tail_reaches_done_without_a_session_slot`：配置 gate.command=true，保持 reuse_for_main_check 缺省值；真实执行合并前门禁至 AwaitMerge，再让另一个 Implement 任务占用唯一槽位。通过真实 apply_trigger(Merge) 和有限次 run_core_step 推进，先断言 main HEAD 等于记录的合并提交且 main_check_reuse 确认可复用，再要求 Done、完成时间、worktree 和分支清理、复用事件、没有额外门禁启动或验证检出、无模型会话及原占槽任务不变。
- `merge_tail_free_session_capacity_control`：只把 parallel 改为 2，其余相同；通过，证明门禁证据、Git 合并及清理夹具有效。

回归两次运行均返回 101，失败为状态断言：实际 `Requeued { resume: MainCheck }`，期望 `Done`。首次两个测试合跑为 1 通过、1 失败；第二次使用状态块原命令仍稳定失败。日志分别为 `.autome/output/locate-tail.log`、`.autome/output/locate-repro-2.log`。不是编译失败、超时或命令不可执行。

测试只用本地临时 Git 仓库、账本和 true 门禁，不依赖网络或真实模型；门禁完成使用既有有界轮询，核心状态推进不依赖睡眠或其他任务释放槽位。并发占用用真实账本 Active/Implement 表示。

## 根因分析

1. `crates/autome-domain/src/task.rs` 的 Node::occupies_slot 排除 Merging 和 Cleanup，但没有排除 MainCheck。is_core_step 虽包含 MainCheck，却不代表该节点不占槽。
2. `crates/automed/src/scheduler.rs` 的 apply_trigger（约 1979 行）在旧状态不占槽、新状态占槽且容量满时保存 deferred_action，并将目标状态改成 Requeued、清空动作。Merging 成功转入 MainCheck 恰好满足条件。
3. 同文件 run_gate（约 3571 行）才检查 reuse_for_main_check、验证检出/job 是否已存在，以及 main_check_reuse；符合条件只记录 gate.reused 并推进，无需启动命令。准入先于此判断，因此无法到达这条轻量路径。
4. fill_slots_reporting_errors 按 parallel 限制取队列，满槽时不会恢复 MainCheck。Cleanup 本身已不占槽；它及后续 after 钩子的延迟是上游被拦截的后果。
5. 排除其他假设：测试在最终状态断言之前确认合并提交已成为 main HEAD，且真实复用判定返回 Some；空槽对照能完成，排除合并失败、树不同或清理夹具异常。T-73 的原回归实际上会推进到完成，但没有配置 gate，因此不经过 MainCheck，遗漏的是有门禁且可复用的尾段。

已核对 T-73 定位及修复/复核记录（`docs/按合并要等会话槽位-合并只是秒级的内核步骤/按合并要等会话槽位-合并只是秒级的内核步骤-bug.md`）：其明确保留 MergeGate/MainCheck 实际门禁占用 loop 槽位，实际启动还受 gate_queue::admit 的 `[gate] concurrency` 限制。本任务应仅让可复用的无命令尾段绕过会话准入；不能简单将所有 MainCheck/is_core_step 从占槽统计中删除。不可复用、关闭复用或无可沿用结果的真实门禁仍应遵循这两层限制，并按任务卡要求明确显示等门禁。该显示要求本轮仅记录，未增加 UI 验收。

## 修复说明

定位轮未修改产品代码。后续修复须保留真实门禁准入、会启动会话的用户动作排队及 slots_in_use 不变量；清理不等会话槽位。不得弱化本轮回归。

验证通过：cargo fmt --all -- --check、git diff --check；slots_in_use 1 项、user_action_slot_ 3 项、merge_core_steps_reacquire_capacity_for_gates 1 项均通过，日志见 .autome/output/locate-{fmt,slots,actions,gates}.log。

本轮未配置 merge.after 钩子，因此不宣称已动态验收钩子执行；后续修复须按原规则验证。未运行全仓门禁或真实 CLI/面板端到端验收。

## 复核意见

待独立复核。
