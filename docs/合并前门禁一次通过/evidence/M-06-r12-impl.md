# M-06 实现 #12

结论：B-3 合并后读取顺序已修复，确定性回归红→绿。浏览器准备被 `spawnSync ps EPERM` 阻塞，尚未满足五次独立复验与变异验收，保持开放，不增加 reopen，不标待审或已完成。implementation-round：12/30。

## 修复与回归归档

- `controls.spec.ts` 合并后先读子任务再读前置，断言「子任务 active ⇒ 随后读到前置 done」。前置 done 单调，正常合并发生在两次读取之间不会误红；未改等待会话记录与 UI 的断言、超时或跳过规则。
- 同类范围：检查 B-3 全部状态读取。approve 后到 await_merge 的读取没有并发 merge（merge 命令在该轮询之后），不可能发生本次 done 窗口，保持原断言。
- 审计的原样检查 `for i in 1 2 3 4 5; do (cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3|BL-01') || exit 1; done` 逐字收进 `apps/web/scripts/test-dependency-repeat.sh`（仓库根执行）。修改前原样执行，修改后通过该脚本执行；两次均首轮 setup 失败，不能冒称浏览器红→绿。
- 审计要求的两次读取之间插入合并由 `test/dependency-poll.test.ts` 确定性实现：直接提取并执行真实 B-3 轮询体，第一次快照返回后推进前置 done、子任务 active；不是复制一份修正逻辑。另测三种合法状态组合及真实提前启动拒绝。加入 unit 分组，常规项目测试可发现它。

## 命令、结果与用例数量

1. 修复前原样五次命令：退出 1，计划 10 次用例执行，首轮 B-3、BL-01 均未执行（0），其后四轮未启动。日志 `.autome/output/m06-r12-before.log`。原因 `spawnSync ps EPERM`；原审计红见 `M-06-r11-audit.md`，本轮未复现浏览器断言红。
2. `(cd apps/web && npx vitest run test/dependency-poll.test.ts)`：修改前总数 5，4 通过、1 失败，失败为两次读取间合并误红（expected queued, received active）。日志 `.autome/output/m06-r12-race-red.log`。
3. `(cd apps/web && npx vitest run test/task-page-effect.test.ts test/live-routes.test.ts test/test-groups.test.ts test/dependency-poll.test.ts)`：修改后总数 **27，全部通过**。相比上一轮固定三文件基线 22，差 **+5**，全部来自新增时序回归；原 22 条未删减，无跳过。日志 `.autome/output/m06-r12-unit.log`。上一实现轮另跑 queue-deps 的 8 条，本轮未改产品组件，未重复该额外子集；若比较其 30 条合计，本轮净差 −3 = 新增 5 − 未重复 8。
4. `npm run build --prefix apps/web && cargo build -p automed -p autome-cli`：退出 0，日志 `.autome/output/m06-r12-build.log`。随后 `sh apps/web/scripts/test-dependency-repeat.sh`：退出 1，仍首轮 setup 的 ps EPERM，计划 10、实际执行 0，计划相对审计基线差 0；未取得审计所需 10 条全绿。日志 `.autome/output/m06-r12-after.log`。
5. `node apps/web/scripts/test-related-mutation.mjs`：退出 1，首次 setup 被 ps EPERM 阻塞，脚本以 `setup failure or truncation is not mutation evidence` 拒绝误判。计划五次 BL-01 变异，实际执行 0，计划差 0；finally 已恢复源码并成功重建，git diff 确认产品源码无变异残留。日志 `.autome/output/m06-r12-mutation.log`。不能声称恒假 5/5 红。
6. `(cd apps/web && npx eslint e2e/controls.spec.ts test/dependency-poll.test.ts test-groups.ts --max-warnings 0 && npx tsc --noEmit)`：退出 0，日志 `.autome/output/m06-r12-static.log`。构建、静态检查不计测试用例。

## 四项自审清单

1. **情形表逐行**：E-1 合并前 queued、合并后 active 与会话/UI 等待保留；新增回归证明跨读取合并不误红、提前 active 仍拒绝。真实 B-3 五次和本轮宿主 controls 快检结果未取得。E-2 原挂载登记、after 变化清旧登记新、卸载清除、详情状态优先与回退四个组件用例全部通过；真实 BL-01 与恒假五次仍被准备权限阻塞。
2. **失败分支**：旧顺序在跨读取合并时实际红；子任务提前 active / 前置仍 active 的非法状态实际被拒绝。变异脚本拒绝把 setup 失败计为有效变异实际触发。五秒超时断言未放宽，浏览器未运行；授权拒绝、业务读盘失败、外部服务不可达不适用，本轮不改这些业务分支。
3. **全部迁移**：确定性回归运行 queued/active、queued/done、active/done 三个合法快照及 queued→active 与前置 active→done 跨读取提交；active/active 非法组合拒绝。组件挂载→变更→空依赖→卸载由原子集覆盖通过。真实合并→启动→首会话及批准→实现尚待浏览器验收，不以替身检查替代。
4. **域边界**：无新业务输入域。原 live-routes 与组件用例中的空 after、多前置、空/无关 subject、状态缺失回退通过；回归增加两次快照之间的提交边界及子任务尚 queued 而前置已 done。日期、前导零、最大数值不适用，本轮没有对应输入。

## 待宿主 / 审计补验

先执行上述构建与 `sh apps/web/scripts/test-dependency-repeat.sh`（B-3、BL-01 各五次），再执行 `node apps/web/scripts/test-related-mutation.mjs`。引用本轮结束后宿主运行整个 controls 文件的快检日志及 B-3 结果；该未来日志本轮无法预先取得。未运行 ci-quick 或 ci-local，均留给宿主。环境限制不是协议失败，不为取证放宽沙箱。
