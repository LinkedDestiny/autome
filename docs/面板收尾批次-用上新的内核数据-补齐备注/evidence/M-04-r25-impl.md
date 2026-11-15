# M-04 · r25 · 实现修复证据

结论：修复 B-3 的瞬时读取竞态；相关单元测试通过，浏览器运行受沙箱阻断，不能声明端到端已通过或全套超时已解决。本轮不推进里程碑，既有表格与 status 保持原值，implementation-round 为 25/65。

## 根因与修复

宿主 df5ba1d19b 门禁 B-3 在 active 后读到 sessions.length = 0；M-04-r7-audit.md 已记录同一检查 2/4 失败（BL-13）。scheduler.rs 的 advance 先 set_task_state（2832 行），后 perform / start_session，3967 行才 insert_session，因此 active 不意味着会话已落账。其它假设是会话永久启动失败或响应缓存；前者应持续为 0，后者不符合 CLI 每次独立读取账本的路径。

原判别检查已在项目测试体系 controls.spec.ts 的 B-3 中，无需另搬临时审计脚本。将一次读取改为 expect.poll，每次重新调用原 task.get，仍要求 sessions.length > 0；沿用 Playwright 默认 10 秒断言预算。前置合并前保持 queued、合并后 active、卡片等待文字消失的检查均保留。未修改产品调度、测试数量、跳过条件、全局 360 秒或生产 3000ms tick。

同类核查：e2e 中另一处会话数检查 manual-items.spec.ts 已用 expect.poll；B-2/B-4 等 active 的用途是确认占用槽位，没有立即推断会话已插入。

## 命令与结果

- 修复前原样运行：`(cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3')`；setup 首次进程检查报 spawnSync ps EPERM，未执行用例。原始输出 .autome/output/r25-b3-before.txt。宿主提供的 Expected > 0 / Received 0 是真实红色证据，本机此次不能声称复现该浏览器失败。
- 原样断言的隔离时序验证：`node .autome/output/r25-poll-check.mjs`，直接抽取 HEAD 旧断言及工作区新断言，使用真实 Playwright expect，以替身 CLI 返回序列 [0,1]：旧断言红（只读 1 次），新断言绿（读 2 次）。[1] 立即绿；[0] 永久空负向对照仍红。4 项结果符合预期。日志 .autome/output/r25-poll-check.txt。此为断言时序诊断，不能代替真实内核/浏览器验收。
- `(cd apps/web && npx vitest run test/board.test.ts test/task-new.test.ts test/task-card.test.ts test/queue-deps.test.ts test/test-groups.test.ts)`：5 文件、50 用例全通过，日志 .autome/output/r25-unit.txt。相对 M-04-r7-audit 的同范围 50 例差 0，无增删跳过。
- 修复后同一 B-3 命令：同样在 setup 被 ps EPERM 阻断，日志 .autome/output/r25-b3-after.txt。选择 1 例、实际执行 0 例，与修复前相同；整个 Web e2e 仍为既有 29 例，本轮未运行全套。诊断额外 4 项相对此前无该诊断为 +4，不计入项目正式用例数。
- 未运行 ci-local.sh 或 ci-quick.sh。宿主负责快检、B-3 真浏览器复验及全量门禁；360 秒总超时仍待宿主实测。

## 四项自审

1. 情形表逐行：设计 B-3 的合并前 queued、合并后 active、实际会话创建、等待文案消失四项断言均保留；会话创建增加重复读取。queue-deps/task-new/task-card/board 对应模型检查通过。真实浏览器四项因 setup 阻断未执行。
2. 失败分支：本次仅修改等待断言，无授权/读盘/外部服务分支变动；永久无会话的超时负向对照失败符合预期，未弱化成只检查 active。其它设计失败分支由本次 50 例子集中的原有校验覆盖，不声称重跑其它里程碑。
3. 全部迁移：本修复涉及会话数量 0→1（旧红新绿）、初读已有 1（绿）、持续 0（超时红）均执行；真实 queued→active→会话创建链路待宿主复验。无产品状态机改动。
4. 输入域边界：会话数空数组与单元素均检查；不适用文本/日期/最大值边界，本次未改变任何输入域。

## 清理与交接

两次 Playwright 均在 start 的首个 leftoverProcesses 调用失败，尚未 prepare 或 spawn 服务/内核/浏览器，本轮没有自起服务需要终止。没有绕过进程检查，也未改 .autome 配置；运行输出仅写入获准的 .autome/output。修复仅测试与本轮文档；不推进新里程碑、不启动下一会话。
