# M-15 独立复验（审计 #15，针对实现 #16）

结论：通过。

## 验收命令
`cargo test -p automed onboarding`：库 16 + end_to_end 1 + prompt_rendering 1 = 18 个通过，与契约预期 18 一致，0 失败。

## 负向对照（临时注入，均已还原，`git status` 干净）
- H·先写状态后提交（`update_project_onboarding` 挪到提交前）：`workspace_onboarding_commit_failure_keeps_state_and_can_retry`、`…single_repo_index_lock_keeps_state` 红，另两条绿，符合契约。
- H·仍在工作区根 `git add`（成员列表换成项目根）：skip、finish、failure 三条红（single_repo 单仓库路径不受影响仍绿）。契约只点名 skip 红，实际更严。
- G·`start_onboarding` 不写 sessions 行：`…writes_a_row_and_start_event`、`…is_reaped_with_session_ended`、`…log_is_readable`、`…project_get_reports_latest`、`…costs_stay_out_of_tasks` 5 条红。

## 自建核对
- 迁移：`store.rs` v8 重建 sessions，task_id 可空、新增 project_id，CHECK `(task_id IS NULL) = onboarding`，索引保留，`SCHEMA_VERSION = 8`；迁移保留既有行由 `…migration_v8_keeps_existing_rows` 钉住。
- 入口分流：`all_running_sessions` 唯一调用方是收割器，已按 Onboarding 分流到 `reap_onboarding`；`session_log` 对 Onboarding 按 project_id 取根；收割报告不把 Onboarding 计入 tasks_advanced。
- `onboarding_step` 顺序：提交成功后才写状态与事件；失败错误码为 rejected，文案符合设计。
- 不跑全量；宿主快检已在 f8a32a7f 通过。
