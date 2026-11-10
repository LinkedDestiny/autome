# M-05 r5 审计复验

结论：**通过**（无 reopen）。

## 负向对照（先做）
- `task_create` 的 `draft` 缺省改成 `true`：`cargo test -p automed --test end_to_end` 大面积 FAILED（因没有会话启动），验收能拦住。已还原，`unwrap_or(false)` 在位。
- 金样 `intake.md` 追加一行：内容门禁 `git diff --quiet` 返回 1；还原后返回 0。

## 验收命令
- `cargo test --workspace` 全绿：306 + 577 + 59 + 11 + 7 + 17 + 8 = 985 passed，0 failed，与契约下限一致（基线 944 + 41）。
- `cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 退出 0；`protocol eval` 8 通过 0 不通过 1 警告（既有静态警告）。
- 验证缺口说明：本 worktree 的 git 历史（`initial`）不跟踪 `crates/ protocol/ apps/`，字面的 `git diff f20fe14 -- …` 把所有文件看成已删除，必然 rc=1，与产品无关。改用临时索引（`read-tree f20fe14` + `add -A`）做同样的内容比对：`protocol/`、`apps/`、金样 `intake.md` 无差异；四个集成测试文件删除行 0（只追加 349 行）；`crates/*/src` 中被改动的既有测试行只有 `assert_eq!(SCHEMA_VERSION, 4)`，属设计 §8 允许。
- 基线 56 个 e2e 在 59 个里全过。

## 新造检查
临时索引内容比对与上面两个注入，均已还原；无需留存。工作区除任务文档外无源码改动。
