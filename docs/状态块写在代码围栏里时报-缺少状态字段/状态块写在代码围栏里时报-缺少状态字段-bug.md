# 状态块围栏导致误导性缺字段错误

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p autome-domain --test fenced_status_regression
根因: 状态解析器无条件跳过代码围栏内容且不保留位置，随后将围栏内已有的 status 误报为缺失；任务整理提示词展示围栏样例却未就地说明禁止围栏。
复核: 待复核

## 现象

设计文档一级标题后放置完整且合法的状态块，仅增加三个反引号或带 text 的代码围栏，解析便返回「缺少状态字段 `status`」，没有指出围栏或行号。无围栏的相同内容解析成功。

任务卡提及的 T-27、T-40 历史事件作为用户提供的背景，本轮未读取其它 worktree 或核实其历史提交。当前检出已独立复现同一解析症状。

需要区分：`protocol/session-protocol.md:84` 已明确「不要把状态块放进代码块」，因此拒绝围栏本身符合当前设计；本 Bug 是拒绝原因不准确，以及任务整理入口未就地解释样例围栏的使用方式。不能笼统说所有协议都没有禁止围栏。

## 复现

本项目是单 Git 仓库，Cargo workspace 的 crates 不是 Autome 多仓库成员，因此省略 `repos:`。在当前 worktree 根目录执行头部的复现命令，不需要切换成员目录。

回归测试：`crates/autome-domain/tests/fenced_status_regression.rs`。不依赖网络、时间、服务或真实会话，直接调用调度器使用的生产解析入口 `status_block::parse`。

- 两个触发用例分别覆盖普通反引号围栏和 text 围栏，围栏起始于第 2 行。每个用例先验证同内容的无围栏对照有效，再验证围栏版本：若接受必须保持完整解析结果；若拒绝必须指出「代码围栏」和「第 2 行」。允许任务卡提出的两种方案，不在定位轮替修复决定策略。
- 两个保护用例验证真正缺少 status 仍被拒绝，以及正文中的围栏示例不覆盖真实头部。
- 连续两次执行均退出 101，结果均为 2 passed、2 failed；失败均来自围栏诊断断言，实际错误均为「缺少状态字段 `status`」。不是编译错误、超时或依赖缺失导致的失败。
- 原始输出：`.autome/output/fenced-status-run1.log`、`.autome/output/fenced-status-run2.log`（本地验证产物，不提交）。
- 既有解析测试：`cargo test --offline -p autome-domain --lib status_block::tests::`，50 passed、0 failed；日志 `.autome/output/status-block-existing.log`。新增测试经 rustfmt 格式化。

本轮没有运行真实任务整理会话或端到端调度验收。修复轮仍需按任务卡补充相应 e2e 证据、提示词及协议版本验证。

## 根因分析

1. `crates/autome-domain/src/status_block.rs:507` 遇到反引号或波浪号围栏便切换 `in_fence`，围栏内所有行直接跳过，没有保存被跳过状态字段及围栏位置。完整状态块因此与真正缺失字段得到同一结果。
2. 同文件 `:763` 起检查必填字段，首先要求 status；`:277` 将该错误格式化为通用缺字段文案。调度器 `crates/automed/src/scheduler.rs:1574` 把解析错误交给 `SessionOutcome::Unparseable`；展示读取入口 `crates/automed/src/dispatch.rs:1767` 也直接使用该错误字符串，没有围栏上下文可供解释。
3. `protocol/prompts/intake.md` 要求生成完整头部，却用 text 围栏展示样例，没有就地提示不要复制围栏。`plan.md` 指向已有禁止规则的 session 协议；`locate.md` 指向明确要求围栏外字段的 bug 协议。因而问题是入口提示不充分，而非所有规则缺失。`loop-protocol.md` 也没有明确的围栏放置要求。
4. 现有单测 `a_status_block_inside_a_fenced_example_is_not_read_as_the_real_one` 只验证示例被忽略，未覆盖唯一真实头部被围栏包裹时的诊断。`crates/automed/tests/end_to_end.rs:4412` 的 `bug_doc()` 先从协议样例中剥离围栏，无法捕捉照搬围栏的输出。

无围栏对照成功排除了字段值和标题位置导致失败的假设；仅添加围栏就稳定失败，与跳过逻辑吻合。修复不能简单读取全文所有围栏，否则会破坏示例隔离；需限定真实头部或提供带行号的明确拒绝。

## 修复说明

第 1/3 修复轮采用方案 (b)：继续拒绝围栏状态块，保持现有协议和示例隔离行为，仅修正诊断。解析器记录头部围栏内字段及围栏起始行；必填字段在围栏外缺失、但在头部围栏内存在时，返回「状态字段不能放在代码围栏里（第 N 行）」并提示移出围栏。围栏外真实字段优先，正文示例不参与缺字段诊断，真正缺失字段仍报原错误。

同步 intake、plan、locate 提示词与 loop 协议的放置说明，强调不要复制样例围栏。协议按 CHANGELOG 规则递增为 `protocol/v5`，记为对既有规则的 `clarify`；本任务固定的 protocol 副本保持不变。提示词快照只增加对应说明，没有修改或弱化定位轮回归测试。

验证结果：

- 原复现命令 `cargo test --offline -p autome-domain --test fenced_status_regression`：修复前 2 passed、2 failed，修复后 4 passed、0 failed。
- `cargo test --offline -p autome-domain --lib status_block::tests::`：52 passed，包含既有 50 项及新增的波浪号围栏中部分字段、正文围栏不能掩盖真实缺字段两个用例。
- `cargo test --offline -p automed --test end_to_end an_intake_fenced_status_block_reports_the_fence_location`：1 passed。真实调度器执行替身整理会话，写入围栏头部后以协议失败结束，原因包含「代码围栏」「第 2 行」，不再包含缺字段文案，且不重试。
- `cargo test --offline -p automed --test end_to_end a_design_document_without_a_status_block_fails_the_task_once`：1 passed，真正缺状态块仍被拒绝。
- `cargo test --offline -p automed --test prompt_rendering`：更新并检查对应快照后 28 passed。intake-card 快照不支持自动更新，手动同步同一句说明后通过；测试逻辑未改动。
- `cargo run --offline -p automed -- protocol eval protocol/`：8 项通过、0 项失败、1 项条文来源警告（43 行缺 CHANGELOG 归因）；新增措辞曾触及正文大小限制，精简后静态门通过。
- 改动 Rust 文件的 rustfmt 检查、`git diff --check` 通过；定位轮回归测试文件无差异。

原始验证输出位于 `.autome/output/fix-*.log`，不提交。未运行全量门禁 `sh scripts/ci-local.sh`，由宿主合并前统一执行；未运行付费模型行为 eval 或真实模型整理会话，以上 e2e 使用项目现有替身会话机制。

## 复核意见

待复核。
