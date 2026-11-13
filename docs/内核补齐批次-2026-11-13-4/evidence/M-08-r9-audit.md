# M-08 r9 审计复验 · 审计 #8

- 验收命令全部通过：automed 库 `operator_note_` 5；autome-cli 单元 3（另有 1 个 e2e 同名前缀 `operator_note_cli_records_and_status_displays_verbatim` 通过）；`write-gate.test.ts` 51。
- 负向对照（均已还原，`git status` 干净）：
  - 去掉 `PROSE_FIELDS['task.note']` → `task.note accepts prose…` 红（50/51）。
  - 8000 上限改 8001 → `operator_note_rejects_blank_and_over_8000_chars` 红。
  - `late` 判定去掉 `MainCheck` → `operator_note_after_merge_started_…warning` 红。
  - 序号 n 偏移 → `…recorded_in_order…` 与 `…task_get_lists_notes…` 红。
- 验收契约里「备注走 apply_trigger 改状态」的对照由 `…without_changing_state` 断言覆盖；`task_note` 源码只 append 事件，不触碰状态。
- 结论：通过，无新造检查需留存。
