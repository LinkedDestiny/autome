# M-03 · 审计 #4 复验

范围：实现 #4 的门禁修复（dd712ae，仅 `dashboard-double.ts` 补 `gate_queue_position: null`）。三个里程碑均已完成，无待审项。
- 负向对照：宿主门禁日志已证明缺该字段时 `notifier-backfill-real` 2 例失败（严格键集合断言保留），修复前即红；本轮未再改动产品代码。
- 本轮自跑：`npx vitest run test/notifier-backfill-real.test.ts test/notifier.test.ts test/store.test.ts` → 39/39 通过。
- 改动仅限测试替身，对齐真实 `dashboard.get` 形状，无产品行为变化。
结论：通过，里程碑表保持三项已完成，不计 reopen。
