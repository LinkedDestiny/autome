# M-01 实现轮 #1

结论：M-01 定向验收通过，提交独立审计；全量门禁受本会话环境限制，未声称全绿。上一轮无审计退回，因此无须移植审计回归检查。

## 实现与范围

- `app/load-context.ts` 用 `Symbol.for('autome.web.services')` 注册唯一键；每请求新建 RouterContextProvider，不提供默认值。KernelGateway 只暴露 read/write，注释保留 W6 增加服务的位置。
- http.ts 的开发、生产 handler 都注入 gateway（缺失时 null），未改变 API、安全或升级路由。
- 根 loader 仅经 read 读取 project.list，项目仅输出 id/name，网关错误原样返回；根页面临时列项目名，事件重验覆盖 task/project/bug/merge。
- 既有 bell-view 的根组件测试增加根 id、loader 和 hydrationData，因为组件现在使用 loader 数据；原断言保留，用例数不变。
- 本机 @react-router/express 类型签名与设计引用一致；实际开发/生产 SSR 均通过。watch:null 可用，无须回退。

## 命令与结果

原始日志均在 `.autome/output/w5-m01-*`，未复制进任务文档。

| 命令 | 退出码 | 结果与日志 |
|---|---|---|
| `cargo build -p automed -p autome-cli` | 0 | 补齐缺失二进制；cargo.txt |
| `npm test --prefix apps/web -- test/load-context.test.ts test/bell-view.test.ts test/live-routes.test.ts` | 0 | 最终 24/24，0 skipped；target-final.txt |
| `npm run lint --prefix apps/web` | 0 | lint.txt |
| `npm run typecheck --prefix apps/web` | 0 | types-final.txt |
| `npm run build --prefix apps/web` | 0 | build.txt；负向对照还原后重建 |
| `npm test --prefix apps/web` | 1 | 最终 310：280 passed、11 failed、19 skipped；all-final.txt |
| `npm test --prefix apps/web -- --exclude test/server.test.ts --exclude test/gateway-http.test.ts --exclude test/eventbus-live.test.ts --exclude test/notifier-live.test.ts --reporter=json --outputFile=../../.autome/output/w5-m01-runnable.json` | 0 | 其余 30 文件、280/280，0 skipped；runnable.json。此命令仅隔离环境阻塞，不替代全量门禁 |
| `npm test --prefix apps/desktop` | 0 | 总数 104，103 passed、1 skipped；desktop.txt。Electron 二进制缺失导致原有 DOM harness skip，未修改或移除测试 |
| `git diff --check` | 0 | 无空白错误 |

**总数差异：**设计基线 287 来自 `vitest list` 静态枚举。本轮同口径 296，增加 9 条，全部来自 load-context。运行时展开后本轮实际为 310；与设计预期 296 的额外 14 来自原有 node-version（静态 4 → 实跑 12，+8）、notifier-rules（静态 6 → 实跑 12，+6）。原测试源码相对 a3455a3 无变化（仅 bell-view 接线修改，不增删用例），故同口径运行基线为 301，本轮 +9。Desktop 总数 104，相对基线 +0。定向总数原 15，本轮 24，+9。

**环境阻塞名单：**gateway-http.test.ts 全部 11 条因 `listen EPERM 127.0.0.1` 失败；server.test.ts 全部 9 条、eventbus-live.test.ts 全部 7 条、notifier-live.test.ts 全部 3 条因 beforeAll 无法监听未执行（19 skipped）。前两者还伴随 watcher EMFILE。精确名称见 all-final.txt。需审计环境复跑原始全量命令，不得把排除版当作全绿。第一次全量还因二进制 ENOENT 失败，补构建后重新执行；最终结果只引用 all-final.txt。

## 标待审之前的自审清单

1. **情形表逐行**：M-01 的九条契约全部执行：独立 provider/services/null；双查询串同键；read 唯一调用；无网关离线；503/504/409 原文；Vite SSR；生产 SSR；两处分支 AST；六类事件重验。24 条定向测试全绿。两种 SSR 都返回 200、含「演示项目甲」、恰一次 project.list read。未设默认键的空 provider 抛错也已断言。
2. **失败分支**：无 gateway、503 外部内核不可达、504 超时、409 读失败均检查 kernel.ok=false、原文 error、空数据。授权拒绝不适用：M-01 不新增授权规则，现有网关白名单测试在可运行全量中通过。磁盘读取失败不适用：此 loader 不直接读盘，网关失败路径统一处理。空项目输入在第 3 条用例实跑。
3. **全部迁移**：本轮不引入状态机；通道覆盖开发/生产 SSR 注入→loader、每请求新 provider、空 provider 失败；事件 task/project/bug/merge 各触发一次，config/session 各不触发。action 暂无实际页面消费者，与 loader 共用已验证的 RouterContextProvider 与 WebServices 类型，不新增产品写操作。
4. **域边界**：gateway 有值/null、项目列表空/单元素、provider 缺值均实跑。日期、前导零、最大数值不适用：M-01 无该类输入域，也不处理任务编号或日期。

## 负向对照与还原

临时脚本 `.autome/output/w5-m01-negative.mjs` 使用 try/finally 原样恢复源文件，每组日志独立保存。

- 去除共享注册、改为模块直接 createContext：测试退出 1，恰第 2、6、7 条失败（3 failed/6 passed），证明跨模块身份确被检查。
- 删生产 getLoadContext：HTTP AST 测试退出 1，报告属性缺失。
- loader 改为 gateway.core.call：typecheck 退出 2，TS2339 明确 KernelGateway 无 core；第 3 条 loader 测试退出 1。
- 还原后定向 24/24、typecheck/lint/build 退出 0；可运行全量 280/280。没有残留负向改动。

## 提交

本轮只推进 M-01，M-02 至 M-05 保持开放。最终提交结果见本轮会话；若提交权限拒绝，提交由 Autome 兜底。
