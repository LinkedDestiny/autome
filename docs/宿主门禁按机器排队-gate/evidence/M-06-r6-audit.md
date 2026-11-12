# M-06 r6 审计复验（审计 #6）· 通过

## 负向对照（先做）
- README 中 `concurrency` 全部替换后跑 `grep -n concurrency README.md`：无命中，退出码 1，验收链断在这里 → 命令能拦住「README 没补」。已还原（`git status` 干净）。
- `technical-design.md` 同样替换：第二条 `grep` 退出码 1。已还原。
- 删旧用例类对照：`--list` 总数为 1262，与契约一致；少一个名字则总数对不上（以总数为判据）。

## 验收命令
- `cargo test --workspace`：除下条外全绿；`--list` 共 1262 个用例（契约 1262，吻合）。
- `cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check`：无输出。
- `npm test --prefix apps/desktop`：105 用例，104 通过、1 跳过、0 失败（与基线相同）。
- 两条 grep：README 第 115/129/136 行、technical-design 第 252/258 行命中，分别落在「Host gate, retries and cooldowns」与 §8.1「宿主准备」「宿主门禁」。

## 一个不稳定用例
`stopping_a_task_kills_the_cli_the_wrapper_started` 在 `cargo test -p automed --test end_to_end` 默认线程数的整套里连续 2 次失败（ParseIntError: Empty，end_to_end.rs:2978）。判定为用例自身的竞态，不是本任务产品行为：
- 该用例读 `cli.pid`：`while !pid_file.exists()` 一见到文件就读，而假 CLI 的 `echo $$ > file` 是先创建（截断）再写入，读到空串即报错；整套并发负载高时窗口被放大。机制与用例代码一致。
- 用例与本任务无关（基线即有，本任务未改动它）；单跑 3/3 通过；`--skip gate_queue_` 106/106 通过；`--test-threads=4` 整套 112/112 通过。
- 因此不是产品缺陷，也不是验收缺口；记入 Backlog B-07，不计 reopen。

## 新造检查
无需新造：契约已由负向对照 + 全量覆盖；`gate_queue_` 6 个用例在 M-05 审计已做过缺陷注入。

## 人工项
H-01 已在 `## 人工验收清单`，由用户合并前确认。

结论：通过，M-06 标 `已完成`。
