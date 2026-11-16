# M-06 实现 #9：宿主快检修复

## 失败与修复

本轮只修复 M-06，保持待审，不推进新里程碑。上轮审计是 M-05 通过，没有审计退回的临时检查要移植；本次两个失败检查本来就在项目测试体系中，保留并修复。

1. 宿主 e656a95b95 的 controls 六例为 5 通过、1 失败；B-3 通过（19969ms）。BL-01 页面已显示“实现”，失败在第 156 行 `detailRevalidated` 仍为 false。读取宿主 Playwright report 与 trace，实际请求为 `/tasks/T-68.data?tab=notes`，没有 `_routes`（提取保存在 `.autome/output/r9-network.txt`）。React Router 已安装源码 `dist/production/lib/dom/ssr/single-fetch.js` 的 `targetRoutes` 只有发现 loader opt-out 才生成；root 同样匹配 task.*，两路都匹配时省略参数是合法行为，原判据错误。
2. BL-01 改用浏览器 WebSocket 转发夹具：仅转发真实前置事件、hello 和 connected，隔离 reload/env.changed/子任务事件的替代刷新。先等页面写操作可用（连接就绪），再批准前置；保留五秒内 UI 变化、子任务 queued、页面加载一次，额外要求收到前置事件。没有扩大时限。相关分支恒假时 root loader 更新不能替代详情 loader；该负向浏览器验证仍须由可启动浏览器的审计执行。
3. 同命令快检复现第二个失败：`test/task-page-effect.test.ts:24 TS2352`，loader 参数缺 `url`、`pattern`。按现有测试夹具填齐这两个字段，移除 `as LoaderFunctionArgs`，让编译器直接验证参数契约。

## 命令、结果与基线

- 原样回放已有 BL-01：`(cd apps/web && npx playwright test e2e/controls.spec.ts -g 'BL-01')`，日志 `.autome/output/r9-red.txt`。globalSetup 的 `spawnSync ps EPERM` 阻止执行，计划 1、执行 0；不能作为本机会话复现断言红的证据，断言红来自上述宿主报告。
- 按用户最后要求运行完整同条快检条件命令（`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`），首次退出 1，5 步中 typecheck 失败，日志 `.autome/output/r9-quick.txt`；修复后相同命令退出 0，5 步全部通过、0 沿用、16 秒，结果见 `.autome/output/r9-quick-green.txt`。未指定起点时该脚本不执行相关测试或浏览器阶段，不能把静态快检绿解释为浏览器绿。
- `(cd apps/web && npx vitest run test/task-page-effect.test.ts test/live-routes.test.ts test/test-groups.test.ts)`：最终 21/21 通过，日志 `.autome/output/r9-vitest-final.txt`。相对实现 #8 同范围 21 条，差 **0**；仅修正既有夹具，没有删增用例。
- `(cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3|BL-01' --repeat-each=5)`：计划 10，执行 0、未执行 10，仍是 `ps EPERM`，日志 `.autome/output/r9-browser.txt`；相对 #8 计划 10 条差 **0**。没有声称红绿浏览器闭环已完成。宿主 controls 基线为 6 条，本轮仍 6 条，差 **0**。
- 没有运行全量门禁 `sh scripts/ci-local.sh`。`git diff --check` 通过。

## 自审清单

1. **情形表逐行**：E-1 未改 B-3 的 queued→done→active→session 独立轮询；引用上轮宿主 B-3 通过，新的五次复验受 EPERM 阻止。E-2 正常根与详情同时匹配不再因 `_routes` 省略而误拒绝；替代刷新在夹具转发入口隔离，等待真实事件及可见文案。组件挂载登记、替换前置、卸载清除三例和分组四例通过。
2. **失败分支**：类型缺字段由原快检先红后修；浏览器原检查尝试回放，环境初始化失败已记录。相关分支恒假浏览器负向对照尚未执行，交审计，不能以组件测试替代。授权拒绝、磁盘读取失败、外部服务不可达不适用：本轮没有变更这些生产分支。
3. **全部迁移**：21 条测试覆盖挂载登记、旧前置清除→新前置登记、单前置→空、卸载→重新挂载；live-routes 覆盖事件批次、导航与失败恢复。浏览器连接就绪→批准前置→前置事件→依赖文案更新仍需宿主实跑；未将静态分析记成通过。
4. **输入域边界**：21 条包含空依赖、单/多前置、空 subject、无关 subject；loader 使用完整 URL 和路由 pattern，通过类型校验。日期/数值最大值不适用，本轮没有该输入域。消息转发只针对服务器协议消息，无新增生产输入解析。

## 交接

保留 M-06 待审，implementation-round 为 9/30。宿主提交后快检为最终判据；审计仍需执行十条重复浏览器验收及相关分支恒假五条全红的负向对照。无新里程碑关闭。
