# 守卫误报审计状态块更新

status: 已定位
fix-round: 0/3
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

定位轮未修复，等待 Autome 调度修复轮。

## 复核意见

待独立复核。
