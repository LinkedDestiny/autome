# M-06 审计 #6 独立复验

## 验收命令（逐条自己跑）
- prompt_rendering 20 通过；automed lib `protocol::` 67 通过；e2e `protocol_before_merge` 2 通过；`every_round_is_handed_a_brief…` 1 通过。
- `protocol eval protocol/` 退出 0：必含/禁含短语 88、eval 用例 19、CHANGELOG 10 条门槛齐全，0 项不通过（1 项既有警告）。
- 12 份现有金样相对 merge-base 未改（for 循环无输出）。

## 负向对照（改错 → 确认失败 → 还原，工作区已干净）
1. impl.md 的 before_merge 段删 `{gate_line}`：`before_merge_prompt_seed_segments_render_in_both_modes` 与 `before_merge_role_prompts_match_their_goldens` 失败。
2. audit.md `each_round` 段改一个字：`every_role_prompt_renders_exactly_what_the_constants_did` 等 2 个失败。
3. fix-review.md before_merge 段改回「独立复验复现命令与项目门禁」：`before_merge_bug_prompts_match_their_goldens` 与 e2e ② 失败。
4. CHANGELOG 去掉 `## protocol/v3` 标题：`every_round_is_handed_a_brief…` 失败。
5. 移走一个 behavioral eval 目录：`protocol eval` 因 include_str! 编译失败（退出 101，非契约写的 1，但同样拦住；更严）。

## 结论
通过。无产品缺陷，无验证缺口。
