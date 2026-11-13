# M-05 r6 审计证据

验收：`cd apps/web && npx vitest run test/board-live.test.ts test/board-live-listen.test.ts test/live-routes.test.ts` → 3 文件 16 用例通过、0 跳过（7+2+7，与预期一致）。本环境可监听，listen 两例实跑；stop→ws 事件 4253 ms（上限 4500，余量不大，但是契约值）。

负向对照（逐个注入、跑 board-live，随后 git checkout 还原）：
- matcher 仅 `byKind('task.')` → (1) 失败
- queued 卡也给开始按钮 → (2) 失败
- 去掉 `notification-read` → (7) 失败
- 页面不挂载 hook → (7) 失败
- `shouldRevalidate` 恒 false → (1)(6)(7) 失败
- eventbus 不向客户端推送实时事件 → board-live-listen (1) 超时 4500 ms 失败

新造检查：无额外；以上注入即判别检查。结论：通过，无 reopen。实现轮「监听 EPERM」的待办已在可监听环境闭合。
