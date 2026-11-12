# 审计 #5 复验（M-03 rebase 后，已关闭里程碑）
- 负向对照：dispatch.rs `task_convert` 的 `source.kind != Bug` 改为 `false` → `task_convert_rejects_a_feature_task` 失败（1 failed / 3 passed）；已还原，git 工作区干净。
- 还原后 `cargo test -p automed task_convert`：9 通过；e2e `bug_loop_not_a_bug_task_convert_…` 1 通过。
- 未重跑 Web 全量与 Electron（沙箱监听受限，沿用 H-01 宿主确认）。
