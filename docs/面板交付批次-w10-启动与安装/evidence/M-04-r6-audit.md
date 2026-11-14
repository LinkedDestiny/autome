# M-04 r6 审计复验

结论：通过。

## 验收命令
`cd apps/web && npx vitest run test/board-payload.test.ts test/live-routes.test.ts test/board-live-timing.test.ts test/board.test.ts`
→ 4 个文件，28 条通过，skipped 0（5+11+4+8，与契约一致）。
相关子集 task-card / board-live / board-live-timing：3 个文件 23 条通过。

## 负向对照（逐个注入、跑 board-payload + live-routes、`git checkout app` 还原，最终 `git status` 干净）
| 注入 | 结果 |
|---|---|
| 去掉 `toCardDetail` 投影 | board-payload 第 1、2、4 条失败 |
| 去掉已完成列 `.slice(0, 20)` | board-payload 第 3 条失败 |
| 重验窗口 1000→0（立即重验） | live-routes 两条窗口用例失败 |
| 窗口内 `queued.push` 改为覆盖 | live-routes「不同前缀取并集」失败 |
四条契约对照全部能被拦住。

## 自造判别检查
对照任务 B-1 要求（只返回卡片所用字段、已完成默认最近若干张、100 任务 ≤200 KB、同一秒合并重验）逐项核对 4d0b5e3a 的源码差异：loader 字段白名单、`needsTaskDetail` 排除 done、待开始卡不渲染门禁行、折叠链接显示全部已完成数，均与设计一致。未发现产品行为偏差。

宿主快检（fmt/clippy/lint/typecheck）已在该提交通过；全量门禁由宿主合并前运行。
