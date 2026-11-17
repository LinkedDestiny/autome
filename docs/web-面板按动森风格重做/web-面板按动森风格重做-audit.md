# 审计结论 · M-11 · 实现缺陷（总览缺值显示）

## Convergence Note

1. 未闭合领域：总览缺值显示。当前失败证据：`docs/images/dashboard.png` 仍显示 `设计轮 undefined / undefined`，且进行中卡片不在首屏。
2. 根因：实现轮 #16 修了 `overview-labels.ts` 的缺值回退，但没有重新生成并替换 PNG（截图脚本被 ps EPERM 阻断），前轮验收只数文件与尺寸，漏检图内容。同类范围：docs/images 下五张图全部需用修复后的脚本重生成。
3. 下一轮：在能跑 `scripts/screenshots.ts` 的环境重生成五图，替换 docs/images，目视或像素检查 dashboard.png 无 undefined 且进行中卡片首屏可见；跑完整验收命令（含 layout.spec.ts 14 条）。若环境仍被 ps EPERM 阻断，应把「重新生成五图」移入人工验收清单，而不是继续保持开放空转。
4. 命令：当前应红——人工查看 dashboard.png；完成应绿、不得退化——`(cd apps/web && npx vitest run test/readme-images.test.ts test/docs-web.test.ts test/overview.test.ts)` 与 layout.spec.ts。

## 结论

退回开放，reopen 3，convergence-mode: milestone-review。违反 G-3 与设计 B-Q2 缺数据回退。证据：docs/web-面板按动森风格重做/evidence/M-11-r16-audit.md。判别检查即 dashboard.png 本身（未新增脚本）。
