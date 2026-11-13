# M-06 · r7 审计复验

结论：通过。
- 验收命令全绿：cli bug_loop_reply_ 3、域 bug_loop_reply_ 4、automed 库 bug_reply_after_interruption 0（命令按验收契约原样，预期 1 但库内无此名；预算与排队路径由域用例 `bug_loop_reply_restores_a_fix_window_without_losing_history` 与 cli 两条用例覆盖）、usage 用例 1。
- 负向对照：①预算不抬高（沿用 budget_n）→ `bug_loop_reply_restores_a_fix_window_without_losing_history` 红；②模式里去掉 Stopped → `bug_loop_reply_after_stop` 红。均已还原，git status 干净。
- 源码核对：limit = max(budget_n 或 B, fix_round + B)，溢出拒绝，空白拒绝，fix_round 不清零；其它状态给中文指引，无「需求转换表」。
- 注意：验收契约第三段 `bug_reply_after_interruption` 实测匹配 0 个（过滤后 0 tests），契约预期的 1 个未体现；行为已由其它用例覆盖，不属产品缺陷（验证缺口，已在审计文件备注，Backlog 建议）。
