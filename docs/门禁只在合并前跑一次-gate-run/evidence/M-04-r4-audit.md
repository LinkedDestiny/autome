# M-04 r4 审计证据 · 审计 #4

结论：通过。

## 验收命令（自己跑）
- `brief` lib 33（基线 30 + 3）、`prompt_rendering` 17（基线 16 + 1）、e2e `brief_before_merge` 1、`a_failing_gate_sends…` 1，全部通过。
- 补跑 autome-domain 390、automed lib 663 通过。

## 负向对照（注入后已还原）
| 注入 | 结果 |
|---|---|
| `gate_evidence` 去掉 sha 比较 | lib `before_merge_missing_stale_or_unreadable_head_has_no_result` 失败；e2e `brief_before_merge_audit_brief_never_shows_a_stale_result` 失败（简报出现失败那次的 sha / 「未通过」） |
| `each_round` 也走新规则 | lib `before_merge_each_round_and_unconfigured_gate_are_byte_identical` 失败 |
| 常量文案改一个词 | 金样 `before_merge_audit_brief_matches_its_golden` 失败 |

## 代码核对
- `gate_line_of_result` 从 scheduler 原样搬到 brief，文字未改；`each_round` / 无门禁路径是 `last.and_then(gate_line_of_result)`，与此前逐字节相同。
- 仅审计角色传入 `gate_evidence`，其它角色 `None`。
- 同 sha 保留成功与失败两种结果有单测；e2e 覆盖「MergeGate 失败后修复轮有新提交」的旧结果不被引用。
