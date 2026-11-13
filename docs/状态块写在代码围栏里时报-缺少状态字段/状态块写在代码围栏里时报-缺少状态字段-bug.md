# 状态块围栏导致误导性缺字段错误

status: 已定位
fix-round: 5/5
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

第 5/5 修复轮：已将本分支 rebase 到本地默认分支 `main`（`d9ad281`），逐步解决 `protocol/CHANGELOG.md` 冲突并完成 rebase，未使用 merge。冲突原因是 main 与本任务都新增了 v5：保留 main 的 Q-1、Q-2 记录，将本任务 F-1 围栏说明顺延为 `protocol/v6`，同步协议版本单测和冻结协议 e2e 的明确版本断言为 v6，其它断言保持不变。沿用已通过的解析器修复，没有新增无关产品改动。

本轮验证结果：

- 原复现命令 `cargo test --offline -p autome-domain --test fenced_status_regression`：4 passed。
- `cargo test --offline -p autome-domain --lib status_block::tests::`：52 passed。
- `cargo test --offline -p automed --lib protocol::`：69 passed，包含协议版本与体积预算检查。
- `cargo test --offline -p automed --test prompt_rendering`：31 passed。
- `cargo test --offline -p automed --test end_to_end an_intake_fenced_status_block_reports_the_fence_location`：1 passed。
- `cargo test --offline -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：1 passed。
- 对比定位提交 `3a26e64`，`fenced_status_regression.rs` 无差异；未修改、删除、跳过或弱化定位回归测试。`git diff --check` 通过，已确认 main 是当前 HEAD 的祖先。

原始输出位于 `.autome/output/round5-*.log`，不提交。未运行 `sh scripts/ci-local.sh`、全量测试或真实模型会话，宿主仍需执行合并前全量门禁。状态保持已定位、复现命令不变，复核重置为待复核；下方前四轮说明与复核意见均为历史记录。

第 4/5 修复轮：当前起点为第 3 轮复核提交 `7fd7bf8`，工作区干净；未提供新的失败用例或宿主失败日志，也没有 replies 补充文件。本轮原样执行定位时的复现命令及直接相关测试，均通过，未复现新的产品缺陷。因此保留前三轮修复与原根因，不追加没有失败依据的产品代码改动；仅按本轮预算更新状态并重置复核。下方前三轮修复、复核意见为历史记录，不代表本轮已复核。

本轮验证结果：

- `cargo test --offline -p autome-domain --test fenced_status_regression`：4 passed。
- `cargo test --offline -p autome-domain --lib status_block::tests::`：52 passed。
- `cargo test --offline -p automed --lib protocol::`：67 passed，包含协议版本与体积预算检查。
- `cargo test --offline -p automed --test prompt_rendering`：28 passed。
- `cargo test --offline -p automed --test end_to_end an_intake_fenced_status_block_reports_the_fence_location`：1 passed。
- `cargo test --offline -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：1 passed。
- 对比定位提交 `3a26e64`，`fenced_status_regression.rs` 无差异；未修改、删除、跳过或弱化回归测试。`git diff --check` 通过。

原始输出位于 `.autome/output/round4-*.log`，不提交。未运行 `sh scripts/ci-local.sh`、全量测试或真实模型会话；相关子集通过不代表宿主全量门禁通过。若宿主仍失败，需由后续节点提供具体失败命令及日志，才能定位剩余问题。

第 3/3 修复轮：复现到 `protocol::eval::tests::a_code_sample_that_is_not_a_status_block_is_not_parsed` 失败。第 1 轮新增协议说明后，正文虽未超预算，但该既有测试追加 shell 示例后达到 24601 字节，超过 24576 字节上限 25 字节。复制协议并追加完全相同的示例执行静态 eval，确认示例解析通过，失败来自正文体积；不是状态解析器再次回归。

最小修复仅精简 `protocol/loop-protocol.md` 中第 1 轮新增的一句说明，保留标题下一行、首个二级标题前、展示围栏不得复制的要求。正文缩至 24533 字节，追加测试示例后为 24571 字节。属于本任务尚未合并的 v5 措辞修正，沿用已有 clarify 记录，不再递增协议版本。不改解析器、预算上限、任何测试或任务固定的协议副本。

本轮验证：

