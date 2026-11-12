# M-05 r9 实现：变基冲突解决与复验

## 范围与状态

简报、设计与审计 #7 均确认没有开放或待审里程碑，M-01–M-05 已由审计关闭。本轮执行用户追加的 rebase 要求，不虚构里程碑、不重新标记已完成或待审。证据归档于最后里程碑 M-05，implementation-round 为 9/25。H-01–H-03 保留为人工验收。

## 变基与冲突解决

- 默认分支由 origin/HEAD 确认是 main；本地 main 为 `ce6ec5f074dfb5c38bdb3a7b3568ffd14420d192`，变基前 HEAD 为 `8c64283477762730102fa5f64f1e38d675e42078`。
- 执行 `git rebase main`，逐处解决冲突并 `git add`、`GIT_EDITOR=true git rebase --continue`，45 步完成；`git merge-base --is-ancestor main HEAD` 返回 0。未 fetch、push、切换分支或操作其它 worktree。
- `crates/automed/tests/end_to_end.rs`：保留主线 merge.after 钩子的完整测试及费用汇总测试，正确分开函数结尾。
- `README.md`、`crates/autome-domain/src/config.rs`：同时保留 merge.after 配置示例、MergeAfterConfig 与预算示例、BudgetConfig。
- `crates/automed/src/store.rs`：保留 conclude_merge_after 和 raise_budget_alert 两个独立事务，各自提交与返回。
- `crates/autome-cli/src/render.rs`：同时保留流事件解析、流测试和费用格式化、预算文案测试。
- `crates/autome-cli/tests/cli.rs`：保留主线完整 run JSON、logs stream 测试，追加本分支完整 cost_fixture 与三个费用用例，避免把相似片段混进不同函数。
- 对上述 e2e、CLI 测试、render 文件，分别与 main、变基前 HEAD 比对函数名，双方缺失集合均为空（`.autome/output/r9-function-preservation.txt`）。这只是保留性检查，运行正确性以测试结果为准。
- 首次 fmt 检查发现 config.rs 合并处多余空行，`cargo fmt --all` 修复；后续完整门禁重新检查。原始变基日志位于 `.autome/output/r9-rebase*.txt`。
- 上轮审计没有退回实现缺陷，故无待逐字迁入项目测试体系的审计检查，也无应红的缺陷复现。

## 四项自审（不重新标待审，仍逐项执行）

1. **情形表逐行**：M-05 表中的任务费用行、零会话费用行由 `cost_cli_status_task_prints_the_cost_line_under_state` 精确检查；ls 费用列、有星号脚注、无星号及空列表不打印脚注由 `cost_cli_ls_appends_cost_and_a_footnote` 检查；运行中行由 `cost_cli_running_row_shows_cost` 检查。五个 cost_cli_ 用例全部通过。
2. **失败分支**：M-05 展示逻辑没有新增拒绝授权、读盘失败、超时或外部服务分支，不适用；缺字段降级由 `cost_cli_cell_marks_tasks_with_sessions_without_cost` 覆盖。预算读取失败、事务失败由完整门禁中的既有 budget_alert_ 测试验证，不修改其断言。
3. **全部迁移**：本轮未改变状态机，不适用。文本与 JSON 两通道分别验证 task status、ls、运行中 status；project ls JSON 汇总也由上述用例断言。主线新增的 merge.after、stream 功能与费用代码分别保留，随完整门禁验证。
4. **输入域边界**：既有测试覆盖金额 0、3.75、1234.5、缺字段、无计费数 0/1、空列表、零会话与运行中会话不计费用；全部通过。本轮不新增日期或数值输入域，闰日、跨年等不适用。

## 验收与数量基线

- `cargo test -p autome-cli cost_cli_`：退出 0，共 5 个（render 2、CLI 3），与上一轮 5 个相比差 0。
- 完整门禁命令：`sh scripts/ci-local.sh > .autome/output/r9-ci.txt 2>&1`；结果见下方最终汇总。专项不重复计入全量总数。
- 基线采用上一实现轮明确统计：Rust 1387、Desktop 105、Web 386，合计 1878；审计 #7 的 Rust 主要组相加为 1386（未计 1 个 ignored），与此前总数一致。
- 静态核对变基前后 Rust 测试定义：主线新增 36 个，CLI main +3、render +5、CLI 集成 +4、domain config +1、merge_after +3、config_io +4、dispatch +1、scheduler +5、store +2、e2e +8。本轮未增删测试；最终实际执行数另列。

### 最终汇总

| 层 | 上一实现轮总数 | 本轮总数 | 差 | 结果 |
|---|---:|---:|---:|---|
| Rust | 1387 | 1423 | +36 | 1422 通过，1 ignored，0 失败 |
| Desktop | 105 | 105 | 0 | 104 通过，1 失败 |
| Web | 386 | 386 | 0 | 349 通过，18 失败，19 skipped |
| 合计 | 1878 | 1914 | +36 | 1875 通过，19 失败，20 ignored/skipped |

Rust 实际分组为 45、23、418、1 ignored、756、145（e2e）、28、7，总数 1423；空测试程序不计数。36 个增量全部来自上述主线新增测试，没有缩小测试范围。费用 CLI 专项与全量重复的 5 个不重复计数。

`sh scripts/ci-local.sh` 返回 1，用时 242 秒。Format、Clippy、Rust 全量、Contract、Web lint、Web typecheck、Web build 全部通过；Desktop、Web test 未通过。完整日志 `.autome/output/r9-ci.txt`，费用专项日志 `.autome/output/r9-cost-cli.txt`。

- Desktop 的唯一失败：`dom-harness.js exited null, signal=SIGABRT`，真实 Electron DOM 断言未执行，不能视为通过。
- Web：45 个文件中 7 失败、38 通过；错误日志包含 `listen EPERM: operation not permitted 127.0.0.1`，server suite 启动还报告 `EMFILE: too many open files, watch`。18 个失败涉及真实 HTTP 网关与 SSR，19 个 skipped 也未当作通过；原始错误保留在门禁日志。本轮不修改测试跳过条件、不削弱断言、不将环境错误标成协议失败。
- 上轮失败的 Rust 队列用例本轮全量通过；本轮没有修改该行为。费用汇总、预算与主线 merge.after e2e 均随 145 个 e2e 通过。

结论：变基完成，Rust 与费用专项通过；全量门禁仍被 Electron、Web 运行环境错误阻断，不宣称全部验收通过。保留既有审计关闭状态，后续需在可运行 Electron、允许本地监听且文件监视资源充足的环境复跑门禁，并完成 H-01–H-03。

`git diff --check` 与状态块/里程碑一致性检查通过。提交本轮记录及格式修复到当前分支；不推送、不启动下一会话。
