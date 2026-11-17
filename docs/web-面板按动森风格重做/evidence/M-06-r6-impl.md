# M-06 · 实现轮 6

结论：页面实现与单元验收通过；浏览器验收及快检内核夹具受沙箱进程检查限制，保留「开放」，不标待审。implementation-round 为 6/65。上一轮审计 #5 通过，无退回缺陷，无需搬运红绿回归检查。

## 实现

- 头部集中状态胶囊、项目、分支、短合并提交和现有 TaskOps，保留全部写操作及禁用原因。
- 接入 M-05 的九步 / Bug 六步汇总、等待和计费信息、里程碑胶囊、六项数字块；单次失败门禁不显示「一次过」。
- 执行过程按阶段倒序分组，保留 SessionLog、稳定键、滚动保存和窗口参数；门禁绿色、用户黄色；筛选通过 withParams 保留 from/doc/tab。
- 四页签使用 SegmentedTabs 链接式样，文档胶囊、改动增删数、待决定计数保持原有数据路径。
- 设计提及的「重新打开」在现有协议中是备注表单的 reopen=yes，不是 WRITE_OPS 中独立操作；保留 await_merge 的该入口并测试，未扩写后端契约。

## 命令与结果

1. M-06 的 vitest 验收命令（设计所列 11 文件）通过：最终验收范围 115 用例。初始同范围为 107，本轮净增 8（task-page +3、task-header +4、task-timeline +1）。最初失败来自旧头部费用断言、中文标签集中约束及缺失颜色令牌，均已修复；未放宽标签或令牌检查。
2. 最终执行上述 11 文件加直接相关 `test/task-summary.test.ts`、`test/task-stage.test.ts`：13 文件、149 用例通过，0 失败、0 跳过。原始输出 `.autome/output/m06-unit-final.txt`。较上一轮 M-05 的 89 增加 60，但范围不同；本轮同范围可比增量是 8，另含原有汇总模型 20 用例及步骤模型 14 用例，不能将 60 当作新增测试数。
3. `(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts -g "详情|W7 H-03|W7 H-04|C-2")`：globalSetup 的 `spawnSync ps EPERM` 阻止夹具启动，6 条目标用例未执行，0 条获得通过证据。输出 `.autome/output/m06-browser.txt`。截图检查已加入 layout，用例未启动所以截图未生成，未宣称一屏或视觉验收通过。
4. 指定快检：`sh scripts/ci-quick.sh --since 304c20d575f2c7393a2498c228bf2dee28092fc5`。最终输出 `.autome/output/m06-quick-final.txt`；结果见下方快检补充。没有运行 ci-local 全量门禁。
5. `git diff --check` 通过。

## 标待审之前的自审清单

1. **情形表逐行**：D-1 头部合并信息、全部可用操作、需求九步与 Bug 六步、等待类名、里程碑保留由 task-header/task-page 检查；D-2 六数与空会话由 task-page 及 task-summary 检查；D-3 阶段倒序、门禁步数、reused/skipped、全部用户操作和筛选由 task-timeline 检查；D-4 四页签由 task-page、docs-view、changes、decisions、notes 检查。均通过。展开日志、真实滚动保持与一屏：浏览器启动阻塞，未验收。
2. **失败分支**：task-page 的离线、503/504、404/not_found、500；docs-view 的读文档错误与缺失；ops-view 的离线禁用；task-summary 的截断门禁缺失；新增单次门禁失败文案检查均通过。未引入新的外部服务或写授权路径，原有失败信息保留。浏览器夹具不可用不能视作页面成功。
3. **全部迁移**：页面无新增业务状态机；头部操作覆盖 draft、待批准、not_a_bug、paused、failed，原有操作测试覆盖取消确认；timeline 保留十类用户操作及 task.decided。三个筛选状态各自渲染全部三种目标链接（3×3），检查 from/doc/tab 不丢失；四页签数据和备注送回入口通过。实际浏览器点击 / SSE 更新下日志与滚动迁移未跑，列为阻塞。
4. **域边界**：空会话、零费用、全部无计费、缺失门禁、单次失败门禁、短哈希截断、from/doc 参数共存由上述测试通过；文本 maxLength 仍来自网关，ops-view 检查通过。本轮未新增日期或数字输入域，前导零、闰日、跨年、最大值输入不适用，沿用现有表单契约。

## 下一轮

在允许 ps/浏览器夹具的宿主环境执行 M-06 原验收命令，查看截图并确认六条浏览器用例；解决快检环境失败后，取得证据再标待审。不得将环境阻塞当作协议失败，也不增加 reopen。

## 快检补充与直接相关回归

- 快检暴露的历史里程碑回退分支 title 类型问题已补齐，`npm run typecheck --prefix apps/web` 复跑退出 0（`.autome/output/m06-typecheck-final.txt`）；lint 已通过。
- 快检中的 `task-timeline-real` 旧断言要求正序并直接匹配 `<h3>角色`，与 D-3 的倒序及胶囊结构冲突；保留完整角色序列断言，按新契约改为倒序胶囊序列。复跑 `npx vitest run test/task-timeline-real.test.ts`：总数 3，2 条真实页面行为通过，1 条收尾进程检查因 `spawnSync ps EPERM` 失败，afterAll 同因失败（`.autome/output/m06-timeline-real.txt`）。这不是上一轮审计退回，不涉及审计检查迁移。
- 独立命令去重计数：149 条单元/模型 + 3 条真实内核 = 152 条，151 通过、1 失败；另有 6 条浏览器用例未启动。不将快检重复执行的用例累加成新增覆盖。
- 指定快检最终退出 1：执行 40 步、失败 7 步，另有 3 步超出 120 秒准入预算未跑。包含已通过直接复跑修正的 title 类型问题、ps EPERM 的内核清理失败，以及 `load-context.test.ts` 的旧总览契约断言（预期 3 次读取实际 4 次、预期无 overview 字段、预期 session 不触发重验）。后者涉及本轮未修改的 root/总览逻辑，未擅自扩展到另一里程碑，也未把所有失败归为沙箱问题。快检执行期间之后的两处修正分别以 typecheck 和 task-timeline-real 直接复跑取证，未宣称完整快检变绿。
