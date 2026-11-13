# M-02 · 审计复验（实现轮 r3）

结论：通过。未读实现轮证据，独立复验。

## 验收命令
`cd apps/web && npx vitest run test/task-stage.test.ts test/task-notice.test.ts test/task-header.test.ts test/labels.test.ts test/task-card.test.ts` → 5 个文件、43 个用例全过，与契约数一致。
`npm run lint`（--max-warnings 0）与 `npm run typecheck` 通过。

## 负向对照（均已注入并还原，`git status` 干净）
- A 实现预算改先取 `impl_round_limit` → task-stage 第 1、12 条失败。
- B 已取消任务直接用 `stageOf`（跳过回溯）→ task-stage 第 5 条失败。
- B2 回溯不排除取消事件本身及之后 → task-stage 第 5 条失败。
- C 失败原因改用 `renderReason` → task-notice 第 1、2 条失败。

## 自造检查
读 `stageBarView` 的 Bug 复现分支：事件按 seq 倒序取最新一条 `bug.reproduced` / `bug.repro_missed`，`why` 取值 `exited_zero` 等与内核（scheduler.rs / bug_loop.rs）一致，无「fix_gate 非零 = 符合预期」路径。未发现缺陷。
未跑全量门禁（宿主合并前运行）。提交由 Autome 兜底（如被拒）。
