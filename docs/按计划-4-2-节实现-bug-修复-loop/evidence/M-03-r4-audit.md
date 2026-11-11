# M-03 审计复验（第 3 轮）

结论：通过。

## 负向对照（先于验收命令；每次改完还原，`git status` 干净）
| 缺陷注入 | 结果 |
|---|---|
| 去掉合并前门禁复用的 `!bug ||`（需求任务也要求命令相同） | `scheduler::tests::bug_loop_merge_gate_does_not_reuse_a_pass_for_a_different_command` 红 |
| 复现判定 `exit_code != 0` 改为 `== 0` | `bug_loop_a_repro_command_that_passes_on_the_locate_commit_returns_to_locate` 红（领域单测另 5 个红） |
| 修复门禁不跑复现命令（`bug_gate` 去掉 `FixGate`） | `bug_loop_a_fix_whose_repro_command_still_fails_returns_to_fix` 等 3 个红 |
| 选表只看 `kind` | `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it` 红 |
| 崩溃重试（`redispatch_with`）给 Fix 计一轮 | `bug_loop_a_crashed_fix_session_is_retried_without_counting_a_round` 红 |

附注：第一次尝试的变体（只改带 `[gate]` 时的组合命令、以及在 `Action::Reenter` 里计轮）没有被拦住，但那是我变错了位置：
前者不影响契约指定的“无 `[gate]`”场景，后者不是崩溃重试的实际路径（重试走 `redispatch_with`）。换到契约所述/真实路径后均被拦住，不构成验证缺口。

## 验收命令
- `end_to_end bug_loop` 8 passed；`scheduler::tests::bug_loop` 3 passed；`end_to_end task_card` 3 passed；
  `cli task_card_a_brief_piped…` 1 passed；`end_to_end` 全量 67 passed（基线 59 + 8）。
- `cargo test --workspace` 全部 0 failed（automed lib 598、domain 345、end_to_end 67、cli 等）。摘要见 `.autome/output/m03-r3-audit-run.txt`。

实现轮 #4 备注的 “Web test 沙箱监听受限” 属于 `apps/` 之外的宿主门禁，不在 M-03 验收命令内。
