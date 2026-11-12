# M-01 r1 审计复验

- 验收命令：store.test.ts + notifier.test.ts 共 29 个通过（store 18 + notifier 11）；lint 零警告；typecheck 通过。
- 负向对照（各自改 server/store.ts，跑 store.test.ts，随后 git checkout 还原）：
  - 播种 INSERT 去掉 `WHERE name='notifier'` → 1 失败（v1 升级播种用例）✔
  - list 改 `SELECT *` → 4 失败 ✔
  - backfill 去掉事务 → 1 失败（backfill 一个事务用例）✔
  - resetLedger 不清 notifier_task_state → 1 失败（resetLedger 用例）✔
  四条对照全部能被验收命令拦住；已还原，工作区 store.ts 与提交一致。
- 全量 vitest：273 通过 / 20 失败 / 12 跳过，总 305；失败全部来自 target/debug/automed 未构建（`请先运行 cargo build`），属 M-03 验收范围（含 cargo build），与 M-01 无关。
- 结论：通过。
