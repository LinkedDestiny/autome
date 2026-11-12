# apps/web 的 notifier-live 用例在门禁并发时超时（"内核在回复前退出"），让门禁误判失败

## 现象

`apps/web/test/notifier-live.test.ts` 的用例「停服期间新增失败，重启只补一条且保留旧已读」今天在驱动跑的门禁里失败 4 次、通过 48 次：

- 失败：`T-35 gate 17:14`、`T-42 fix_gate 17:45`、`T-35 gate 18:08`、`T-37 gate 18:09`（日志在 `.autome/output/gates/<task>/`）；
- 4 次全部发生在 3–4 个门禁（每个都是完整 `sh scripts/ci-local.sh`，含 cargo test）同时在跑的时段；其余 48 次（机器较空）全部通过。

失败原文（`T-37/gate-ecd9edf2aef6.log` Web test 段）：

```
× 停服期间新增失败，重启只补一条且保留旧已读 30005ms
Error: 内核在回复前退出 (code=null, signal=null)
FAIL test/notifier-live.test.ts > 停服期间新增失败，重启只补一条且保留旧已读
Error: Test timed out in 30000ms.
```

后果：T-35 和 T-37 各被退回实现轮重做一次，改动本身并没有问题；`[gate] same_failure_limit` 也会被这种环境抖动吃掉。

## 复现步骤

1. 在 `apps/web` 里让机器处于高负载（例如并行跑 3 个 `cargo test --workspace`），再跑 `npx vitest run test/notifier-live.test.ts`；
2. 用例在 30 秒处超时，错误是「内核在回复前退出」。

稳定的自动化复现：把 `notifier-live.test.ts` 里 `execFile` 的 `timeout: 15000` 与用例的 `30000` 当作被测常量，用一个人为放慢的内核（或假的 `AUTOMED_BIN`，响应前 sleep 超过现有期限但远小于合理期限）证明用例因期限过短而失败，而不是因为逻辑错误。

## 期望行为

- 与 T-33 对 Rust e2e 的处理一致：用例等待的是"事件到达 / 进程退出"这种条件，期限应当宽松到在并发门禁下不误报（T-33 把 30 秒等待改成了按条件等待 + 充足上限）；
- 子进程被 `execFile` 的 timeout 杀掉时，错误里要说清是超时（现在是 `code=null, signal=null` 的「内核在回复前退出」，看不出是期限问题）。

## 实际行为

见现象。

## 环境

- `apps/web/test/notifier-live.test.ts`：`execFile(..., { timeout: 15000 })`（第 17 行），三个用例各 `30000`（第 31/36/48 行）；
- 门禁：`[gate] command = "sh scripts/ci-local.sh"`，`[loop] parallel = 3`，常有 3–4 个门禁并发。

## 线索（可选）

- 参考 T-33（`crates/automed/tests/end_to_end.rs` 的 `settle` / 截止时间改法）。
- 根因层面的缓解在 T-47（门禁按机器排队），但用例自身的期限也应当经得起并发。
