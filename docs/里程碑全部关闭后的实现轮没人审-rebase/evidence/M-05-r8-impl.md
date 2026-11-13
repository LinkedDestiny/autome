# 实现 #8：合并前门禁失败修复

## 范围与结论

本轮为修复轮，不推进新里程碑。五个已完成里程碑及其最新审计证据指针保持原样；implementation-round 更新为 8/25。仅精简协议源 session-protocol.md 的引言和会话起止说明，并补充尚未合并的 protocol/v8 J-3 说明。未修改固定协议副本、测试、大小限制、内核契约或门禁脚本。

宿主失败提交 e5cee3d0f106；原日志 `/Users/dannie/project/autome-v2/.autome/output/gates/T-68/merge_gate-e5cee3d0f106.log`：automed lib 899 例，898 通过、1 失败。唯一失败为 `protocol::eval::tests::a_code_sample_that_is_not_a_status_block_is_not_parsed`。摘要存 `.autome/output/r8-host-failure.txt`。

## 红绿证据与修复

- 原样命令：`cargo test -p automed --lib protocol::eval::tests::a_code_sample_that_is_not_a_status_block_is_not_parsed`。修改前 1 例失败，Cargo 退出 101，输出 `.autome/output/r8-red.txt`；修改后 1 例通过，退出 0，输出 `.autome/output/r8-green.txt`。它已在项目测试体系内，逐字保留，无需复制或改写。
- 排查假设：大小预算、围栏误解析、其他静态规则。把原协议复制到 `.autome/output/r8-protocol/` 并原样追加测试的 shell 样例，再执行 `cargo run -p automed -- protocol eval .autome/output/r8-protocol/`，诊断仅指向大小超限：24584 > 24576，见 `r8-diagnostic.txt`。正文原长 24546，剩余 30 字节，样例追加 38 字节，超限 8 字节。
- 精简未被 brief-map 摘录的文字 69 字节，规则含义不变。正文现长 24477，剩余 99 字节；追加同一样例后 24515，仍余 61 字节。保留预算及原测试，避免把集成缺陷藏进测试准备条件。
- 上一轮审计结论为通过，没有审计退回检查需要迁移；本轮宿主抓到的原用例就是永久回归。

## 验收命令与数量

| 命令 | 结果 | 原始输出 |
|---|---|---|
| `cargo test -p automed --lib protocol::` | 70 通过，0 失败/忽略，退出 0 | `.autome/output/r8-protocol-tests.txt` |
| `cargo run -p automed -- protocol eval protocol/` | 退出 0，内核契约与静态规则通过 | `.autome/output/r8-eval.txt` |
| `cargo test -p automed --test prompt_rendering` | 40 通过，0 失败/忽略，退出 0 | `.autome/output/r8-prompts.txt` |
| `test "$(grep -c '^## protocol/v' protocol/CHANGELOG.md)" = 8 && grep -q '^## 集成复验$' protocol/loop-protocol.md && grep -q '集成复验' docs/development/technical-design.md` | 退出 0；与上两条构成完整 M-05 验收链 | 终端短输出 `M-05 exit=0` |
| `git diff --check` | 退出 0 | 无输出 |

本轮 Rust 测试去重总数 **110**（70+40），单独红绿运行的 1 例已包含在 70 中，不重复计数。与宿主日志中同名 70 例相比数量差 **0**，由 69 通过/1 失败转为 70 通过；prompt 40 与上一轮实现/审计基线 40 相同，差 **0**。与实现 #7 总数 71 相比为 **+39**，源于验证范围换成协议模块的 70 例，替换上轮除 prompt 外的 31 例；不是新增 39 个测试。本轮未新增、删除或跳过项目用例。另有 2 次手工字节边界实验，不计入 Rust 用例总数。不声称宿主 899 例已全绿。

## 标待审前自审清单（修复轮仍逐项执行）

1. **情形表逐行**：本轮对应 M-05 协议发布与文案落法。正文压缩：原样样例回归转绿；内核契约保持：protocol eval 通过；普通 prompt/brief 不变：40 个金样通过；v1–v8 标题、集成复验二级标题、技术设计条目：M-05 三项检查通过。全部里程碑已关闭，本轮不重新标待审。
2. **失败分支**：协议模块 70 例覆盖大小超限拒绝、错误状态块/表格、内核契约变化、缺失模板变量、缺失目录等拒绝路径，全部通过。拒绝授权、读盘失败、超时、空输入及外部服务不可达：本轮仅压缩说明文字，没有新增或修改这些运行路径，不适用；没有以本轮子集声称它们已被重新验收。
3. **全部迁移**：不适用；未修改调度、状态机或通道逻辑。原状态字段和所有里程碑状态保持，仅轮次与下一步更新。
4. **输入域边界**：对临时协议副本用 ASCII 空格补齐正文到 24576 字节，运行 `cargo run -p automed -- protocol eval .autome/output/r8-protocol/` 退出 0；再追加 1 字节，同命令退出 1 且明确报告 24577 超限。输出分别 `r8-boundary-at.txt`、`r8-boundary-over.txt`。原样 shell 样例在预算内成功、既有超限单测拒绝，两侧均验证。前导零、闰日、跨年等不适用，未涉及此类输入域。

## 交接

未运行全量门禁或快检；由 Autome 在提交后/合并前运行，其结果以宿主为准。本轮改动应独立集成复验，重点核对文案语义、原测试未改及字节预算；不改已关闭里程碑的最新证据指针。提交时使用仓库支持的 `NO_CI=1`，避免启动用户禁止的全量门禁。
