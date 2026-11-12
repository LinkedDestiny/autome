# rebase 冲突退回实现不受预算约束

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --lib rebase_retry_regression -- --nocapture
根因: Rebase 冲突分支无条件启动实现轮，未检查已用实现预算或累计同文件集冲突；已完成里程碑又使审计直接进入复盘和 rebase，形成无限回路。
复核: 通过

## 现象

任务卡报告 T-27 的实现轮达到 25/20，同一 render.rs 冲突约重复 19 次。历史任务分支和账本未在本轮读取，此处历史数字引用任务卡；本轮独立复现当前代码的同类调度缺陷。

当前基线为 `0275afc`，包含任务卡所述 `9fd82e2` 注入文案修复。宿主 abort rebase 后，冲突标记消失，但分支与默认分支的冲突仍然存在；无代码修改的实现会话结束后，已完成的里程碑让审计通过、复盘结束，再次触发相同冲突。

## 复现

从当前 worktree 根目录运行状态块中的命令。当前是单 Git 仓库，Cargo workspace 的 `automed` 是 crate，不是 Autome 工作区成员，故无需成员目录 `cd` 或 `repos:`。

新增测试在 `crates/automed/src/scheduler.rs` 的 `#[cfg(test)]` 模块内，使用现有 `World`：真实临时 Git 仓库、worktree 和 SQLite store；CLI 启动为 Dry 模式，显式交付成功的无操作会话结果。不依赖网络、模型、sleep 或真实终端。Git 缺失会断言失败，不会静默跳过。

两个用例共享最小冲突夹具：默认分支与任务分支同时修改 README.md 的同一行。唯一里程碑已完成；每轮只提交实现轮次状态，README.md 始终不变。通过实际后继状态驱动实现、审计、复盘，不手动把循环状态重置成 Rebase。

1. `rebase_retry_regression_exhausted_budget_stops_new_implementation`：预算为 1，首次冲突允许启动实现；实现结束后再次冲突必须停下，不得启动第 2 个实现会话。当前断言失败：`exhausted 1/1 rebase retry must stop, got Active { node: Implement }`。
2. `rebase_retry_regression_same_conflict_stops_before_nineteen_retries`：预算为 100，排除预算耗尽影响；每次检查注入中的冲突文件集都是 README.md、abort 后文件仍为任务分支内容。要求在重演 19 轮空转前停止；任务卡未指定 N，测试不擅自把阈值定成 3。停止必须是可辨识 rebase 原因的失败状态，不能用配置或协议错误冒充熔断，且不能留有运行会话。

命令输出保存在 `.autome/output/rebase-regression-final.log`。正常路径对照：`a_conflicting_requested_rebase_goes_back_to_implement_and_clears_the_request` 和 `a_clean_rebase_advances_to_the_merge_stopping_point` 各通过 1 个测试；日志分别为 `.autome/output/control-conflict.log`、`.autome/output/control-clean.log`。

实跑结果：两次完整运行均为 2 failed / 0 passed，退出码 101，分别约 134.21 秒和 133.68 秒；最终版本第二例失败信息为 `same README.md conflict launched 19 no-op implementation rounds without stopping`。均为上述行为断言失败，不是编译、超时或命令缺失。宿主执行需给足真实 Git 调度回路约 3 分钟的时间。`cargo fmt --all -- --check`、`git diff --check` 通过；未运行全量测试或发布门禁，本轮刻意保留失败回归。

## 根因分析

1. `crates/autome-domain/src/task.rs:1196`：`Rebase + Conflict` 直接返回 `Implement + StartRole(Impl, RebaseConflict)`；没有预算分支。`Context` 只有预算上限、配置与决策，不携带已用实现轮次或冲突连续计数。相比之下，同文件 `after_gate` 在实现门禁失败时读取状态块轮次，达到预算便停止。
2. `crates/automed/src/scheduler.rs:2778`：`run_core_step` 执行 `rebase_all` 后直接把结果交给 `advance`。该链路没有针对相同节点和冲突文件集的持久计数、指纹比较或熔断检查。不能因账本里存在状态事件就认为它们参与了停止决策；当前只是记录，并未用于此处判断。
3. `crates/automed/src/scheduler.rs:2364`：启动会话会从账本取得 `next_round`，并把 round/limit 放入提示；这只展示预算，不阻止超预算启动。因此问题不是完全没有计数，而是冲突路径没有执行预算约束。
4. `crates/autome-domain/src/task.rs:1093`：审计先判断所有里程碑完成，成立时直接进入收尾，预算判断位于其后的分支。关闭审计时 `:715` 的完成判断同样先于预算。冲突没有重新打开里程碑，因此无操作会话仍能回到 rebase。正常完成应允许收尾，修复应在冲突重试入口约束继续启动的轮次，不能简单禁止所有预算边界上的成功收尾。
5. `crates/automed/src/git.rs:621`：rebase 失败后收集冲突文件并 abort，恢复工作树。这解释了会话看不到冲突标记，但不等于冲突已解决；已修的注入文案无法代替宿主重试上限。

