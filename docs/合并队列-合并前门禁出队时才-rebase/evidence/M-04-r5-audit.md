# M-04 r5 审计复验

- 验收命令（merge_view_ 三段）：lib 3、cli 2、e2e 1，共 6 个用例全过。
- 负向对照 1：`git::behind_count` 范围写反为 `<base>..<branch>` → `merge_view_behind_count_counts_commits_only_on_the_base`、`merge_view_task_get_reports_a_stale_await_merge` 与 e2e 用例失败；已还原。
- 负向对照 2：`merge_queue::view` 去掉 AwaitMerge 限定 → `merge_view_task_get_reports_a_stale_await_merge` 失败；已还原。
- 还原后 git status 干净；cargo fmt --check、clippy（automed、autome-cli，-D warnings）通过。
- 结论：通过。无实现缺陷、无验证缺口。
