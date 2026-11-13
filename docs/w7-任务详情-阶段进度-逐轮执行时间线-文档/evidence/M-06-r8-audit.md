# M-06 审计复验（审计 #6，对应实现 #8）

结论：通过。

## 验收命令
`cd apps/web && npx vitest run test/task-changes.test.ts test/task-decisions.test.ts test/task-merge-real.test.ts test/task-live-real.test.ts test/task-page.test.ts test/styles.test.ts`
→ 6 个文件、33 个用例全过（5+3+4+3+6+12），107.9 秒。契约写 32，差 1 来自 task-page 实际 6 条；已把契约改为 33。本环境可监听端口、可用 ps，实现轮缺的实时与进程收尾证据在此补齐。

## 负向对照（逐个注入、复验、还原；git status 干净）
1. 合并进度在 `main_check`/`cleanup` 就记「已完成」(progress 1→2)：task-changes「合并进度逐节点…」与 task-merge-real「main_check 挂住期间…」失败。
2. 事件前缀只留 `task.`、`session.`：task-live-real「八个前缀…」与 task-page「仅本任务八类事件触发重验」失败。
3. `.task-timeline` 去掉 `overflow-y: auto`：task-page「页面定高、两栏与每个内容区内部滚动」失败。
三条均与契约预期一致，验收命令能拦住错误实现。

## 版本兼容复验（T-70）
`git archive main` 构建 main 内核（约 11 秒），以 AUTOME_BIN/AUTOMED_BIN 跑 task-ops-real、task-merge-real、task-live-real：3 文件 23 个用例全过。

## 进程收尾
按本 worktree 路径查 ps，无存活的 automed/子进程（系统里其它 worktree 的 e2e 进程不属本任务）。

## 人工项
H-03/H-04/H-05 等需肉眼的项已在 `## 人工验收清单`，不阻塞关闭。无新造判别检查需保留。
