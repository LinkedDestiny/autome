# M-04 r5 独立复验（审计）

- 验收命令 `cd apps/web && npx vitest run test/markdown.test.ts test/task-docs-view.test.ts test/task-docs-real.test.ts`：3 文件、16 用例全过（55s）。本环境 `ps` 可用，实现轮因 EPERM 缺的「收尾无存活进程」用例已真实跑过；跑后 `ps` 无 automed 残留。
- 负向对照 1：`safeHref` 改成对原始目标做区分大小写、不去空白的前缀比较 → markdown.test 「链接协议白名单…」失败（1 failed / 7 passed）。已还原。
- 负向对照 2：表格 `milestone` 恒为 true（不限定「## 里程碑」）→ task-docs-view 「仅设计文档里程碑章节表格当前行高亮」失败（1 failed / 4 passed）。已还原。
- 负向对照 3：源码无 `dangerouslySetInnerHTML`（grep 为空），验收含对源码的该断言。
- 还原后 `git status` 干净。
- 结论：通过。
