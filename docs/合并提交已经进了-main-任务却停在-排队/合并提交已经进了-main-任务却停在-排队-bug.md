# 合并后的可复用验证被会话槽位阻塞
status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --lib merge_tail_reaches_done_without_a_session_slot -- --test-threads=1
根因: Merging 不占会话槽位而 MainCheck 占槽，合并成功后的统一准入先将 MainCheck 改为 Requeued，尚未执行无负载的门禁结果复用判断。
复核: 通过
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

修复第 1/3 轮：在 `advance` 的统一槽位准入前，使用与 `run_gate` 共用的 `reusable_main_check` 检查复用开关、单仓库合并提交、验证检出/job 是否存在，以及原有提交父子关系、树、实际命令和成功门禁证据。满足条件时记录 `gate.reused`，通过原状态机的 MainCheck/GateDone(Passed) 转移进入 Cleanup，不持久化超容量的 Active/MainCheck，也不启动额外命令或会话。

真实门禁仍按原规则占用 loop 槽位并遵循 gate concurrency；未改变 Node::occupies_slot、slots_in_use、清理和 after 钩子的执行规则。原定位回归测试保持不变，新增关闭复用、门禁命令变化两种满槽负向场景，确认 main 已合并但真实 MainCheck 仍排队、不启动门禁且保留 worktree。

验证结果：

- 原复现命令修复前返回 101，实际为 Requeued/MainCheck；修复后原命令通过（1 项），日志 `.autome/output/fix-before.log`、`.autome/output/fix-repro-final.log`。
- `merge_tail_` 3 项通过；`main_check_reuse` 4 项通过；`merge_core_steps_reacquire_capacity_for_gates` 1 项、`user_action_slot_` 3 项、`slots_in_use` 1 项通过。
- `gate_queue_` 24 项、`merge_after_` 10 项通过，覆盖既有门禁准入与合并后钩子规则。测试均以 `cargo test --offline -p automed --lib <过滤名> -- --test-threads=1` 执行，日志在 `.autome/output/fix-*.log`。
- `cargo fmt --all -- --check`、`cargo clippy --offline --all-targets -- -D warnings`、`git diff --check` 通过。

未运行全量门禁 `sh scripts/ci-local.sh`；前端 lint/typecheck 留给宿主快检。本轮未改 UI，未进行真实 CLI/面板端到端验收；合并后钩子使用既有专项测试验证，不宣称已覆盖可复用满槽尾段与钩子的组合端到端场景。

## 复核意见

独立复核通过（第 1/3 轮）。
- 差异检查：定位提交 8528c26 到修复提交 270ce52，回归测试文件只有新增行，无删除、跳过或弱化；原两个测试未改，另加关闭复用、门禁命令变化两个满槽负向场景。
- 复验：原复现命令及 `merge_tail_`（3 项）、`main_check_reuse`（4 项）、`merge_core_steps_reacquire`（1 项）均通过。
- 根因：Merging 不占槽而 MainCheck 占槽，准入先于 run_gate 的复用判断，分析与代码一致。
- 范围：仅 scheduler.rs 抽出 `reusable_main_check` 供准入与 run_gate 共用（条件相同），可复用时在准入前走 GateDone(Passed)；不可复用的真实门禁仍占槽并排队，未改 occupies_slot。范围最小。
- 未运行全量门禁，由宿主合并前运行。
