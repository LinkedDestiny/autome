# M-04 实现轮 #5

## 范围与结论

实现顶栏铃铛及单连接消息出口，提交审计；只推进 M-04。M-03 上轮审计通过、没有实现缺陷，因此没有需逐字搬入并红绿复现的审计检查。M-05 系统通知尚未实现，M-06/M-07 保持开放。

- 新增 `bell-client.ts`：挂载/hello/notification/notification-read 拉取、在途合并、已读 POST 后拉取；未读数完全取服务端响应。通知刷新失败仍调用 onNotification(message, null)，保留计数并显示中文错误。
- 新增 `bell-targets.ts`、`bell.tsx`：中文按钮、徽标、正文/本地时间/未读状态、固定容器与内部滚动。当前目标不可用时纯文本与提示；路由开放后链接先提交已读。root 挂载独立组件，便于 W5 搬迁。
- `live-client.ts` 只扩展消息出口与异常隔离的订阅表，四类内置消息处理保留；`live.tsx` 补导入并在原连接配置中传入回调。组件只订阅，卸载停止控制器并退订。
- 原有测试文件均未修改；新增 bell-client 6 例、bell-view 7 例。未添加依赖。

## 命令与结果

原始输出都在 `.autome/output/`。每条命令独立运行并检查退出码。

| 命令 | 退出码 | 结果 / 原始输出 |
|---|---|---|
| `npm test --prefix apps/web -- test/bell-client.test.ts test/bell-view.test.ts` | 0 | 2 文件 13/13；`m04-target-final.txt` |
| `npm test --prefix apps/web` | 1 | 32 文件，291 例：261 通过、11 失败、19 初始化失败后跳过；`m04-web.txt` |
| `npm run lint --prefix apps/web` | 0 | 无警告、无错误；`m04-lint.txt` |
| `npm run typecheck --prefix apps/web` | 0 | 通过；`m04-types.txt` |
| `npm test --prefix apps/desktop` | 0 | 104 例，103 通过、1 基线跳过；`m04-desktop.txt` |
| `npm run build --prefix apps/web` | 0 | 负向对照还原后的生产构建通过；`m04-build-final.txt` |

用例总数相对上一轮审计基线：Web **278 → 291（+13）**，增量全部来自上述 6+7 个铃铛用例，没有删除旧用例；文件数 30 → 32。Desktop **104 → 104（+0）**，原有 1 个跳过不变。临时静态预览用例已还原，不计入项目用例数。

全量环境阻断：`gateway-http.test.ts` 11 例均因 `listen EPERM 127.0.0.1` 失败；`eventbus-live.test.ts` 7、`server.test.ts` 9、`notifier-live.test.ts` 3 因初始化不能监听而跳过（server 同时记录 watcher EMFILE）。共 30 例受阻，均为原有监听型测试。没有把它们改成 skip、没有弱化断言；审计轮需在允许监听的环境复跑完整 291 例。这不是全部测试通过的证据。

## 标待审前四项自审

### 1. 情形表逐行

| 设计 §6/§7 与 M-04 契约 | 检查与结果 |
|---|---|
| 挂载先取一次；三类消息刷新，其余忽略 | client #1，GET 调用次数、服务端 unread=7 不因通知自增；通过 |
| notification 刷新后传消息及列表；失败传 null | client #1、#2，成功/失败/在途消息均断言；通过 |
| 拉取单飞、在途多条合并 | client #2，用可控 Promise，第一请求未释放前只有 1 次 GET，两条消息合并为 1 次补拉，逐条交付；通过 |
| 单条/全部已读与跨标签页计数 | client #3、#6，检查 POST 地址/体、随后 GET；两个控制器接收已读广播后服务端计数 2→1→0；通过 |
| 内置消息与扩展出口、订阅异常隔离、重连 | client #4/#5/#6；旧 socket/stop 后不交出、坏 JSON/非字符串 type 不交出；通过 |
| 只有一个 socket，root 挂载且不丢子路由 | client #6 与 view #6；源码守卫和 renderToString(createRoutesStub)；通过 |
| 徽标、列表、空态/错误、内部滚动 | view #1/#2/#4/#5；0/1/99/100、标题/正文/时间/圆点/操作、固定定位和滚动属性；通过 |
| 目标不可用不跳 404；开放后任务/项目链接 | view #3/#7；默认表无链接、注入可用表正确链接、无归属无链接、与真实 routes.ts 守卫；通过 |

### 2. 设计点名的失败分支

