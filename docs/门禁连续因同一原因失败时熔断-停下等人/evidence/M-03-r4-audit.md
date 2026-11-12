# M-03 审计 #3 独立复验（对应实现轮 r4）

结论：通过。

## 负向对照（逐条把实现弄错，确认验收会失败，随后 `git checkout` 还原，工作区已干净）
| 注入 | 结果 |
|---|---|
| `config_io.rs` 范围 `1..=10` → `0..=10` | `same_failure_config_invalid` 失败 |
| `last_event_in` 去掉按 `node` 过滤 | `same_failure_other_nodes_and_boundaries`、`same_failure_result_fields` 失败 |
| 关闭重放判断（`&& false`） | `same_failure_replay_tripped`、`same_failure_replay_untripped` 失败 |
| 查询只认 `gate.result`，忽略 `gate.reused` | `same_failure_reused_resets` 失败 |
| 计数只存内存 | 未单独注入；实现从账本 `last_event_in` 取值，无内存状态，`same_failure_database_reopen` 用文件库重开验证 |

## 验收命令
- `cargo test -p automed --lib same_failure`：12 通过。
- `cargo test -p automed --lib`：618 通过（606+12）。
- `cargo test --workspace`：全部通过（domain 374、automed 618、end_to_end 80、cli 19+11、prompt_rendering 16、real_cli 7，1 忽略），0 失败。
- 对 36890bf..HEAD 的 crates 改动，删除行只有 `run_gate` 里内联写 `gate.result` 的代码，已重构进 `record_gate_result`；已有字段（node/sha/ok/exit_code/command/log/tail）由 `same_failure_result_fields` 守住。

## 关于实现轮 #4 的「原端到端失败」
门禁整体通过本轮 workspace 测试复验；该环境阻塞（Web 监听被沙箱拒绝）属门禁环境问题，不是产品缺陷，由 Autome 门禁自行重跑。
