# M-07 · 实现 #13 · 宿主首屏十条检查修复

本轮起点 `c9eaa3868338e0b2063d3d82d8da2d76f8949dd9`。仅修复宿主退回，不推进新里程碑；M-07 保持待审，implementation-round 为 `13/66`。

## 缺陷、检查与修复

宿主原检查已在项目测试体系 `apps/web/e2e/layout.spec.ts:92`，逐字保留：`expect(box.y + box.height).toBeLessThanOrEqual(bounds!.y + bounds!.height);`。宿主红结果为第十条底边 931.53125 > 920，越界 11.53125px；35个e2e中34通过、1失败。没有删减条目、改变起始滚动位置、放宽44px或改动原几何断言。

核对宿主截图 `.autome/output/m07-collapsed-detail.png` 及页面结构：阶段区仍占约96px（CSS上限），上轮把阶段文字改单行没有令该区实际节省预计的20px。顶部五个区块间四段8px间距占32px。本轮仅在既有 ≥1100×800 桌面媒体查询中将区块间距改为4px，四段合计16px，释放16px给时间线；其他视口不变。按宿主基线推算第十条底边915.53125px，余量4.46875px。此为几何预算，**不是本轮浏览器实测通过**。阶段、提示内部滚动及统计卡内容均不变。

## 红绿与验收命令

- 修复前原样运行 `(cd apps/web && npx playwright test e2e/layout.spec.ts -g '1512×944 需求详情展开日志')`，准备阶段 `spawnSync ps EPERM`；实际执行0个，相对宿主同子集1个少1个。日志 `.autome/output/m07-r13-red.txt`。原检查已正式登记，不重复复制用例。
- 先更新既有桌面预算守卫，再运行 `(cd apps/web && npx vitest run test/task-timeline.test.ts)`：37个，36通过、1失败（预期4px，产品仍8px），总数较上轮37个差0。日志 `.autome/output/m07-r13-unit-red.txt`。
- 修复后运行 M-07 原验收命令 `(cd apps/web && npx vitest run test/task-timeline.test.ts test/timeline-failure-path.test.ts test/task-detail-ssr.test.ts test/task-page.test.ts test/testids.test.ts test/styles.test.ts)`：**86/86通过、0跳过**。同一集合上轮86，本轮差0，只更新一个既有守卫。包含上述37个回归，不重复计数。日志 `.autome/output/m07-r13-accept.txt`。
- 修复后原样重跑浏览器子集，仍在准备阶段 `ps EPERM`，实际执行0个，较修复前差0，较宿主少1个；日志 `.autome/output/m07-r13-browser-after.txt`。没有本地几何绿证据。

## 标待审之前的自审清单（本轮保持待审）

1. **情形表逐行**：D-Q1会话/门禁/用户/系统四类行，时间、类别、文字、耗费、结果顺序及完整文字展开结构均由 task-timeline 验证；空态、更早入口、40px行及分组间距守卫通过。D-Q2运行日志默认开、结束默认收，文字与日志独立结构通过；原浏览器实际十条完整可见检查保留但受阻。桌面区块间距按五个直接子区块逐项核对，四段均减4px。
2. **失败分支**：timeline-failure-path六项通过，覆盖失败/超时、准备失败、缺失路径、中文路径、原始字段；原日志失败/截断检查保持。拒绝授权、外部服务不可达不适用，本轮仅CSS间距，无新增请求或权限路径。
3. **全部迁移**：task-timeline覆盖运行/正常结束/停止/超时/崩溃、门禁排队/运行/成功/失败、退回及筛选状态；本轮无新增状态机。真实浏览器展开/关闭、实时更新保留滚动尚未本地实跑成功，不能用静态检查替代。
4. **输入域边界**：原集合空/单条、长系统文字、多里程碑摘要、更早0/1、60/65会话及150KB预算通过。媒体查询仍限定1100×800，其他视口沿用原规则；断点两侧与目标1512×944的实际几何受环境阻塞。前导零、闰日等解析边界不适用，本轮没有解析逻辑。

## 快检与交付

未运行全量门禁 `sh scripts/ci-local.sh`，未修改内核、切分支、操作其他worktree、推送或启动下一会话。指定快检结果见下；最终以 Autome 提交后复跑为准。

已执行 `sh scripts/ci-quick.sh --since c9eaa3868338e0b2063d3d82d8da2d76f8949dd9`：21步，19成功、2失败，58秒；相关阶段37秒≤120秒。Format、Clippy、Web lint/typecheck/build通过。失败为 shell-live 开发SSR `listen EPERM` 和 Web e2e 准备/清理 `spawnSync ps EPERM`。日志 `.autome/output/m07-r13-quick.txt`。**未取得快检全通过或原几何断言绿证据**。

快检Vitest总数90，89通过、1环境失败、0跳过；较上一轮168个 **−78**。差来自依赖选择：本轮加入 task-timeline37；不再选择 task-summary21、live-routes15、task-detail-data12、task-detail-payload4、task-header7、task-page-effect4、task-page9、task-stage14、testids15、ui-base11、task-live-real3（合计115），37−115=−78。未删除测试；M-07专门验收仍覆盖task-page/testids。e2e35个定义均未执行，相对宿主35个少35个执行，非删除用例。重叠命令不累计为去重覆盖。`git diff --check`通过。
