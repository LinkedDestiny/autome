# M-02 · 实现 #3

结论：验收通过，提交待审。本轮仅推进 M-02；上一轮审计 M-01 已通过，没有退回缺陷或需要逐字迁移的审计检查。

## 改动与依据

- 新增完整任务卡模型和组件：需求七阶段、Bug 五阶段、里程碑/修复轮次、会话/等待/冷却/重试/失败动态、真实门禁和独立合并提交信息、需要处理角标。标题链接与卡片容器分离。
- 看板 loader 并发读取可见非草稿详情；旧完成卡仅展开时取详情，只有重试才查询 dashboard。`needYouKinds` 从既有规则计算，SQLite 经 main → http（开发/生产两处）→ load-context 接线。详情或 dashboard 的网关拒绝原样返回页面。
- 源码确认 `dispatch.rs` 的重试字段是 `attempts`，不使用虚构的 `attempt`。中文句式集中在 labels，样式沿用现有 token；首页简版卡未改。
- `board.test.ts` 第 5 项的替身按 method 返回 project/detail，原「只有 project.get」断言改为「project.get 恰好一次」。原因是 M-02 按设计增加 task.get；仍保留原计数交叉核对，详情调用数另由 task-card (10) 和真实内核 (2) 约束。
- 使用 `bash apps/web/test/fixtures/w4-ledger.sh "$PWD/.autome/output/m02-record" "$PWD"` 重录真实完成任务，保存 task.get payload 为 `apps/web/test/fixtures/w6-task-get.json`。真实 w5-states 没有完成任务，完成卡真实门禁由此录制夹具在 task-card (8)/(10) 覆盖。

## 命令、数量与结果

所有命令从当前 worktree 执行；原始输出均在 `.autome/output/`，未转存进本目录。

1. 改动前基线：`cd apps/web && npx vitest run test/labels.test.ts test/overview.test.ts`：2 文件、12 用例通过（7 + 5），0 失败、0 跳过。输出 `m02-baseline.txt`。
2. 里程碑验收：`cd apps/web && npx vitest run test/task-card.test.ts test/board-detail-real.test.ts test/labels.test.ts test/overview.test.ts`：4 文件、27 用例通过，0 失败、0 跳过。相对同口径基线 **+15**：任务卡新增 12、真实内核新增 2、Runtime 枚举交叉核对新增 1；overview 5 不变。首次与负向对照还原后的最终复跑均通过；输出 `m02-acceptance.txt`、`m02-acceptance-final.txt`。
3. 直接相关回归：`cd apps/web && npx vitest run test/board.test.ts test/styles.test.ts test/bell-view.test.ts test/live-routes.test.ts test/task-columns.test.ts test/load-context.test.ts`：6 文件、49 用例通过，0 失败、0 跳过。相对上一轮 M-01 的 40 个 **+9**，差额完全来自本轮接线相关的既有 load-context 9 项，原 40 项没有删减。输出 `m02-regression.txt`。
4. 本轮最终验证集合共 **76 个不同用例**；相对上一轮 40 个 **+36** = M-02 验收集合 27 + 接线回归 9。该集合不等于全项目总数；没有把负向变异复跑累计成新用例。
5. `node .autome/output/m02-mutations.cjs`：五项设计负向对照全部被断言拒绝（不是导入/语法失败）：Bug 改读里程碑→(4) 红；空门禁当通过→(8) 红；草稿取详情→(10) 红；完成任务不取详情→(10) 红；遗漏 not_a_bug 通知→(11) 红。每次 finally 还原源码，最终验收 27 项全绿。各次输出为 `m02-mutation-*.txt`，汇总 `m02-mutations.txt`。
6. `git diff --check`：通过。

真实内核测试准备/执行/清理首次 4777 ms，最终 8677 ms；beforeAll/test 超时 120000 ms，夹具进程超时 60000 ms，均超过实测两倍。没有监听端口。会话刚启动按设计显示「刚开始」；真实内核用例将同一会话的视图时钟推进一分钟，验证「运行」文案，不靠 sleep 等待。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §2 行 | 执行检查及结果 |
|---|---|
| 类型标签 | cardView 使用 KIND_LABELS，SSR task-card (1)/(4) 覆盖两种卡片；未知类型保留原值的实现已核对 |
| id、标题 | task-card (1) SSR 验证 taskHref 链接；组件 article 内独立标题链接，源码核对通过 |
| 阶段进度条 | (1)/(2) 遍历需求/Bug 全节点和 active/queued/paused/stopped/failed 状态；完成末段、未知节点不高亮通过 |
| 附加进度 | (3)/(4) 验证实现/修复阶段、其它阶段隐藏、零里程碑、单里程碑及 Bug 不显示里程碑；通过 |
| 当前动态 | (5) 会话与分钟边界；(6) 冷却精确匹配/不匹配；(7) 四等待短语优先、renderReason、队列位置/缺省；真实内核 (1)；通过 |
| 门禁结果 | (8) 排队、无结果、通过、失败退出码、完成任务真实 gate 与合并短 SHA 并列；通过 |
| 等你处理角标 | (9) 列与数字；(11) kind 来源；(12) 真实 SQLite 2 条 await_merge + done + 已读排除，markAllRead 后无数字；通过 |
| 详情取数 | (10) 草稿不查、最近完成查、历史折叠不查/展开查、无重试不查 dashboard/有重试一次；真实内核 (2) 调用数交叉核对；通过 |

### 2. 失败分支

- 项目网关 503/409、未接网关：board (7) 原样错误通过。
- task.get / dashboard.get 的 409/503：task-card (10) 原样错误通过。
- 任务失败、门禁失败与缺失结果：task-card (7)/(8) 和真实内核 (1) 通过；不把未跑过误报通过。
- 空输入：未知空节点、空任务列表、空未读集合、无会话/详情/里程碑由 task-card (1)/(3)/(5)/(9) 与 board (1)/(6) 检查通过。
- 拒绝授权、上传读盘失败、外部上传服务超时：不适用，本轮为只读任务卡，不新增这些通道；原网关不可达用 503 分支覆盖。

### 3. 全部迁移

- 本轮不新增状态机写操作，事件驱动换列和通知订阅属于 M-05，不宣称本轮已完成。卡片投影按两类任务全部阶段节点 × active/queued/paused/stopped/failed 检查，另覆盖 done/draft/cancelled/未知状态。
- 折叠→展开和展开→折叠、三筛选值之间全部组合由 board (4)/(6) 检查；详情折叠/展开请求由 task-card (10) 检查。
- 通知未读→已读：真实 SQLite markAllRead 后重跑 loader，无数字；没有伪称浏览器实时更新已验收。

### 4. 输入域边界

- 阶段未知值/空串/`__proto__` 不误高亮；全部已知节点均覆盖。
- 里程碑总数 0/1，未读数 0/1/2；无 queue_position、无 gate、无 status_block；通过。
- 会话时间 0 分钟、59 秒、12 分钟、未来时间（钳到刚开始）；冷却精确相等和差一分钟；通过。
- 14 天恰好/多一秒、空完成时间、闰日/跨年、任务 ID 前导零及空列表沿用 board (1)/(3) 回归通过。
- 任意字符串解析或数字最大值的新增接收域：不适用，卡片不新增用户输入接口，使用既有内核返回值。

## 未运行与交接

- 按本轮边界未运行全量门禁、宿主快检、全量 Web 测试、build。
- 浏览器截图、一屏测量和 H-07 视觉人工验收未取证；本轮证据为 SSR、纯函数、真实内核与 SQLite，不替代人工验收。
- M-02 交审计独立复验；不关闭里程碑、不启动下一会话。
