# M-10 · 审计 #17 复验（实现 #18 之后）

- 独立运行 `cd apps/web && /usr/bin/time -p npm run e2e`（ps 可用）：预检通过，Playwright 共 20 条，13 通过、7 未运行，0 跳过；real 170.64，globalTimeout 触发，teardown 也超时。
- 已通过：D-1（44.1s）、D-2（14.5s，节点投影修复有效）、layout 11 条。
- 失败：D-3 被总时限切断，T-33 迟迟未到 needs_you；D-4 与 manual-items 5 条未运行。违反「20 条全过、≤180 秒」。
- 结论：实现缺陷（时限，R-6），reopen 2，e2e-flow，domain-review。负向对照因套件未通过未做。
- 日志：.autome/output/a17-e2e.log。
