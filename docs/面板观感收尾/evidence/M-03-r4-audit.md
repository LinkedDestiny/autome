# M-03 r4 审计证据（审计 #3）

结论：通过。

## 验收命令
- vitest 10 个文件 137 个用例通过、0 失败（预期 ≥136）。
- `playwright test --list` 的 grep 计数 = 16（预期 16）。

## 负向对照（均已还原，git status 干净）
1. 追加 `.overview-heading h1 { font-size: 22px }` → ui-base「没有其它针对丝带标题的字号覆盖」失败（136/137）。
2. 关闭按钮移出 `header.drawer-head` → ui-base 的 Drawer 结构与 Esc/关闭两条失败（135/137）。
3. 从 e2e 用例表删掉两条抽屉用例 → grep 输出 14（不是 16），计数对删减敏感。
4. 脏文本正则去掉 `null` → dirty-text 第 1 条失败（136/137）。
还原后复跑 137 通过。

## 自造检查
- 确认 `e2e/harness/browser.ts` 的 `oneScreen` 对页面可见文本调用 `dirtyText`；`playwright.config.ts` 注释已写 35 条。
- 抽屉布局用例的真实浏览器执行（footer 在视口内、控件 appearance 为 none）本轮不能跑，由宿主门禁的 e2e 承担。

## 备注
zsh 下 `$F` 不分词会让 vitest 报 No test files，须用 `${=F}`；非产品问题。
