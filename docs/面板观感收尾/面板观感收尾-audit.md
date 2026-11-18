# 实现审计

## M-13 · 通过 · 审计 #14

- 验收命令 5 文件 66 用例通过；drawer-focus.test.ts 11 用例（原 ui-base 的 Drawer Esc 用例迁入该文件，未丢失）。已登记 unit 分组，test-groups 通过。
- 负向对照（均已还原，git status 干净）：删焦点恢复 → 2 失败；去掉 Shift+Tab 反向循环 → 2 失败；不移除监听 → 3 失败；不做初始聚焦 → 4 失败；Drawer 不挂载 manageDrawerFocus → 2 失败。
- 源码核对：移入首个可聚焦控件、无控件回退容器（aside tabIndex=-1）、每次按键重新查询候选、首尾循环、卸载移除监听并归还触发元素（已脱离 DOM 则跳过）。Esc 行为保留。
- 无人工项；真实浏览器焦点行为由宿主门禁 e2e 承担，沙箱无法跑。
- 证据：docs/面板观感收尾/evidence/M-13-r21-audit.md
