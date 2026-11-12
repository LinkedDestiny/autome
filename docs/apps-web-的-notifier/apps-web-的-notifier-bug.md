# notifier-live 并发门禁期限定位

status: 已定位
fix-round: 0/3
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
