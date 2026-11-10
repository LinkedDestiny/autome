# M-05 实现轮 #10

## 范围与修复

- 本轮仅推进最小开放里程碑 M-05。上一轮 M-04 审计通过，无退回缺陷或需逐字迁入的审计检查。
- `crates/autome-cli/src/main.rs` 的 `idle` 将 `Requeued { .. }` 与 `Queued` 同列为待办，防止 `run --until-idle` 提前退出；不改变未来重试自身的既有空闲语义。
- 同文件新增两个真实内存 SQLite 账本单元测试，调用生产 `idle`；无环境变量、外部服务或固定等待。核对 CLI 全部 `TaskState::Queued` 生产匹配，仅此一处。

## 红绿证据与验收

- 红：先只加入测试，保持生产判定不变，执行 `cargo test -p autome-cli idle_tests`。2 个用例均失败：单任务在 `Requeued { resume: Implement }` 断言失败；冷却组合在 `resume queue must keep the driver alive` 断言失败。原始输出 `.autome/output/m05-red.txt`。这也验证移除本轮 Requeued 判定会被新增用例拦截。
- 绿：`cargo test -p autome-cli`，16 个用例通过（binary 12 + integration 4），相对上一轮 14 个增加 2 个，均来自 `idle_tests`，旧测试未改。输出 `.autome/output/m05-cli.txt`。

## 标 `待审` 之前的自审清单

1. **情形表逐行**：M-05 点名两种情形均执行：并行上限 1、一个 Active 实现任务已占槽且重试时间在未来、另一个 Audit 恢复任务排队，`idle=false`；无任务时 `idle=true`。补充对照：仅未来冷却任务时为 true，加入恢复排队后 false，取消恢复排队后 true；均通过。
2. **失败分支**：不适用。M-05 没有点名授权拒绝、读盘失败、超时或外部服务不可达分支，本轮不改这些语义。空输入按域边界测试，已通过。
3. **全部迁移**：本轮不新增状态机迁移，检测的是账本快照。已执行无待办→恢复排队→取消恢复排队的 idle true→false→true；单任务 Queued、Requeued（Implement、Gate）、Active Gate 均为非空闲，Paused Implement、Stopped Audit、Cancelled 均为空闲。Audit 恢复排队在组合用例覆盖。原恢复状态机迁移由工作区既有测试复验。
4. **域边界**：空账本、单任务、两任务组合均通过；槽位上限取 1 并断言占用数恰为 1；重试时间取远未来 `9999-12-31T23:59:59Z`，排除测试运行速度影响。没有字符串数值、日期解析等新输入域，前导零/闰日等不适用。

## 验证边界

本轮以生产 idle 的真实账本单测证明退出谓词，CLI 既有集成用例和工作区测试验证整体回归；未声称本轮新建了真实进程的冷却等待验收。H-01 至 H-03 桌面人工验收仍由用户合并前确认，宿主 `sh scripts/ci-local.sh` 留给 Autome 执行。

## 最终验收结果

- 完整验收命令按契约依次执行：`cargo test -p autome-cli && cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`，退出码 0。
- 工作区总数 916，相对 M-04-r9-audit 的 914 增加 2，全部来自本轮 CLI 单元测试；916 通过、0 失败、0 忽略，无删减旧用例。输出 `.autome/output/m05-workspace.txt`。
- clippy 无告警，fmt 无差异，`git diff --check` 通过；输出 `.autome/output/m05-clippy.txt`、`.autome/output/m05-fmt.txt`。
- 四项自审完成，M-05 标为待审，待独立审计关闭。
