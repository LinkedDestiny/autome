# M-02 r3 审计复验 · 审计 #3

结论：通过。

## 验收命令
`cargo test -p automed --lib guards && cargo test -p automed --lib integration_wiring` 退出码 0：guards 34 个通过（基线 24，+10），integration_wiring 5 个通过（基线 0）。用例总数与契约预期一致。

## 负向对照（均注入后还原，工作区无残留）
1. 集成复验保留「全已完成且表未变即免查」例外 → guards 3 个失败（含契约点名的 still_table_and_no_evidence、prewritten_evidence，另 evidence_of_an_earlier_round）。
2. 指针检查不限于轮前已完成里程碑 → closed_pointer_rules_leave_open_milestones_and_fences_alone 失败。
3. 不比对最近实现会话 id → integration_wiring_a_later_impl_session_supersedes_the_range 失败。
4. 证据只看存在、不比对 stamp（契约第 4 条的等价注入：文件存在即算证据）→ integration_wiring_evidence_stamp_is_taken_at_the_first_audit_attempt_and_detects_a_rewrite 失败；守卫层的 prewritten_evidence 用例在注入 1 中亦被拦。

## 判别检查
阅读 guards.rs / integration.rs / scheduler.rs 接线：apply_guards 传入 design_before（最近 task.head 处文档）、launch_session 调 integration_for_audit，版本门（模板须含独占行标记）在函数开头。无需新造检查，四条负向对照即判别检查。

## 结论
未发现产品行为缺陷。fmt/clippy 由宿主快检已通过（610f8b3，退出码 0）。
