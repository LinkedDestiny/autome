# M-02 r2 审计复验

## 负向对照（先做，改后全部还原，`git status` 干净）

| 注入 | 结果 |
|---|---|
| 跳过 `continue` 改 `break` | `after_create_skips_blocked_without_changing_position` 失败；e2e `after_create_b_waits_for_a_…` 失败 |
| 依赖在创建时不落库（等价于 tick 之后才写） | e2e `after_create_b_waits_for_a_…` 失败 |
| 所有 `Requeued` 都按依赖跳过（去掉 `check_editable` 判断） | `after_create_requeued_checks_design_and_implementation_history` 失败 |
| 完全不约束 `Requeued` | 同一用例失败 |

## 验收命令（还原后）

- `after_create_`：lib 5 + e2e 1，共 6 通过。
- `fill_slots_`：2 通过。`the_parallel_limit`：2 通过。

## 源码核对

`create_task_after` 在同一事务内插入任务、排队序号与依赖边，tick 在其后；`is_satisfied` 仅 `Done`，Cancelled/Failed 依赖保持阻塞；被跳过的任务不占 `in_use`、不改 `queued_seq`。

## 结论

通过。无新增判别检查（以上注入即判别）。全量门禁未跑，由宿主合并前承担。
