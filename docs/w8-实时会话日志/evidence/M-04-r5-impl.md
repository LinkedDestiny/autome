# M-04 · 实现 #5

结论：M-04 验收通过，提交独立审计；implementation-round 为 5/25。上一轮 M-03 审计通过，没有审计退回检查需要迁入项目测试体系。

## 实现

- live-client 返回有 hello 就绪门控的 send，断开与 stop 后禁发；模块级发送者由 useLiveEvents 登记、清理。日志消息不改变账本事件检查点。
- 新增纯日志模型：文件 next 与文本 UTF-8 字节数分开；去重、缺口续传；保留全文，以整项窗口限制 DOM 至 262144 字节；暂停固定窗口，支持双向翻页、跳到开头与最新。
- 新增控制器：只接收本会话日志；hello 重新订阅；关闭活跃订阅并注销监听。未涉及 M-05 组件、路由或样式。

## 命令与结果

1. `npm test --prefix apps/web -- test/live-send.test.ts test/session-log-model.test.ts test/session-log-controller.test.ts test/live-client.test.ts`：退出码 0，4 文件、28 用例，28 通过、0 失败、0 跳过。首次及负向注入还原后的最终复验均通过。原始输出：`.autome/output/m04-r5-tests.txt`、`.autome/output/m04-r5-tests-final.txt`。
2. `npm run lint --prefix apps/web`：退出码 0；输出 `.autome/output/m04-r5-lint.txt`。
3. `npm run typecheck --prefix apps/web`：退出码 0；输出 `.autome/output/m04-r5-types.txt`。
4. `git diff --check`：退出码 0。

用例总数与基线差：同范围此前只有 live-client 的 8 条，本轮新增 live-send 5、model 10、controller 5，共 +20，达到设计预期 28。上一轮 M-03 验收总数为 21，本轮比上一轮 +7；这是验收范围从后端集成/core-sidecar 切到前端四文件造成，不能解释为全项目只新增 7 条。本轮未运行全量测试、宿主快检组合命令或 ci-local 门禁。

## 标待审之前的自审清单

### 1. 情形表逐行

设计 M-04 的 20 条验收情形与项目测试一一对应，以下各行已执行通过：

| 情形 | 项目用例与结果 |
|---|---|
| 发送 1：hello 前后 | live-send #1：未就绪不发，就绪发 JSON |
| 发送 2：断开和重连 | live-send #2：禁发直到新 socket hello；旧 socket close 不干扰 |
| 发送 3：hello 回调发送顺序 | live-send #3：resume 在订阅前，回调发送 true |
| 发送 4：无发送者/stop | live-send #4：均 false；send 抛错也 false |
| 发送 5：日志分发隔离 | live-send #5：三类消息原样回调，无事件/reload，resume after 保持 10 |
| 模型 1：连续/重复/缺口 | model #1：next 推进、丢重复、缺口从 next 订阅；替换字符文本不误算文件偏移 |
| 模型 2：跟随/暂停 | model #2：跟随 bottom，暂停后 from/to/text 不变且 unseen |
| 模型 3：滚动恢复 | model #3：离底暂停；末尾恢复；非末尾不可恢复 |
| 模型 4：跳最新 | model #4：末尾窗口、清 unseen、bottom |
| 模型 5：UTF-8 限量 | model #5：中文按字节，窗口不超 N、total 完整、中文隐藏提示 |
| 模型 6：小历史全文 | model #6：100 KiB 两段加 end 全文相等、无隐藏；空文本可用 |
| 模型 7：结束/拒绝/重试 | model #7：四类 end、五类 rejected 原样显示；retry 保留 parts 与 next |
| 模型 8：hello | model #8：五种 phase 逐一检查是否发送与关闭行为 |
| 模型 9：大历史全文 | model #9：超过 600 KiB 加 end；top 找到 HEADER；later/earlier 各页拼接等于全文；jump 回末尾 |
| 模型 10：非末尾追加 | model #10：视窗固定、total 增加、无 scroll；jump 后恢复 bottom |
| 控制器 1：open 和发送失败 | controller #1：offset 0、subscribing、hello 再发 |
| 控制器 2：会话隔离 | controller #2：其它会话三类消息均忽略，本会话推进 |
| 控制器 3：续传 | controller #3：两段后 hello 带 next=2 |
| 控制器 4：关闭 | controller #4：活跃退订，ended 不退订；关闭后不再响应；重复关闭不重复退订 |
| 控制器 5：协议约束 | controller #5：open/缺口/hello/retry/close 发出的全部消息经正式解析器通过，键白名单正确 |

### 2. 失败分支

发送通道未就绪、断开、停止、send 抛错：live-send #1/#2/#4；发送失败等 hello：controller #1；日志重复与缺口：model #1；服务端 invalid/limit/not_found/offline/kernel 拒绝、rotated/deleted/error 结束：model #7，全部通过。空日志：model #6。读盘失败与外部服务不可达在前端的落点分别是 error/offline 消息，本轮验证其状态与文案；真实读盘、授权检查和服务端超时不适用本里程碑，属于已审计后端范围，前端不直接执行这些操作。

### 3. 全部迁移

已执行：idle→open→subscribing；subscribing→chunk→live；live 连续追加与重复/缺口；subscribing/live 的 hello 续传；live→四类 end→ended；live→五类 rejected→rejected→retry→subscribing。五个 phase 的 hello 与 close 均由 model #8 遍历（close 后 idle；仅 subscribing/live 退订）。follow→pause/scrolled(false)/top/earlier→暂停，暂停追加固定窗口；末尾 scrolled(true)、jump、later 到末尾恢复 follow，非末尾 scrolled(true) 不恢复。两向页面遍历由 model #9 通过全文相等证明。通道方向：下行 hello/log.* 分发，上行 subscribe/unsubscribe 与既有 resume 的顺序均通过；现有 live-client 8 条未修改且全部通过。

### 4. 输入域边界

已执行：空文本、单个中文字符、UTF-8 多字节、大于 N、恰好 N、N+1（整项裁剪）、零 offset、最大安全整数 next 的 hello 续传、重复/缺口偏移、文件 next 不等于文本替换字符字节数。历史窗口首尾边界与双向无遗漏已核对。日期、前导零日期、闰日和跨年不适用：该模型无日期输入。模型接收合法协议 chunk（单条文本不超过 64KB），不另设浏览器侧协议解析器。

## 负向对照

临时修改生产源码后运行对应项目测试，五项均退出 1，随后 finally 原样还原；最终四文件 28 条再次通过。输出均在 `.autome/output/m04-r5-negative-<名称>.txt`：

- ready：hello 保持 ready=false，live-send 的回调发送判别失败。
- hello：控制器忽略 hello，controller 首次失败重试与 next 续传判别失败。
- pause：暂停追加也返回 bottom，model 暂停与非末尾用例失败。
- utf16：用 text.length 代替 UTF-8 字节，model 字节总量与窗口上限判别失败。
- discard：每次追加只保留最后一项，model 历史全文、HEADER 与翻页判别失败。

实际鼠标、滚动容器及焦点行为属于 M-05/H-01..H-05，本轮没有声称已做浏览器验收。
