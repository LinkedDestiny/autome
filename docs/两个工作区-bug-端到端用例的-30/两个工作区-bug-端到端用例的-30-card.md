## 现象

T-26 合并后的 MainCheck（main `281a575`）报 `main_gate_failed`：`End to end` 一步里两个用例失败——

- `bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos`（`crates/automed/tests/end_to_end.rs:4531` 附近）
- `bug_regression_guard_checks_each_workspace_member`（`:5069` 附近）

两者都是自己写的 `tick` 循环，截止时间硬编码 `Duration::from_secs(30)`（文件里只有这两处用 30 秒）；失败时任务都已推进到 `merge_gate`、仍在正常前进，只是 30 秒用完了。当时机器上同时在跑 3 个任务的门禁、会话和这次 MainCheck。

同一提交上单独重跑这两个用例：2 个通过，用时 8.7 秒（证据：`.autome/output/t26-maincheck-rerun.txt`；失败日志：`.autome/output/gates/T-26/main_check-281a575235bf.log`）。

同文件的 `settle()` 早已因同一原因把截止时间从 20 秒放到 90 秒，并在注释里写明：它是挂死探测器，不是性能预算。

## 期望

- 这两个用例的等待不再因机器负载误报；与 `settle()` 的口径一致（挂死探测，不是性能预算）。
- 最好收敛成一个共用的「tick 直到条件成立」辅助函数，别再出现第三处手写的短截止时间。
- 不放宽任何断言本身。

## 复现思路

在机器满载时（例如同时跑另一个 `cargo test --workspace`）跑 `cargo test -p automed --test end_to_end workspace`，两个用例会在 30 秒处 panic，`why` 输出显示任务停在 `merge_gate` 而非挂死。

## 补充（11-11 22:50，范围扩大）

T-16 合并后的 MainCheck（main `fb13dbd`，`.autome/output/gates/T-16/main_check-fb13dbd7e86d.log`）又报 `main_gate_failed`，这次是第三个手写截止的用例：

- `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it`（`end_to_end.rs:4132` 起，截止 15 秒，panic「legacy loop did not settle」）

同文件里手写的短截止一共有：`:468`（10s）、`:2796`（10s）、`:2808`（10s）、`:4132`（15s）、`:4521`（30s）、`:5115`（30s）（行号以 main `fb13dbd` 为准）。

**本任务范围扩大为：** `end_to_end.rs` 里所有手写的「tick 直到条件成立」循环统一改用一个共用辅助函数，截止时间与 `settle()` 一致（挂死探测，90 秒），逐个确认它们是等待条件而不是性能断言；若某处确实是在断言「应当很快」，保留并在证据里说明。回归测试可以用「在机器满载时跑」的方式取证，或者用可注入的慢 tick 证明旧截止会误报。

## 补充二（11-12，并入 T-34）

T-34「端到端 settle 在待重试未到期时提前返回」并入本任务，T-34 取消。它的成因与修法：

- `World::has_pending_work()` 只把**已到期**的待重试算作未完成（`pending_retry` 的时间 `<= now_iso()`）；限流改派后的重发若在某一瞬间还没到期，`settle()` 就在任务停在 `adjudicate`、没有会话在跑时提前返回。T-16 的合并前门禁（`11cd3af`）因此在 `a_rate_limited_round_moves_to_the_other_cli_when_that_keeps_roles_apart_and_waits_when_not` 上失败（左 `adjudicate`，右 `await_design_approval`），单跑 3/3 通过。
- T-34 的定位轮与修复轮已经写出一版（提交仍在对象库里，可 `git show` 参考，不必照抄）：回归用例 `3b07a52`（`settle_keeps_waiting_for_a_retry_that_is_not_due_yet`），修复 `3553378`——`has_pending_work` 把任何待重试都算未完成；新增共用的 `tick_until(done)`（90 秒挂死探测）由 `settle()` 与新的 `wait_for_retry(task)` 复用；原先依赖「未到期即返回」的部分脚本用例改用 `wait_for_retry`。

**合并后的范围：** ① `settle()` 不在还有待重试时提前返回；② 所有手写的短截止循环统一改走 `tick_until` 这一个辅助函数。两件事都在 `crates/automed/tests/end_to_end.rs` 的测试基础设施里，回归用例写在同一文件中即可（内核现在只保护定位轮新增的测试块，辅助函数可以改）。
