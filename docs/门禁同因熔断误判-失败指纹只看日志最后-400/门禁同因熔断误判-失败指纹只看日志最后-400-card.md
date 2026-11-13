# 门禁同因熔断误判：失败指纹只看日志最后 400 行，前面步骤里的真实失败看不到，三次不同的失败被算成「连续 3 次同一原因」

## 现象

T-50（Bug）在 2026-11-13 07:00–07:29 连续三次合并前门禁失败后被熔断：

```
失败（合并前门禁）：门禁连续 3 次因同一原因失败：Test (unit, end to end, adapter flags)：Error: 内核在回复前退出 (code=null, signal=null)
```

三次的 `gate.result.fingerprint` 都是 `b886b56b86275c1a`，但三次的真实原因各不相同，每次都被上一轮修复解决了：

1. 第 1 次：rebase 后协议版本断言对不上（修复提交 `fdf7beb fix(protocol): align version assertions with v5 clarification`）；
2. 第 2 次：协议 eval 预算超限（修复提交 `7eee424 fix(protocol): keep fenced status clarification within eval budget`）；
3. 第 3 次：`crates/autome-cli/tests/cli.rs` 的 `run_json_tick_error_and_interrupt_have_no_idle_trailer` 时序性失败（与本任务无关，另见同日提的 Bug）。

摘要里那句「Error: 内核在回复前退出」根本不是失败原因：它是 Web test 步骤里一个**通过**的用例打印的日志行。

## 复现步骤

1. 构造一份 `ci-local.sh` 风格的门禁日志（可用 `.autome/output/gates/T-50/merge_gate-7fd7bf81bff5.log` 的结构，542 行）：第 96 行附近是 Rust 测试的 `panicked at …` 与 `test … FAILED`，之后还有 400 多行其它步骤（Desktop、Web test）的正常输出，其中含一行 `Error: 内核在回复前退出 (code=null, signal=null)`，末尾是汇总 `FAILED  Test (unit, end to end, adapter flags)`。
2. 再构造第二份：同样结构，但第 96 行附近换成另一个完全不同的失败用例名与 panic 文本。
3. 对两份日志调 `autome_domain::gate_failure::fingerprint(log, 1, false)`。

## 期望行为

- 两份日志的 `fingerprint.id` 不同（失败的用例不同就不是「同一原因」），不会累计到同因熔断。
- `fingerprint.summary` 指向真实失败：失败步骤名 + 该步骤里的失败行（例如 `test run_json_tick_error_and_interrupt_have_no_idle_trailer ... FAILED` 或 `panicked at crates/autome-cli/tests/cli.rs:1016:5`），而不是别的步骤里通过用例打印的 `Error:` 行。
- 同一个失败连续出现（同一用例、同一 panic 位置）时指纹仍然相同，熔断照常生效；超时、127/126 等既有规则不变；指纹对行号以外的易变内容（临时目录、pid、耗时、会话 id）继续做归一化。
- 读取量仍有上限（现有 256 KiB 窗口可以保留或调整，但不能再因为「后面步骤输出多」就看不到前面步骤的失败）。

## 实际行为

`crates/autome-domain/src/gate_failure.rs` 的 `fingerprint` 只取日志**最后 400 行**（`log.split('\n').rev().take(400)`；`crates/automed/src/gate.rs` 另有「last 256 KiB 里最多 400 行」的窗口）。`ci-local.sh` 的 Rust 测试是靠前的步骤，它失败后后面的 Desktop / Web 步骤照常跑完并输出 400 多行，于是窗口里只剩汇总行 `FAILED  Test (…)` 和后面步骤的无关 `Error:` 行。只要失败发生在同一个步骤，指纹就相同。

后果：Bug 任务三轮修复预算被三个不相干的失败耗尽后还被熔断；需求任务同理会在合并前门禁上被误停。`before_merge` 下所有失败都集中在合并前门禁，这个误判更容易触发。

## 环境

main `bf399db`；`crates/autome-domain/src/gate_failure.rs`（`fingerprint`、`key_line`、`normalize`）、`crates/automed/src/gate.rs`（指纹窗口）、`crates/automed/src/scheduler.rs`（`gate_failure::streak` 的调用处）。门禁命令 `sh scripts/ci-local.sh`。

## 线索（可选）

- 一个方向：先在全窗口里找失败步骤的边界（`ci-local.sh` 每步以 `==> <步骤名>` 开头，汇总里有 `FAILED  <步骤名>`），关键行只从失败步骤的那一段里取；找不到步骤边界时退回现有逻辑。不要把 `ci-local.sh` 的格式写死成唯一来源——别的项目的门禁命令没有这种结构，通用规则仍然要成立（例如优先取 `test … FAILED` / `panicked at` / `error[E…]` / `✖` / `not ok` / `FAIL ` 这类明确的失败标记，再考虑泛泛的 `Error:`）。
- 窗口上限：把「最后 400 行」换成「最后 256 KiB 的全部整行」是否足够，由定位轮按本仓库真实日志（一次完整 `ci-local` 约 2000 行、约 150 KiB）判断。
- 回归测试放在 `crates/autome-domain/tests/` 或 `gate_failure.rs` 的单测里；复现命令用仓库根目录可直接执行的形式（如 `cargo test --offline -p autome-domain gate_failure_window`），不要写成 `cd 子目录 && …`。
