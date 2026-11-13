# M-05 实现 #6

## 范围与结论

实现草稿卡 fetcher「开始」、项目 action 的 task.start 接线、生产通知订阅与卸载清理。已有账本 matcher 保留（含 project.），不改变全局实时链路、网关、安全检查或列归属。标题链接与按钮分离，按钮沿用 btn-primary、写闸门和 pending 禁用态。只推进 M-05，交独立审计；真实浏览器 H-04/H-05/H-06/H-07 未取证。

上一轮审计 M-04 通过，没有需要逐字迁入的缺陷检查。首次运行新文件 7 个断言用例均通过，但 teardown 对已停止任务再次 stop 导致套件失败；清理改为仅处理 active/queued，后续套件通过。这是本轮夹具修复，不是历史审计退回。

## 命令、结果与基线

- 基线：`cd apps/web && npx vitest run test/live-routes.test.ts`：1 文件 7 例通过。日志 `.autome/output/m05-baseline.txt`。
- 契约：`cd apps/web && npx vitest run test/board-live.test.ts test/board-live-listen.test.ts test/live-routes.test.ts`：3 文件、总数 16，14 通过、2 失败、0 跳过。两个失败均为 `listen EPERM: operation not permitted 127.0.0.1`，发生在监听建立阶段；另有 Vite watcher EMFILE 日志。符合设计 §6 与 M-05 契约的受限环境分支，不能声称完整契约全绿。日志 `.autome/output/m05-contract.txt`。真实内核非监听套件连 setup/cleanup 9839 ms，测试/钩子超时 120000 ms，大于实测两倍；监听链路耗时未取证。
- 直接相关回归：`cd apps/web && npx vitest run test/board-live.test.ts test/board-live-listen.test.ts test/live-routes.test.ts test/task-card.test.ts test/board.test.ts`：5 文件总数 36，34 通过、2 个同样的 EPERM、0 跳过。日志 `.autome/output/m05-final.txt`。
- 负向对照：暂时删除 `revalidatesOnMessage` 中 notification-read 分支，运行 `cd apps/web && npx vitest run test/board-live.test.ts`：7 例中 6 通过、1 失败；第 7 例实际仍为「等你处理 1」，期望「等你处理」。日志 `.autome/output/m05-negative.txt`。源文件在 finally 原样还原。
- 还原后的直接相关非监听复验：`cd apps/web && npx vitest run test/board-live.test.ts test/live-routes.test.ts test/task-card.test.ts test/board.test.ts`，结果见 `.autome/output/m05-green.txt`（本记录在确认结果后报验）。
- `git diff --check` 通过。未运行全量测试、ci-local、宿主快检、lint/typecheck/build。

用例总数：M-05 同口径由 7 增至 16，差 +9 来自 board-live 7 和 board-live-listen 2；没有参数化扩张或跳过。加直接相关旧回归 task-card 12、board 8 后为 36。上一轮 M-04 记录为 12 例，本轮契约总数相比 +4，但两轮范围不同（退出 M-04 的 12、进入 live-routes 7 与新增 9），不能将这个差理解为删掉了用例。非监听复验 34 = 36 减去已经实际运行且 EPERM 的 2 例，不算全契约通过。

## 标「待审」之前的自审清单

### 1. 情形表逐行

- draft：SSR 有开始按钮；写闸门允许/拒绝均覆盖，拒绝时 disabled 与原因同在（board-live 2）。
- queued：没有开始按钮，显示排队第 1 位；active/stopped/done 同样不展示开始（2）。
- start：action 精确发送 task.start/task_id；成功结果保留；未知或空 intent 返回 400 且不调用 write（3）。
- 空槽/满槽：CLI 建草稿经真实 action 启动，分别 started:true 入进行中与 started:false 留待开始、排队第 1 位（4、5）。
- task./bug./merge./gate./session./runtime. 命中重验；config./integrity_warning/空类型不命中；非事件导航遵循默认（1）。
- 外部 stop：真实 CLI + events.since + 持久 createMemoryRouter 使用项目 loader 与 shouldRevalidate，将原运行卡移入等你处理（6）。
- notification-read 消除数字、notification 补回数字；取消订阅后消息不重验；真实 SQLite store 与生产发布/订阅链路，hook 与页面挂载源码守卫（7）。
- WebSocket ≤4500 ms 与新请求 SSR 落列：测试已加入，受 EPERM 阻断，未取证。WebSocket 断言同时要求目标 task.updated 的 payload.state.state 为 stopped，防止历史启动事件误判。

### 2. 失败分支

- 写授权/连接拒绝：关闭写闸门 SSR disabled 并显示原因（2）；缺网关返回内核未连接（3）。
- 内核拒绝 409、网关拒绝 400、离线 503、超时 504：替身逐条返回原 error，action 原样保留（3）；真实网关空 task_id 返回非空字符串拒绝（3）。UI 原文展示由 fetcher.data.error 渲染，真实点击错误态仍待 H-05。
- 未知/空 intent：400；event/hello/空消息类型不触发通知重验（1、3、7）。
- 读盘失败：不适用，本里程碑无新读盘分支；沿用既有 loader 错误行为，board/task-card 回归已覆盖。
- 外部服务不可达：503 替身、缺网关与关闭写闸门覆盖；本任务没有新第三方服务。

### 3. 全部迁移

- 新增开始行为：draft → active（空槽）、draft → queued（满槽）均实跑；未知 intent 拒绝且不写入；queued 不提供重复开始入口。
- 本里程碑指定跨进程迁移：active → stopped，对应 running → needs_you，在同一个 router 上实跑。其余原有内核状态迁移没有新增规则，本轮不重做内核验收。
- 通知：有数字 → 无数字（标已读）、无数字 → 有数字（新通知）、已订阅 → 取消订阅（后续消息不改变数据）均实跑。
- fetcher 提交/重验的浏览器视觉迁移属于 H-05，未使用 SSR 冒充浏览器点击证据。

### 4. 输入域边界

- intent=start / 未知 / 空串；task_id 空串真实网关拒绝及正常单任务 id 成功；未知/空事件类型与单事件批次均检查。
- 并发槽 parallel=1：0 个占用与恰好 1 个占用；队列第 1 位；未读数 0/1 边界均通过。
- 日期闰年、前导零、文件大小：不适用，本里程碑不新增日期解析、数字输入或上传域。

## 留给审计与宿主

在可监听环境独立运行完整 M-05 契约，确认 WebSocket stopped 事件实测 ≤4500 ms、SSR 列位置。H-04/H-05/H-06/H-07 仍在设计人工清单；本轮无真实浏览器视觉验收。宿主运行既定快检及合并前全量门禁。

最终还原复验确认：4 文件、34/34 通过、0 失败、0 跳过，10.65 秒；真实内核套件 8716 ms。以上四项自审完成后，按契约受限环境分支标 M-05「待审」，不是「已完成」。
