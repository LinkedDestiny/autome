# 守卫误报审计状态块更新

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --lib scheduler::tests::audit_design_diff_allows_protocol_updates_but_rejects_body_edits -- --exact
根因: scheduler.rs 的设计文档差异过滤只放行里程碑行和最新证据行，遗漏协议状态块字段，导致正常状态更新被标记为审计改设计。
复核: 待复核

## 现象

任务卡报告审计更新状态块、里程碑和最新证据后触发 `audit_edited_the_design`。在当前基线 `8415043` 上可稳定复现状态块误报。当前代码已经放行中文冒号的「最新证据：」行，因此不能把证据行单独归为当前误报根因；未重放任务卡所述历史 T-12/T-13/T-15 会话。

## 复现

当前目录是 autome-v2 单仓库 worktree，包含 Cargo workspace，不是多个 Git 仓库组成的 Autome 工作区项目，因此不添加 `repos:`。在当前 worktree 根目录执行状态块中的命令，不需要切换成员目录。

新增测试位于 `crates/automed/src/scheduler.rs` 的 `#[cfg(test)]` 模块，复用现有 `World`，建立真实临时 Git 仓库、设计文档提交和内存事件存储。把审计前 SHA 写入 `task.head`，逐个提交修改后的设计文档，通过生产函数读取真实 `git diff <before>..HEAD`，不模拟判定结果。临时仓库由已有测试设施清理。

| 差异场景 | 预期判定 | 当前判定 |
|---|---|---|
| 内容不变 | false | false |
| 仅里程碑从待审改已完成 | false | false |
| 仅最新证据指针 | false | false |
| 仅 status/current-milestone/next-action | false | true |
| 状态块、里程碑、证据一起更新 | false | true |
| 上述更新同时改写设计正文 | true | true |

已连续执行两次，均为测试断言失败，退出码 101；第二次明确执行 `--lib`，结果为 0 passed、1 failed、0 ignored。不是编译失败、超时或工具不可用。完整复跑输出在 `.autome/output/regression-repeat.log`（临时产物，不提交）。所有场景始终与同一个审计前 SHA 比较，未把前一场景的提交错误地当作新基线。

## 根因分析

`crates/automed/src/scheduler.rs::design_changed_outside_milestones` 从事件存储读取 `task.head`，对设计文件执行零上下文 Git diff，取增删行、去掉符号及空白，最后仅放行 `| M-` 和 `最新证据：` 前缀。状态块字段增删行不在允许范围，因此返回 `Some(true)`。

该值传入 `guards::Round`；`crates/automed/src/guards.rs::audit_stayed_in_its_column` 在值为 `Some(true)` 时生成 `audit_edited_the_design` 警告。任务卡中的函数位置线索需更正：计算在 scheduler，guards 负责消费结果。

`protocol/session-protocol.md` 的「状态块」要求维护头部状态字段；`protocol/prompts/audit.md` 也要求审计维护状态并在退回时更新 convergence-mode。任务卡明确允许状态块更新，现有过滤与此冲突。证据行单独通过、正文修改被识别的对照排除了证据前缀及比较基线异常作为本次复现根因。

后续修复应识别头部状态块的允许更新，同时保留正文改动报警；不能直接关闭该守卫。本轮只增加回归测试，没有修改生产逻辑。

## 修复说明

本轮修复 scheduler 的设计差异过滤：使用完整 Git diff 上下文，分别跟踪修改前后的文档头部和代码围栏，只放行第一个二级标题之前、围栏之外的八个协议状态字段（支持中英文冒号）。里程碑行和最新证据行沿用原规则，正文中的同名字段仍触发报警。未修改 guards 的报警逻辑。

定位轮回归测试未修改、删除、跳过或弱化。原复现命令修复前断言失败（0 passed、1 failed），修复后通过（1 passed、0 failed）；日志分别为 `.autome/output/fix-before.log`、`.autome/output/fix-after.log`。新增独立反例测试确认正文状态字段和头部围栏示例被修改时仍报警，并确认头部 convergence-mode 使用中文冒号更新可通过；两项差异测试均通过，日志为 `.autome/output/fix-boundaries.log`。

配置的项目门禁 `sh scripts/ci-local.sh` 已完整执行，退出码 1，用时 176 秒，日志为 `.autome/output/fix-gate.log`。Format、Clippy、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过；Test 和 Web test 未通过，不能视为完整门禁通过。

- Test 阶段的 `stopping_a_task_kills_the_cli_the_wrapper_started` 在读取 PID 时遇到 `ParseIntError { kind: Empty }`（end_to_end.rs:2749），该组 77 通过、1 失败。门禁后续 End to end 阶段通过；另用 `cargo test --offline -p automed --test end_to_end stopping_a_task_kills_the_cli_the_wrapper_started -- --exact` 单独复跑也通过（`.autome/output/fix-stop-recheck.log`），未改动该无关测试或生产代码。
- Web test 为 3 个文件失败、19 个通过，11 个测试失败、198 个通过、16 个跳过，包含 `listen EPERM: operation not permitted 127.0.0.1` 的沙箱监听限制。未调整测试或门禁配置；仍需宿主在允许本地监听的环境运行完整门禁。

`git diff --check` 通过，定位回归测试与定位提交逐字一致。本轮保持 `status: 已定位`、原复现命令及 `复核: 待复核`，等待 Autome 后续处理。

## 复核意见

待独立复核。
