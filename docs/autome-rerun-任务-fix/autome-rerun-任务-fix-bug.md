# rerun fix 绕过修复轮预算定位

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --test end_to_end bug_loop_rerun_fix_cannot_bypass_exhausted_budget -- --exact
根因: Bug Loop 的 RerunFrom(Fix) 直接创建修复会话，绕过 start_fix 的预算预检；调度器仍记账但未阻止超预算启动。
复核: 通过

## 现象

基线提交 `3555c97d0dfc385e4e9c74037da56168b8e86240`。预算为 1 的任务正常耗尽预算后，连续执行两次 `task.rerun_from { node: "fix" }`，修复会话累计从 1 增至 3，上限仍为 1。每次门禁失败后又回到 `Failed(FixBudget)`，可继续重跑。

任务卡所述绕过现象成立，但“没有计数”不准确：手动重跑有计数，缺少的是启动前的上限检查。

## 复现

在本 worktree 根目录执行状态块中的命令。本项目为单 Git 仓库（Cargo workspace），不是需要切换成员检出的 Autome 多仓库工作区，因此无需成员 `cd` 或 `repos`。

新增测试位于 `crates/automed/tests/end_to_end.rs`，使用现有 World 夹具：真实 Git、命令分发、调度器、进程启动、协议解析和事件存储，仅以脚本替代模型。

1. 配置 `fix_rounds = 1`，定位脚本创建稳定失败的 `regression.sh`，修复脚本不改变故障。
2. 等待自动流程因 `FixBudget { limit: 1 }` 失败，确认预算和已启动轮数均为 1。
3. 连续重跑 fix 两次，每次等待流程结束，确认任务仍预算失败、预算未被重置。
4. 断言已启动修复会话仍为 1。当前实际为 3，在该断言稳定失败，退出码 101。
5. 测试还包含追加 1 轮后允许启动下一轮的正向断言；当前受前述失败阻断，尚未执行到该断言。既有 `bug_loop_fix_rounds_run_out_and_extend_adds_more` 单独验证 extend 路径。

夹具将独立的 `same_failure_limit` 设置为 10，避免三次失败先触发同因熔断；项目门禁为 `true`，失败来自真实复现命令。使用现有有界调度轮询，不依赖网络或固定睡眠判定。Git 缺失时显式失败，不静默跳过。

最终测试连续执行两次，均在 `left: 3 / right: 1` 的会话计数断言失败。原始日志：`.autome/output/rerun-fix-regression-1.log`、`.autome/output/rerun-fix-regression-2.log`。早期夹具配置错误和同因熔断不作为最终复现证据。

## 根因分析

- `crates/autome-domain/src/bug_loop.rs:47`：`start_fix` 读取任务预算或配置上限，在 `progress.fix_round >= limit` 时返回 `FixBudget`。
- 同文件 `:148`：Failed / Stopped 的 `RerunFrom` 分支将 Fix 映射为 BugStep::Fix，检查角色后直接调用 `session`，没有调用预算预检。
- `crates/automed/src/dispatch.rs:437`：公开命令转为 `Trigger::RerunFrom`，没有独立的修复轮预算检查。
- `crates/automed/src/scheduler.rs:2325`、`:2571` 附近：调度器记录 `bug.fix_round_requested` 和 `bug.fix_round_started`；`:1637` 从事件数取得已用轮数。计数存在且未重置，重跑后数值确实增长；调度层没有补回启动前的修复轮上限校验。
- 需求 Loop 核对：`crates/autome-domain/src/task.rs:1027` 同样在 Failed / Stopped 的 RerunFrom(Implement) 分支直接返回 StartRole，没有预算预检；自动流程则在实现/审计完成等路径检查实现预算。不能把当前需求 Loop 当成已具备预算防护的实现样板。本轮只静态核对该路径，没有声明已实跑需求 Loop 超预算场景，也未扩大修改范围。

文档差异：`README.md:219` 和 `docs/development/technical-design.md:265` 明确记录手动重跑绕过检查。这与当前实现一致，但任务卡已说明这些是 T-45 按实际行为补写的文档，并明确要求堵住绕过；本轮以任务卡为首要输入判定为已定位，未以现状说明否定本任务。修复轮需同步这两处文字。

另一个边界：当前 Bug Loop 普通 rerun 不接受 repro_check，仅允许 locate、fix、fix_review、rebase（同门禁熔断有专门分支）。任务卡提到的 repro_check 不应被解读为新增支持范围。本轮不改变任何非 fix 节点行为。

## 修复说明

第 1/3 轮修复：Bug Loop 的 Failed / Stopped 手动重跑 fix 在角色检查后复用 `start_fix`，按任务预算（未设时取配置）检查已启动轮数。预算耗尽时使用原节点返回 `Failed(FixBudget)`，不启动会话；预算内重跑正常启动并继续记账。其它重跑节点及需求 Loop 不变。同步 README 与技术设计中的旧行为说明。

定位轮的端到端回归测试未修改。更新既有领域单测中超预算仍启动的旧预期，并新增配置预算 / 任务预算、Failed / Stopped、上限前 / 达到上限 / 超出上限的边界断言。

验证：原复现命令修复前失败（会话数 3 而非 1），修复后通过（1 passed），包含连续两次重跑不启动会话及 extend 后允许再启动一轮的正向控制。Bug Loop 领域测试子集通过（33 passed）；既有 `bug_loop_fix_rounds_run_out_and_extend_adds_more` 通过（1 passed）。`cargo fmt --all -- --check` 与 `git diff --check` 通过。日志在 `.autome/output/fix-1-before.log`、`fix-1-after.log`、`fix-1-domain.log`、`fix-1-extend.log` 和 `fix-1-fmt-check.log`。

未运行 `sh scripts/ci-local.sh`，全量门禁留给宿主合并前执行；未启动后续会话。

## 复核意见

复核通过（第 1/3 轮）。

- 复现命令独立复验：`cargo test --offline -p automed --test end_to_end bug_loop_rerun_fix_cannot_bypass_exhausted_budget -- --exact` 1 passed；修复前断言会话数 3≠1，现为通过。
- 相关子集：`cargo test --offline -p autome-domain --lib bug_loop` 33 passed；`bug_loop_fix_rounds_run_out_and_extend_adds_more` 1 passed。
- 根因站得住：`apply_for` 的 Failed/Stopped RerunFrom 分支原先对 Fix 直接 `session`，绕过 `start_fix` 的预算检查。修复仅在角色检查后对 `BugStep::Fix` 复用 `start_fix(*at, …)`，其它节点与需求 Loop 不变，范围最小。
- 回归测试有意义且未弱化：端到端测试未改动，覆盖连续重跑不增会话与 extend 后正向放行；领域单测把旧的“超预算仍启动”预期改为 FixBudget，并新增配置/任务预算 × Failed/Stopped × 上限前/达到/超出的边界断言，无删除或跳过。
- README 与技术设计文档同步更新，与新行为一致。
- 未运行全量门禁（留给宿主合并前）。
