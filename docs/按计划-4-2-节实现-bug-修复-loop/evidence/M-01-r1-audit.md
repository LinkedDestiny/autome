# M-01 第 1 轮审计独立复验

结论：通过。

## 验收命令（自己跑）
- bug_loop 29、bug_doc 8、bug_fix_rounds（domain）2、（automed --lib）3，均 0 failed；`cargo test -p autome-domain` 345 passed；`cargo build --workspace --all-targets` 通过。
- `cargo test --workspace` 全绿（automed 585、domain 345、其余套件均 0 failed）。输出在 `.autome/output/audit-accept.txt`、`audit-ws.txt`。

## 负向对照（逐条把实现弄错、跑测试、用 `git checkout` 还原；工作区回到干净）
| 注入 | 失败的用例 |
|---|---|
| Feature 的 `Queued` 不走需求表 | `bug_loop_feature_kind_matches_the_feature_table_exactly` |
| 超时也算复现 | `…timeout_and_unrunnable_commands_do_not_count_as_reproduced`、`…second_consecutive_miss_waits_for_info` |
| 预算判断 `>=` 改 `>` | `bug_loop_fix_budget_runs_out_after_the_configured_rounds` |
| 解析器只认半角冒号 | `reads_all_five_fields_with_either_colon`、`repos_is_optional_…` |
| Bug 节点加进 `Node::ALL` | `bug_loop_feature_node_list_is_unchanged`、`exactly_six_nodes_carry_a_configurable_role`、`requeued_dispatches_all_thirteen_nodes` |
| 防御挪到 `apply_inner` 第一行（控制段之前） | 第 4、12、29 号三条，与契约预言完全一致 |

六条注入全部被拦住，验收命令有区分力。

## 判定
未发现产品行为缺陷，也无验证缺口。无人工验收项。
