# M-02 独立复验（审计 #2，对应实现轮 r2）

结论：通过。

- 验收命令 `cargo test -p automed --lib`：676 通过（契约 655+21）；`scheduler::tests::gate_queue_` 21 个全过。全量 `cargo test --workspace` 全绿（domain 393 沿用 M-01 审计已说明的基线笔误；automed lib 676；e2e 106 等），`clippy --workspace --all-targets -D warnings` 干净。
- 负向对照（先于其它检查，逐个注入后还原，工作区干净）。脚本 `.autome/output/mut.sh`：
  | 注入 | 结果 |
  |---|---|
  | 占用去掉规则 (b)（已结束未收不占名额） | 3 个失败（含 finished_uncollected_blocks_earlier_task） |
  | `held` 只数 Active 任务的活作业 | paused_live_job_holds 失败 |
  | `perform` 直接写 `bug.fix_round_started` | fix_round_is_counted_once 失败 |
  | 忽略准备→门禁的连占（continuation） | setup_and_gate_keep_one_slot 失败 |
  | 不做全任务超时回收 | active_unreachable_jobs_timeout 失败 |
  | 不做旧 head 清扫 | 2 个 stale head 用例失败 |
  | 目录读取失败按 0 计 | unknown_directory 失败 |
- 契约中「排队事件不打断同因计数」的注入点：`any_event_after` 仅在 scheduler.rs:3378 以 `gate.started/session.started` 为列表，实现未把 `gate.queued` 加入；对应用例通过。未另行注入。
- 无产品行为缺陷。判别检查为一次性变异，不留存。
