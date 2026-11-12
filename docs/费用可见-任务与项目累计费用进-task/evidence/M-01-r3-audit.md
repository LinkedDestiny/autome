# M-01 r3 审计复验

- 验收命令：`cargo test -p autome-domain --lib cost_summary_`（4 通过）+ `cargo test -p automed --test end_to_end cost_rollup_`（1 通过），合计 5，与契约一致。
- 负向对照（均已还原，git status 干净）：
  - 跳过 `cost_usd=None` 会话：域单测 3 个失败；e2e `cost_rollup_task_get_...` 失败。
  - 运行中会话计入：`cost_summary_skips_running_sessions` 失败。
  - 缺失会话改变总额口径：第 1 个单测失败。
- 全量 `sh scripts/ci-local.sh`：本轮自己跑，退出 0，Format/Clippy/Test/Desktop/Contract/Web 全绿（648s）。
- 结论：通过。
