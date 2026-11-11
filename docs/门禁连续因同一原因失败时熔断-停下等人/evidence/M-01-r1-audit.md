# M-01 审计 #1 复验证据

结论：通过。

## 负向对照（先做，逐个注入后还原，`git diff` 确认还原）
- 去掉数字串→`#`：`volatile_details_do_not_change_identity`、`identifier_digits_are_preserved`、`t14_fixtures` 失败（3 个）。
- 窗口 400→60 行：`t14_fixtures`（摘要无 `ws`）、`window_is_last_400_lines` 失败。
- `streak` 不看 `tripped`：`tripped_result_resets` 失败。
三条均能拦住错误实现。

## 验收命令
- `cargo test -p autome-domain gate_failure::`：16 通过 0 失败。
- `cargo test -p autome-domain`：361 通过（345+16，差 16 即新增单测）。
- `AUTOME_T14_GATE_DIR=… --test t14_gate_logs -- --ignored`：1 通过（1–18 相同，19 不同）。
- clippy（domain，-D warnings）、`cargo fmt --check` 无输出。

## 自建检查（一次性，已删）
对 3 份夹具直接求指纹：01=18（636e8702…），19 不同；01 摘要为 `Web typecheck、Web test：Error: Cannot find package 'ws' …`，与设计一致。另验：制表符分隔的 `FAILED\tWeb test` + 不同路径/行列号 → id 相同；退出码不同 → id 不同。均符合。

## 全量
`cargo test --workspace`：autome-domain 361、automed 库 606、cli 19+11 全过；end_to_end 并发另一 worktree 的 e2e 时 `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it` 超过其 15 秒内置期限失败；空载单跑 4.3s 通过。本分支对 automed 零改动（仅 domain 新模块与夹具），判定为负载引起的时序波动，与 M-01 无关。
