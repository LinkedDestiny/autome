# 门禁只在合并前跑一次：`[gate] run = "before_merge"`，实现轮 / 修复轮之后不再跑完整门禁

## 背景

配了 `[gate]` 的需求任务，每个实现轮之后都跑一次完整门禁（Gate），合并前再跑一次（MergeGate），合并后又跑一次（MainCheck）。Bug 任务每个修复轮之后跑 `( 复现命令 ) && ( 门禁命令 )`（FixGate），合并前再跑一次。

2026-11-12 实测：一天 102 次门禁，共 18.6 小时机器时间，中位数 10–13 分钟一次；T-35 一个任务过了 16 次门禁，T-37 过了 8 次。并行 3 个任务时 3–4 个完整 `ci-local` 同时跑，Rust 测试从约 3 分钟涨到 10 分钟，负载下的超时误报又把任务退回实现轮（22 次失败占 4.7 小时）。用户拍板：**降低门禁运行次数，开发全部完成后再统一跑门禁，避免门禁抢占**。

## 要做什么

1. 项目配置 `.autome/config.toml` 的 `[gate]` 新增 `run`，取值：
   - `"each_round"`（缺省）：与今天逐事件相同；
   - `"before_merge"`：实现轮 / 修复轮之后不跑完整门禁，只在合并前（MergeGate）跑。
   其它值在解析时报错，文案与现有 `[gate]` 校验同一风格。全局配置不认这个键。
2. `before_merge` 下的需求 Loop：实现轮结束后直接去它在「没配门禁」时会去的地方（审计，或审计关闭时的下一步），不进 `Gate` 节点、不写 `gate.started`。所有里程碑完成、复盘、rebase 之后的 MergeGate 照常跑完整门禁；失败照现有路径退回实现轮并带 `Inject::GateFailed`，修好后再回到 MergeGate。
3. `before_merge` 下的 Bug Loop：FixGate **只跑复现命令**（验证这一轮修复确实让复现命令通过，秒级到分钟级），不拼门禁命令；MergeGate 仍是 `( 复现命令 ) && ( 门禁命令 )`。126/127 不算失败、同因失败熔断等现有规则不变。
4. 审计 / 复核简报里的宿主门禁证据：`before_merge` 下没有本轮门禁结果时，简报写明「本项目门禁只在合并前运行，本轮没有宿主门禁结果」，不留空、不沿用旧结果。金样测试覆盖。
5. 读者：`autome status <任务>` 的门禁行、`task.get.gate` 在没有结果时的显示不变；`autome run` 不新增事件。README Host gate 一节与 `docs/development/technical-design.md` §8.1 各补一段，写明取舍：门禁次数从「每轮一次」降到「每个任务一次」，代价是门禁问题到合并前才暴露、会多退回一次实现轮。
6. 本仓库的 `.autome/config.toml` 不在本任务里改，由用户合并后自行加 `run = "before_merge"`。

## 不做什么

- 不改 `each_round`（缺省）下的任何行为，现有用例不改就通过。
- 不做跨任务的合并队列（几个任务一起 rebase、只跑一次门禁）——另提。
- 不改 MainCheck（T-60 处理合并后重复的那一次）、不改门禁排队（T-47）。
- 不改 `scripts/ci-local.sh`，不改会话自己跑测试的方式。

## 验收标准

1. 单测（`autome-domain`）：`before_merge` 下实现轮完成后的转换等于没配门禁时的转换；MergeGate 通过 / 失败两条路与今天相同；`each_round` 下所有现有转换测试不改。
2. e2e（需求）：`run = "before_merge"`，两个里程碑的任务从创建到合并，门禁脚本恰好被调用一次（MergeGate），事件里没有 `node: gate` 的 `gate.started`；让门禁第一次失败 → 退回实现轮且 prompt 带门禁尾部 → 修好后再过 MergeGate → 合并。
3. e2e（Bug）：`before_merge` 下 FixGate 的命令只含复现命令，MergeGate 的命令含两者；修复没生效时 FixGate 失败退回修复。
4. 解析：`run = "sometimes"` 报错；缺省与显式 `each_round` 行为相同；`save_project` 不改写用户的 `[gate]`。
5. 简报金样测试覆盖第 4 条文案。
6. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

## 参考

- `crates/autome-domain/src/task.rs`：实现轮之后进 `Gate` 的转换、`after_implement_without_audit`、`MergeGate`。
- `crates/autome-domain/src/bug_loop.rs`：`Node::Fix => core(Node::FixGate)`、FixGate / MergeGate 的结果处理。
- `crates/automed/src/scheduler.rs` 的 `run_gate`（FixGate / MergeGate 命令拼装）、`crates/automed/src/brief.rs`（门禁证据）。
- `crates/autome-domain/src/config.rs` 的 `GateConfig`、`crates/automed/src/config_io.rs` 的 `[gate]` 解析（`same_failure_limit` 的写法可照搬）。
- T-47（门禁排队）、T-60（跳过重复的验证 main）改相邻代码，rebase 时注意。
