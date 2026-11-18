# M-08 r15 审计复验

- 验收命令：7 个测试文件、84 用例全部通过（契约 ≥82）。
- 负向对照（均已还原，git status 干净）：
  1. `task-ops-view.tsx` 去掉空操作返回 null → 「无可见操作与结果时隐藏」失败。
  2. `.task-milestones` 改回 `flex-wrap: wrap` → 「30 个里程碑全量单行滚动」失败。
  3. `.task-docs nav a { padding: 8px 12px }` → 「38 份证据…无额外上下内边距」失败。
- 阶段格 `small` 的 nowrap/省略规则落在 `.summary-steps small`，与测试断言一致，title 保留完整文本。
- 未跑浏览器 e2e（实现轮记录 ps EPERM；e2e 由宿主合并前门禁执行）。浏览器肉眼项 H-05 已在人工清单中。
- 结论：通过。
