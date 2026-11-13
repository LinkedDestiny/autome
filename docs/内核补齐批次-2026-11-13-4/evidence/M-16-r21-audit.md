# M-16 r21 集成复验（审计 #20）· 通过

范围：`b2a5b64`（仅 crates/automed/tests/end_to_end.rs，+17/-2）。触发：合并前门禁 Test 步失败。

1. 改动目的：两处都只针对失败用例。
   - `the_merge_panel_offers_the_merge_when_only_main_has_moved`：旧断言合并后 `available=false`，与设计 M-13（F-R3：分支删除且有 merge_commit 时返回 `available:true, merged:true` 与改动清单）冲突；新断言 available/merged 为 true 且 files 非空，是对设计的对齐，不是削弱（断言更多、更严）。
   - `legacy_pointer_scenario`：夹具改为在 `await_design_approval` 停靠、确认无运行会话后再改旧协议模板并 approve，消除 resume_setup 启动的第 5 步会话 git commit 与夹具提交竞争。未改被测断言（failed / 恢复后状态等原样保留），只加了前提断言（无运行会话）。
2. rebase 中消失的提交：0 个，无产品行为丢失。产品代码无改动。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' b2a5b64 -- . ':(exclude)docs/...'` 为空。
4. 受影响验收（M-13 `task_changes_status_`）：2 个 e2e 通过；`the_merge_panel_offers…` 1 通过；`legacy_pointer_e2e` 3 个连跑 3 次全过（无竞争复发）。
5. 负向对照：把 dispatch.rs 的 `merged` 改成恒 false，`the_merge_panel_offers…` 在 end_to_end.rs:4837 红（left false / right true）；已 `git checkout` 还原，工作区干净，复跑转绿。
本轮新造检查（负向注入）通过后丢弃。未跑全量门禁（由宿主合并前运行）。H-01 仍待用户确认。
