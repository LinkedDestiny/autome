# M-01 第 1 轮审计复验

结论：通过。

## 验收命令（自跑）
- `cargo test -p autome-domain task_card`：7 passed
- `cargo test -p automed --lib store::tests::task_card`：5 passed
- `cargo test -p automed --lib store::tests::requeued_old_ledger_rows_and_new_rows_roundtrip`：1 passed
- `cargo test --workspace` 全量：全部 ok、0 failed（autome-domain 306、automed lib 560 等）

## 负向对照（逐个注入、确认失败、还原）
| 注入 | 失败的用例 |
|---|---|
| `TaskState::Draft` 转成 `Queued{resume:None}` 线上形状 | `draft_has_its_own_wire_shape_and_is_not_queued` |
| v5 块去掉 `DEFAULT 'feature'` | `a_v4_ledger_migrates_to_v5_and_every_old_task_reads_as_a_feature` |
| `row_to_task` 不读 `kind`（恒 Feature） | `kind_round_trips_through_the_ledger` |
| `queued_tasks` 只按 `created_at` 排序 | `a_started_draft_queues_behind_tasks_that_were_already_queued` |
| 以 `COALESCE(created_at, queued_seq)` 为排序键 | 同上 |

五处注入均被拦住；还原后 `git status` 干净（备份在 `.autome/output/*.bak`）。

## 代码核对
`Trigger::Start` 只接受 `Draft`；`Draft` 的 `Cancel` 走 `RunCancel`，`Pause` 报「任务尚未开始」；
`task_counts` 有 draft 桶；`queued_tasks` 以 `queued_seq` 排序（NULL 旧行在前、按 `created_at`）并过滤到 `Queued|Requeued`。
无产品缺陷。人工验收项：无。
