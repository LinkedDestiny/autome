# M-05 · 实现 #7 · 审计复验

结论：通过。

## 验收命令
`cargo test -p autome-cli deps_cli_ && cargo test -p autome-cli --test cli`：bin 单测 3 个、cli 集成 4 个通过；`tests/cli.rs` 共 28 个通过（基线 24 + 4），含 `cost_cli_ls_appends_cost_and_a_footnote`。

## 负向对照（均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| `ls` 排队段改按任务 id（创建序）排序，不用 `queue_position` | `deps_cli_queue` 失败 |
| `set_after` 允许 `--none` 与列表同时出现 | `deps_cli_after` 失败（退出码 2 断言） |
| `task new --after` 改成先 `task.create` 再 `task.set_after` | `deps_cli_create` 失败（多出 `task.after_set`），连带 `deps_cli_queue`、`deps_cli_status` 失败 |

## 自造检查
读 diff 核对：`queue move` 开关个数、动词名、空参照、`--after` 仅限 `task new`、`status` 排队段与依赖行、`ls` 文本与 JSON 同源（均取自内核 `after`/`blocked_by`/`queue_position`）。未发现与设计 §7 不符之处。
