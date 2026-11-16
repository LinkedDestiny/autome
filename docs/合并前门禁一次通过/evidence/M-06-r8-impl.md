# M-06 实现 #8

## 修改与结论

BL-01 的详情重验判据现在要求 `_routes` 非空且含 `routes/tasks.$id`，整页重验不再充当相关事件命中的证明。保留 B-3 对 active 后第一条 session 的独立轮询。新增 unit 组 TaskPage effect 三例，实际 SSR 渲染页面并截获、执行其 effect 与 cleanup，不以直接调用登记函数替代挂载证明。

会话可执行验收通过，提交待审。设计「验收契约」的 ★ 命令因沙箱拒绝交审计执行；尚无浏览器通过结论。上一轮审计 M-05 已通过，没有须逐字移植的退回检查。

## 命令与结果

- 改动前：`(cd apps/web && npx vitest run test/live-routes.test.ts test/test-groups.test.ts)`：18/18 通过（14 + 4），日志 `.autome/output/m06-baseline.txt`。
- 验收：`(cd apps/web && npx vitest run test/task-page-effect.test.ts test/live-routes.test.ts test/test-groups.test.ts)`：21/21 通过（3 + 14 + 4），日志 `.autome/output/m06-vitest.txt`；变异还原后同命令 21/21，日志 `.autome/output/m06-restored.txt`。总数相对本轮改动前同范围基线 18 增加 3，全部来自新组件生命周期测试。上一轮 M-05 是另一组命令，不能将其总数作为本组增减依据。
- ★ `npm run build --prefix apps/web && cargo build -p automed -p autome-cli && (cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3|BL-01' --repeat-each=5)`：两项构建成功；Playwright globalSetup 报 `spawnSync ps EPERM`，清理的进程扫描也遭同一拒绝。计划 10 条（B-3、BL-01 各 5），实际执行 0、未执行 10，数量相对设计基线 10 无增减，不是 10 条通过。日志 `.autome/output/m06-browser.txt`。
- 未跑项目快检或全量门禁。宿主在本轮结束后才跑快检，因此本轮无法引用未来日志。审计须补引用该宿主日志的 controls.spec.ts / B-3 结果，并执行 ★ 10 条及浏览器负向对照；不可用本轮组件测试代替。

## 自审清单（标待审之前执行）

1. **情形表逐行**：E-1 保留 B-3 合并前 queued、合并后 active、最终 sessions > 0 的分别断言；真实浏览器复验被 EPERM 阻止，交 ★ 审计。E-2 请求判据拒绝缺失与空 `_routes`，接受含目标路由的列表；真实请求是否被 Router 合并仍待 ★ 复验。组件三行：挂载登记、after 替换、卸载清除均实际执行并通过；test-groups 证明新文件恰属 unit。
2. **失败分支**：相关分支临时替换成 false，运行 `npx vitest run test/task-page-effect.test.ts` 得 3/3 失败（退出 1，`.autome/output/m06-negative-related.txt`）；临时去掉 TaskPage effect 的 cleanup 返回，运行同命令得 2 失败、1 通过（退出 1，`.autome/output/m06-negative-cleanup.txt`）。两处源码均完整还原，再运行验收 21/21 绿。本里程碑没有新增授权、读盘、超时或服务调用失败分支；浏览器初始化权限失败已如实记录。BL-01 浏览器恒假变异与 B-3 单读变异尚未执行，原因是原实现也不能通过环境初始化。
3. **全部迁移**：未登记→挂载登记；多前置→清除旧前置→登记新前置；单前置→空依赖；挂载→卸载；卸载→重新挂载→再次卸载，各由组件测试执行。自身事件仍匹配、前置 task/merge 匹配、前置 session 与无关任务不匹配；live-routes 既有 14 条覆盖事件批次、失败恢复、导航默认行为、任务隔离。
4. **输入域边界**：空依赖、单前置、多前置、空事件 subject、无关 subject 均通过；没有日期、数值上限、前导零输入域，不适用。组件测试同时检查真实 effect 的 `[taskId, after]` 依赖，after 变化确实改变依赖值。

## 宿主 / 审计交接

设计已允许 ★ 验收因沙箱拒绝交审计。请在审计证据补齐：本实现轮之后宿主 B-3 结果、10 条重复运行、相关分支恒假使 BL-01 红的判别证据。当前未将环境失败解释为产品通过，未关闭里程碑。

补充静态检查：仅对三个改动的 TypeScript 文件运行 `npx eslint test/task-page-effect.test.ts e2e/controls.spec.ts test-groups.ts --max-warnings 0`，退出 0（`.autome/output/m06-eslint.txt`）；`git diff --check` 通过。确认两处临时变异生产文件与 HEAD 无差异。
