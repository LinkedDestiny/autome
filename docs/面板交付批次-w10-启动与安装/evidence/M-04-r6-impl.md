# M-04 实现轮 #6

结论：待审。仅推进 M-04；上一轮 M-03 已通过，无待迁移的审计缺陷检查。

## 实现

- 完成列默认取 14 天内按完成时间倒序的 20 张；展开仍取全部；列计数和「查看全部（60）」基于完整轻量任务列表。
- 任务投影为卡片字段（另保留共享 ShellTask 所需 project_id）；完成卡不读取 task.get。toCardDetail 只保留门禁、排队、重试、里程碑与当前会话的显示字段；当前会话用单个 session 表示，不传历史 sessions。
- 待开始列的草稿与排队卡均不渲染门禁行，其它列继续显示。
- 第一批事件开启 1000 ms 窗口，事件与全量重验请求合并；到点交给路由 matcher，下一批另开窗口。仍在途的事件保留到后续重验，旧完成按 generation 防止清除新批。
- 真实通知回归原 1000 ms 等待与新窗口相等，实跑第 7 条超时失败；仅将通知断言等待设为 2500 ms 后通过，断言不变。wait-for-transition 的 4500 ms 预算不变。

## 命令与结果、总数差

所有原始输出位于 `.autome/output/`。没有运行 ci-local.sh、宿主快检或其它全量门禁。

1. 改前基线：`cd apps/web && npx vitest run test/live-routes.test.ts test/board-live-timing.test.ts test/board.test.ts`：3 文件 20 条通过，0 跳过。输出 `m04-baseline.txt`。
2. 验收：`cd apps/web && npx vitest run test/board-payload.test.ts test/live-routes.test.ts test/board-live-timing.test.ts test/board.test.ts --silent=false`：4 文件 28 条通过，0 跳过；比同范围基线 20 增加 8（board-payload 5，live-routes 3）。输出 `m04-acceptance.txt`。
3. 验收加直接相关任务卡：上述命令增加 `test/task-card.test.ts`：5 文件 40 条通过，0 跳过；最后结果 `m04-final.txt`。task-card 12 条数量未变，增加 504 失败输入、同步完成卡不取详情断言。
4. `cd apps/web && npx vitest run test/board-live.test.ts`：修正通知等待前 7 条中 6 通过、1 失败（`m04-board-live.txt`）；修正后 7 条全部通过（`m04-board-live-green.txt`）。真实内核用夹具创建临时 AUTOME_HOME；测试 finally 停止内核、删除临时目录。
5. 本轮最终去重测试总数 **47**，全通过、0 跳过。相同文件的改前用例总数为 **39**（实跑基线 20 + 原有 task-card 12 + 原有 board-live 7），净增 **8**。上轮 M-03 的 26 条属于安装子集，不能与本轮不同范围相减。
6. 对本轮 10 个变更 TS/TSX 文件执行 `npx eslint <文件列表>`，退出 0，无输出（`m04-eslint-final.txt`）。`git diff --check` 通过。未运行全项目类型检查，留给宿主快检。

100 任务夹具实测：默认 loader 数据合计 33,632 字节、加 SSR 标记 70,496 字节；展开全部 33,631 / 92,775 字节；均低于 204,800 字节上限。root 数据使用真实 loadShell，非草稿详情来自原 w6 夹具，任务克隆 w5 夹具。

## 标 `待审` 之前的自审清单

1. **情形表逐行**：B1 默认 / done=all 文档与数据量分别跑 board-payload #1/#2；默认 20、完成时间倒序、60 总计与展开/收起跑 #3；完成卡 0 次 task.get、其它 30 次及字段白名单跑 #4；待开始两类卡无门禁且运行卡保留门禁跑 #5。B2 同秒 5 批一次、不同前缀并集、reload 并入、下窗口分别跑 live-routes 新增 3 条。全部通过。
2. **失败分支**：board #7 检查无 gateway、503、409、无错误文字回退；task-card #10 对 task.get/dashboard.get 各跑 409、503、504。live-routes 最后一条检查重验 Promise 拒绝后状态清理及下一窗口恢复；board-live-timing 检查丢事件超时、命令失败与取消订阅。均通过。文件读盘失败不适用：本轮生产改动不直接读文件；网关错误分支已覆盖。授权拒绝在本轮只读网关 409 错误传递边界验证，不新增鉴权逻辑。
3. **全部迁移**：board #1 覆盖 14 种状态及未知状态的列映射；#4 覆盖 all/feature/bug 的 3×3 筛选迁移、计数及输入不变；board-payload #3 跑默认→全部→默认；live-routes 覆盖空闲→等待→在途→空闲、窗口内事件/all 合并、在途叠加下一批、旧完成先结束、新批仍在途、旧完成时新批尚在等待、失败→恢复；导航与表单仍遵循默认重验。board-live 用真实内核验证启动、排队、停止换列、通知已读/新通知。均通过。
4. **域边界**：board-payload #3 跑完成卡 0/1/19/20/21；board #3 跑 14 天含边界、超出 1 秒、无日期/null/空串、跨年和闰日；board #1 跑 T-010 前导零编号；投影检查无会话、多个历史会话与一个运行会话、源对象不变；live-routes 检查 999/1000 ms、空事件和未知前缀。均通过。任务 ID 最大数值域未由本里程碑声明，不适用。

## 负向对照

逐次临时修改生产源码、运行指定项目用例、finally 原样恢复。四次均退出 1；没有保留突变。

- 取消详情投影：`npx vitest run test/board-payload.test.ts -t '1 default'`，数据已达 224,222 字节而失败，输出 `m04-negative-projection.txt`。
- 去掉 20 张上限：`npx vitest run test/board-payload.test.ts -t '3 default'` 失败，输出 `m04-negative-cap.txt`。
- 窗口改为 0 ms：`npx vitest run test/live-routes.test.ts -t '同一秒'` 失败，输出 `m04-negative-window.txt`。
- queued 使用覆盖而非并集：`npx vitest run test/live-routes.test.ts -t '窗口内不同'` 失败，输出 `m04-negative-union.txt`。

恢复后最终 40 条与真实内核 7 条均通过。未进行真实浏览器视觉验收，M-04 契约以 SSR/模型测试取证，浏览器巡检由后续 D 里程碑执行。
