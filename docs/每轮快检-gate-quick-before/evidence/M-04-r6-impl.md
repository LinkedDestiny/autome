# M-04 · 实现 #6 · 合并前门禁修复

本轮为修复轮，implementation-round: 6/25。不推进里程碑，不更改已由审计关闭的五项状态。完整门禁以 Autome 重跑为准。

## 根因与范围

读取宿主 T-67 的 `merge_gate-d4b93c2afcf0.log`，确认 Clippy 的 `large_enum_variant` 与 Rust 单测的协议版本断言失败；摘要输出存于 `.autome/output/r6-host-failures.txt`。Rust 单测失败使后续集成测试没有执行，不能把它们视为通过。

- quick 配置字段增大了 ResolvedConfig，Route 的 Rerouted 变体至少 224 字节。改用 Box 承载，保留原路由分支与配置值；没有屏蔽 lint。
- 协议 CHANGELOG 已发布 v5，单测仍固定 v4。搜索协议模块和全部 automed 集成测试的 `protocol/v4`，发现另一个 e2e 同类断言，单跑也红。两处更新为 v5，哈希、冻结、版本选择断言均保留。设计 §13 已列明这一必要例外；未修改金样或协议正文。
- 上一审计没有退回缺陷，也没有外部临时判别脚本需迁入；本次发现的两个失败检查本身已在项目测试体系内，原样复现后修正预期，继续作为回归用例。

## 红绿证据与命令

所有原始输出在 `.autome/output/`，不转存正文。

1. `cargo clippy --all-targets -- -D warnings`：修前 101，Route 触发 large_enum_variant（`r6-clippy-red.txt`）；修后 0（`r6-clippy-green.txt`）。
2. `cargo test -p automed --lib protocol::tests::the_version_in_force_is_the_newest_changelog_heading_over_the_seed`：修前 101，1 个用例失败，实际 v5、预期 v4（`r6-version-red.txt`）。
3. `cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：修前 101，1 个用例失败于 v4 前缀断言（`r6-e2e-red.txt`）。
4. `cargo test -p automed --lib -- protocol::tests:: bug_loop_cooldown`：修后 0，12 个通过（`r6-lib-green.txt`），包含第 2 项。
5. `cargo test -p automed --test end_to_end -- every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task a_rate_limited_round_moves_to_the_other_cli_when_that_keeps_roles_apart_and_waits_when_not`：修后 0，2 个通过（`r6-e2e-green.txt`），包含第 3 项。
6. `cargo test -p automed --test prompt_rendering`：0，31 个通过（`r6-golden-green.txt`）。
7. `cargo fmt --all -- --check`：0（`r6-fmt.txt`）；`git diff --check`：0。

本轮最终测试用例总数 **45 = 12 + 2 + 31**，全部通过，失败 0、忽略 0；筛选外用例没有计入。相对上一实现轮 M-05 的 14 个验收用例是 **+31**：移出 CLI/桌面 14 个，换入本次修复相关 45 个，不是新增 31 个用例。本轮没有增删测试，同范围用例数量相对修前代码 **净变化 0**；修前只实跑两个失败用例，不冒称修前 45 个全部跑过。上一审计额外运行的 CLI 全 crate 子集与本次范围也不同，不用其通过数作为同范围基线。

## 标待审之前的自审清单

本轮不重新标待审，仍执行四项检查。

1. **情形表逐行**：设计 §9 协议版本发布核对 CHANGELOG v5、current 返回 v5、任务冻结 v5 与哈希，单测和 e2e 通过；§13 旧金样不变由全部 31 个 prompt_rendering 用例确认。其余门禁情形表不适用本次改动：没有修改门禁命令选择与收结果逻辑。
2. **失败分支**：真实 Git e2e 注入 CLI 限流失败，允许分离角色则改道，否则等待；Bug Fix/FixReview 冷却保持角色分离单测通过。授权拒绝、读盘失败、门禁超时、空输入、外部服务不可达不适用：本轮只改枚举载荷存储及版本预期，没有更改这些分支。
3. **全部迁移**：Route 的 Launch（e2e 正常轮次）、Rerouted（限流后改到另一 CLI）、Deferred（实现与审计不得共用 CLI，排入重试）均通过；Bug Fix/FixReview 的 Deferred 均通过。无新增状态或迁移。
4. **输入域边界**：协议版本解析已有“按数字而非位置选择最新版本”用例覆盖 v10/v9 及无版本标题，随协议单测通过；v5 发布与任务冻结覆盖当前版本。日期、前导零、闰日、最大整数不适用：本轮未改此类输入解析。

## 交付边界

未运行 `sh scripts/ci-local.sh` 或全量 Rust 测试，未改 `.autome/` 配置、规则或协议；仅按要求在 output 下保存日志与临时脚本。未改分支、未操作其它 worktree、未推送。H-01/H-02 人工项维持原状态。
