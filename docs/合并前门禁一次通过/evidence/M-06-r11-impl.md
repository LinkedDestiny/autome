# M-06 实现 #11（快检修复轮）

结论：保持开放，不推进新里程碑，不增加 reopen。implementation-round 为 11/30。本轮修复类型检查与依赖显示的数据来源冲突；浏览器验收仍需宿主取得，不能标待审或已完成。

## 原因与修复

- 宿主 62e609282a 快检失败项为 Web typecheck、BL-01。变异脚本的 command、args、error 缺 JSDoc 类型；已补全，不屏蔽检查。
- BL-01 冻结根 loader 后，TaskDependencies 仍优先取根任务状态，遮住详情 loader 更新的 blocked_by。现在优先取详情依赖状态，缺失时保留根状态后备，两者均缺失显示状态未知。未放宽五秒断言、未移除根隔离或事件过滤。
- 新增组件回归到已有 task-page-effect.test.ts，真实渲染组件并提供冲突的根/详情 loader 数据，同时检查后备与未知状态。审计原样恒假检查仍在 scripts/test-related-mutation.mjs，未削弱判据。

## 红绿证据与数量

- `npm run typecheck --prefix apps/web`：修复前 3 个 TS7006，修复后退出 0；日志 `.autome/output/m06-r11-type-red.log`、`m06-r11-type-green.log`。静态检查不计测试用例。
- `(cd apps/web && npx vitest run test/task-page-effect.test.ts)`：旧优先级下 4 条中 1 红、3 绿，红为 T-9 仍显示「等你批准设计」；日志 `.autome/output/m06-r11-component-red.log`。
- `(cd apps/web && npx vitest run test/task-page-effect.test.ts test/live-routes.test.ts test/test-groups.test.ts)`：22/22 通过，上一轮 21，差 **+1**，来源为冲突数据与后备组件回归；无删减或跳过。日志 `.autome/output/m06-r11-unit.log`。
- `(cd apps/web && npx vitest run test/queue-deps.test.ts)`：8/8 通过，已有用例未增减，作为直接受影响组件补充检查。与上述合计 30 个唯一用例；相比上一轮所跑的 21 多 9（新增 1，扩大检查范围 8）。日志 `.autome/output/m06-r11-deps.log`。
- 修复前原样 `(cd apps/web && npx playwright test e2e/controls.spec.ts -g BL-01)`：计划 1，实际 0，setup `spawnSync ps EPERM`。日志 `.autome/output/m06-r11-browser-red.log`。未取得浏览器红，宿主失败原文是已知红证据。
- 修复后 `node apps/web/scripts/test-related-mutation.mjs`：构建成功，但首次浏览器准备即 ps EPERM，检查拒绝把 setup failure 当变异证据；finally 已恢复源码并重建。计划五次，实际 0；相对上一轮计划差 0。日志 `.autome/output/m06-r11-mutation.log`。不能声称恒假五次对照通过。
- 用户末段指定的原样快检 `if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`：退出 0，5 步通过，27 秒；未提供起点，相关测试未跑，测试用例数 0，不能与宿主六条 controls 用例通过等同。日志 `.autome/output/m06-r11-quick.log`。

## 四项自审清单

1. **情形表逐行**：E-1 B-3 的 queued→active 与会话轮询未改，宿主上一轮报告 B-3 通过，本轮未取得新浏览器结论；E-2 组件挂载、变更、卸载三项通过，新增冲突状态回归红→绿。真实 BL-01 正向/恒假对照因准备失败仍待宿主复验。
2. **失败分支**：类型检查原样红→绿；旧根状态遮蔽详情新状态的组件检查红→绿；缺详情状态、全部状态缺失的后备检查通过。变异脚本拒绝 setup 失败已实际触发。授权、读盘与外部服务异常不适用，本轮不改这些分支；五秒超时保留，未获得浏览器通过证据。
3. **全部迁移**：挂载登记→换依赖清旧登记新→空依赖→卸载与重挂载在原有组件用例通过；旧根批准态与详情实现态冲突由新增回归通过。浏览器批准→实现和合并→启动未在本环境实跑。
4. **域边界**：已有用例覆盖空 after、多前置、空 subject、无关 subject；新增用例覆盖详情有值/详情缺失/两处均缺失。日期与数值边界不适用，本轮不引入这些输入域。

## 带起点快检补验

`sh scripts/ci-quick.sh --since ad9c7be0`：退出 1，22 步中 2 步失败，63 秒；相关阶段 42 秒。日志 `.autome/output/m06-r11-quick-related.log`。相关 Vitest 总计 85 条，82 通过，task-live-real 的 3 条因 beforeAll 监听 127.0.0.1 EPERM 未执行（报告显示 skipped，未增加跳过条件）；该套清理另报 ps EPERM。Web e2e 清单总数 58，实际 0；同样在 globalSetup 因 ps EPERM 全部未执行。相比上一轮宿主 controls 的 6 条，计划范围 +52，原因是本轮修改产品组件触发整套回退，并非新增 52 条用例。此次改产品组件使相关测试范围扩大，与上一轮只跑三文件 21 条基线不可直接比较：M-06 固定子集仍是 +1；快检选择的额外用例不是新增测试。未跑 ci-local，全量门禁留给宿主；原样无起点快检通过不能替代本次失败或提交后的宿主结果。
