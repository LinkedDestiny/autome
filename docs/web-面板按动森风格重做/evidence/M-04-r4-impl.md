# M-04 · 实现轮 4/65

结论：项目看板已实现，Vitest 验收通过；浏览器验收被沙箱 `spawnSync ps EPERM` 阻断，M-04 保持「开放」，不标待审。上一轮审计 M-03 通过，没有退回缺陷或需要逐字搬入的审计检查，本轮不适用该类红绿复现。

## 实现与修复

- 四列使用棕 / 青 / 黄 / 绿小号丝带，队列说明改为「用卡片上的按钮调整顺序」；保留键盘可用的调序按钮。
- 在途卡片复用总览 `overviewCardView` 与 `toOverviewDetail` 投影，显示摘要、花费、用时和当前会话；运行卡用进度条替代旧阶段条，待处理卡提供受控详情入口。待开始显示创建时间、等待原因和位置。
- 已完成使用六字段展示投影（编号、标题、完成时间、实现轮、退回、花费）；实现轮只读 `metrics.impl_rounds_used`，Bug 和缺指标显示「—」，花费缺失显示「无计费」。不读取已完成任务详情。
- 服务端时区划分今天 / 昨天 / 本周更早 / 更早，以周一为周起点；默认前三组展开，「更早」只有计数与 `?done=all` 链接，已取消折叠及查询参数共存保留。
- 总计包含全部已完成需求与 Bug，排除取消；平均仅含有指标的需求，空分母「—」，悬停说明口径。
- 空门禁返回 `null` 且卡片不渲染门禁行；保留门禁排队和实际成功 / 失败结果。详情通知的非空队列调用同步适配可空类型。
- 测试中移除旧「14 天 / 20 张」断言，替换成日期组行为；保留 200 KB 与 30 次 task.get 上限。真实夹具在 merge 后持续 scheduler.tick，等真实 done 再取指标，未把异步 merge 返回当作完成。

## 验收命令与结果

1. `(cd apps/web && npx vitest run test/board.test.ts test/task-card.test.ts test/task-columns.test.ts test/board-payload.test.ts test/board-cancelled.test.ts test/board-done-real.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts)`：9 文件 **82/82** 通过、0 跳过。日志 `.autome/output/m04-acceptance.txt`。真实用例复用 w7-flow.sh、release-main-check、autome merge，两个断言均通过。
2. `(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "项目|全部已完成|已取消展开|B-2|B-3")`：全局 setup 在进程检查中被 `ps EPERM` 拒绝，**实际执行 0 条**，未获得截图、一屏、调序及依赖浏览器证据。日志 `.autome/output/m04-browser.txt`。没有修改或绕过进程检查。该正则除契约点名的 5 条外还匹配「项目设置」与「添加项目补全」，实际选择 7 条，均未进入断言。
3. `npm run typecheck --prefix apps/web`：修复可空门禁调用后通过。`git diff --check` 通过。
4. 最终相关子集合并运行上述 9 文件及 `test/styles.test.ts test/clock.test.ts test/task-page.test.ts test/overview.test.ts`：**13 文件、130/130 通过、0 跳过**，日志 `.autome/output/m04-final-tests.txt`。同范围基线 119，增量仍为 +11；额外四文件没有新增用例。
5. 指定快检：`sh scripts/ci-quick.sh --since 0041176b500163b10de96c2e369c0529cfd2928e`，结果见本文件最终补记；日志 `.autome/output/m04-quick.txt`。未运行全量门禁。

### 用例总数与基线差

同范围起点基线为 **71**，本轮 **82，+11**：board 13→19（+6）、task-card 14→16（+2）、labels 10→11（+1）、新增 board-done-real 2（+2）；task-columns、board-payload、board-cancelled、testids、test-groups 数量不变。上一轮 M-03 报告 72 条属于不同验收范围，不能当作本轮增量分母。浏览器选择数量仍为 7，增量 0，新增断言放在既有「项目」用例内；实际执行 0，不计作通过。

