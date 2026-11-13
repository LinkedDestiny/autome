# 单个会话的时长上限：超时就终止并按重试策略处理

## 背景

内核对会话只有一种兜底：日志长时间不再更新（`scheduler.rs` 的 `log_idle_secs` 心跳兜底）就判定会话消失。一个**一直有输出**的会话可以无限跑下去——比如实现轮反复跑测试、反复改同一处、或者 CLI 在工具调用里打转。预算（`budget_factor`、`fix_rounds`）只限制轮数，不限制一轮有多长。宿主门禁有 `[gate] timeout_minutes`，会话没有对应的东西。

2026-11-12 的 T-35（W5）实现第 7 轮从 16:12 跑到 17:14 才结束，第 8 轮 17:14 开始；并行 3 个任务时一个失控会话就占掉三分之一的产能，而且费用一直在涨。靠 Loop 自主推进项目的产品需要这条安全线。

## 要做什么

1. 配置：`[loop] session_minutes`，全局默认值和项目覆盖项都要有（照 `parallel` / `design_rounds` 在 `LoopDefaults` / `LoopOverrides` 里的写法）。默认 120。`config.validate` 要求 ≥ 5。
2. tick 里检查每个在跑的会话：`started_at` 到现在超过上限，就用和人叫停一样的方式结束它（整个进程组），记事件 `session.timeout { session_id, minutes, limit }`。
3. 结果处理和会话崩溃一样走 `[retry]`：计一次重试，退避后在原节点重新派一个会话；重试用尽才进入失败。**不**算作协议失败，也不消耗轮数预算。
4. 下一个会话的 prompt 里注入一段说明（新的 `Inject` 分支，文案放 `launcher.rs`，加金样测试）：上一个会话因为超过 N 分钟被终止，附最后 40 行日志，要求这一轮先把进度写进文档、缩小本轮目标。
5. 可见：`autome status <任务>` 的会话行显示"已运行 37 分钟 / 上限 120"；`session.log`、`task.get` 的会话条目带 `timed_out: true`；`dashboard.get` 等你处理的"失败"理由里能看出是超时。

## 不做什么

- 按角色（设计 / 实现 / 审计）分别设上限。
- 按费用设上限（等 T-44 把费用汇总做出来之后另提）。
- 改变心跳兜底的行为。

## 验收标准

1. e2e：假 CLI 每秒写一行日志永不退出，上限设得很短，驱动在上限到了之后终止会话，任务留在原节点、会话行是超时、事件里有 `session.timeout`，随后按 `[retry]` 重新派一个会话；重试用尽后任务失败，失败原因写明超时。整条用例 30 秒内跑完（上限在测试里怎么缩短由设计决定，但不能给生产留一个测试专用的后门开关；把单位做成秒再由 `session_minutes` 换算是可以接受的做法）。
2. e2e：会话在上限之前正常结束，行为与今天完全相同，现有 e2e 不改就过。
3. 单测：终止后 wrapper 和它的子进程都不在了（照 stop 的现有测试）。
4. 金样测试覆盖注入文案。
5. `config.validate` 对 `session_minutes = 1` 报错，对缺省值通过；文档 `README.md` 配置一节加一行。
6. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

## 参考

- `crates/automed/src/scheduler.rs`：`log_idle_secs`、`reap_session`、重试退避（`session.retry`）、`stop_running_session` 的进程组终止方式。
- `crates/autome-domain/src/config.rs`：`LoopDefaults`、`LoopOverrides`、`RetryPolicy`、`GateConfig.timeout_minutes`。
- `crates/automed/src/launcher.rs`：`Inject` 各分支与金样测试。
- `crates/automed/src/gate.rs`：门禁超时（exit 124）的处理可以参考。