- GET 网络不可达：client #1，保留 unread/items、中文加载错误、onNotification 仍收到 null；下一次 hello 成功清错。
- POST 拒绝（403）及网络不可达：client #3，中文标记失败、计数不变、不伪造已读结果。
- 订阅者异常：client #5，后续订阅者及内置 onEvents 仍执行。
- 无效 JSON、无/非字符串 type、旧连接迟到与停止后消息：client #4，均不交回调。
- 目标页面未开放、无目标：view #3/#7，不出链接、不进入不存在的路由。
- 授权拒绝不适用：属于 M-05；读盘失败不适用：铃铛无本地磁盘操作；专用超时不适用：设计没有铃铛超时机制，fetch 拒绝统一按网络错误处理。

### 3. 全部状态迁移

- 控制器：未启动→首次拉取→成功；空闲→三类消息→拉取；拉取中→多消息→一次补拉；成功→失败（保留数据）→恢复（清错）；运行/在途→stop（迟到响应不更新，后续 start/read/message 不请求）。client #1/#2 全部执行。
- 已读：未读→单条→GET；未读→全部→GET；失败→原计数不变；其他标签页收到广播→GET 同步。client #3/#6 全部执行。
- 连接：首次建 socket→消息→断线→重连 hello→补拉→停止；旧 socket 迟到丢弃。client #4/#6 全部执行，旧 live-client 8 例原样保留。
- 订阅：订阅→接收、退订→不接收、一个异常→其余继续。client #5 通过。
- 展开/收起输出、未读/已读输出、目标未开放/开放/无归属输出：view #2/#3/#5 通过。真实鼠标交互与两个真实标签页效果仍由 H-01/H-05/H-07 确认，不冒充浏览器验收。

### 4. 输入域边界

- 未读计数 0/1/99/100：隐藏、原数、99、99+；view #1 通过。
- 列表空/单元素、有/无正文及有/无 read_at：view #2/#4；两种已读状态正确显示操作；通过。
- 任务优先、只有项目、两者为空、带斜杠/空格/问号/中文的 ID 编码：view #3 通过。
- 消息边界 null/数组/数字/字符串/空对象/数字 type/坏 JSON：client #4 通过。
- 日期格式仅展示服务端 ISO 时间，检查本地 01-02 03:04 的前导零；日期输入解析与闰日有效性不是本组件业务输入域，不另作日期校验。
- GET limit 和 POST ids 数量域由 M-03 接口验证，本轮不新增 UI 输入控件或改变接口约束。

## 负向对照

临时注入脚本 `.autome/output/m04-negative.mjs` 每项恢复原文后再运行下一项。六项均退出 1，最终全部还原，13 例重新全绿。错误改动未提交。

| 注入 | 观察 |
|---|---|
| notification 自增计数而不拉取 | client #1/#2/#6 失败，`m04-negative-increment.txt` |
| onMessage 仅扩展类型才交出 | client #4/#6 失败，`m04-negative-default-only.txt` |
| 取消订阅者异常捕获 | client #5 失败，`m04-negative-subscriber-throw.txt` |
| 铃铛 effect 自建 createLiveClient | view #6 源码守卫失败（不是 SSR ReferenceError），`m04-negative-second-connection.txt` |
| 徽标阈值 >=100 改 >100 | view #1 失败，`m04-negative-badge.txt` |
| TARGETS 在真实路由未上线时改 true | view #3/#7 失败，`m04-negative-targets.txt` |

## 视觉检查与限制

通过临时 renderToString 用例生成 `.autome/output/m04-preview.html`（16 条通知，内部滚动静态预览），随后恢复测试文件。浏览器 resize/evaluate 工具要求批准但当前策略 never，file URL 又被工具禁止，未取得有效截图；`m04-bell.png` 为 about:blank，不作为证据。H-01/H-05/H-07 保留人工验收。没有进行 W12 端到端实现，也没有新增依赖。

## 补充验证与交接

- 独立命令 `npm test --prefix apps/web -- --exclude test/eventbus-live.test.ts --exclude test/server.test.ts --exclude test/notifier-live.test.ts --exclude test/gateway-http.test.ts` 退出 0，28 文件 **261/261**，0 跳过，输出 `.autome/output/m04-non-listen.txt`。这是受限环境的补充验证，不替代上面的完整命令。
- 负向对照恢复后新增收起状态与无正文断言，最终定向仍为 13/13（退出 0）。
- `git diff --check` 通过；状态块保留既有字段，implementation-round 为 5/35，M-04 为待审，其他里程碑不变。下一实现轮在独立审计通过后推进 M-05。
