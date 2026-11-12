# notifier-live 并发门禁期限定位

status: 已定位
fix-round: 1/3
复现命令: cd apps/web && node --test scripts/notifier-live-regression.mjs
根因: notifier-live 的命令期限仅 15000ms，重启用例总期限仅 30000ms，健康但较慢的串行命令会被提前终止或被用例总期限截断。
复核: 待复核
repos: apps/web

## 现象

任务卡报告四次并发门禁失败、48 次通过，以及 30000ms 用例超时和「内核在回复前退出 (code=null, signal=null)」。这些历史统计与原始门禁日志本轮未核实。

当前源码仍有上述 15 秒命令期限和 30 秒重启用例期限。重启流程依次读取旧通知、停服、创建任务、运行任务、重启、等待补发。多阶段共享总预算，单阶段均可成功也不代表总时间能满足 30 秒。

## 复现

从当前 worktree 根目录执行状态块命令。需要支持 stripTypeScriptTypes 的 Node（本轮 v26.5.0）；无需 npm 安装、Rust 构建、网络或真实 sleep。

新增 `apps/web/scripts/notifier-live-regression.mjs`：加载真实 notifier-live 源码，擦除 TypeScript 类型并替换依赖导入，在 VM 内获取原始 command 和用例注册。不会执行构建、数据库、HTTP 和真实内核。没有复制被测期限常量。

1. 使用 Node 虚拟计时器模拟一个 20 秒后正常回复的子命令；同时按真实 command 传入的 execFile timeout 安排终止。断言成功回复到达。当前在虚拟 15 秒被终止，产生明确 AssertionError，不靠测试进程超时判失败。
2. 检查真实重启用例注册预算能容纳两个串行 20 秒成功命令以及 5 秒生命周期/补发余量。当前 30000ms 不满足 45000ms 场景，明确断言失败。若去掉局部覆盖则读取真实 Vitest 默认配置。20 秒及 5 秒是受控回归场景预算，并非历史耗时测量。

连续运行两次，两项断言均稳定失败。日志：`.autome/output/notifier-regression-1.log`、`notifier-regression-2.log`。单次小于一秒，无命令缺失或宿主超时。

正对照：仅在 `.autome/output/notifier-control/` 的临时源码副本将命令期限改为 60000ms、原 30000ms 注册期限改为 120000ms，同一测试 2/2 通过，见 `.autome/output/notifier-control.log`。没有修改真实 notifier-live 或产品实现。这只验证回归的辨别能力，不代表修复完成。

未跑完整 Web/Vitest、Rust 或并发门禁：当前 worktree 无 Web node_modules；定位测试刻意不依赖安装和机器负载。回归脚本由状态块命令显式执行，不自动纳入 Vitest。

## 根因分析

`apps/web/test/notifier-live.test.ts` 的 command 使用 `promisify(execFile)`，其 timeout 为 15000。until 同样有 15000ms 截止时间。重启用例显式 30000ms 覆盖了 vitest.config.ts 的 60000ms 默认值。日志包装 step 只记录耗时，没有增加期限。并发负载使有效响应延迟增大时，这些期限会提前判定失败；回归覆盖命令层和总预算层，不证明通知业务逻辑存在缺陷。

必须纠正错误来源的推断：`apps/web/server/core/sidecar.ts` 中该中文错误由 `_finish(code, signal)` 构造；spawn 的 error 回调会主动调用 `_finish(null, null)`，而 exit 回调传递真实退出参数。它与测试 command 的 execFile 是不同子进程链路。execFile 超时通常带 killed/signal 元数据，不能仅凭历史字符串认定它直接产生了 sidecar 的 null/null 错误。

因此已定位并锁定的是任务卡明确提出的测试期限缺陷；历史四次失败是否还包含 spawn 资源不足、清理竞态等原因，仍需原始分阶段日志、sidecar failureReason/底层 error 及退出时序佐证。后续若增强超时诊断，应保留原错误 cause、killed、signal，并区分真正退出、启动失败和超时，不能将所有 null/null 错误改报超时。

## 修复说明

第 1/3 轮仅调整 `apps/web/test/notifier-live.test.ts` 的期限：单条命令由 15000ms 增至 60000ms；两个包含串行创建/运行命令的通知用例由 30000ms 增至 180000ms，给两条命令之外的连接、停启服和通知等待留出余量；初始化 hook 包含三条串行命令，单独增至 240000ms。保留按条件等待、原有断言、分阶段日志和错误传播，不修改产品代码或定位轮回归脚本。

同一个状态块复现命令在修改前退出 1，两项回归均失败；修改后退出 0，2/2 通过、无跳过。日志分别为 `.autome/output/fix-before.log`、`.autome/output/fix-after.log`。这证明受控 20 秒健康命令和 45 秒重启场景的期限缺陷已修复，不代表历史 null/null 错误已复现或修复。

当前 `.autome/config.toml` 已注释宿主 gate，本轮仍按任务卡手动执行 `sh scripts/ci-local.sh`，329 秒后退出 1，日志为 `.autome/output/fix-gate.log`：

- 通过：Format、Clippy、Rust 单元/端到端/适配器测试、Desktop/Web install、Contract、Web lint、Web build。Rust 原有 1 项 ignored，本轮未改动。
- Desktop：104 通过、1 失败；Electron DOM harness 以 SIGABRT 退出，未完成真实 DOM 验收。
- Web typecheck：定位提交中的 `scripts/notifier-live-regression.mjs` 有 14 条隐式 any 类型错误（TS7006/TS7034/TS7019/TS7005）。遵守不得修改定位轮回归测试的要求，未修改该文件，也未排除或关闭类型检查。
- Web test：38 个文件通过、7 个失败；347 项通过、18 项失败、19 项因运行失败被跳过。日志包含本机监听 `listen EPERM: operation not permitted 127.0.0.1` 和文件监听 `EMFILE`。notifier-live 初始化因监听 EPERM 失败，其 3 项测试未执行；不是本轮添加 skip。真实通知链路及并发门禁验收仍未通过。

`git diff --check` 通过。修复范围仅期限和本说明；完整门禁仍失败，保留上述阻断供 Autome 后续处理。

## 复核意见

待独立复核。
