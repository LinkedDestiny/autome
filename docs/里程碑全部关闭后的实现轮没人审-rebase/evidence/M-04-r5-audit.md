# M-04 r5 独立复验（审计 #4）

结论：通过（含一处验证缺口，已就地补强）。

## 验收命令（自己跑）
退出码 0：`integration_e2e` 4 通过（基线 0）；`before_merge` 11 通过；`a_gate_failing_after_the_automatic_rebase…` 1；`a_failing_gate_sends_the_work_back` 1；`end_to_end.rs` 相对 merge-base 无被删/改的旧行。
宿主快检在 63fe8719 通过，仅引用。

## 负向对照（均已还原，git status 干净）
1. `apply_guards` 构造 `Round` 时 `integration: None`：`integration_e2e_audit_without_evidence_stops_with_a_readable_reason` 失败。拦住。
2. 契约原写法：集成简报段 `short()` 的 `take(7)` 改 `take(12)`：`brief_before_merge_audit_brief_never_shows_a_stale_result` 与 `integration_e2e` 4 个**仍全部通过**——契约这条对照是惰性的。原因：门禁修复轮的范围起点 H0 取在简报提交之后，失败提交 SHA 不出现在集成段里；7 位约束实际由 M-03 的 `integration_section_contains_facts_and_seven_character_shas` 拦。
   这是验证缺口（产品行为没错：段里确无失败 SHA 与「未通过」）。改契约：对照换成「触发原因行写出『：未通过』」，实测 `brief_before_merge_audit_brief_never_shows_a_stale_result` 失败，还原后通过。
3. e2e 只增不改检查是 diff 机械判定，无需注入。

## 判别检查
上面三项即本轮自造检查；无实现缺陷。
