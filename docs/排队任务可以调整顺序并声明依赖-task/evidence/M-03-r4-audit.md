# M-03 · r4 · 审计复验 · 通过

## 验收命令
`cargo test -p automed after_blocked_ && cargo test -p automed after_create_`：after_blocked_ 7 通过（lib 6 + e2e 1），after_create_ 6 通过（lib 5 + e2e 1）。与契约预期一致。

## 负向对照（逐条注入、跑 after_blocked_、用 git checkout 还原，最后 git status 干净）
| 注入 | 失败用例 |
|---|---|
| 每个 tick 都写 task.blocked | after_blocked_cancelled_is_notified_once_even_with_full_slots、after_blocked_failure_resets_on_rerun_and_notifies_again |
| task.set_after 不调 tick（TickReport 默认值） | e2e after_blocked_cancelled_dependency_holds_b_until_cleared |
| 校验放在写入之后 | after_blocked_set_rejects_without_side_effects |
| check_editable 忽略 impl_started | after_blocked_set_rejects_without_side_effects |
| set_task_after 删除全部旧行（notified 丢失） | after_blocked_cancelled_is_notified_once_even_with_full_slots |

五条全部被拦住。脚本与输出：.autome/output/a3-mut.sh、a3-mut.log。

## 另行核对
- 读代码：scheduler 通知循环在槽位满时仍执行（先通知后 break）；notified 与 reason 比较，状态复位后可再发；dashboard.get 新增 queued 组，waiting/running 构造未变，只加字段。
- autome-cli status 相关 4 个现有用例通过（现有读者未坏）。
- 新造判别检查：无新增。
- 全量门禁未跑，由宿主合并前承担。
