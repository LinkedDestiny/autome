# M-06 · 审计 #7 复验证据

- `node --test scripts/test/web-groups.test.mjs`：1 通过（真实启动三个 project，约 2.4 s）。
- `sh scripts/test/web-full-run-check.sh`：退出 0。
- `cd apps/web && npm run test` ×2：111 文件 / 840 用例（与预期一致）；第一次 2 个失败（styles、project-confirm-real），第二次 1 个失败（styles），200.65 s。
- `npx vitest run test/styles.test.ts` 单跑：红；`test/project-confirm-real.test.ts` 单跑：5/5 绿。
- 溯源：`style={` 出自 M-03 提交 1349131e 的 `task-timeline.tsx:10`，基线无。
- 负向对照：eventbus-live 放进 unit → test-groups 一个用例红，已还原。
- 结论：实现缺陷，退回，reopen 1→2，领域「Web 内联样式规则」。基准与五连跑未取数。
- 输出文件：`.autome/output/m06b/`（不入仓库）。
