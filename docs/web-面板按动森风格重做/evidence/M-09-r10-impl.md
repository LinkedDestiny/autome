# M-09 · 实现 #10 · 宿主快检修复

本轮只修复宿主报告的 F-1，不推进新里程碑。M-09 原为开放，本轮保持开放；implementation-round 为 10/65。未运行全量门禁，未启动下一会话。

## 根因与修复

宿主原文定位为 controls.spec.ts:257 的 getByLabel('项目', { exact: true }).selectOption('') 命中了 nav.shell-sidebar，报 Element is not a <select> element。技能页的隐式 label 包含下拉选项文本，不能用精确“项目”标签定位；侧栏恰好有同名 aria-label。改为 getByRole('combobox', { name: /^项目/ })，限定真实表单角色并匹配标签前缀。没有更改产品、业务断言、用例标题、超时或清理机制。

检索 apps/web/e2e 中同类 getByLabel('项目' 调用仅此一处。上一轮审计是 M-08 通过，无新造的退回检查；本次失败检查已在项目 controls.spec.ts 的 F-1 中，原地修复并保留整个业务流程，无需复制另一条相同测试。

## 命令与证据

- 修复前原样执行：(cd apps/web && npx playwright test e2e/controls.spec.ts -g 'F-1')。日志 .autome/output/r10-red.log。globalSetup 报 spawnSync ps EPERM，实际执行 0/1，未在本机重现选择器红灯；选择器失败证据来自用户提供的宿主原文。
- 修复后执行：(cd apps/web && npx vitest run test/skills-model.test.ts test/skills-page.test.ts test/skills-real.test.ts test/open-path.test.ts test/write-gate.test.ts test/gateway.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts)。9 文件、145 用例全通过，日志 .autome/output/r10-accept.log。
- 修复后执行：(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g '技能|F-1')。退出 1，仍被 ps EPERM 阻断，实际执行 0/2，不能记录为绿。日志 .autome/output/r10-green-attempt.log。
- 指定快检：sh scripts/ci-quick.sh --since 0e2e915d5537735800db72fc4ec0643b4ddceb17。退出 1，执行 12 步、沿用 0、失败 1，用时 31 秒；唯一失败 Web e2e，原因 ps EPERM。日志 .autome/output/r10-quick.log。vitest related 没有匹配文件，本轮已显式补跑上述 145 用例。最终快检结论以 Autome 提交后复跑为准。
- git diff --check 通过。

用例总数对照：本轮 M-09 固定验收范围仍为 145 Vitest + 2 浏览器 = 147，与 r9 同范围差 0；没有新增、删除或跳过业务用例。本轮实际通过 145，浏览器 2 条准备阶段阻断。r9 额外守卫 40 条本轮未重复执行，故与其 185 个实际通过数相比为 -40，来自缩小到本轮直接相关范围，非用例删除。修复前单跑 F-1 不重复计数。

## 四项自审

1. **情形表逐行**：F-1 四种计数与搜索由 skills-model 通过，无项目选项、卡片、空态由 skills-page 通过；F-Q2 项目隔离、全局继承、仅模型覆盖与显式技能覆盖、取消后读回名单由 skills-real 通过；F-Q3 纯全局/纯项目/同名混合 × 两绑定范围由 skills-model 和 skills-real 通过；F-Q4 服务端解析来源及打开器由 open-path 通过。实际项目切换及截图被浏览器启动阻断。F-Q1 描述不适用，归 M-12。
2. **失败分支**：本次重跑覆盖失去授权禁用、打开器拒绝、配置读盘失败、超时 504、空名称、非法角色、无项目范围、无网关、不可见技能内核拒绝及部分保存停止，全部通过。外部服务不可达不适用，本功能无第三方服务。浏览器失败不是产品拒绝分支，不用模型通过代替浏览器结果。
3. **全部迁移**：全局/项目未绑定→绑定→取消、项目继承→显式覆盖、全局更改后项目保留覆盖、部分成功→拒绝停止，真实内核和模型通过。页面抽屉开关、范围往返、返回全局仍保留在 F-1；本机阻断，等待宿主复验，未声称通过。
4. **输入域边界**：空清单、空搜索、无匹配、大小写和首尾空白、单元素取消为空列表；来源 0/1、负数、小数、字符串、超安全整数、最大安全整数越界；路径注入、未知名和空名均由本次测试通过。日期边界不适用，无日期输入。

## 交接

宿主需复跑相同快检与两条 M-09 浏览器验收，确认 F-1 返回全局并隐藏本项目筛选，核对截图后再推进状态。本轮不规避进程扫描限制，不把环境阻断称为协议失败。