- 原复现命令：4 passed；定位提交起 `fenced_status_regression.rs` 无差异。
- `cargo test --offline -p automed --lib protocol::eval::tests::`：修复前目标用例 1 failed，修复后完整子集 28 passed。
- `cargo test --offline -p autome-domain`：通过；`cargo test --offline -p automed --test prompt_rendering`：28 passed。
- 静态协议 eval：8 项通过、0 项失败，条文归因警告仍存在。
- 定位时启动的 `cargo test --offline -p automed --test end_to_end`：146 passed；`git diff --check` 通过。
- 定位时扩展运行 automed lib：759 passed、2 failed；除上述已修复用例外，`gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 的队列启动计数断言失败，单独原样复跑 1 passed。未修改该测试或队列代码，不能据此宣称全量门禁已通过。

日志位于 `.autome/output/round3-*.log`，不提交。未运行 `sh scripts/ci-local.sh`，留给宿主。复核已重置为待复核；下方前两轮复核意见仅为历史记录。

第 2/3 修复轮：上一轮将协议递增为 `protocol/v5`，但协议版本单测与任务冻结协议的 e2e 仍硬编码期望 `protocol/v4`。本轮分别实跑两个用例，均因实际 v5 与期望 v4 不符而失败（不是围栏解析再次失败）。宿主提供的末尾日志没有具体失败用例，因此这里只确认本地复现的两处失败，不声称已排除全量门禁中的其它问题。

最小修复仅将 `crates/automed/src/protocol/mod.rs` 和 `crates/automed/tests/end_to_end.rs` 中两处既有版本期望同步为 v5，仍严格校验明确版本；保留协议哈希、冻结引用及其它全部断言。没有修改定位轮的 `fenced_status_regression.rs`，没有更改解析器、协议正文或本任务固定的协议副本。

本轮验证结果：

- 原复现命令 `cargo test --offline -p autome-domain --test fenced_status_regression`：4 passed。
- `cargo test --offline -p autome-domain --lib status_block::tests::`：52 passed。
- `cargo test --offline -p automed --lib protocol::tests::`：11 passed；其中版本用例修复前 1 failed。
- `cargo test --offline -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：修复前 1 failed，修复后 1 passed。
- `cargo test --offline -p automed --test end_to_end an_intake_fenced_status_block_reports_the_fence_location`：1 passed。
- 两个改动 Rust 文件的 rustfmt 检查及 `git diff --check` 通过。

原始输出保存于 `.autome/output/round2-*.log`，不提交。未运行全量门禁 `sh scripts/ci-local.sh`，留给宿主合并前执行。本轮复核重置为待复核，下方第 1/3 轮复核意见仅为历史记录。

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

通过（第 4/5 轮复核）。

- 第 4 轮差异 ad822c1 仅改本 bug 文档（状态与说明），无产品代码、测试变动；符合「无新失败依据不追加改动」的最小原则。
- 复现命令独立复跑：4 passed；`fenced_status_regression.rs` 自定位提交 3a26e64 起无差异，回归测试未被删除、跳过或弱化。
- 相关子集复验：`automed --lib protocol::` 67 passed；e2e `an_intake_fenced_status_block_reports_the_fence_location` 1 passed。
- 根因与前几轮结论不变，成立。未跑全量门禁，由宿主合并前运行。

通过（第 3/3 轮复核）。

- 复现命令独立复跑：4 passed；定位提交 3a26e64 起 `fenced_status_regression.rs` 无差异，回归测试未被删除、跳过或弱化。
- 第 3 轮差异 7eee424 仅改 `protocol/loop-protocol.md` 一行（精简措辞，保留“标题下一行、首个二级标题前写状态块；不得复制展示用围栏”），不碰解析器、预算上限或测试，范围最小。
- 相关子集复验：`protocol::eval::tests::` 28 passed；`protocol::` 全部 67 passed；`prompt_rendering` 28 passed。
- 根因与前两轮结论不变；未跑全量门禁，由宿主合并前运行。

通过（第 2/3 轮复核）。

- 复现命令独立复跑：4 passed、0 failed；定位提交 3a26e64 起 `fenced_status_regression.rs` 无任何差异，回归测试未被删除、跳过或弱化。
- 第 2 轮差异 fdf7beb 仅两行：`protocol/mod.rs` 版本单测与 `end_to_end.rs` 冻结引用断言由 `protocol/v4` 同步为 `protocol/v5`，与上一轮合法的协议递增一致，仍严格校验明确版本，哈希等其它断言保留。
- 相关子集复验：`protocol::tests::` 11 passed；e2e `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task` 1 passed。
- 根因与第 1 轮结论不变，范围最小，无无关改动。
- 未跑全量门禁，由宿主合并前运行。

第 1/3 轮复核意见（历史记录）：
通过（第 1/3 轮复核）。

- 复现命令独立复跑：4 passed、0 failed；定位提交 3a26e64 的测试文件在 aa19642 中无改动，未删除、跳过或弱化。
- 相关子集：`status_block::tests::` 52 passed；e2e `an_intake_fenced_status_block_reports_the_fence_location` 1 passed；`prompt_rendering` 28 passed。
- 根因成立：解析器丢弃围栏内字段与位置，导致合法字段被误报缺失。修复只在缺字段时回查头部围栏内同名字段并报行号，围栏外字段优先，正文围栏示例不参与，隔离行为不变。
- 回归测试有意义：既有对照的无围栏版本，加上围栏版本的诊断断言，另有真缺失与正文示例两个保护用例，新增单测覆盖波浪号与正文围栏。
- 范围：产品改动集中于 status_block.rs，其余为提示词说明、快照与 CHANGELOG，符合任务要求，无无关改动。
- 未跑全量门禁，由宿主合并前运行。
