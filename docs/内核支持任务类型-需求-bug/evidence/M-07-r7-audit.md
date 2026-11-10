# M-07 r7 审计复验

- 负向对照 1：只有新建文件才把卡片路径加入提交（`paths.push` 移进 `if !dest.exists()`）→ `task_card_retry` 1 passed / 1 failed（重试用例 `HEAD:plans/retry/retry-card.md` 不在 HEAD）。已还原（cmp 一致）。
- 负向对照 2：把 `!dest.exists()` 改成 `true`（每次重写卡片）→ 2 个用例都失败（mtime/字节断言）。已还原。
- 验收命令 `cargo test -p automed --lib scheduler::tests::task_card_retry`：2 passed（基线 0，+2）。
- `sh scripts/ci-local.sh` 全量：退出码 0；automed lib 582 passed（下限 582），各 crate 合计 1050 ≥ 990。日志 `.autome/output/ci-audit7.log`。
- 代码审读：`write_task_inputs` 对已有卡片不重写，路径仍进 `commit_paths`；index.lock 造出的 git add 失败后重试能提交，HEAD 在失败时未动。
- 结论：通过。
