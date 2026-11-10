# M-05 r10 审计复验

- 负向对照：把 `idle` 里的 `Queued | Requeued { .. }` 改回 `Queued`，`idle_tests` 两个新用例均失败（main.rs:1014、1039 断言），已还原（git diff 为空）。
- 验收命令全跑：`cargo test --workspace` 全绿（cli 12、domain 288、automed lib 545、e2e 50 等，0 失败）；clippy -D warnings 无输出；fmt --check 通过。
- 判据核对：未来冷却占槽 + Requeued → 非空闲；取消后 → 空闲；空表空闲；Paused/Stopped/Cancelled 空闲；Requeued 任意节点均待办。
- 结论：通过。
