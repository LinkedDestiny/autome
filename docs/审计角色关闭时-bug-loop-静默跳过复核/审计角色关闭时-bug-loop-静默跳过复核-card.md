## 现象
Bug Loop 的复核（fix_review）是计划 §4.2 的必经节点，并要求与修复用不同模型（SAME-MODEL）。但 `crates/autome-domain/src/bug_loop.rs:296`：审计角色被关闭（`config.is_enabled(Role::Audit)` 为假）时，修复门禁通过后直接 `core(Rebase)`，复核被静默跳过，没有任何事件或提示；SAME-MODEL 约束随之落空。
## 复现步骤
1. 项目或全局配置里关闭审计角色。
2. 创建一个 Bug 任务，让定位、复现检查、修复、修复门禁依次通过。
3. 任务直接进入 rebase，账本里没有 `fix_review` 会话。
## 期望行为
- Bug 任务在审计角色关闭时拒绝启动（`task.create`/`task.start` 报「Bug Loop 需要审计角色做独立复核，请先启用」），或至少在进入修复门禁通过分支时写一条 `bug.review_skipped` 警告事件并在 `autome status` 与 `run` 输出里可见。设计轮二选一，并在证据里说明理由（倾向前者：静默跳过不可接受）。
- 补 Bug 任务专用的 SAME-MODEL e2e：修复与复核角色配置成同一模型时被拒绝（现有 `a_same_model_collision…` 只覆盖需求任务）。
## 环境
main，`crates/autome-domain/src/bug_loop.rs:290-300`、`crates/autome-domain/src/{role.rs,config.rs}`、`crates/automed/tests/end_to_end.rs:1084`。
