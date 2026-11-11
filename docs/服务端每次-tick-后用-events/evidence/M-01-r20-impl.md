# M-01 · 实现 #20 · 升级路径测试修复

本轮为用户指定的门禁修复轮，implementation-round 为 20/20，起始 HEAD 为 6a4ec6e，宿主失败提交为 5f12bcd627。不推进新里程碑；四个里程碑原状态均为开放，保持不变。审计文件不存在，没有待搬入的审计临时检查。宿主已给出的失败检查本来就在项目 gateway-http.test.ts 中。

## 修复与范围

M-01 在 main.ts 注册 /ws 后，gateway-http 的「合格但无处理器 upgrade 回 404」仍请求 /ws，前提已经失效。请求只有 Connection、Upgrade、Origin，没有 Sec-WebSocket-Key / Sec-WebSocket-Version；经过 Host/Origin 守卫后会进入真实 ws.handleUpgrade 的 400 分支，而不是未注册路径的 404 分支。

将原有 404 检查的请求路径改为 /unregistered-ws，保留原断言；新增「/ws 已注册，缺少握手字段回 400」，保留原请求作为独立负向检查。未修改生产逻辑、安全守卫、断言强度、超时或跳过条件。同类核对包括 security.test.ts 的已注册/未注册矩阵，以及 eventbus-live.test.ts 的合法握手、外站 Origin、HMR 共存；纯守卫测试自建未注册服务器，前提仍成立，无需修改。

## 命令与结果

- 修改前原样运行 sh scripts/ci-local.sh：退出 1，用时 149 秒，日志 .autome/output/r20-ci-red.txt。除 Web test 外十个步骤通过，未运行可选 package。独立 npm test --prefix apps/web 同样退出 1，日志 .autome/output/r20-web-before.txt。
- 原用例在本地未到达 404 断言：启动监听先报 listen EPERM: operation not permitted 127.0.0.1；eventbus-live/server 前置启动同样被阻断，伴随 watcher EMFILE。独立 node:net.createServer().listen(0, '127.0.0.1') 也报 EPERM（.autome/output/r20-listen.txt），说明监听阻塞独立于项目代码。
- 使用项目真实 installUpgradeGuard、createWsUpgrade、createEventBus，以内存 Duplex 代替 TCP 输入同一请求（.autome/output/r20-upgrade-probe.mjs）。node .autome/output/r20-upgrade-probe.mjs old 退出 1，原 /ws 的 404 预期得到 400（.autome/output/r20-upgrade-red.txt）。node .autome/output/r20-upgrade-probe.mjs fixed 退出 0，未注册路径 404、/ws 无握手字段 400、外站 Origin 403 三项通过（.autome/output/r20-upgrade-green.txt）。这是分派逻辑的红绿证据，不冒充原 HTTP 集成用例的红绿实跑。
- 修改后再次原样运行 sh scripts/ci-local.sh：退出 1，用时 161 秒，日志 .autome/output/r20-ci-after.txt。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过，仅 Web test 因监听 EPERM 失败；新增用例也在启动阶段被阻断。没有取得完整门禁全绿证据。状态块七字段、20/20、四项原状态和唯一最新证据指针的 Node 断言通过。
- git diff --check 通过；git diff 30e5d51 --stat -- crates apps/desktop protocol plugins 输出为空，没有越界修改。

## 用例总数与上一轮基线差值

修改前 Web 18 个文件、188 个用例，164 通过、10 失败、14 跳过；与实现 #19 总数差 0。宿主提供的 186 通过、2 失败也合计 188，但本地监听阻塞使失败分布不同，不能把它当作宿主两项缺陷的复现。

修改后实测 Web 18 个文件、189 个用例：164 通过、11 失败、14 跳过。新增 1 个 gateway-http 无效握手用例，较上一轮及宿主基线均 +1；原 404 用例保留，仅调整目标路径，没有删除用例。内存探针的 3 项断言不计入项目测试用例总数。

Desktop 基线为 104 个用例、103 通过、1 原有跳过；修改前后两次门禁均保持此结果，总数差 0。Web 的 14 个跳过均由 eventbus-live/server 前置监听失败造成，并非主动跳过；不满足 M-01 验收。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：M-01 八项纯事件总线测试通过，分别覆盖首次游标初始化、2500 条三页、单飞补拉、读盘失败重试、重复页去重、hello 后增量、订阅退订、关闭。升级分派额外检查已注册无效握手 400、未注册路径 404、外站 Origin 403，内存传输全部通过；真实合法握手/HMR/跨进程 cancel/SIGTERM 被监听权限阻断，未取证。M-02 续传表不适用于本轮修复。
2. **失败分支**：读盘失败保留游标和中文日志随事件总线单测通过；无效握手和外站 Origin 拒绝随内存检查通过；真实端口占用、超时、缺失内核的 HTTP 行为被监听权限阻断，未取证。未引入新的失败分支。
3. **全部迁移**：初始化→就绪、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及在途结果丢弃随原单测通过。真实 HTTP→WebSocket 和 SIGTERM 关闭迁移未取证；此次改动仅纠正测试路径，不新增状态机。M-02/M-03 的续传、浏览器重连迁移不适用。
4. **输入域边界**：游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 上限、2500 跨页和重复序号随原单测通过；此次直接验证了缺少握手字段、未注册路径、错误 Origin 三个请求边界。没有新增产品输入域；resume 参数边界不适用于本轮。

## 未闭合项与交接

用户粘贴的宿主日志只展示第二个失败位置，未展示第一个失败的名称和断言；本地集成测试被 EPERM 抢先阻断，不能确认另一个宿主失败的根因，也不能声称本轮已修复两项。需要允许监听的宿主在本轮提交上重跑同一 sh scripts/ci-local.sh，并依据完整失败日志继续定位。本轮预算已达 20/20，不启动下一会话，不标已完成；环境阻塞不是协议失败。
