# M-03 · 审计 #3 独立复验

结论：**通过**（含一处验证缺口，当场补强并复验）。

## 验收命令（自己跑）
- `launcher::tests::task_card scheduler::tests::task_card`：5 passed；`prompt_rendering`：11 passed；`end_to_end task_card`：3 passed；0 failed。
- `cargo fmt --check` 通过；`cargo test --workspace` 合计 977 通过、0 失败（autome-domain 306、automed lib 577、end_to_end 59、prompt_rendering 11、real_cli 7、cli 13+4）。

## 负向对照（逐条注入，跑验收，还原）
| 注入 | 被拦下的用例 |
|---|---|
| `write_task_inputs` 无附件即提前返回 | scheduler `a_card_without_attachments_…`；e2e `task_card_a_drafted_bug_waits_…` |
| 卡片段放在模板渲染结果之后 | `the_intake_prompt_names_the_card_before_the_request`；金样 `…matches_its_golden`；e2e 第一个 |
| 有正文时对整段含 request 的结果做占位符替换/检查 | `a_request_with_placeholder_text_is_kept_literally_…` |
| `queued_tasks` 的过滤放进 `Draft` | **原验收放过**（见下） |

## 验证缺口
契约声称「queued_tasks 包含草稿时 `fill_slots_never_starts_a_draft` 失败」不成立：注入后该用例与 e2e 仍全绿，因为状态机对 `(Draft, SlotAvailable)` 拒绝，`fill_slots` 只是被动跳过。产品行为（草稿从不启动）是对的，由状态机兜住，属纵深防御缺失检查而非产品缺陷。
补强：在 `fill_slots_never_starts_a_draft` 里加 `assert!(store.queued_tasks(project).is_empty())`。再次注入同一缺陷 → 该用例失败；还原后 5 passed。不计 reopen。

## 其它
所有注入均已还原（store.rs、launcher.rs、scheduler.rs 仅余补强那一行断言）。无人工验收项需移动。
