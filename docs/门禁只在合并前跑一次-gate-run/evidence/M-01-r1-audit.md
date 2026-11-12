# M-01 r1 审计复验

结论：通过。

## 负向对照（先于验收命令）
| 注入 | 结果 |
|---|---|
| `task.rs` 实现轮结束处改回 `gate.is_some()` | `before_merge_implement_with_audit_equals_no_gate`、`..._without_audit_...` 失败，已还原 |
| `config_io` 对非法 `run` 不报错（回落 EachRound） | `before_merge_invalid_run_is_rejected`、`before_merge_empty_command_still_validates_run` 失败，已还原 |
| 自造：rebase 处（1262）/ 合并处（1293）改成 `gates_each_round()` | 各自使 `before_merge_rebase_and_merge_keep_full_gate_nodes` 失败，已还原 |
| 自造：`resolve()` 忽略 `project.gate_run` | `before_merge_defaults_override_and_json_compatibility` 失败，已还原 |

验收命令连错的实现都拦得住。工作区在每次注入后 `git status` 干净。

## 验收命令
`cargo test -p autome-domain before_merge && cargo test -p autome-domain && cargo test -p automed --lib config_io`：
过滤 6 个通过；autome-domain 390 通过（基线 384 + 6）；集成 1 个默认忽略（t14，既有）；config_io 45 通过（基线 40 + 5）。
automed lib 全量 655 通过（基线 650 + 5，差全部来自 config_io 新增）。e2e 与 prompt_rendering 与本里程碑无关，由宿主门禁承担。

## 判定
产品行为符合设计 §1、§2：缺省 each_round，非法值报中文错误，before_merge 下实现轮转换与无门禁相同，MergeGate/MainCheck 不变。无缺陷、无验证缺口。
