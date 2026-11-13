# 实现审计

## 审计 #4 · M-04 · 通过

- 验收命令：queue_move_ 7 通过（lib 6 + e2e 1），task_start_queues_a_draft 1 通过，task_card_a_started_draft_waits_behind_the_parallel_limit 1 通过。
- 负向对照四条（只给被移动者赋号不重编号、task_start 无条件 mark_queued、重编号波及别的项目、拒绝未移动的待开始参照）均使指定用例失败，已还原，工作区干净。
- 新造判别检查：无新增；核对了 `queue_move_rejects_without_changes` 已覆盖参数互斥、非法类型、跨项目、全部不可移动状态且无 task.moved 事件。
- 证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-04-r6-audit.md
- 全量门禁未跑，由宿主合并前承担。
