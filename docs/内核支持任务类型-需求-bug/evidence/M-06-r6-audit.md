# M-06 审计 #6 独立复验

- 验收命令 `cargo test -p automed --lib dispatch::tests::task_card_read`：3 passed, 0 failed。
- 负向对照（改 `task.get` 的 `brief` 字段后还原，`git diff` 确认无残留）：
  - `.trim()`：字节逐一断言与归档用例失败（2 failed）。
  - 返回空串：同上 2 failed。
  - `unwrap_or_default()`（无正文返回 "" 而非 null）：`missing_brief_is_explicit_null` 失败。
  三种错误实现都被拦住。
- 全量 `cargo test --workspace`：17+8+306+580+59+11+7 = 988 passed，0 failed；automed lib 580（≥ 门禁下限 577 起算 M-06 的 +3）。
- 代码核对：`task_json` 未带 brief，project/dashboard 用例断言无 brief；取消归档后仍读 `task_briefs` 表。
- 结论：通过，无人工项。
