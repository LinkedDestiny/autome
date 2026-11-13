# M-10 审计复验（实现 #11 → 审计 #10）

## 验收命令
`cargo test -p automed -- closed_pointer_ legacy_pointer_ && cargo test -p autome-domain protocol_ref_version && cargo test -p autome-cli legacy_pointer_`
结果：automed 库 9 + end_to_end 3 = 12，域 1，CLI 1，合计 14，全部通过，与预期一致。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| 模板缺标记时 `integration_for_audit` 返回 `None` | 库 `legacy_pointer_integration_payload_…` 红；e2e `legacy_pointer_e2e_v7_…` 红 |
| `restore_closed_pointers` 返回整篇 `before` | `closed_pointer_restore_rewrites_only_the_pointer_lines` 红 |
| 只对 v8 还原（v7 不还原） | e2e v7 红（v8 用例同红） |

fixture `protocol-v7-audit.md` 不含 `integration-audit` 标记，确为旧模板。
## 结论
通过。未造新的判别检查；无产品缺陷。
