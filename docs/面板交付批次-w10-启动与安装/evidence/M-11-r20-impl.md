# M-11 · 实现 #20

结论：实现与定向单测完成，浏览器验收受环境阻塞，保持开放；未标待审，reopen 保持 0。implementation-round 为 20/65。

## 改动

- 总览使用 stageStatusLine 合成去重的一行阶段与状态。
- 项目 loader 缺省仅传已取消计数；展开时返回四字段投影，按编号倒序，不读取 task.get。已完成列内部显示折叠区，类型筛选与两个查询参数各自保留。
- 门禁、setup、merge.after 共六种日志事件显示文件名，完整路径留在 timeline-log 的 title；会话失败摘要属于 M-13，本轮未扩入。
- e2e 临时账本夹具增加 30 张已取消卡，layout 增加第 12 个状态和区块所在滚动容器断言。

## 命令与结果

所有输出在 `.autome/output/m11-*`，未运行项目全量门禁或宿主快检。

1. 修改前：`npm run test --prefix apps/web -- test/overview.test.ts test/board-payload.test.ts test/task-timeline.test.ts test/testids.test.ts test/board.test.ts`：43 条通过，0 失败、0 跳过。日志 `m11-baseline.txt`。
2. 设计验收命令：`cd apps/web && npx vitest run test/overview.test.ts test/board-cancelled.test.ts test/board-payload.test.ts test/task-timeline.test.ts test/testids.test.ts test/board.test.ts && npx playwright test e2e/layout.spec.ts`：单测 53 条通过，0 失败、0 跳过；Playwright globalSetup 在 spawn 前的进程扫描报 `spawnSync ps EPERM`，teardown 同样无法扫描。12 条浏览器用例未执行，不能报成跳过或通过。日志 `m11-acceptance.txt`。
3. `npm run build --prefix apps/web`：通过，日志 `m11-build.txt`。对本轮 14 个 TS/TSX 文件定向 `npx eslint <文件列表>`：退出码 0，日志 `m11-eslint.txt`。
4. 缺陷注入：`node .autome/output/m11-negative.cjs`：六种注入各自对应测试退出码 1，逐次 finally 还原。分别是恢复 task-status 第二行、cancelled 计入 done、默认 loader 泄漏取消对象、取消链接丢 done、门禁路径不截短、setup/merge 四种路径不截短。日志 `m11-negative.txt` 及六份分项日志。
5. 注入还原后重跑上述六文件 vitest：53 条通过，日志 `m11-final-unit.txt`。`git diff --check` 通过。

总数说明：同组基线 43 → 53，净增 10（overview +1、board-cancelled +5、board-payload +1、task-timeline +2、testids +1）。设计原估计 40 → 50，差的 3 条来自 M-10 已添加的时间线节点回归，本轮保留。当前分布 6 + 5 + 6 + 17 + 11 + 8 = 53。layout 定义 11 → 12，+1 为已取消展开；因环境阻塞，本轮浏览器实跑数为 0，上轮 M-10 的全套 20 条不是本轮通过证据。

测试开发时折叠链接用例曾失败：SSR 夹具没有 ClockProvider，默认时钟为 1970，旧完成任务被误当近期卡；给夹具补页面时钟后四种参数组合通过，未改弱断言。

## 标待审之前的自审清单

1. **情形表逐行**：BL-02 两种任务类型 × 全部阶段节点 × 八种状态，含未知/空节点，单行无重复，通过；BL-03 默认、展开、Bug 筛选、两个开关四种组合、零/单张与数字倒序、四列不受取消影响、140 张载荷上限，通过；BL-04 门禁开始/结果与 setup/merge 四种轻条目，文字及悬停路径检查，通过。30 张已取消的一屏与截图检查已写入 e2e，但未执行。
2. **失败分支**：门禁非零退出与超时、setup/merge 成功与失败结果均执行通过；现有 board 读取失败与 timeline 读取失败提示用例保持通过。授权、外部服务不可达不适用：本里程碑不增加授权或网络调用。浏览器进程扫描拒绝见上文，没有删除检查或条件跳过。
3. **全部迁移**：两个折叠开关的四种组合均检查双向 href；门禁排队→运行→成功/失败/超时、重跑及中断的既有测试通过，任务类型筛选计数与列表一致。本轮不新增内核状态迁移。
4. **输入域边界**：0/1/3/40 张取消任务、T-010 前导零和 T-100/T-2 数字排序、空路径/根路径/单文件名/中文空格路径、未知与空节点均通过。原 board 的十四天边界、闰日和跨年用例保留通过；不存在本轮声明的数值最大值。

上轮审计 M-10 为通过，没有需逐字搬入并红绿复现的退回检查。BL-10 是明确非阻塞的 M-10 Backlog，本轮只推进 M-11。

## 待后续复验

在允许 ps 和监听的宿主环境运行完整 M-11 验收命令，检查 12 条 layout、残留断言与截图；通过后再标待审。当前没有服务/内核/浏览器从该 globalSetup 启动，独立残留扫描仍因 EPERM 未验证。视觉验收未完成。
