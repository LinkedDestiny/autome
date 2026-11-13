# M-03 r4 独立复验（审计 #3 的后续：审计 #4 序列外，本轮为 M-03）

结论：通过。

## 验收命令（自己跑）
- `prompt_rendering`：35 通过（基线 31，+4）。
- `integration_section`：6 通过（brief 3、launcher 2、eval_run 1）。
- `protocol eval protocol/`：退出码 0。
- `git diff --quiet --diff-filter=MD <merge-base> -- crates/automed/tests/golden/`：退出码 0（既有金样无改删）。
- 宿主快检（fmt/clippy/web lint/typecheck）在 b5f2b53f 通过，仅引用。

## 负向对照（均已还原，工作区干净）
1. `short()` 的 `take(7)` 改 `take(10)`：`integration_section_contains_facts_and_seven_character_shas` 失败。
2. 去掉统计段对完整 SHA 的替换：同一用例失败。
3. `select_integration_audit` 恒保留集成块（等价于模式选取失效）：`before_merge_role_prompts_match_their_goldens`、`every_role_prompt_renders_exactly_what_the_constants_did`、`the_budget_placeholder_…` 3 个失败。
验收命令能拦住上述错误实现。

## 判别检查
上面三项即本轮自造检查；无实现缺陷、无验证缺口。
