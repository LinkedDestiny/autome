# M-04 r5 审计复验 · 审计 #4

- 验收命令（loop-page / ui-base / styles / testids / labels）：5 文件 71 用例通过、0 失败（≥ 70；loop-page 18）。
- 负向对照（均已 `git checkout` 还原，工作区无产品改动）：
  1. Effort 胶囊移出 `.loop-role-model` → 「模型和 Effort 同行…」失败。
  2. 开关移进编辑按钮 → 「六张卡有编辑提示且开关独立于伸展按钮」失败。
  3. 删 `.loop-role:hover, :focus-within` 规则 → 「卡片密度及悬停聚焦反馈…」失败。
- 判别检查：`vitest related` 对 loop.tsx、app.css 跑 16 文件 139 用例全过；`playwright --list` 1512×944 用例数 16；layout.spec 含模型/Effort 顶边与范围选中态断言。
- 浏览器实跑（e2e）未在本轮执行，由宿主合并前门禁承担。
- 实现轮证据按简报要求未先读。
- 结论：通过。
