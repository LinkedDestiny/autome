# M-03 · 实现 #4（门禁修复）

结论：保持待审，不推进新里程碑。修复真实失败事件夹具缺少重试配置的问题；新增真实 CLI 回归已红→绿。本地监听受 EPERM 限制，完整门禁未通过，最终以 Autome 在提交后的重跑为准。

## 根因与修复

Autome 在 9fb3d7d54f 的门禁为 Web 277 项中 3 失败、274 通过，失败集中于原有 notifier-live 三项。原样保留这三项断言与等待上限。上一轮审计只审 M-02 且结论通过，没有待搬入的审计临时检查；本次门禁检查本来就在项目测试体系中。

本地原样运行 live 先遇到 listen EPERM，无法声称复现了同一个超时。为区分事件未产生与推送丢失，使用同样的临时 home、真实 autome、退出 3 的假 CLI 运行原 fixture 的 task new/run/events.since/task.get：账本出现 session.retry（attempt=1、of=2），任务仍 active/intake，没有终态 failed。源码 crates/autome-domain/src/config.rs 的默认策略是重试 2 次、首轮等待 60 秒；crates/automed/src/scheduler.rs 在重试尚未耗尽时不会进入 failed。原测试只等 15 秒，故后续读取未读数和重启补发也无法达到预期。

新增 notifier-live-fixture.test.ts，直接执行真实 CLI 并断言 session.retry 为 0、终态 failed 恰为 1。先使用空配置原样重现 fixture，测试退出 1（实际 session.retry 数量为 1）；再将共享 configureFailureRepo 写为 attempts=0、backoff_seconds=0，测试退出 0。live 同样调用该配置函数。生产代码未改，未增加超时、未删断言、未加 skip。检查同类真实通知夹具：w4-ledger.sh 已显式禁用重试，不需改动。

## 命令与结果

原始输出均在 `.autome/output/`；下列退出码是命令退出码。

| 命令 | 退出码 | 结果与日志 |
|---|---|---|
| `sh scripts/ci-local.sh`（修复前启动） | 1 | r4-ci-red.txt：Web 277 项，247 通过、11 失败、19 因初始化失败未执行；4 个监听文件受 EPERM 阻断；其余门禁通过 |
| `npm test --prefix apps/web -- test/notifier-live.test.ts`（修复前） | 1 | r4-live-red.txt：3 项均未执行，listen EPERM |
| `bash .autome/output/r4-fixture-probe.sh` | 0 | r4-probe-red.txt：真实账本证明进入 session.retry 而非 failed；命令成功不代表断言通过 |
| `npm test --prefix apps/web -- test/notifier-live-fixture.test.ts`（空配置） | 1 | r4-fixture-red.txt：1 项失败，实际存在 1 条 session.retry |
| `npm test --prefix apps/web -- test/notifier-live-fixture.test.ts test/notifier-api.test.ts`（修复后） | 0 | r4-fixture-green.txt：8/8 通过，0 跳过 |
| `npm test --prefix apps/web -- test/notifier-live.test.ts`（修复后） | 1 | r4-live-after.txt：3 项仍因 listen EPERM 未执行，未取得双连接/HTTP/重启完整通过证据 |

## 标待审前自审清单

本轮不重新推进状态，仍对 M-03 四项逐项检查。

1. **情形表逐行**：§5 tick 同步返回、bus 完成后再拉通知、慢通知不阻塞后续 tick，API 第 6 项通过；connected 补拉、第 7 项通过。§6 列表九字段/level/未读总数，第 1 项通过；单条/全部已读先落库后广播，第 2/3 项通过；无变更不广播，第 4 项通过；非法请求体拒绝且库不变，第 5 项通过。真实 failed 事件夹具新增回归通过。双连接推送、Origin 拒绝、停服补发仍因 EPERM 未取证。
2. **失败分支**：非法 limit、空输入、非法 ids/all 已实跑；读盘/迁移、超时/离线补齐与明确拒绝由全量 store/notifier 现有测试覆盖。HTTP Origin 拒绝及安全路由前移负向对照受监听限制未取证。此次直接实跑会话退出 3，并检查终态 failed 而非重试。外部服务不可达不适用此次夹具修改：假 CLI 不访问外部服务。
3. **全部迁移**：API 实跑 starting/offline/restarting/connected/stopped，只有 connected 补拉；单条/全部未读→已读、重复已读→无变化、未知 id→无变化，以及 bus 在途→完成→通知在途→后续 tick 均通过。真实 queued→active→退出 3→failed 回归通过；空配置的 active→session.retry 是红色对照。notification/notification-read 双连接及停服→新增事件→重启仍未取证。
4. **输入域边界**：API 实跑 limit 缺省、1、01、200；空串、0、负数、201、小数、指数、空白、非字符串、超安全整数拒绝；ids 单元素、重复、空字符串未知 id、200 元素、64 字通过；空数组、201 元素、65 字、非字符串、ids/all 同给与额外字段拒绝。新夹具实跑 retry.attempts=0 边界。闰日/跨年不适用：修改没有日期输入。

## 数量与交接

上一轮及 Autome 基线 Web 29 文件、277 项；本轮预期 30 文件、278 项，差 +1，全部来自新增的真实 CLI 夹具回归，原有用例未删改断言。Desktop 基线 104 项（103 通过、1 原有跳过），预期差 0。监听初始化造成的未执行项不能称为通过。

M-03 保持待审，状态 implementation-round 为 4/35。Autome 需在最终提交上重跑原门禁，并在可监听环境独立复验 M-03 三项 live 与安全负向对照。本轮没有启动下一会话。

## 最终门禁与提交

- 修复后 `sh scripts/ci-local.sh` 退出 1，用时 211 秒，原始输出 `.autome/output/r4-ci-after.txt`。Format、Clippy、Rust Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过；仅 Web test 未通过。
- Web 实际 30 文件、278 项：248 通过、11 失败、19 因监听套件初始化失败未执行；相对修复前本地 277 项差 +1，新增夹具回归通过。4 个失败文件仍为 gateway-http、eventbus-live、server、notifier-live，日志为 listen EPERM，并有 Vite watcher EMFILE；未将未执行项算作通过。
- Desktop 实际 104 项，103 通过、1 基线跳过，差 0。`git diff --check` 与状态块/里程碑/证据指针/retro 格式检查退出 0。
