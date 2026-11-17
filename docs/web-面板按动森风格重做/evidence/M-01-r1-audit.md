# M-01 r1 审计复验

结论：通过。

## 验收命令
`npx vitest run test/ui-kit.test.ts test/styles.test.ts test/labels.test.ts test/testids.test.ts test/bell-view.test.ts test/test-groups.test.ts`：6 个文件、68 个用例全过（契约下限 58）。

## 负向对照（注入后失败，已逐项还原，git status 干净）
| 注入 | 失败用例 |
|---|---|
| 进度条加 `style={{width}}` | styles「TSX 没有内联样式」、ui-kit 进度夹取 |
| app.css 写 `var(--ui-nope)` | styles「app.css 的每个 var 都有令牌定义」 |
| 进度条不夹取 | ui-kit 进度夹取、非有限数 |
| 组件里写 `#fff` | styles「样式与 TSX 不散写颜色」 |

## 自造判别检查
- Stepper 去掉 `value<=min` 禁用 → ui-kit 步进器用例失败。
- Tile 缺失值不显示「—」 → ui-kit 数字缺失用例失败。
均已还原，复跑 ui-kit+styles 35 个全过。

## 备注
快检浏览器部分（Chromium 预览）未在本轮取证，不属 M-01 验收命令；由宿主合并前门禁承担。
