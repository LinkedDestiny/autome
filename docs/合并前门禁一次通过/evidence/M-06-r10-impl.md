# M-06 实现 #10

结论：保持开放，implementation-round: 10/30。不增加 reopen；浏览器证据受环境阻断，不是新的实现缺陷或协议失败。不标待审，不关闭里程碑。

## 修复

审计 #6 证明相关分支恒假时 BL-01 仍过：根路由匹配 task.*，single-fetch 替代刷新掩盖详情 matcher 失效。本轮在 BL-01 拦截根路由浏览器模块，仅替换 shouldRevalidate 导出为恒假；校验恰好一个导出且拦截实际发生。真实详情 matcher、effect、loader、前置事件及五秒断言保留。没有改产品源码，没有放宽超时。

审计的检查（把 `relatedSubjects.get(...)` 那行换成 `&& false`，重建后跑 BL-01）已原样固化到 `apps/web/scripts/test-related-mutation.mjs`，五次独立调用，必须是文案断言失败才认可，准备失败、截断或其它错误均拒绝。finally 恢复原始源码并重建。

## 命令与结果、基线

- 修改前原样注入 `&& false`，`npm run build --prefix apps/web` 后 `(cd apps/web && npx playwright test e2e/controls.spec.ts -g 'BL-01')`：构建通过，准备阶段 `spawnSync ps EPERM`；1 个计划、0 执行。日志 `.autome/output/m06-r10-original-negative.log`。未取得红证据；原始缺陷的两次错误通过见 M-06-r9-audit.md。源码已恢复。
- `(cd apps/web && npx vitest run test/task-page-effect.test.ts test/live-routes.test.ts test/test-groups.test.ts)`：3 文件、21 用例全部通过。相对 #9 的 21 条差 **0**，未删减、未跳过。日志 `.autome/output/m06-r10-unit.log`。
- `npm run build --prefix apps/web && cargo build -p automed -p autome-cli && for i in 1 2 3 4 5; do (cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3|BL-01') || exit 1; done`：两项构建通过，第一次浏览器调用准备阶段 ps EPERM 后退出；总计划 10（与上一轮计划差 **0**），实际执行 0，第一次列出 2 未执行，其余四次未启动。日志 `.autome/output/m06-r10-browser.log`。
- `node apps/web/scripts/test-related-mutation.mjs`：退出 1；准备阶段 ps EPERM，脚本在 `setup failure or truncation is not mutation evidence` 断言拒绝，未取得修复后绿证据。计划 5 次变异对照（将既有人工检查持久化，没有新增浏览器 case），执行 0。finally 恢复源码及生产构建通过。日志 `.autome/output/m06-r10-mutation.log`。
- `(cd apps/web && npx eslint e2e/controls.spec.ts scripts/test-related-mutation.mjs --max-warnings 0)` 与 `git diff --check` 通过；不属于用例数。未跑 ci-local、ci-quick 或全量测试。
- 本轮之后的宿主快检尚未发生，不能引用未来日志。审计 #6 引用的 f2eac1503fef 宿主结果为 B-3/BL-01 各一次通过，只是旧基线，不能证明本轮。

## 四项自审清单

1. **情形表逐行**：E-1 保留 B-3 前置合并前 queued、合并后 active、sessions 大于零轮询；本轮浏览器受阻未复验。E-2 相关分支隔离已实现，正负浏览器未取证；组件挂载登记、after 变化清旧登记新、卸载清理全部单测通过。五次独立调用保留，未用 repeat-each。
2. **失败分支**：真实发生准备/清理 ps EPERM，变异脚本明确拒绝将其当绿；相关分支恒假浏览器验证仍阻断。无清理函数和 B-3 单次读会话数的变异本轮未跑，不能声明这些对照通过。拒绝授权、外部服务不可达不适用：本轮只修改测试隔离，不新增此类产品分支。
3. **全部迁移**：组件未登记→挂载→旧依赖清除→新依赖登记→空依赖→卸载→重新挂载均由 3 条 effect 测试覆盖通过；自身事件、前置事件、无关事件拒绝由同组测试通过。浏览器前置批准→实现、子任务保持 queued、合并→子任务启动仍需真实浏览器证据。
4. **域边界**：单前置、多个前置、空 after、空 subject、无关 subject、前置不支持的 session 类型均由 effect 测试通过。根模块导出数量须为 1、拦截须发生，静态复核有失败断言，真实执行未取证；日期/数字边界不适用。

## 下一步

在具备进程表与浏览器权限的宿主/审计环境运行设计中的正向五次及持久化恒假五次对照；若仍失败，继续修正隔离，不能根据本轮单测结果关闭 M-06。本轮未证实浏览器修复有效。
