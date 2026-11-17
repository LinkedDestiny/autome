# M-11 r15 审计复验

1. 负向对照：移走 `docs/images/loop.png` → `vitest readme-images` 3 failed / 1 passed；还原后 `git status` 干净。
2. 验收命令：readme-images 4 过、docs-web 2 过（共 6）；`playwright layout.spec.ts` 14 passed（日志 `.autome/output/layout-a12.log`）。
3. 自建检查：目视 `docs/images/dashboard.png`：「设计轮 undefined / undefined」；进行中卡片在首屏之外。源码 `apps/web/app/lib/overview-labels.ts:25` 无字段回退。
4. 结论：实现缺陷，领域「总览缺值显示」，reopen 2。判别检查草稿 `.autome/output/overview-undefined.check.test.ts`。
