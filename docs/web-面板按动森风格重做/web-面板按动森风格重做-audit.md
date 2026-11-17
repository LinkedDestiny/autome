# 审计结论 · M-11 · 实现缺陷（总览缺值显示）

领域：总览缺值显示；reopen 2（该领域首次，收敛模式仍 normal）。

- 验收命令全绿：readme-images 4 + docs-web 2 = 6 过；layout.spec.ts 14/14 过（本机无 ps EPERM）。负向对照（移走 loop.png）readme-images 3 条失败，还原后全过。
- 缺陷：`docs/images/dashboard.png`（README 交付物）里「批准合并」的 bugx 卡片显示 `设计轮 undefined / undefined`。根因 `apps/web/app/lib/overview-labels.ts:25`：`round` 在 `block` 存在时直接拼 `block.design_round / block.design_round_limit`，状态块缺这两个字段（Bug 任务）就输出字面 undefined；`overview-labels.ts:17` 的 badge 同类拼接。违反 G-3（交付可用截图）与 B 组总览卡片「缺数据回退」（设计 B-Q2）。验收命令只数文件与尺寸，拦不住这一点。
- 另：总览截图里「进行中」卡片被三张等待卡挤到首屏之外，README 看不到进行中任务（设计原型 H-01 要求有任务在跑与等批准同屏）。
- 判别检查：`.autome/output/overview-undefined.check.test.ts`（对缺设计轮字段的状态块调用 overviewCardView，断言结果不含 undefined；运行需在 apps/web 内，本轮未能跑通因 `--root /` 过慢，下一轮实现轮搬进 `apps/web/test/overview.test.ts` 先看红再修）。证据之一是 `docs/images/dashboard.png` 本身。
- 下一轮：修 `round`/badge 缺值回退（用 `—`）；调整总览截图夹具使进行中卡片首屏可见；重生成 dashboard.png 并复制进 `docs/images/`；验收命令与快检再跑。