任务卡提到的 T-28 门禁熔断在当前 checkout 未找到对应实现，后续不能直接假设已有可复用机制。N 的具体值仍未指定；后续修复还应验证文件集顺序不影响指纹、文件集变化与成功会中断连续计数，以及用户 rerun/extend 后从零计数。本轮测试锁定已观察到的两个缺陷，不宣称覆盖这些尚不存在的机制。

可选的“冲突退回会话结束后、审计前提前检查”不作为本轮必要条件。未修改产品逻辑，未创建需求设计或里程碑文档；修复和独立复核由后续节点处理。

## 修复说明

第 1/3 轮：在 scheduler 的 rebase 结果转移入口读取账本中的下一实现轮次；超出任务预算时停止在 Rebase，以现有 ImplBudget 原因保留追加预算能力，不再启动实现会话。正常成功收尾路径不变。

以账本事件持久保存连续冲突文件集和次数，文件名排序、去重后比较。本轮采用连续 3 次相同文件集冲突即停止（最多自动退回实现 2 次）的阈值；这是修复选择，并非任务卡给定数字。文件集变化重新计数，非冲突 rebase 结果及已接受的 rerun/extend 操作重置计数。重复冲突以 RebaseConflict 原因停下，等待用户检查后重跑。

定位轮两个回归用例完整保留；新增边界测试覆盖文件顺序、重复文件名、文件集变化、成功和用户恢复操作的计数行为。修复前原命令实跑 0 passed / 2 failed，退出码 101（`.autome/output/fix-before.log`）；产品逻辑修复后同命令 2 passed / 0 failed（`.autome/output/fix-after-final.log`）。新增边界测试 1 passed / 0 failed（`.autome/output/fix-boundaries.log`）。`git diff --check` 通过。

最终代码执行配置门禁 `sh scripts/ci-local.sh`，退出码 1，用时 543 秒，完整日志 `.autome/output/fix-gate-final.log`。Format、Clippy、workspace 全量 Rust 测试、End to end、Adapter flags、Desktop、Contract、Web lint/typecheck/build 均通过；仅 Web test 失败（4 个文件失败、29 个通过；11 failed / 271 passed / 19 skipped）。失败栈报告本地监听 `listen EPERM: operation not permitted 127.0.0.1`，live 测试还报告文件监听 `EMFILE` 与启动等待超时。前一遍完整门禁也出现相同环境失败（`.autome/output/fix-gate.log`）。没有修改这些测试、手动跳过检查或将门禁标为通过；当前环境不允许权限升级，需宿主在具备本地监听和文件监听能力的环境复跑门禁。未运行可选 `--package` 发布打包。

## 复核意见

复核: 通过。

- 复现命令独立重跑：2 passed / 0 failed（`.autome/output/review-repro.log`）。边界测试 `rebase_conflict_streak_normalizes_files_and_resets_on_recovery` 通过；两个正常路径对照测试通过。
- 差异 `cbbe2c1..0c63c8d` 对 `crates` 仅新增（scheduler.rs +171/-2，其中 -2 为 `let mut transition`），定位轮两个回归测试原样保留，无删除、跳过或弱化。
- 根因站得住：`Rebase+Conflict` 无条件退回实现；修复在 scheduler 的 `advance` 入口补上 ImplBudget 检查与账本持久的连续相同文件集计数（排序去重、变化/成功/rerun/extend 归零），改动集中在冲突路径，范围最小；正常收尾路径不变。阈值 3 是修复选择，已在修复说明中如实标注。
- 门禁：clippy `-D warnings` 通过；workspace 测试中 lib 623、domain 345 等通过；集成测试 `bug_loop_a_workspace_bug_checks_out_...` 在整体负载下失败一次（停在 MergeGate），单独重跑 1 passed，该用例属工作区 Bug 流程，与 rebase 冲突路径无关，判为负载下的时序抖动。Web test 的 EPERM/EMFILE 为沙箱环境限制，需宿主复跑。
