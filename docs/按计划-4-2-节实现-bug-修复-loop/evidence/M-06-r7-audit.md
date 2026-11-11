# M-06 审计复验（r7）

- 验收命令：`cargo test -p automed --test end_to_end bug_regression_guard`（4 passed）、`cargo test --workspace`（1075 passed / 0 failed，= M-05 基线 1071 + 4）、clippy `-D warnings`、`cargo fmt --check` 全部通过。
- 负向对照 1：把 `deliver_bug_outcome` 里的 `guard_bug_regression` 调用换成恒 Ok → 专项 4 个全部 FAILED（含拒绝修改/删除/重命名用例）。已还原。
- 负向对照 2：`regression_file` 改成 `named || inline || script || true`（保护集扩大到所有产品文件）→ 专项全红，含 `allows_product_only_fixes`。已还原。
- 代码走读：定位起点 head 在进程启动前写入 `session.started`；保护集按成员仓库取 `AM` 且 `--no-renames` 的文件，记录 ls-tree 的 blob/mode；修复轮核对 HEAD 的 tree 条目与工作区 diff，删除/重命名/改模式/改内容均拒绝；空保护集在修复轮拒绝；快照取 `last_event`，重新定位建立新快照；需求与旧 Bug（无快照）不启用。
- 还原后 `git status` 干净（scheduler.rs 与原件一致）。
