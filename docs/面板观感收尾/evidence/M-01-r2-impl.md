# M-01 · 实现轮 2/55 · 宿主快检修复

结论：M-01 保持待审，不推进新里程碑。里程碑验收 108 个用例通过（上一轮 107，增加 1 个样式回归守卫）。原 C-2 浏览器行为未能在本机复验，须以宿主提交后快检为准。

## 失败与修复

- 宿主起点 `90b3508afaef3583937299ef98323a7ff8ee55eb` 的 C-2 在 `manual-items.spec.ts:103` 外层时间线断言失败：期望偏差 ≤ 2，实测 915；前面的日志展开与内层滚动断言已通过。失败发生于第 23 个会话到来后，尚未点击文档或加载更早会话。对应任务约束 10、设计 D-Q2 / R-5。
- 机制判断：`timelineGroups` 按时间倒序，新会话插在顶部；`.task-timeline` 未禁用浏览器滚动锚定。浏览器可随旧锚点向下补偿 scrollTop，而 C-2 契约要求保持数值。稳定 React key 只能保留节点，不能阻止浏览器锚定。已有显式恢复逻辑仅在点击“更早会话”时触发，与此次触发点不符。此判断由宿主失败位置和源码支撑，尚无本机浏览器因果验证。
- 修复仅在 `.task-timeline` 加 `overflow-anchor: none`。同类检查：内层 `.session-log-text` 已有该声明；加载更早会话的 onClick 保存与 useLayoutEffect 恢复、SessionLog key/defaultOpen、订阅和自动跟随逻辑均保持不变。
- 抓到缺陷的检查已经在项目测试体系：`apps/web/e2e/manual-items.spec.ts` 的 C-2 原用例完整保留，未改断言、时限或流程，也不复制成重复用例。没有上一轮审计文档。新增 `styles.test.ts` 守卫保证外层与内层均禁用锚定、外层仍可滚动；该静态守卫不替代原浏览器行为测试。

## 命令与证据

1. 修复前原检查：`(cd apps/web && npx playwright test e2e/manual-items.spec.ts --grep 'C-2 展开日志')`，退出 1，setup `spawnSync ps EPERM`，未执行用例体。选中 1 个，较宿主同名用例范围差 0；不能称为本机复现红。宿主原始红是上述 915px 偏差。输出 `.autome/output/m01-r2-red.txt`。
2. 新守卫先红：`(cd apps/web && npx vitest run test/styles.test.ts)`，总计 16，15 通过、1 失败，失败是外层缺少 `overflow-anchor: none`；较上一轮 styles 15 增加 1。输出 `.autome/output/m01-r2-style-red.txt`。
3. 首次修复后验收发现 CSS 规则前的解释注释被现有简易规则解析器并入选择器，导致新增守卫找不到规则（107 通过、1 失败，共 108）。删除该非必要注释，不放松断言。输出 `.autome/output/m01-r2-tests.txt`。
4. 最终验收：`(cd apps/web && npx vitest run test/ui-base.test.ts test/ui-kit.test.ts test/styles.test.ts test/task-timeline.test.ts test/board.test.ts test/testids.test.ts test/test-groups.test.ts)`，7 文件，**108 通过、0 失败、0 跳过；较上一轮 107 增加 1**，仅来自 styles 15→16，其余为 4、21、29、19、15、4。新增守卫已绿。输出 `.autome/output/m01-r2-tests-final.txt`。
5. 修复后原检查：与第 1 项命令逐字相同，仍在 setup 遇到 `spawnSync ps EPERM`，选中 1 个、执行用例体 0 个，与修复前差 0；未取得行为绿证据。输出 `.autome/output/m01-r2-e2e-final.txt`。

## 四项自审

1. **情形表逐行**：A-Q1 的 Loop 全局/项目范围、项目配置/信息页签、任务四页签、看板任务类型、时间线三筛选，由本轮 ui-base/ui-kit/board/task-timeline 重跑通过；嵌套胶囊不受影响、radio 选中/禁用样式由 ui-base/ui-kit 通过。技能项目选择仍留 M-06，文档胶囊仍留 M-08，不适用本轮。新增时间线外层与既有日志内层的锚定规则由 styles 通过。
2. **失败分支**：本轮 CSS 不新增授权、读盘、超时、外部服务或空输入分支，不适用；原 board 的失败处理、task-timeline 的读取失败/截断/空事件提示随验收执行通过。浏览器初始化失败单独记为环境阻塞，不算行为通过。
3. **全部迁移**：无新增状态机；原分段控件与筛选迁移由验收子集覆盖。与修复相关的“新会话到来→保留日志和两个 scrollTop”“文档切换→保留”“加载更早会话→保留”均在原 C-2，已原样尝试但受环境阻塞；不能以样式守卫代替这三个浏览器迁移的通过证据。
4. **输入域边界**：本轮无新增数值/日期输入域，不适用；既有空/单项/禁用分段组件、空时间线与未知事件由验收子集执行通过。顶部新增条目的浏览器布局边界仍待宿主 C-2 确认。

## 快检与交付

- 指定命令两次均为 `sh scripts/ci-quick.sh --since 90b3508afaef3583937299ef98323a7ff8ee55eb`。首次 61s，最终代码重跑 59s，均退出 1、执行 21 步、沿用 0、失败 2。原始日志分别为 `.autome/output/m01-r2-quick.txt`、`.autome/output/m01-r2-quick-final.txt`。
- 最终快检 Vitest 汇总总数 **69**（68 通过、1 失败），两次同范围差 0；较上一轮快检记录的 800 减少 731，原因是 `--since` 起点与改动范围不同，本轮只改 CSS 和样式测试，相关测试选取变窄，不能把动态快检范围与固定里程碑 108 混算或视为删除用例。Web e2e 在 setup 阶段失败，用例体未执行。
- 两个失败步骤：`test/shell-live.test.ts` 的 `3 listening development SSR contains the real project and needs-you count` 报 `listen EPERM 127.0.0.1`；Web e2e setup/cleanup 报 `spawnSync ps EPERM`。没有绕过权限、修改门禁或弱化用例。无法证明夹具进程已清理，保留宿主清理与复验责任。
- 快检中的静态检查通过；`git diff --check`、状态字段检查通过；`git diff --exit-code -- apps/web/e2e/manual-items.spec.ts` 通过，确认原回归用例未修改。未运行 `sh scripts/ci-local.sh`，未启动下一会话。
- 宿主下一步：在提交后运行同一快检，确认原 C-2 全流程通过；本机尚无快检全绿或浏览器修复成功证据。
