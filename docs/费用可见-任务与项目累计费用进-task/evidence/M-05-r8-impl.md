# M-05 r8 实现：默认分支 rebase 与兼容验证

## 本轮范围与状态

简报、设计与最近审计一致：M-01–M-05 均已由审计关闭，没有开放里程碑、没有审计退回缺陷。本轮执行用户明确追加的 rebase 指令，不虚构开放项、不把已完成改成待审；保留既有审计状态，implementation-round 更新为 8/25。本文件归档在最后里程碑 M-05 下，不代表重新完成该里程碑。H-01–H-03 仍待人工验收。

## rebase 与修复

- 基点：本地默认分支 main，SHA `90802f65af43734a1235d9f51ad7f10d7d832a8d`；原分支 HEAD `1d74aec`。未 fetch、未 push、未切换分支或操作其它 worktree。
- `git rebase main`，逐个解决冲突后 `git add`、`GIT_EDITOR=true git rebase --continue`，39 步完成。`git merge-base --is-ancestor main HEAD` 退出 0。
- `end_to_end.rs`：保留主分支全部 review-scope / task.convert 等测试，同时保留费用汇总与预算 e2e。
- `dispatch.rs`：看板同时保留 `gate_queue_position` 和费用参数；编译发现新增 `task.convert` 返回路径缺失，恢复完整 JSON 返回并使用新签名计算费用。
- `config_io.rs`：两侧新增测试均保留，补全函数边界与 `#[test]`。
- `cli.rs`：保留主分支 task.convert 测试及完整费用 fixture/用例。
- `dom.test.js`：沿用主分支已具备的 signal/error 诊断，不改退出码或断言。
- 原始日志：`.autome/output/r8-rebase*.txt`；rebase 后完整门禁首跑日志 `.autome/output/r8-ci.txt`。首跑编译失败是冲突解决时遗漏返回对象造成，修正后重跑 Rust、fmt 和 clippy；首跑不作为最终源码通过证据。

## 标待审前四项自审（本轮不改为待审，仍逐项执行）

1. **情形表逐行**：M-05 的任务费用行、零会话行、列表费用列、有/无脚注、运行中费用行由既有 `cost_cli_` 五个测试覆盖；全量 CLI 中五个均通过。`task.convert` 原有 CLI 用例也通过，保留新增路径响应结构。
2. **失败分支**：CLI 展示没有新增授权、读盘、超时、外部服务失败分支，不适用；空列表和缺字段由既有 CLI 测试覆盖。预算配置读取失败的既有回归通过。上轮审计通过，没有需要逐字迁入并红绿复现的退回检查。
3. **全部迁移**：本轮不改变状态机，不适用。文本/JSON 通道由五个 CLI 用例分别覆盖 status 任务、ls、运行中 status 和 project ls JSON；task.convert 路径通过既有 CLI 用例验证。
4. **输入域边界**：CLI 测试覆盖 0/3.75/1234.5、无计费计数 0/1、缺字段、空列表、运行中与已结束会话；不新增日期或预算输入域，日期边界不适用。预算配置既有边界测试随 Rust lib 运行通过。

## 未闭合问题

- Rust 主分支门禁队列测试 `scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 在第 225 行失败，期望 C 的 gate.started 为 0，实际 1。单独复跑仍失败（`.autome/output/r8-queue-retry.txt`）。该测试、gate.rs、gate_queue.rs 与 main 无差异。夹具无 PID、仅写 started，而 gate.rs 对无 PID 的宽限为 10 秒；单跑耗时 15.56 秒，存在时序相关的可能，但未把此推断当作已证明根因，也未修改主分支队列行为。
- Desktop：105 个中 104 通过、1 失败，Electron harness 启动 SIGABRT，内部 DOM 断言未执行；不是已通过。
- Web 出现真实 HTTP/SSR 初始化失败及监听限制，详见本轮完整门禁日志；不弱化测试或改 skip 条件。

## 验收命令与基线

- `cargo fmt --all -- --check`：修正后退出 0（`r8-fmt-check.txt`）。
- `cargo clippy --all-targets -- -D warnings`：修正后退出 0（`r8-clippy.txt`）。
- `cargo test --workspace --all-targets`：退出 101，前五个测试程序共 1215 个（1213 通过、1 失败、1 ignored），被上述队列测试阻断；日志 `r8-rust.txt`。费用单测及 CLI 五个专项全部通过。
- 为补足被提前中止的测试层，另跑 `cargo test -p automed --test end_to_end`（`r8-e2e.txt`）以及 `cargo test -p automed --test prompt_rendering --test real_cli`（`r8-other.txt`，28+7 全通过）。这些是不同 suite，统计不重复计数。
- `sh scripts/ci-local.sh`：首跑退出 1，344 秒；Contract、Web lint、Web typecheck、Web build 通过。首跑 Format/Clippy/Rust 的编译失败已在修正后重跑，不冒充整条门禁最终通过。
- Desktop 105 个（104 通过、1 失败）；Web 386 个（349 通过、18 失败、19 skipped）。其产品代码未因后续 Rust 返回对象修复而变化。原始日志全部在 `.autome/output/`。
- 最近审计 r7 只记 CLI 5 个及全量绿，没有逐层总数。因此比较采用最近明确的总数基线 `M-05-r7-impl.md`：Rust 1223、Desktop 105、Web 302，合计 1630。Web +84 来自主分支新增测试；Desktop 差 0，仍未执行内部 DOM 断言。对比审计的全绿不能说明本轮环境全绿。

### 最终汇总（不重复计入单跑）

完整 e2e：137/137 通过，286.27 秒；其中费用汇总 1 个与预算 2 个均通过。Rust 合计 1387 个（1385 通过、1 失败、1 ignored），相对上一明确基线 1223 增加 164，来自 rebase 引入的主分支门禁队列、任务转换、回归保护等测试；本轮没有增删测试。

| 层 | 上一明确基线 | 本轮总数 | 差 | 结果 |
|---|---:|---:|---:|---|
| Rust | 1223 | 1387 | +164 | 1385 通过、1 失败、1 ignored |
| Desktop | 105 | 105 | 0 | 104 通过、1 失败 |
| Web | 302 | 386 | +84 | 349 通过、18 失败、19 skipped |
| 合计 | 1630 | 1878 | +248 | 1838 通过、20 失败、20 ignored/skipped |

结论：rebase 已完成；费用相关既有测试通过，但当前全量门禁不通过。已完成里程碑的历史审计结论保留，本轮不宣称最终分支全量验收通过。合并前需复核队列失败、在允许 Electron/监听的环境重跑门禁并完成 H-01–H-03。

`git diff --check` 通过，状态块保持协议格式与既有里程碑表一致。改动提交到当前任务分支；不推送、不启动下一会话。
