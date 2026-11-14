# M-06 r8 审计复验

- 验收命令：`cd apps/web && npx vitest run test/testids.test.ts test/w9-layout.test.ts test/styles.test.ts` → 3 文件 26 条全过（10 + 4 + 12），skipped 0。
- 负向对照（均已还原，`git status` 干净）：
  1. 去掉 `env.tsx` 的 `data-command` → testids 1 条失败（组件选择器用例）。
  2. 页面里手写表外 `data-testid="rogue-x"` → 源码守卫失败。
  3. `shell.tsx` 的 `.shell-main` 加 `data-scroll-region` → 滚动区白名单用例失败。
- 自造检查：核对 `TEST_IDS` 表项在各路由/组件中均有使用，且 `data-scroll-region` 仅出现在 7 个指定区域，结果一致。
- 结论：通过。无实现缺陷、无验证缺口。
