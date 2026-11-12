# M-05 r10 实现轮：通知重启门禁修复候选

## 范围与结论

implementation-round 为 **10/25**，基线 HEAD aaeb036，宿主失败提交 755af52b8f0a。本轮只修复门禁测试，不推进新里程碑；M-01..M-05 已由审计关闭，表格原样保留。修改通知集成测试的服务模式为 production，显式在独立 beforeAll 中构建产物。**本地没有取得原超时检查红转绿证据；修复候选需 Autome 在提交后复验。**

## 原检查与证据

- 宿主日志（只读）：/Users/dannie/project/autome-v2/.autome/output/gates/T-35/gate-755af52b8f0a.log。错误摘录：.autome/output/r10-host-errors.txt。
- 原检查名称逐字为「停服期间新增失败，重启只补一条且保留旧已读」，已经在 apps/web/test/notifier-live.test.ts 项目体系内，无临时检查需要搬迁；审计 #6 为通过，无审计退回检查。
- 宿主红灯：Test timed out in 30000ms。read existing notification 为 5 ms，close server 为 **28057 ms**，随后 create offline task 尚未结束便耗尽 30000 ms。前一次宿主同一关闭阶段为 11412 ms。证据确定慢在关闭阶段，不证明通知重放本身错误，也不能定位关闭阶段内具体资源。
- 改前原样运行 npm test --prefix apps/web -- test/notifier-live.test.ts：退出 1，3 条均被 beforeAll 的 listen EPERM 阻断；日志 .autome/output/r10-notifier-before.txt。这是环境失败，不能冒充宿主超时复现。
- 无监听探针调用真实 createHttpServer 再 closeMiddleware：development 创建 6476.64 ms、关闭 3.85 ms，期间有 watcher EMFILE；production 创建 257.90 ms、关闭 0.015 ms。见 r10-middleware-probe.txt、r10-production-probe.txt（均在 .autome/output/）。探针未复现 28 秒慢关闭。

## 修改及取舍

通知用例只通过 /api/notifications、/api/notifications/read 和 /ws 验证真实内核事件、双连接广播、权限拒绝、已读持久化及离线重放，不请求开发页面或 HMR。用 production 启动和重启，使该检查不再依赖 Vite watcher/HMR 的创建与回收；不改产品关闭逻辑，不宣称解决开发服务慢关闭。开发 SSR/HMR 的 server.test.ts、shell-live.test.ts、load-context.test.ts 覆盖保留。

原三个 test 的名称、断言、顺序以及初始化/重启原有 30000 ms 限时逐字保留，无新增 skip。仅新增独立构建 beforeAll（60000 ms，沿用全局 hook 默认预算），避免单跑时依赖其它测试的构建产物。最初将构建放入原 beforeAll 并限 15000 ms 的试跑失败（r10-notifier-after.txt）；最终已拆成独立 hook，不增加原业务检查的时间预算。

## 标待审之前的自审清单

1. **情形表逐行**：本轮不新增待审、不改 W5 情形表。通知三种原场景（失败通知双连接、带/不带 Origin 已读、停服后重放且旧已读保留）断言全部保留；本地因监听限制未执行，不能报通过。
2. **失败分支**：原无 Origin 403 且库不变、真实 CLI 退出 3、原 30 秒超时仍保留。构建失败会直接使 suite 失败，初版前置已实跑此分支。授权弹窗、读盘失败、外部服务不可达不适用，本轮不改相关产品路径。
3. **全部迁移**：原连接→失败通知→已读广播→关闭→离线失败→重启→重放的顺序未改；两次 start 都选择 production。真实迁移在本地未取证；不以无监听中间件探针代替。
4. **输入域边界**：不适用；本轮未增加输入域，原五键消息、两条通知、未读 1、旧记录逐字段相等等断言不变。

## 全量验证

- sh scripts/ci-local.sh 首次退出 1，用时 737 秒，日志 .autome/output/r10-ci-before.txt。原通知用例执行结束后才修改测试；后续单跑构建曾与剩余全量阶段重叠，因此本次仅作诊断结果，不作为独立干净基线。
- 首次 Web **364 条 / 43 文件**：333 通过、12 失败、19 skipped；相对宿主 364 差 **0**。Desktop **105 条**：104 通过、1 失败，相对上一实现轮 105 差 **0**，相对宿主完整 165 差 **-60**（Electron 启动 SIGABRT，内部 60 条未展开，未删除）。Rust **1194 条**：1193 通过、1 原有 ignored，相对上一实现轮差 **0**。
- 修改后再次独立执行相同门禁，结果在下节记录。Web 用例总数预期仍为 364，新增 hook 不是用例，不改变数量。

## 最终验证与交接

- 修改后独立运行 **sh scripts/ci-local.sh**：退出 **1**，359 秒，完整日志 .autome/output/r10-ci-after.txt；摘要 r10-results.txt。期间未启动其它构建或改测试代码。
- Format、Clippy、Rust Test、Contract、Web lint、Web typecheck、Web build 均通过。Desktop 仍为 Electron SIGABRT；Web 的 notifier-live、eventbus-live、server 前置及 gateway-http/shell-live 监听路径受 listen EPERM / watcher EMFILE 阻断。
- Web **364 条 / 43 文件**：333 通过、12 失败、19 skipped；较宿主基线 364、上一实现轮 364 及本轮修改前均差 **0**。通知 suite 3 条因监听前置失败未执行，构建前置已通过；无新增/删除/主动 skip。
- Desktop **105 条**：104 通过、1 失败、0 skipped；较上一实现轮差 **0**，较宿主完整 165 差 **-60**，原因仍是 Electron 未启动，内部子检查未展开。Rust **1194 条**：1193 通过、1 原有 ignored，差 **0**。
- git diff --check 通过。程序核对状态字段各出现一次、implementation-round 10/25、repos 原样、里程碑表与 HEAD 一致、原三个 test 正文逐字一致、retro 五段且小于 200 字均通过。
- 未取得用户要求的同命令绿灯，未宣称已消除宿主超时；保留候选修改，由 Autome 对最终提交复跑。若 production 模式仍慢，优先细分 core.stop 与 HTTP close 阶段，不继续盲目放宽用例限时。不是协议失败。
