# M-05 独立复验（审计 #5）

结论：通过。

- 验收命令全部自跑：test / clippy / fmt / protocol eval / git diff --exit-code 均退出 0。`cargo test --workspace` 合计 1071 用例，0 失败 0 忽略；M-01 基线 990（1032−42）+81 = 1071，吻合。日志 `.autome/output/a5-*.log`。
- 宿主门禁 `ci-local.sh` 在 c842d4c 通过（引用，不替代上面）。
- `git log 81a4394.. -- .autome/` 无输出，无会话改动。
- 负向对照一：向 `protocol/prompts/impl.md` 追加一行，`git diff --exit-code` 退出 1；已还原。
- 负向对照二：把 `uses_bug_loop` 改成对所有任务为真（需求任务进 Bug Loop），全量出现 5 个失败（含 `bug_loop_legacy_bug_tasks_already_in_feature_nodes_keep_the_feature_table` 与既有 e2e/scheduler 用例）；已还原，工作区干净。
