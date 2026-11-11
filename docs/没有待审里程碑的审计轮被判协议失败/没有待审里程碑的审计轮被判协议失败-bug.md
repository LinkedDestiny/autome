# 没有待审里程碑的审计轮被判协议失败

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed guards::tests::rebase_repair_with_all_milestones_done_does_not_require_empty_audit_evidence -- --exact
根因: 全部里程碑已完成的冲突修复仍被送入审计，而审计证据守卫无条件要求本轮证据，导致合法空审计被转为协议失败。
复核: 通过
repos: autome-v2

## 现象

任务卡描述 T-13 在 2026-11-10 21:29 的审计 #8 失败。本轮没有读取该历史运行日志；在当前基线 `69f901b91f71dfe47d4f8be88a2605e59c15deb4` 上，通过真实状态机和真实守卫稳定重现相同失败。

全部里程碑已完成，rebase 冲突退回实现；实现只修冲突，保留已完成表并写实现证据。门禁通过后仍启动审计。审计仅追加一行“无事可审”，没有新审计证据，结果为 `Failed { at: Audit, reason: Protocol { ... } }`，错误信息为“本轮没有留下证据文件”。

## 复现

从当前 worktree 根目录执行状态块中的命令。本项目是单 Git 仓库的 Cargo workspace，成员包为 `crates/automed` 和 `crates/autome-domain`，无需切换到其他仓库；`repos` 点名实际仓库 autome-v2。

回归位于 `crates/automed/src/guards.rs` 的测试模块：

- `rebase_repair_with_all_milestones_done_does_not_require_empty_audit_evidence`：两个已完成里程碑，冲突退回实现，检查实现产出合法，发送实现结束及门禁通过事件；若仍进入审计，则检查仅追加一行 retro、保留旧审计证据和本轮实现证据的产出，再将守卫结果交回状态机。最终必须进入复盘或 rebase，允许任务卡中的两种修复路径。
- `an_audit_that_closes_a_pending_milestone_still_requires_current_evidence`：开始时待审、结束时已完成，只有旧审计证据，必须仍返回 Error 级 `evidence_missing`。不能仅依据审计结束后的全完成状态免除证据要求。

实跑：精确回归命令退出 101，1 个测试失败；随后运行 `cargo test --offline -p automed guards::tests::` 再次命中同一回归失败，其余守卫测试通过。失败是业务断言触发，不是编译错误、超时或缺失命令。测试无网络、外部进程、时序依赖；门禁成功由领域事件输入，不实际执行门禁 shell。

原始输出：`.autome/output/locate-regression.log`、`.autome/output/locate-guards.log`。已运行 `cargo fmt --all` 和 `git diff --check`；未运行全仓测试或真实 tmux/Agent 会话。

## 根因分析

1. `crates/autome-domain/src/task.rs:1196` 附近：rebase 冲突无条件退回 Implement；实现结束有门禁时进入 Gate（1071 附近）。
2. `crates/autome-domain/src/task.rs:1320`：Gate 通过且 Audit 启用时，未检查所有里程碑已完成就启动 Audit。
3. `crates/automed/src/guards.rs:115`、`:136`：Audit 总是调用 evidence_file，以当前 impl_round 匹配审计证据后缀，没有无待审对象的例外。
4. 同文件 `:212` 的 audit_moved_the_table 明确允许无待审项时表不变，说明空审计已经是被认可的状态；但这个豁免不影响独立的 evidence_file 检查。原有全完成表测试提供了虚构的本轮审计证据，掩盖了缺口。
5. `crates/automed/src/scheduler.rs:587` 将 Error 守卫结果转成 GuardFailed；领域状态机再将其转成 Protocol 失败，尚未到达 Audit 全完成时的正常退出分支。

因此不是单纯“审计忘写文件”：内核调度了没有审计对象的轮次，却继续要求按里程碑命名的新审计证据。缺陷处于调度与产出约束的边界。具体采用跳过空审计还是允许空审计无证据，由修复轮决定；回归不强制其中一种。

## 修复说明

第 1/3 轮采用“允许合法空审计无证据”的最小修复：仅调整 `crates/automed/src/guards.rs` 的 Audit 分支。开始快照必须非空、全部里程碑已完成，且结束时的里程碑 ID/状态表与开始快照一致，才免除本轮审计证据要求。仍执行审计推进、retro 单行和编辑范围检查；实现轮的证据要求不变。

该判断依据开始快照，不能通过本轮把最后一个待审项改为已完成来绕过证据要求。快照缺失、空表或里程碑表变化也不适用豁免。保留现有调度路径，空审计经原状态机进入复盘或 rebase。

定位轮新增的两个回归测试均未修改。原精确复现命令修复前触发业务断言失败，修复后退出 0，目标测试 1 个通过。原始输出保存在 `.autome/output/fix-before.log` 和 `.autome/output/fix-after.log`。

验证结果：

- `cargo test --offline -p automed guards::tests::`：24 个通过，包括本轮关闭待审里程碑仍要求当前证据的反例；日志 `.autome/output/fix-guards.log`。
- 配置门禁 `sh scripts/ci-local.sh`：已完整运行，退出 1，用时 257 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过；Web test 未通过。
- Web test：19 个文件通过、3 个文件失败；198 个测试通过、11 个失败、16 个跳过。失败涉及 `gateway-http.test.ts`、`server.test.ts`、`eventbus-live.test.ts`，日志为 `listen EPERM: operation not permitted 127.0.0.1`，部分启动日志另含文件监听 `EMFILE`，导致服务启动失败或超时。当前受限环境不允许提权，未改测试或门禁规避环境限制；需要宿主在允许监听端口的环境重跑配置门禁。完整日志 `.autome/output/fix-gate.log`。
- `git diff --check` 通过。未运行可选 package 打包、真实 Agent 会话或浏览器人工验收。

## 复核意见

通过。
- 差异 8ce6a69..c345f92 仅改 `crates/automed/src/guards.rs` 的 Audit 分支（+14/-1），两个回归测试未改动、未跳过。
- 精确复现命令通过；`guards::tests::` 24 个全过，含反例 `an_audit_that_closes_a_pending_milestone_still_requires_current_evidence`；clippy `-D warnings` 无告警。
- 根因成立：空审计被调度，而证据守卫无条件要求本轮证据。豁免依据开始快照（非空、全部 Done、结束表与之一致），无法靠本轮关闭最后一个待审项绕过；retro 单行、推进与编辑范围检查仍保留。
- 未重跑完整门禁；修复轮报告 Web test 因沙箱 `listen EPERM` 失败，与本改动无关，需宿主环境重跑。
