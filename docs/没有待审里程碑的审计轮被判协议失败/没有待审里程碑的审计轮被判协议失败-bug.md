# 没有待审里程碑的审计轮被判协议失败

status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p automed guards::tests::rebase_repair_with_all_milestones_done_does_not_require_empty_audit_evidence -- --exact
根因: 全部里程碑已完成的冲突修复仍被送入审计，而审计证据守卫无条件要求本轮证据，导致合法空审计被转为协议失败。
复核: 待复核
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

定位轮仅新增回归测试和本文档，未修产品代码。修复轮次为 0/3。

## 复核意见

待独立复核。
