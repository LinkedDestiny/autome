# M-07 · 实现 #12 · 宿主布局失败修复轮

起点 `bc98ab330e83fafe4ecbf1b282b6d3623fc11374`。本轮不推进新里程碑，M-07 保持待审，状态为 `12/66`。

## 失败检查与修复

宿主失败检查已在项目测试体系 `apps/web/e2e/layout.spec.ts:92`：`expect(box.y + box.height).toBeLessThanOrEqual(bounds!.y + bounds!.height);`。原检查逐字保留；不改十条、44px、滚动区，也不通过滚动移动十条的起点。宿主红证据：第十条下边缘 943.53125 > 920。

查看了宿主 `.autome/output/m07-collapsed-detail.png`：阶段格「claude · codex · 无计费」换行，末行被裁；顶部高度挤占时间线。按操作者授权提前实现 D-Q6：阶段 `small` 与标签 `b` 单行省略，全文同时保留在正文和 title 中；时长、计费、门禁次数、合并状态文案不变。桌面 ≥1100×800 的统计卡上下内边距从 8px 改为 2px，小视口统计卡不变。没有修改时间线行高、分组、数据及交互。

## 操作者备注与预算

采纳备注方案 1（阶段单行省略）和方案 2（统计卡收紧，但每边收 6px，以增加余量）；不采纳方案 3，保留设计要求的 40px 最小行高。阶段信息仍是标题、时长、计费三类内容，压掉的是计费内部的额外换行，不删任何一类。

以下是按宿主实测基线计算的预算，**不是本轮浏览器实测**：

| 区块 | 宿主原高度 | 修改后预算 | 节省 |
|---|---:|---:|---:|
| 标题（y≈105 起） | 70px | 70px | 0 |
| 阶段条 | 87px | 约67px | 约20px |
| 六指标卡 | 77px | 65px | 12px |
| 提示横幅 | 43px | 43px | 0 |
| 其余区块间隔合计 | 约48px | 约48px | 0 |
| 时间线上方总占高 | 325px | 约293px | 约32px |

时间线预计从 y≈430 上移至 y≈398，下边缘仍 920，高度约522px；十条及三个分组、筛选占高不变，第十条下边缘预计 `943.53125 - 32 = 911.53125px`，余量约8.47px。阶段行高受字体实际排版影响，最终必须由宿主原几何断言确认，不能用这张预算表代替绿证据。

## 红绿与验收

- 修复前原样运行 `(cd apps/web && npx playwright test e2e/layout.spec.ts -g '1512×944 需求详情展开日志')`，准备阶段 `spawnSync ps EPERM`，0 个实际执行；日志 `.autome/output/m07-r12-red.txt`。宿主失败已有正式回归，无需复制一份重复用例。
- 新增阶段单行与完整计费 title 回归后、修改产品代码前，`(cd apps/web && npx vitest run test/task-summary.test.ts)`：21 个，20 通过、1 失败；较原20个 +1。日志 `.autome/output/m07-r12-unit-red.txt`。
- 首次实现后新增测试抓到编辑时美元符号丢失：107 个中106通过、1失败，日志 `.autome/output/m07-r12-accept.txt`；已修复，原文案保持不变。
- 最终执行 `(cd apps/web && npx vitest run test/task-timeline.test.ts test/timeline-failure-path.test.ts test/task-detail-ssr.test.ts test/task-page.test.ts test/testids.test.ts test/styles.test.ts test/task-summary.test.ts)`：**107/107通过**，0跳过，7文件。M-07原验收集合86个，相对上一轮86个差0；直接相关 task-summary 21个相对原20个 +1，合计相同文件集合106→107。日志 `.autome/output/m07-r12-accept-green.txt`。
- 为输出实测字节数，再跑 `(cd apps/web && npx vitest run test/task-detail-ssr.test.ts --reporter=verbose --disableConsoleIntercept)`：3/3通过，相对原3个差0；60会话 SSR **81582字节**、数据响应 **25689字节**，65会话固定窗口 SSR **93254字节**，均≤153600。日志 `.autome/output/m07-r12-ssr.txt`。此重复运行不另算新增用例。
- 修复后原样重跑浏览器子集，仍 `ps EPERM`，0个执行；相对本轮修复前差0，相对宿主子集1个少1个。日志 `.autome/output/m07-r12-browser-after.txt`。未取得本地几何绿证据。

## 标待审之前的自审清单（本轮保持待审）

1. **情形表逐行**：D-Q1四类事件、紧凑行、完整文字、结果与更早会话入口均由原 task-timeline 检查通过；D-Q2日志与文字独立展开结构通过。D-Q6覆盖全无计费工具串及混合计费长串正文/title完全一致，时长、合并、门禁次数与标签共用单行省略。首屏十条原几何检查保留，实际浏览器受阻。
2. **失败分支**：timeline-failure-path六项通过，覆盖门禁失败/超时、准备失败、空缺失路径、中文路径及原始字段；既有日志失败/截断检查未删。本轮仅展示调整，拒绝授权、外部服务不可达不适用，无新增请求或权限分支。
3. **全部迁移**：原集合验证运行/结束/停止/超时/崩溃、门禁排队/运行/结果、退回、筛选及更早会话结构；summary既有等待进入/保持/退出与切换检查通过。无新增状态机；文字与日志打开/关闭的浏览器实际迁移未执行成功，不声称已完成浏览器验收。
4. **输入域边界**：原集合空/单条、长系统文本、多里程碑摘要、earlier=0/1、60/65会话通过；新增计费边界是13会话中12个无计费与全部13个无计费。日期解析、前导零不适用，本轮未改解析。小视口与断点两侧几何尚未实测，desktop padding的适用范围明确保留。

## 快检与交付

未运行全量 `sh scripts/ci-local.sh`；未切换分支、建worktree、推送或启动下一会话。快检最终结论以 Autome 提交后重跑为准。

指定命令 `sh scripts/ci-quick.sh --since bc98ab330e83fafe4ecbf1b282b6d3623fc11374` 最终重跑：31步、28成功、3失败，86秒；相关阶段67秒≤120秒。Format、Clippy、lint、typecheck、build通过。最终日志 `.autome/output/m07-r12-quick-final.txt`。首次快检运行于美元符号修正前，额外有该回归失败，日志 `.autome/output/m07-r12-quick.txt`，不作为最终结果。

最终快检Vitest共168个定义，164通过、1失败、3跳过；相对上一轮快检90个 **+78**。来源是改动依赖集变化：本轮不再选择 task-timeline 的37个（本轮专门验收仍运行了它），新增选择 task-summary21、live-routes15、task-detail-data12、task-detail-payload4、task-header7、task-page-effect4、task-page9、task-stage14、testids15、ui-base11、task-live-real3，合计115，115−37=78。这里仅1个是新写用例，不能把依赖扩选当新增覆盖。

失败原因：shell-live开发SSR `listen EPERM`；task-live-real准备/收尾 `ps EPERM`（3个跳过）；Web e2e准备 `ps EPERM`，35个未执行，相对宿主35个少35个，测试定义未减少。**未取得快检全通过或原几何检查绿证据**，不得将静态通过等同浏览器验收。各命令存在重叠，不累计为去重用例总数。`git diff --check`通过。
