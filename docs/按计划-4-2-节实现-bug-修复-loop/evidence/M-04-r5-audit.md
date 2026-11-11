# M-04 独立复验（审计 #4，对应实现 r5）

## 验收命令（自己跑）
- `end_to_end bug_loop`：14 passed；`dispatch::tests::bug_loop`：3 passed；`autome-cli bug_loop`：2+2 passed；`autome-cli` 全量：19 单元 + 10 集成，0 failed。均符合契约预期。
- `cargo test --workspace`：合计 1071 通过、0 失败；`cargo clippy --all-targets -D warnings` 通过。宿主门禁 `ci-local.sh` 在 214a5dc 亦通过。

## 负向对照（注入后已全部还原，工作区干净）
1. `task.reply` 对需求任务也走 `Trigger::Reply` → `bug_loop_reply_on_a_feature_task_is_exactly_reject` 失败。
2. `task.get` 对所有任务输出 `fix_round` → `bug_loop_task_get_reports_the_fix_round_for_bugs_only` 失败。
3. 补充信息不写 `-replies.md`（不提交）→ `bug_loop_cannot_reproduce_waits_for_info_and_a_reply_returns_to_locate_with_it` 失败。
未另测的契约项（工作区 repos 守卫、ExtendBudget 委托）由用例 4、6 覆盖，本轮未注入。

## 结论
通过。无实现缺陷、无验证缺口、无人工验收项。
