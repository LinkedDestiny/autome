# M-02 独立复验（审计 #2）

## 验收命令
- `cargo test -p automed --lib scheduler::` → 91 passed
- `cargo test -p automed --test end_to_end` → 115 passed
- `cargo test --workspace` → 1222 passed + 1 ignored（基线即有）= 1223，与契约一致（1216 + 7）

## 负向对照（每个变异后跑相关用例，确认红，再从备份还原）
| 变异 | 被谁拦住 |
|---|---|
| (a) 不看 `gate_reuse_for_main_check` | e2e `main_check_reuse_switched_off_matches_today_event_by_event` |
| (b) `commit != from_sha` 代替第二父提交比较 | e2e `main_check_reuse_skips_the_run_...` |
| (c) 不比树（`tree != from_tree` 恒假） | `..._when_main_advanced_after_...`、`..._decision_table` |
| (e) 不检查最近一条 `ok` | `..._decision_table` |
| (g) Bug 任务不比较命令 | `..._decision_table` |
| (h) 不比较 `gate.started.command` | `..._ignores_a_result_whose_command_changed_mid_run`、`..._decision_table` |
| (i) 找不到 `gate.started` 时放行 | `..._decision_table` |

(d) 早退位置：未做变异，E1 断言 `.worktree` 下无 `_verify-` 目录（end_to_end.rs:6507），按代码阅读该守卫存在。
调用点 `if resolved.gate_reuse_for_main_check && !repo.join(&rel).exists() && !job.exists()` 位于建检出之前。
还原后 `git status` 干净。

## 结论
通过。新造检查为上述变异注入，结果均被拦住，无需保留。
