# M-02 实现轮 #23

结论：实现及不依赖监听的验证完成，交审计复验。真实 HTTP/WebSocket 验收受本轮沙箱 `listen EPERM 127.0.0.1` 阻断（同时出现 watcher EMFILE），不能据此声称全量通过。按设计「风险与验证安排」第 4 项交审计轮复验；未改验收契约。实现预算 23/30。

## 改动及上轮审计

- `eventbus.ts`：连接检查点、续传队列与分页、reload 基线重置、失败延后重试、core 状态去重广播、恢复拉取及扩展消息/断开挂点。
- `eventbus-protocol.ts`：reload 信封、浏览器消息解析（非负安全整数）；`eventbus-ws.ts` 将文本消息交给总线。
- 所有 `events.since`（含续传）共用 drain 的单飞顺序，tick 不 await；补发先收完整批次再发送，读盘失败不会提交半批。离线请求保留 after，下一次成功 pull 重做；关闭/断开丢弃在途结果。
- 上轮审计 #1 通过 M-01，只补强了升级监听器数量断言，无退回缺陷，无需搬运红绿回归检查；该断言原样保留。
- 未改 M-03、M-04，也未修改 crates、desktop、protocol、plugins 或新增依赖。

## 验收命令及用例基线

各命令单独运行，原始输出只保存在 `.autome/output/`。

| 命令 | 退出码 | 结果与输出 |
|---|---|---|
| `npm run lint --prefix apps/web` | 0 | `.autome/output/m02-final-lint.txt` |
| `npm run typecheck --prefix apps/web` | 0 | `.autome/output/m02-final-typecheck.txt` |
| `npm test --prefix apps/web` | 1 | 最终 19 文件、208 用例；181 过、11 失败、16 因套件前置失败而跳过；`.autome/output/m02-final-web.txt` |
| `npm run build --prefix apps/web` | 0 | `.autome/output/m02-final-build.txt` |
| `npm test --prefix apps/desktop` | 0 | 总数 104、103 过、1 跳过，较上轮总数差 0；`.autome/output/m02-desktop.txt` |
| `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` | 0 | 空输出，`.autome/output/m02-scope.txt` |

Web 总数较审计 #1 的 189 **+19**：协议 3、eventbus 新增 14、真实集成新增 2。eventbus 新增 14 中，设计要求 10，额外 4 分别覆盖断开/关闭中的续传、多页固定上界、续传空页重试、续传分页中账本回退。设计 M-02 契约预期 203，实际多 5：契约沿用了 M-01 的旧基数 188（实际 189，差 1），再加上述 4 个补充分支用例；没有删除或减少既有测试。

首次全量为 206（179 过、11 失败、16 跳过）；最后补充空页与续传中账本回退两个分支后最终 208。初次纯总线/协议验证 23/23 通过；最终全量对应总线/协议 25 个用例全部通过。

环境受阻共 27：`gateway-http.test.ts` 11 个均在 listen EPERM 失败；`server.test.ts` 9 个、`eventbus-live.test.ts` 7 个前置启动失败，Vitest 报 skipped。没有添加 skip 或降低断言；真实二进制存在。单跑 `npm test --prefix apps/web -- test/eventbus-live.test.ts` 同样退出 1、7 个未执行（`.autome/output/m02-live.txt`），确认阻塞在监听而非续传断言。真实延迟、SIGKILL 恢复、重连收到历史帧本轮未取证，需审计在允许监听的环境复验。

## 标待审之前的自审清单

### 1. 情形表逐行

| §3 续传情形 | 项目测试与结果 |
|---|---|
| after == sentSeq | 「相同检查点无消息…」：只保留 hello，通过 |
| after > sentSeq 且 <= latest | 同例：检查点 6，排队的 6 不重发，7 正常送达，通过；先判领先检查点再判缺口 |
| after > latest | 「检查点超过账本…」：100 对账本 5 回 reload，下一条 6 可收，通过 |
| hello 后已发 event，再收到旧 after | 「hello 后发过事件…」：不逆序补发，reload 后 7 正常收，通过 |
| 缺口 > 1000 | 「缺口 1000 补发，1001 只 reload」：1001 仅 reload，后续事件可收，通过 |
| 缺口 <= 1000 | 同例 1000 全部补发；「小缺口分页…」「续传多页读取固定上界…」：live 队列不丢不重、顺序正确，通过 |
| call 抛错 | 「续传内核不可用、超时或读盘失败…」：保留检查点，成功 pull 后完整补发，通过 |
| 全局账本回退 | 「账本回退取消续传…」「续传分页期间账本回退…」：全连接 reload，旧批丢弃，新小序号各收一次，通过 |

