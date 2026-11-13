# M-13 独立复验（审计 #13）

结论：通过。

## 验收命令
- automed：库 3（`task_changes_status_`）+ 2（`events_by_subject_`）+ 1（`session_ended_vanished_…`）= 6，end_to_end 7（task_changes_status 2、events_by_subject 1、session_ended_e2e 4）→ 合计 13，与契约一致。
- autome-cli `session_ended_run_json_prints_the_event` 1 个通过。
- `read-gate.test.ts` 16 个通过（既有 15 + 1）。

## 负向对照（均已还原，git status 干净）
- `session.ended` 不在 `stop_running_session` 写：`session_ended_e2e_stopped` 红。
- `events.since` subject 过滤改用载荷 `task_id`：`events_by_subject_filters_before_pagination…` 红。
- 契约字面的「去掉 `-M`」：测试**仍绿**——git 默认 `diff.renames` 开启，`-M` 冗余，该对照字面上无牙。改用 `--no-renames` 后 `task_changes_status_all_types_and_unusual_paths` 红，说明 renamed 断言确实有判别力。产品行为正确，记 Backlog（契约措辞）。

## 自造检查
- 读 `record_session_ended`：两个写入点共用一个函数、按 `session.ended` 去重、subject 与 started 一致、映射 exited/crashed/stopped/timed_out/Vanished 与设计一致。
- 读 `task_changes` 汇总：`deleted_count`、`merged` 字段存在。

产品行为符合 F-R3/R4/R5/V3–V6，无实现缺陷。
