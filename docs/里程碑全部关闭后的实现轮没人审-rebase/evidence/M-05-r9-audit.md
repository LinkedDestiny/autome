# M-05 r9 审计（集成复验：门禁修复轮）

对象：提交 51542801，仅改 `crates/automed/tests/end_to_end.rs:1951` 一行，`protocol/v7@` → `protocol/v8@`。

- 事实核对：`protocol/CHANGELOG.md` 有 8 个 `## protocol/v` 标题（最新 v8），与设计 §10 发布 v8 一致；期望值改动符合版本事实，保留完整引用相等断言。
- 本轮运行：宿主失败用例 `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task` 通过（1/1）。
- 负向对照：临时把字面量改回 `v7@`，该用例在 end_to_end.rs:1951 失败；已 `git checkout` 还原，工作区干净。
- 无冲突标记；除该一行外无产品或测试改动。
结论：通过。无实现缺陷、无验证缺口。
