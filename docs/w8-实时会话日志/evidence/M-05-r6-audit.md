# M-05 r6 审计复验

结论：通过（未看实现轮证据，独立复验）。

- 验收命令：4 个文件 29 个用例通过，skipped 0，与契约一致。lint、typecheck、build 退出码均为 0。
- 负向对照（逐条注入、跑验收命令、还原，还原后 `git status --porcelain` 为空）：
  1. `applyScroll` 无条件设 scrollTop → 退出 1，`session-log-view` 滚动适配器用例红。
  2. `applyScroll` 里调 `focus()` → 退出 1，同一条红。
  3. `bell-targets.ts` 的 `/tasks/:taskId` 改 `available:false` → 退出 1，3 条红（bell-view 两条 + task-page 第 1 条）。
  4. `routes.ts` 去掉任务路由 → 退出 1，bell-view「目标可用性与真实路由表一致」与 task-page 第 1 条红。
  5. `app.css` 追加 `#ff0000` → 退出 1，`styles`「样式与 TSX 不散写颜色」红。
- 自建检查：grep 组件源码无 `.focus(`、`scrollIntoView`、`autoFocus`、`aria-live`（无命中）；读 `routes/tasks.$id.tsx`：loader 只调一次 `task.get`，离线/找不到有中文文案，无写操作。
- 未取证项：H-01–H-05 已在设计文档「人工验收清单」，由用户合并前确认，不计 reopen。
- 提交由 Autome 兜底（如提交失败）。
