# M-02 第 2 轮审计复验

结论：通过。

## 验收命令（自跑）
- `cargo test -p automed --lib dispatch::tests::task_card`：12 passed，0 failed
- `cargo test --workspace` 全量：全部 ok、0 failed（autome-domain 306、automed lib 572 即基线 560 + 12、集成 56 等）

## 负向对照（注入、确认失败、还原；cmp 确认已还原）
| 注入 | 失败的用例 |
|---|---|
| 上限判断改 `>= 65536` | `a_brief_of_exactly_64_kib_is_accepted_and_one_byte_more_is_refused` |
| `create` 忽略 draft（恒 Queued 并 tick） | 草稿不开始/不排队、task.start 排队、retro 拒绝草稿 共 3 个 |
| `task.created` 存储载荷加 `kind` | `create_without_the_new_params_is_unchanged` |

## 代码核对
`task_json` 带 `kind`（project.get/task.get/dashboard.get 共用）；`task.start` 已注册、不在 READ_METHODS；
校验顺序：kind、brief 非空与 ≤65536 字节、draft 布尔、标题推导。无产品缺陷。人工验收项：无。
