# 实现 #9：合并前门禁版本断言修复

## 范围、原因与修复

本轮仅修复宿主门禁失败，不推进新里程碑。implementation-round 为 9/25；五个已完成里程碑、reopen 和最新审计证据指针均保持原样。

宿主提交 `eef466bd82c0` 的日志为 `/Users/dannie/project/autome-v2/.autome/output/gates/T-68/merge_gate-eef466bd82c0.log`。库测试 899 例全部通过，端到端 187 例中 186 通过、1 失败：`every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`，在 `end_to_end.rs:1951` 要求 `protocol/v7@`，实际为 `protocol/v8@211462db063692e485250badf0f6ccc459b527b3e66d1994b157edbd32b69500`。摘要见 `.autome/output/r9-host-summary.txt`。

根因为协议已按设计 §10 发布 v8，但端到端版本字面量漏更新。候选原因还包括版本解析错误、冻结了错误内容；既有完整引用相等断言已通过，CHANGELOG 最新标题与 `the_version_in_force_is_the_newest_changelog_heading_over_the_seed` 均要求 v8，排除这两种解释。

仅将该用例中的 `protocol/v7@` 改为 `protocol/v8@`。这对应设计明确要求的版本发布事实，不是放宽断言来适应错误行为：保留版本字面量检查、完整引用（含哈希）相等、所有简报及冻结副本检查；不更改 fixture、配置、流程或产品逻辑。对 crates 内所有 `protocol/v7` 的扫描见 `.autome/output/r9-v7-occurrences.txt`，其余均为旧版本示例或解析/多版本比较测试，不应替换。

上一轮审计通过，无退回检查需要迁移。宿主抓到的检查本就在项目测试体系中，原样执行取得红色证据后，按当前协议版本修正期望并执行绿色验证。

## 命令与结果

以下原始输出均在 `.autome/output/`。

| 命令 | 结果 | 输出文件 |
|---|---|---|
| `cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`（修改前） | 1 例失败，退出 101；与宿主同一断言 | `r9-red.txt` |
| 同上（修改后） | 1 例通过，退出 0 | `r9-green.txt` |
| `cargo test -p automed --test end_to_end brief` | 2 例通过，退出 0 | `r9-brief.txt` |
| `cargo test -p automed --test end_to_end protocol_before_merge` | 2 例通过，退出 0 | `r9-protocol-e2e.txt` |
| `cargo test -p automed --lib protocol::` | 70 例通过，退出 0 | `r9-protocol.txt` |
| `cargo test -p automed --test prompt_rendering` | 40 例通过，退出 0 | `r9-prompts.txt` |
| `cargo run -p automed -- protocol eval protocol/` | 退出 0；8 项通过、0 项不通过、1 项既有警告（41 行条文缺少 CHANGELOG 理由） | `r9-eval.txt` |
| `test "$(grep -c '^## protocol/v' protocol/CHANGELOG.md)" = 8 && grep -q '^## 集成复验$' protocol/loop-protocol.md && grep -q '集成复验' docs/development/technical-design.md` | 退出 0；与前两条组成 M-05 验收链 | 短输出，无独立文件 |
| `git diff --check` | 退出 0 | 无输出 |

绿色测试去重总数 **114**（70 库测试 + 40 prompt + 4 e2e）；单独运行的回归已包含在 brief 的 2 例中，不重复计数。相比实现 #8 / 审计 #7 的 110 例为 **+4**，来自本轮纳入的 4 个相关 e2e；没有新增、删除或跳过测试。与宿主相同四个 e2e 的数量差为 **0**，从 3 通过/1 失败变为 4 通过。未重跑其余 183 个 e2e，不声称整个 187 例套件或全量门禁已通过。

## 标待审前自审清单（修复轮逐项执行）

1. **情形表逐行**：本轮对应 M-05 协议发布。v8 版本记录与完整哈希引用由原失败 e2e、protocol 单测验证；普通角色简报由 brief 子集验证；需求/Bug 的 before_merge 提示由 protocol_before_merge 两例验证；普通 prompt 40 个金样通过；版本标题、集成复验标题、技术设计引用由 M-05 grep 链验证。本轮不重标里程碑。
2. **设计点名的失败分支**：本轮仅修正版本期望，不改变设计 §3 的 Git 读取失败、§7 的缺失审计证据等产品分支，故这些分支重验不适用。相关协议错误版本由修改前原断言复现；协议模块 70 例的静态拒绝分支通过。拒绝授权、超时、外部服务不可达等未改动，不适用，不以此子集声称已重验。
3. **全部状态迁移**：无状态机或通道改动，迁移枚举不适用。原 e2e 仍完整执行 plan/review/adjudicate/impl/audit/retro 到 await_merge，未删减任何步骤；状态块仅更新轮次和下一步，里程碑表不变。
4. **输入域边界**：版本解析的空标题、按数字选最新版本（v9/v10）与哈希一致性由 protocol 模块既有用例通过；本次固定版本期望不增加输入域。前导零、日期、最大值等不适用。v7 过期期望对当前 v8 的红色运行保留了版本不匹配的判别证据。

## 交接

按本轮用户指令，不运行全量门禁或宿主快检（优先于任务文件中旧的会话内 fmt/clippy 要求）。宿主提交后重跑快检、合并前重跑全量门禁，其结果由宿主判定。本轮提交使用仓库 post-commit 支持的 `NO_CI=1`，避免启动被禁止的全量门禁。下一轮应独立复验这一处测试期望调整与设计 v8 发布事实是否一致。
