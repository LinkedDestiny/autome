# M-01 r1 审计复验

## 负向对照（均已还原，`git status` 干净）
1. `scope::restore` 不处理会话新增的越界文件 → `scope::tests::restore_*` 10 个红（含 `restore_reverts_modified_added_and_deleted_paths_and_keeps_docs`）。
2. `restore_session_scope` 直接失败（等同「不还原」）→ `scope_restore_feature_audit_edit_is_reverted_and_handed_to_next_impl` 红。
还原后该 e2e 用例复跑通过。

## 验收命令（自己跑）
域 `scope_patch` 13；automed 库 19；end_to_end 16；autome-cli `scope_` 4；合计 52，与契约预期一致。

## 判别检查
`git diff HEAD refs/keep/t69-scope-self-heal -- crates apps protocol` 为空：产品代码与测试与原 T-69 ref 逐字一致，断言未改。
快检门禁（宿主，a31a5cfa）已通过。结论：通过。
