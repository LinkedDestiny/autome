# M-05 r8 实现轮：通知重启门禁超时调查

## 结论与范围

本轮为门禁修复轮，不推进里程碑。基线 `ee0b26a`，宿主失败提交 `6b4656308ccfaf89015130c91df1e7b799526e4d`。M-01..M-05 均已经审计关闭，本轮没有修改任何里程碑状态；implementation-round 为 **8/25**。**只取得失败用例定位与诊断增强，未修复或验证宿主超时，不报门禁通过。**

## 宿主失败原文与检查

只读查询本机账本中的 T-35 `gate.result`，获得该 SHA 对应的日志位置；没有修改主工作树或账本。原日志：`/Users/dannie/project/autome-v2/.autome/output/gates/T-35/gate-6b4656308ccf.log`。摘录保存在本 worktree `.autome/output/w5-r8-host-failure.txt`、`w5-r8-host-notifier.txt`。

- 失败用例：`test/notifier-live.test.ts > 停服期间新增失败，重启只补一条且保留旧已读`。
- 原断言错误：`Error: Test timed out in 30000ms.`，指向 `test/notifier-live.test.ts:44:1`；该用例记录耗时 31378 ms。
- 同文件前两条通过，Web 总计 **364**，363 通过、1 失败、0 skipped；不是此前实现轮的端口权限失败。
- 09:12:08.574267Z 已记录旧内核 `stdin closed, shutting down`，此后没有本用例的新内核启动日志。这个时序将调查范围缩小到停服、离线 CLI、服务器重启阶段，**不能据此判定具体哪个 await 卡住**。

上一轮审计 #6 为通过，没有退回缺陷的临时检查需要搬迁。宿主抓到问题的检查已经是项目原有测试，本轮保留其用例名称、断言、顺序和 30000 ms 超时，不新增替代检查、不放宽超时、不跳过测试。

## 命令与结果

1. 先原样启动 `sh scripts/ci-local.sh`，日志 `.autome/output/w5-r8-gate-before.txt`。该命令中的 notifier-live 文件执行时尚未改动；之后才为已执行完的用例加入诊断记录。本次命令不能充当最终修改版的全量通过证据。
2. 修改前原样执行 `npm test --prefix apps/web -- test/notifier-live.test.ts`：退出码 **1**；1 个文件前置失败，3 条 skipped，报 `listen EPERM: operation not permitted 127.0.0.1`，并有 watcher `EMFILE`。日志 `.autome/output/w5-r8-notifier-before.txt`。没有进入宿主失败用例的正文，因此不是原超时的红灯复现。
3. `npm test --prefix apps/web -- test/notifier-live-fixture.test.ts`：退出码 **0**，1/1 通过，4.99 秒。日志 `.autome/output/w5-r8-fixture.txt`。真实 CLI 产生终态失败事件的独立夹具可执行；这不证明 HTTP/WS 停服重启链路正常。
4. 诊断改动后再次执行同一个 notifier-live 命令：退出码 **1**，仍为前置监听 EPERM、3 条 skipped。日志 `.autome/output/w5-r8-notifier-after.txt`。**没有红转绿，不把环境失败与宿主超时混为一谈。**
5. `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`：均退出码 **0**，日志 `.autome/output/w5-r8-lint.txt`、`w5-r8-typecheck.txt`。

## 诊断改动与未闭合假设

只修改 `apps/web/test/notifier-live.test.ts` 的原重启用例：为读取旧通知、关闭服务器、创建离线任务、运行离线任务、重启服务器、补齐通知六个 await 阶段输出 `[notifier restart]` 开始/通过/失败及耗时。`fail('second fail')` 展开为其原有的两次 CLI 调用，参数不变，以区别创建与执行阶段。原结果断言逐字保留，异常原样重抛。

这是一项保留在测试中的宿主诊断，不是产品修复。没有改变 W2/W3/W4 语义、服务关闭逻辑或内核代码。现有证据仍无法在以下假设间作选择：关闭服务等待未结束；离线 CLI 两次调用消耗累计预算；开发服务器重启迟滞。只读检查 `main.close`、sidecar stop、事件总线关闭及 Vite 关闭实现，未取得可确认的根因；不凭猜测改生产行为。

## 标待审前的四项自审

1. **情形表逐行**：不适用；本轮不新增待审里程碑、不改 W5 情形映射。原通知用例的“旧通知已读、停服新增失败、重启恰好补一条、旧已读保持”四项断言保留；本地前置阻断，逐项均未执行，不能报通过。
2. **失败分支**：实际执行了前置监听失败，修改前后均退出 1；既有不监听端口的失败事件夹具 1/1 通过。宿主的 30 秒超时未在当前权限环境复现。授权、读盘、空输入等 W5 分支不适用，本轮未改对应产品路径。
3. **全部迁移**：原检查覆盖“在线→停服→离线新增事件→重启补齐”，执行顺序与断言未变。需要监听的实际迁移均未取证；不把静态检查算作迁移通过。
4. **输入域边界**：不适用；没有修改输入解析或域边界，CLI 参数、通知期望值与原测试相同。

## 后续责任

由 Autome 在提交后的 SHA 上运行原门禁 `sh scripts/ci-local.sh`，以该结果为准。如再次超时，用最后一条 started 及之前各段 passed 耗时识别阻塞阶段，再复现并修复。当前沙箱禁止监听且审批模式为 never，没有尝试绕过权限。本轮不是协议失败。

## 全量结果、基线差与提交检查

原命令最终退出码 **1**，耗时 **678 秒**。Format、Clippy、Rust Test、Contract、Web lint、Web typecheck、Web build 通过；Desktop、Web test 失败。原始汇总见 `.autome/output/w5-r8-final-counts.txt`，错误摘录见 `w5-r8-final-errors.txt`。

- **Web 总计 364 条 / 43 文件**：333 通过、12 失败、19 skipped；相对宿主 `6b4656308c` 的 364 和实现 #7 的 364 均为 **0**。12 条失败为 gateway-http 11 条与 shell-live SSR 1 条的监听 EPERM；19 条未执行为 notifier-live 3 条、server 9 条、eventbus-live 7 条，前置受 EPERM/EMFILE 阻断。没有增加、删除或主动跳过用例。
- **Desktop 总计 105 条**：104 通过、1 失败、0 skipped；相对实现 #7 的 105 为 **0**，相对审计 #6 的完整 165 为 **-60**。`electron dom harness: real-DOM assertions against the actual renderer` 报 `dom-harness.js exited null`、`null !== 0`；DOM 层未展开导致少 60 条，不能因 skipped 为 0 报完整通过。
- **Rust 总计 1194 条**：1193 通过、1 原有 ignored；相对实现 #7 已执行的 1171 为 **+23**，来自本次 end_to_end 101/101 通过后继续执行的后续 16 条与 7 条套件，不是新增测试。上一轮失败的 `bug_regression_guard_survives_retry_and_extend` 本次通过，但没有修改其实现，因此不声称修复。
- 修改前后的 notifier-live 独立运行各登记 3 条，差 **0**；fixture 的 1 条是现有用例的重复运行，不计新增。

诊断改动后的 lint、typecheck 单独通过；原用例复跑仍阻断，故不再重复整套门禁制造相同权限失败，未取得最终修改版全量通过证据。状态字段唯一、implementation-round 8/25、里程碑表与 HEAD 完全一致、repos 原样、retro 五段且不超过 200 字、证据文件存在的脚本检查通过；`git diff --check` 通过。
