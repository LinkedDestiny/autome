# M-16 r20 审计证据（集成复验，审计 #19）

范围：提交 c53b2b2（evals.rs / scheduler.rs / store.rs）。结论：**通过**，里程碑表不动。

1. 改动范围：只含三处——evals.rs 嵌入 `audit-checks-operator-notes` 三个文件（SEED 133→136，源文件存在于 protocol/evals/）；scheduler.rs 新增测试辅助 `seed_located_bug` 并在门禁/重入测试中调用；store.rs 迁移测试补 `downgrade_sessions_to_v7`、断言改为 `SCHEMA_VERSION`（当前 9，旧值 8 已过时）。无产品逻辑改动；无 rebase 丢失提交（简报 0 个）。
2. 测试弱化检查：scheduler 夹具反而更真实（写入定位轮真实产物并断言快照非空）；store 断言从硬编码 8 改为常量，迁移到当前版本，不放宽。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' c53b2b2 -- . ':(exclude)docs/…/'` 为空。
4. 验收命令 `cargo test -p automed -- protocol_integrity_ the_version_in_force`：2 + 6 = 8 通过。
5. 负向对照：把 evals.rs 还原为 3db886e（不嵌入新 eval）后 `cargo test -p automed --lib protocol::` 多条红（eval 嵌入完整性用例）；还原后 `protocol::` 72 通过，`store:: scheduler::` 268 通过，git 工作区无产品改动。
6. 全量门禁由宿主合并前运行；D-1 H-01 人工项仍待用户确认。
