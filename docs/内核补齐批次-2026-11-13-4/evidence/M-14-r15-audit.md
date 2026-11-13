# M-14 审计复验（实现轮 r15）

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| probe 路径下 `contains_project` 分支改为 `false` | `probe_rejection_matches_project_add_for_each_kind` 红 |
| `dashboard_get` 里 `project_warnings` 失败改 `return Err`（等价 `?`） | `dashboard_tolerates_a_broken_project_config` 红（首次注入类型不通过编译，改成带类型标注的版本后得到红） |
| `config_get` 去掉 `gate_run/gate_quick/setup` 补齐 | `config_get_reports_every_resolved_key_and_marks_defaults` 红 |

## 验收命令
`cargo test -p automed --lib -- probe_rejection_ config_get_ dashboard_tolerates_`：6 passed（与契约预期 6 一致）。
`cargo test -p autome-cli dashboard_tolerates_`：1 passed（预期 1）。
注入前后源码均与 HEAD 一致。

## 对照设计 G-R1/G-R3/G-R4/G-R5
`add_rejection` 为 project_add 与 project_probe 共用；`config_get` 含 `defaults`；dashboard 与 project.get 带 `config_error`。未见产品行为偏离。

## 结论：通过
