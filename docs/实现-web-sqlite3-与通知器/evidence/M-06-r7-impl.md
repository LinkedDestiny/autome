# M-06 实现轮 #7：消息回调独立验收

本轮只推进 M-06；实现沿用 M-04，不重复改动产品代码。上一轮审计 M-05 通过，没有退回缺陷，也没有需逐字搬入的审计失败检查。新增 bell-client 一个边界用例；原 live-client 测试未改。

## 标待审前自审清单

1. **情形表逐行**（§6 / M-04 (4)(5)(6)）：hello 调 core/connection 后交出口；event 保持异步合批并交出口；core、reload 保持内置处理并交出口；notification、notification-read 只交出口；非法 JSON、非对象、非字符串 type 不交；旧 socket 与 stop 后消息不交；未传回调兼容；双订阅、退订、抛错隔离；单 socket 到铃铛重拉、重连 hello 补拉、停止后不重连。专项均通过。新增用例验证同步内置处理顺序、未知类型原样交付、无订阅者发布。
2. **失败分支**：订阅者抛错仍通知其他订阅者及 onEvents；连接创建失败仍重试；GET 网络失败仍交 notification + null、恢复后清错误；POST 拒绝/网络失败保留计数。均由专项执行通过。读盘失败、系统授权和服务端 RPC 超时不适用：本里程碑不执行这些操作。
3. **全部迁移**：连接中→重连中（创建失败/关闭）、连接中/重连中→在线（hello）、在线→重连中→在线；hello 建检查点/重连 resume、reload 清检查点/清旧批、空检查点重连重验；connecting/open/reconnecting→stop 清计时器与事件批；订阅→广播→退订、一个订阅抛错→其余继续。live-client 8 项和 bell-client 串联检查通过。入站内置四类与扩展两类、出站 resume 均覆盖；没有新增传输通道。
4. **输入域边界**：非法 JSON、空原始串、null、数组、数字、字符串、缺 type、数字 type 拒绝；空字符串 type 与未知字符串 type 合法、原样交付；seq=0、重复/回退序号、MAX_SAFE_INTEGER 保持去重合批；退避 500→10000 封顶及 hello 重置；无订阅者、两个订阅者、退订均通过。日期/闰日/跨年不适用：本里程碑不解析日期。

## 负向对照

两次均在新增边界用例前运行同一专项命令：`npm test --prefix apps/web -- test/live-client.test.ts test/bell-client.test.ts`。

- 将 switch 后 onMessage 移入 default：退出 1，14 项中 3 失败、11 通过；M-04 (4)(5)(6) 失败，包含内置消息出口和重连补拉。日志 `.autome/output/m06-default.log`。
- 移除 publishLiveMessage 的 try/catch：退出 1，14 项中 1 失败、13 通过；M-04 (5) 报“订阅错误”。日志 `.autome/output/m06-throw.log`。
- finally 原样恢复产品文件后 `git status --porcelain` 输出为空（此时尚未新增边界测试及本轮文档）。没有保留变异。
- 还原后新增边界测试，专项 15/15 通过、0 跳过，退出 0，日志 `.autome/output/m06-target-final.log`。比此前专项 14 项增加 1，来自新增边界检查。

## 全量验收

各命令独立执行，不用成功短路串联。原始输出均在 .autome/output/。

| 命令 | 退出码 | 结果 / 日志 |
|---|---|---|
| npm test --prefix apps/web -- test/live-client.test.ts test/bell-client.test.ts | 0 | 最终 15/15，0 跳过；m06-target-final.log |
| npm run lint --prefix apps/web | 0 | m06-lint.log |
| npm run typecheck --prefix apps/web | 0 | m06-typecheck.log |
| npm test --prefix apps/desktop | 0 | 总数 104，103 通过、1 基线跳过；m06-desktop.log |
| npm run build --prefix apps/web | 0 | m06-build.log |

Desktop 对上一轮 104 项差值 0，跳过与基线一致。首次全量在增加边界用例前启动：300 项，270 通过、11 失败、19 因 suite 启动失败而跳过，退出 1（m06-web.log）。四个受影响文件为 gateway-http、eventbus-live、server、notifier-live；错误为 listen EPERM 127.0.0.1 / watcher EMFILE。不是产品验收全绿，也没有通过修改测试跳过环境失败。最终版本全量结果见下。


最终 `npm test --prefix apps/web` 退出 1：33 文件、301 用例，271 通过、11 失败、19 跳过（suite 启动受阻），日志 `.autome/output/m06-web-final.log`。相对上一轮审计的 300 项总数增加 1，唯一来源为本轮 bell-client 边界用例；其余测试未删减。真实内核 notifier-real 用例通过，19 项跳过属于上述监听服务套件的初始化失败。M-06 的 15 项与两项负向对照已取证，环境阻断交审计轮在可监听的环境复验，不能称全量通过。

修复说明：未发现本里程碑产品缺陷，产品代码无改动；仅补充一项输入边界/处理顺序测试。`git diff --check` 通过；原 live-client 测试与产品文件 diff 为空。按协议提交当前分支，不启动下一会话。