## 标待审之前的自审清单（本轮已执行，环境项仍未满足）

1. **情形表逐行**
   - C-1 四列丝带：board #14 验证四种色与小号；卡片分类沿用 columnOf，board #1/#2 与 task-columns 覆盖。共用总览投影；字段缺失时保留摘要不可得与无计费回退。
   - C-Q4 调序：保留 QueueControls 和 task.move；board #11–#13 验证内核位置排序与按钮条件。B-2/B-3 浏览器执行被环境阻断，未声称实际操作通过。
   - C-Q1 条目与统计：board #15–#18 验证四组、六字段、Bug/缺指标、累计与平均口径；board-done-real 2 条用真实账本比对。
   - C-Q2 折叠与体积：board #6/#19、board-payload 6 条、board-cancelled 5 条验证可逆链接、查询共存、取消不混入完成、200 KB 与 30 次详情读取。
   - C-3 空门禁：labels 新用例、task-card #15/#16 检查 null、在途不渲染及各列无旧文案。
2. **失败分支**
   - 项目读取拒绝 409、离线 503、缺 gateway：board #7；配置错误保留原文与任务：board #9/#10。
   - 详情读取 409/503/504 与冷却 dashboard 失败：task-card #10，验证详情缺失不吞任务、冷却失败明确返回错误；空门禁不误报。
   - 直接读盘、写授权新增分支：不适用，本轮沿用 gateway 与既有队列写入；真实浏览器授权路径未新增且因环境未复验。
3. **全部迁移**
   - all / feature / bug 的 3×3 筛选：board #4，计数与源数据不变。
   - 默认→全部→默认：board #6、board-payload #3；取消展开 / 收起与 done 参数共存：board-cancelled 与 board #19。
   - 既有需求与 Bug 节点及 paused/stopped/queued/failed 映射：task-card #1/#2；真实需求 await_merge→done：board-done-real。
   - 没有新增内核状态迁移；B-2/B-3 的交互迁移未取得浏览器证据。
4. **输入域边界**
   - 周一 00:00、周日、跨年、闰日、上海与 UTC 日界、空串 / null / 无效完成时间：board #3/#15/#16。
   - 空任务、单任务、前导零编号、0/1/19/20/21 条、100 条加 40 取消：board #1/#12、board-payload #3/#6。
   - 无指标、Bug 指标、无花费与空平均分母：board #17/#18。日期比较使用日历日序号，避免按小时长度推算本地日界。

## 环境与交接

M-04 保持开放，reopen 不增加（本轮不是审计退回）。下一步在允许进程检查与浏览器的宿主环境重跑 M-04 验收和快检，检查一屏与截图后再标待审。视觉核验未完成，人工 H-03 未勾选。

## 快检修复记录

首轮快检发现圆角 token、日期模块边界、总览链接的旧源码检查三项失败，以及浏览器环境阻断。圆角改用既有 token；日期分组计算移入 clock；总览视图真正复用 taskHref，测试沿 overview-card → overviewCardView → taskHref 检查，保留共用详情入口的原意。上述三项失败用例均已在最终 130 条子集中变绿。设计列出的负向注入本轮未另外执行，不声称已完成变异验证。

### 最终快检与收尾

- 相同指定快检重跑：退出码 **1**，执行 77 步、失败 1 步，用时 144 秒。唯一失败为 Web e2e 的 `ps EPERM`，静态检查通过，已执行 Vitest **578/578** 通过；85 个步骤由快检预算规则标为未跑，不能当作通过，也未另行扩大到全量门禁。最终日志 `.autome/output/m04-quick-final.txt`。
- 快检按改动依赖自动选择的 578 条范围与本轮验收范围不同，不与上轮 72 条直接比较；本轮可比的最终验收及直接相关子集仍为 **130 对 119，+11**。
- 状态块字段、4/65、M-04 开放、证据文件存在、retro 仅一行且小于 200 字均已检查。`git diff --check` 通过。
