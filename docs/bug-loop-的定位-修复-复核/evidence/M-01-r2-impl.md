# M-01 · 实现 #2（门禁修复轮）

结论：M-01 保持待审，本轮不推进 M-02。修复协议升级遗漏的集成测试断言，定向红绿与 Rust 全量通过；完整门禁未通过，环境阻塞由 Autome 在提交后复验。无上一轮审计文件，无审计检查需要迁入。

## 修复与原样回归

Autome 在 3e839ce416 上报告 Test 与 Desktop 失败。本轮从 258d6ad 开始，先运行原命令 `sh scripts/ci-local.sh`（日志 `.autome/output/impl-2-ci-red.txt`）。Rust 端到端 99 通过、2 失败：版本冻结断言仍要求 protocol/v2；另一个 setup 用例期望 await_merge 却得到 None。Desktop 104 通过、1 失败。Web 出现 EMFILE 并长时间不进展，终止首轮，退出码 130；未把它记为完整执行。

现有项目回归用例原样执行：

`cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task -- --nocapture`

- 红：退出码 101，0/1 通过，实际 protocol/v3@7b050646…，断言要求 protocol/v2@；日志 `impl-2-version-red.txt`。
- 修复：仅将 end_to_end.rs 的版本前缀断言改为 protocol/v3@；保留与 current().0.to_wire() 的完整相等断言。协议版本与生产行为均未再改动。
- 绿：退出码 0，1/1 通过；日志 `impl-2-version-green.txt`。现有测试即回归用例，没有重复新增用例。
- 同类检查：检索 tests 和 Desktop 的 v2/v3；Desktop 中 v2 为自带 fixture，与当前协议无关，保持不变。
- setup 用例原样单跑通过（1/1，`impl-2-setup-check.txt`），修复后全量也通过。暂未证明稳定缺陷，未改调度生产代码或放宽断言；保留首次偶发失败记录。

## 验收命令与总数

所有日志位于 `.autome/output/`。

1. M-01 原验收链：`cargo test -p automed --lib launcher && cargo test -p automed --test prompt_rendering && cargo test -p automed --lib protocol:: && cargo run -p automed -- protocol eval protocol/ && git diff --name-status 83c80e9 -- crates/automed/tests/golden/prompts/ protocol/`。退出码 0，日志 `impl-2-m01.txt`；62 + 19 + 61 = 142 个全通过，上一轮 142，差 0。静态门 8 通过、0 不通过、1 既有警告，范围仍恰好七行。
2. 同命令复验 `sh scripts/ci-local.sh`，日志 `impl-2-ci-final.txt`：Rust 全量 1201 通过、0 失败、1 既有忽略，总数 1202；含端到端 101/101、金样 19/19、real_cli 7/7。与首次运行已到达的 Rust 总数 1176 相比 +26，来自首次端到端失败后 cargo 没有运行的金样 19 和 real_cli 7，非新增测试。上一轮证据未记录全量总数，无法虚构该基线；本轮代码没有增加或删除测试。
3. Desktop 原样 `npm test --prefix apps/desktop` 和完整门禁内运行均为总数 105，104 通过、1 失败，两次差 0；Electron DOM 子检查未启动，不能视为全部 DOM 用例已执行。最小探针直接 spawn Electron `--version` 与 `dom-harness.js` 均 status=null、signal=SIGABRT、无输出，证明失败发生在 DOM 断言之前。没有删除、跳过或弱化该检查；当前权限不允许提权运行。
4. Format、Clippy、Contract、Web lint、Web typecheck 均通过。Web 全量结果见下方最终补记；Autome 提供的上轮 Web 基线为 301。本地文件描述符上限 256，首次 Web 报 EMFILE。本轮不修改 Desktop/Web 或门禁脚本，不将启动受阻当成功。

## 标待审之前的自审清单（本轮保持待审，仍逐项复验）

1. **情形表逐行**：Locate、Fix、FixReview 的附件+参考、无输入、仅附件、仅参考、用户花括号五类均由 launcher 测试逐角色执行通过；三个模板位置由新旧金样通过验证。整理轮原值与需求金样通过，协议 v3 检查通过。
2. **失败分支**：三角色缺预算、缺模板均拒绝，模板未知占位符拒绝；空输入无多余标题/占位符/空段，用户花括号不拒绝，全部由本轮 M-01 套件执行。授权、读盘、超时、外部服务失败不适用：本里程碑为纯渲染，本轮仅同步版本测试断言，没有新增这些分支。
3. **全部迁移**：本轮无状态机改动，迁移修改不适用；三个渲染通道及有任务卡定位入口均通过，现有端到端 101 个全通过。M-02 的新增附件 e2e 未实施，不借本次门禁关闭 M-02。
4. **输入域边界**：空列表、空需求、单元素、多附件、绝对路径、空格、花括号、空字符串/根路径回退、预算 0/1、1/1、20/20、u32::MAX 均由上述 142 个用例执行通过。日期边界不适用：无日期输入域。版本边界由本轮冻结协议测试确认 v3 完整引用与当前内核相等。

## 交付范围

仅修改 end_to_end.rs 一条断言，以及本轮状态、证据、retro。没有推进新里程碑，没有修改 .autome 配置、协议副本、生产状态机、Desktop/Web、插件或其它 worktree；日志按授权写入 .autome/output。最终门禁以 Autome 在提交上的重跑为准。

最终补记：修复后的完整门禁在 Web test 阶段再次长时间未产出用例结果，主动终止，退出码 130；未获得 Web 测试总数，不能与上轮 301 作数值差比较，更不能称少掉的用例通过。Rust 门禁已完整通过，Desktop 启动仍失败。Web build 单独复验，结果记录在 impl-2-web-build.txt。当前会话未取得同命令全绿证据，这是环境验证阻塞，不是协议失败。

Web build 独立复验退出码 0。git diff --check 通过；状态块字段保持原格式，implementation-round 为 2/10，M-01 待审、M-02 开放。
