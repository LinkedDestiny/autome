# M-07 r8 审计复验（审计 #7）

结论：通过。

## 验收命令
`cargo test --offline -p automed --test end_to_end -- bug_regression_base_ bug_regression_guard_ bug_cd_` → 19 passed / 0 failed（契约预期 19：base 6 + guard 8 + cd 5）。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| 基线改取最近一次会话的 `locate_heads`（`.last()`） | base 组 6 中 4 红：second_locate_without_test_changes、command_only_relocate、rerun_locate_after_failure、rebased |
| 去掉 merge-base 回退（祖先检查恒假、回退处报错） | base 组 5 红，含 `rebased_branch_falls_back_to_merge_base` |
| `rerun fix` 不再补算快照（`start_session` 该分支 `if false &&`） | `rerun_fix_without_snapshot_computes_one`、`rerun_fix_rejects_empty_or_unreadable_baseline` 红，其余绿 |

## 自查
读 `test_fix.rs::regression_set`：候选集 = 旧快照文件 ∪ `base..HEAD`（base 为任务首个定位会话的 head，不在祖先链时退回 merge-base）中的测试文件，故第二次定位改了别的测试文件也不会丢第一次的文件（C-E1）；无快照的 `rerun fix` 在 `start_session` 里补算，空集/读盘失败走协议失败。与 C-E1、C-T1 对得上。

构造的判别检查：上述三组缺陷注入，结果如表。未发现实现缺陷。
