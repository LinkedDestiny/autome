# M-01 r1 审计复验

- 负向对照 1：task.rs 实现轮结束判断改回 `gates_each_round()` → `gate_quick_implement_with_audit_enters_gate`、`gate_quick_implement_without_audit_enters_gate` 失败（14 过 2 败）。已还原。
- 负向对照 2：config_io 校验里「有 command」检查改为恒不触发 → `config_io::tests::gate_quick_requires_full_command` 失败（12 过 1 败）。已还原，`git status` 干净。
- 还原后验收命令：autome-domain 16 通过；automed config_io 13 通过，与预期一致。
- 实现轮对既有代码只改一行（`gates_each_round` → `gates_after_implement`），无现有用例被改或删。
- 未读实现轮证据，独立复验。