core 映射表五行由协议测试逐行比较；总线测试覆盖 starting → connected → restarting → offline → stopped，重复状态不重复推送，reason 改变另推一次。恢复测试确认自动 pull 使用原游标而非 latest 重新初始化。W4/W8 挂点测试覆盖订阅、退订、broadcast、connection.send、按 type 分发、未知 type、非法 JSON、断开清理且不影响事件过滤，均通过。

### 2. 设计点名的失败分支

- 读盘失败、内核不可用、超时：各自注入 call reject 后等待重试，通过；原 M-01 拉取失败保留游标测试继续通过。
- 非法 JSON、缺 type、负数/小数/字符串 after、空输入：协议解析和总线 receive 测试均通过，连接不断。
- 续传未读完但返回空页：不发半批，记录中文失败，下次重试通过。
- 账本在分页期间回退、断开/关闭时读取尚未返回：均有独立测试，通过。
- 拒绝授权：本任务没有新增登录授权。已有 Host/Origin HTTP 拒绝测试保留，但本轮受监听 EPERM 阻断，未取证；不得表述为通过。
- 外部服务不可达：对应本地内核 call reject 已验证；不依赖外网服务。

### 3. 全部迁移

- live + 新事件 → live（仅 seq 递增）；live + 相同 after → live 无输出；live + 旧检查点 → resuming；live + 已发事件后的旧 after → reload/live。
- resuming + live 拉取 → 排队；resuming + 补发成功 → 按序发送并清队列/live；resuming + call 失败 → 保持 resuming；后续成功 pull → 重试/live；resuming + 重复 resume → 忽略，不覆盖在途检查点。
- live/resuming + 账本回退 → reload/live，统一重置；live/resuming + disconnect/close → 删除连接、丢弃在途结果。已有 close 用例和新增在途续传用例覆盖。
- core 五种输入与同状态去重、不同 reason、offline → connected 自动补拉均执行通过。
- 通道方向：events.since → 广播/订阅、客户端文本 → resume/扩展 handler、扩展 handler → connection.send、broadcast → 全体、disconnect → cleanup 均执行通过。真实 WS 双向通道及升级守卫受环境阻断，等待审计。

### 4. 输入域边界

- after：0、1、1000、MAX_SAFE_INTEGER 接受；-1、小数、null、空串、字符串前导零、超安全整数拒绝；JSON 数字前导零本身非法，同样拒绝。
- 缺口：1000/1001；空事件页、单事件、分页末尾固定上界、回退到小序号均已执行通过。
- 日期闰日、跨年不适用：协议输入不含日期字段。

## 设计负向对照

对本轮源码临时替换，依次运行 `npm test --prefix apps/web -- test/eventbus.test.ts test/eventbus-protocol.test.ts -t <过滤词>`；每次在 finally 恢复原文件。

| 注入 | 过滤词 | 结果 |
|---|---|---|
| resuming 事件直接发送，不排队 | 小缺口分页 | 退出 1，递增/完整序列断言失败 |
| 缺口条件改为 >=1000 | 缺口 1000 | 退出 1，1000 不再补发 |
| restarting 映射 connected | 五种 | 退出 1，协议及总线映射断言失败 |
| connected 时强制重新初始化 cursor | 恢复后立即 | 退出 1，离线期间事件丢失 |
| reload 不更新 sentSeq | 账本回退 | 退出 1，新账本小 seq 丢失 |

原始输出：`.autome/output/m02-mutation-{queue,boundary,mapping,recovery,reset}.txt`。全部注入已还原，最终验收使用恢复后的源码。`git diff --check` 通过；注入后工作区含本轮有意改动，因此提交前 `git status --porcelain` 不为空，未将其谎报为干净；以最终提交后状态检查确认没有临时注入残留。
