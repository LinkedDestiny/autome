# 审计 #17 · 集成复验（实现 #18 rebase 冲突）

结论：**通过**，里程碑表不动（M-01—M-16 均已完成）。

## 1. 改动是否只针对触发原因；消失提交是否丢行为

- 冲突文件只有 `crates/automed/tests/end_to_end.rs`；实现轮另外调整了 cli.rs、write-gate.test.ts、technical-design.md、store.rs（均为同一 rebase 的合并冲突或 schema 顺延，见 impl 证据）。
- 5 个消失提交（a31a5cf、39bc973、20e5821、777c156、f8a32a7）逐个与 rebase 后对应提交（9bbec3b、35a8b5f、89f4d18、d1f7acc、ed2d2e0）比对非文档 diff 的 +/- 行：前四对完全一致，第五对（onboarding）只差 schema 7→8 顺延到 8→9（`SCHEMA_VERSION`、`version < 9`、`sessions_v9`、`user_version = 9`、测试名与断言）。没有丢产品行为。`.autome/output/r18-audit-patchdiff.txt`。
- main（fcabc5e）带入的 `review_scope_assert_failure` 助手与「失败 + 只能修改」断言在 HEAD 不存在：这是原分支 M-01（T-69 越界自愈，已验收）本来就改写过的断言（ORIG_HEAD 35ac036 里同样如此），新语义是还原后重修；`review_scope_*` 仍断言越界编辑被还原（`session.scope_restored.repos[0].paths == [REVIEW_SCOPE_OTHER]`）、不被采纳。不是本次 rebase 削弱。
- 测试函数名集合：main ∪ ORIG_HEAD 的 end_to_end / cli / store / write-gate 中，HEAD 缺的只有上述助手、被改名的 `onboarding_session_migration_v8_keeps_existing_rows`（现 `_v9_`）和被提升可见性的 `remove_db`。

## 2. 测试 diff

未发现断言削弱。store.rs 的迁移测试改成共用 `onboarding_migration_keeps_existing_rows(version)`，v7 与 v8 起点各一条，并增加 `kernel_mark` 保留断言——只增强。

## 3. 冲突标记

16 个列出提交逐个执行 `git grep -nE '^(<<<<<<<|>>>>>>>) ' <提交> -- . ':(exclude)docs/内核补齐批次-2026-11-13-4/'`，全部为空。

## 4. 验收与自造检查

| 命令 | 结果 |
|---|---|
| `cargo test -p automed -- protocol_integrity_ the_version_in_force`（M-16 验收） | 8 通过（库 2 + e2e 6），退出 0 |
| `cargo test -p automed --test end_to_end -- review_scope_ scope_restore_ kernel_` | 23 通过 |
| `cargo test -p automed onboarding` | 库 17 + e2e 1 + 金样 1 通过 |
| `cargo test -p autome-cli --test cli -- kernel_ operator_note_` | 10 通过 |
| `npm run test --prefix apps/web -- test/write-gate.test.ts` | 54 通过 |

M-16 契约负向对照（全部注入后红、已用备份/`git checkout` 还原，`git status` 干净）：
- 副本哈希不去掉 `evals/`（`task_copy` 改为不过滤）→ `protocol_integrity_version_hash_equals_a_fresh_task_copy_hash` 红。
- 去掉告警去重 → `protocol_integrity_edited_copy_warns_exactly_once_over_two_sessions` 红（2 ≠ 1）。
- 旧口径迁移一律视为不可核实、重建基线 → `…legacy_ref_is_migrated_once_without_warning` 与 `…legacy_ref_with_an_edited_copy_still_warns` 均红。

本轮自造（集成）：schema 迁移条件由 `< 9` 改 `< 8` → `onboarding_session_migration_from_v8_preserves_kernel_mark` 红（`no such column: project_id`），v7 起点用例仍绿，说明新增 v8 用例确实区分错误实现；还原后绿。
未跑全量门禁（由宿主合并前运行）。宿主快检在 899b860 上通过（简报）。

## 提醒（非缺陷）

H-01 人工验收与既有 D-1 eval 缺失仍按前序记录处理；本轮不改。
