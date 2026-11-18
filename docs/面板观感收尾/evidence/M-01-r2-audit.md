# M-01 r2 审计证据（审计 #1）

结论：通过。

- 基线：验收命令 7 文件 108 用例通过、0 失败（≥107）。
- 负向对照（均已 `git checkout` 还原，工作区干净）：
  1. 删 `.seg > [aria-current="page"]` 选中规则 → `ui-base`「共享尺寸、白底阴影…」失败（107/108）。
  2. 时间线筛选改回 `nav.timeline-filters` → SSR 共用 seg 与「时间线使用 SegmentedTabs」两条失败。
  3. `.seg label > span` 改 `.seg span` → 「共享尺寸…」与「分段内胶囊保留自身标记」失败。
  验收命令能拦住三种错误实现。
- 自造检查：grep `app.css` 中 `.timeline-filters/.board-filters/.task-detail-tabs` 仅剩 `flex-shrink: 0`，无独立选中样式；任务文档胶囊（`pill`）不在 `.seg` 内，符合「胶囊不受影响」。
- C-2 浏览器滚动保持：本沙箱无法起浏览器；静态断言（`overflow-anchor: none`）在 `styles.test.ts` 通过。浏览器部分移入人工验收清单 H-10，M-01 按可跑项关闭。
- 项目快检由宿主在 f8bc7b82 通过（退出码 0）。
