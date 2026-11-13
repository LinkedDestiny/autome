# M-01 · 审计复验（实现轮 r2）

结论：通过。未读实现轮证据，独立复验。

## 验收命令
`cd apps/web && npx vitest run test/task-events.test.ts test/task-docs.test.ts test/task-detail-data.test.ts test/task-detail-real.test.ts test/task-page.test.ts`
→ 5 个文件、22 个用例全过（61 秒）。本宿主 `ps` 可用（无 EPERM），真实内核 4 条含收尾用例通过；跑完后 `ps` 无命令行含 `w7-` 的残留。

## 负向对照（均已注入后还原，`git status` 干净）
- A `forTask` 不按 subject 过滤 → task-events 3 条失败（首次分页、账本重建、噪声/5000 边界）。
- B 忽略 `latest_seq < 游标` → task-events「账本被换后清空重建」失败。
- C `teardown` 不杀进程、不检查残留 → task-detail-real 第 4 条失败（应 reject 却 resolve）；该对照确实遗留一个 `w7-sleep` 进程，已手动 kill，证明收尾用例能抓到泄漏。
- D 文档读取改用自拼路径 → task-docs 2 条失败（全等取项、reader 收到清单 absolute）。

## 其他
`npm run lint`（--max-warnings 0）与 `npm run typecheck` 均通过。未跑全量门禁（由宿主合并前运行）。提交由 Autome 兜底（如被拒）。
