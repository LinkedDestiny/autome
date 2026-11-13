# M-11 r12 审计复验

- 验收命令（独立运行）：域 5 + automed 库 32（gate_queue_ 24 + gate_lane_ 8）+ end_to_end 9（6+3）+ CLI 1 = 47，全部通过，与设计预期的 47 一致。
- 负向对照 1：把 `admit` 的占用计数改回全局（`held_in_lane(ctx, None)`）→ 库 `gate_lane_restart_recovers_both_queues`、`gate_lane_setup_cannot_transfer_its_heavy_slot_to_light` 红；e2e `gate_lane_e2e_quick_runs_beside_a_sleeping_merge_gate` 红（E-V1）。已还原。
- 负向对照 2：域 `position` 去掉同车道过滤 → `gate_lane_position_counts_only_the_same_lane`、`gate_lane_admission_counts_held_per_lane` 红。已还原。
- 配置：`config_io` 的 `quick_concurrency` 解析边界用例存在并覆盖 1..=10 与越界报错文案。
- 还原核对：`git status` 干净，产品代码无差异。
- 结论：通过。
