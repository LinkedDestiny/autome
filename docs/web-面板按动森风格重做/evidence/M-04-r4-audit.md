# M-04 r4 审计复验

- 验收命令一：vitest 9 文件，82 用例全过（基线要求 ≥75）。
- 验收命令二：playwright 7 条全过。
- 负向对照（均注入后跑、失败、`git checkout` 还原）：
  1. `gatePhrase` 无门禁返回「门禁未跑过」：labels 1 例、task-card 3 例红。
  2. `doneGroup` 周起点改周日：board「今天昨天本周更早与更早」红。
  3. board-data 投影 `impl_rounds_used` 取 `budget_n`：board-done-real 2 例红。
- 新造检查：无额外；负向对照已覆盖判别。
- 结论：通过。无人工验收项。
