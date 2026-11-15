# M-04 · r7 · 审计独立复验
结论：通过。详见 面板收尾批次-用上新的内核数据-补齐备注-audit.md。
- vitest 50/50；Playwright B-2、B-4 通过；B-3 竞态失败 2/4（controls.spec.ts:100 未轮询），其余通过 → BL-13。
- 负向对照三项（排序、top、after）单元与浏览器均能拦住，已还原。
