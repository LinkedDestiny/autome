# M-01 r1 审计复验

## 负向对照（先做，改后全部还原，`git status` 干净）

| 注入 | 结果 |
|---|---|
| `check_after` 的环检测只查直接边（不递归 `visit`） | `deps::tests::three_node_cycle` 失败（11 过 1 败）；`direct_cycle` 仍过，符合契约 |
| v6 块 `CREATE TABLE IF NOT EXISTS task_after` → `CREATE TABLE task_after` | `a_v4_ledger_migrates_to_v5_…` 失败：`table task_after already exists`（56 过 1 败） |

## 验收命令（还原后）

- `cargo test -p autome-domain deps::`：12 通过，与契约一致。
- `cargo test -p automed --lib store::`：57 通过（契约预期 57 = 基线 52 + 5）。

## 自己读源码核对

- `check_after`：先按输入项数判上限（9 个相同 id 也拒绝），再去重保序，再逐项自引用/不存在/跨项目，最后 DFS 成环并输出完整路径。
- `set_task_after`：删除不在新列表的边，保留边用 `ON CONFLICT DO UPDATE SET ord`，不触碰 `notified`。
- `create_task_after`：任务、简报、依赖同一事务；外键失败回滚用例已覆盖。
- `has_impl_session` 用的 `kind` 序列化与会话入库（`serde_json::to_string(&session.kind)`）同源。
- `reorder_queue` 只更新 `project_id` 匹配的行。

## 结论

通过。无实现缺陷，无验证缺口。
