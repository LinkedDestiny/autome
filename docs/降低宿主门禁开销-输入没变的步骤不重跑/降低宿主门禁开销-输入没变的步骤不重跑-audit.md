# 审计 #7 · M-06 · 实现缺陷（退回，领域：Web 内联样式规则）

分组配置本身已修好：`node --test scripts/test/web-groups.test.mjs` 1 个通过，`sh scripts/test/web-full-run-check.sh` 退出 0（完整 `npm run test` 能启动，111 文件 / 840 用例，与预期一致）。

失败命令：`cd apps/web && npm run test`（验收要求全量 5 次全绿）。两次全量输出尾部：
- 第一次 `Test Files 2 failed | 109 passed (111)`，`Tests 2 failed | 838 passed (840)`；
- 第二次 `Test Files 1 failed | 110 passed (111)`，`Tests 1 failed | 839 passed (840)`，200.65 s。

两次都红的是 `|unit| test/styles.test.ts > TSX 没有内联样式`（`expect(tsx).not.toMatch(/style=\{/)`）。根因：`apps/web/app/components/task-timeline.tsx:10` 的 `<p style={entry.unverified ? {…} : undefined}>`，由 M-03 提交 1349131e 引入（基线 a762d3f8 该文件无 `style={`）。单跑 `npx vitest run test/styles.test.ts` 同样红，与分组无关。违反：M-06 验收「全量 Web 测试 5 次全绿」及项目测试体系须通过。
另一个只在第一次全量里红：`|kernel| test/project-confirm-real.test.ts > fixture 的 probe/add/skip 与真实回包逐层键集合一致`，单跑 5/5 绿、第二次全量也绿——偶发，疑与并行负载下 probe（输出含「缺少 Git，或两个 CLI 都不可用」）有关，未能定位；实现轮需查清它是否因 4-worker 并行才出现。

负向对照：把 `eventbus-live` 移进 `unit` → `test-groups` 用例「内核、端口、进程及构建测试不进入 unit」红，已还原（`git status` 干净）。
未取数：交替基准、连续五次、空闲口径比值（本轮 load 3–6，无空闲窗口；200 s 的墙钟没有同口径旧配置对照）。ENOTEMPTY 结论仍待实现轮。

判别检查：`.autome/output/m06b/`（fr.log、fr2.log 为两次全量输出）；回归用例即现成的 `test/styles.test.ts`，修复后应绿。
