# M-16 r17 审计复验（审计 #16）

结论：通过。

## 验收命令
`cargo test -p automed -- protocol_integrity_ the_version_in_force`：库 2 + end_to_end 6 = 8 个全过，与契约预期一致。
`cargo fmt --all -- --check` 通过；`cargo clippy -p automed --all-targets -D warnings` 通过。

## 负向对照（均已 `git checkout` 还原，工作区无产品代码差异）
| 注入 | 结果 |
|---|---|
| `current()` 改回 `files.hash()`（含 evals） | 库 2 个红 |
| 去掉告警去重（`if true`） | `…warns_exactly_once_over_two_sessions`、`…legacy_ref_with_an_edited_copy_still_warns`、`…edit_after_migration_still_warns` 红（3 个） |
| 旧任务迁移时无条件用副本现算哈希做基线 | `…legacy_ref_with_an_edited_copy_still_warns` 红 |

## 逻辑复核
- `task_copy()` 同时用于 `current()` 哈希与 `copy_into_task`，两边同一函数；边界（空集、`evals-other.md` 不被误剔、`contract.toml` 保留）有库测。
- 迁移分三类与设计 §9 一致：可验证旧任务写可信预期哈希而非现算哈希；不可验证者带 `limitation` 重建基线；每任务至多一条 `protocol.hash_scheme`（事务内二次检查）。
- 其它 `.hash()` 使用点只有 `rules_hash`（与协议版本无关），无遗漏口径。

未发现产品缺陷；无新增判别检查需保留。
