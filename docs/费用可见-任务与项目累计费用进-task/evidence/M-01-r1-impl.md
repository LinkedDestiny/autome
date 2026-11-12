# M-01 实现轮 #1

## 范围与实现

新增 `CostSummary::of`、`RoleCost` 与 `SessionKind::cost_key`，只汇总已结束会话，缺失费用计 0 并单列计数。`task.get` 增加 `cost`；`so_far` 和终态指标保持原口径，仅补说明注释。未推进其他里程碑，未改现有测试断言。

## 验收与修复

- 验收命令：`cargo test -p autome-domain --lib cost_summary_ && cargo test -p automed --test end_to_end cost_rollup_`。
- 本轮无上一轮审计，不涉及审计检查搬迁。设计记录的过滤用例基线为 0，本轮新增 4 个 domain 单测和 1 个 e2e，共 5 个，差值 +5。
- e2e 初次准备错误地假定提供 brief 会跳过 intake，实际仍执行 intake，导致脚本错位并在 review 等待超时（`.autome/output/m01-e2e-initial.txt`）。修正为创建 draft 后在账本将任务置于 `Requeued { resume: Design }`，随后全部会话通过真实调度、Git worktree、包装脚本和模型替身执行；未修改生产状态机。
- `cargo test -p autome-domain --lib`：388 个通过、0 失败、0 忽略；既有 384 个加本轮 4 个，差值 +4，旧缺失费用指标测试仍通过。原始输出：`.autome/output/m01-domain-all.txt`。
- 负向对照：临时让聚合跳过无费用会话，第一个单测失败；临时让运行中会话进入聚合，第二个单测失败。各 1 个用例，均为预期红；还原后 4 个聚合单测通过。原始输出：`.autome/output/m01-negative-missing.txt`、`m01-negative-running.txt`、`m01-domain.txt`。缺陷注入均已还原。

## 标待审之前的自审清单

1. **情形表逐行**：已结束 Some(1.25)/Some(2.5) 合计 3.75、会话计数 2；加入 None 后金额不变、会话数 3、缺失数 1；运行中即便带 Some(99) 也不计入，且不产生角色键。对应前三个聚合单测。
2. **失败分支**：空输入由 `cost_summary_of_no_sessions_is_zero_not_absent` 检查，JSON 精确为数值 0、计数 0、空角色对象；缺失费用由第一个单测覆盖。授权拒绝、读盘失败、外部服务不可达和业务超时不适用：本里程碑新增的是纯聚合与现有查询结果的序列化，没有新增 I/O 或错误处理分支。
3. **全部迁移**：没有新增状态机。会话从 Running 变为 Exited(0)、Exited(1)、Killed、Vanished 四种结束状态均检查从不计入到计入（会话数 3→4、金额 3.75→102.75）。全部 6 个 Role、3 个 BugStep、intake、onboarding 的稳定键和每键金额/计数/缺失数都在第三个单测中检查；尚未出现的角色没有键。
4. **输入域边界**：空集合、单个 None、单个 Some(0)、小数 0.001、大额 1,000,000 均检查金额与计数；原始金额不取整。金额沿用现有 SessionMetrics 的 f64，无新增金额范围限制；前导零、闰日、跨年、空串不适用，因为没有解析日期或数字字符串。u32 会话计数极限需数十亿条实际会话，不属本设计点名的输入范围。

## 尚未执行

全 workspace、桌面/Web 和宿主全量门禁留给 M-05/宿主；本轮不声称通过这些检查。人工 H-01–H-03 未执行。

## 最终复验结论

- 最终验收：共 5 个用例通过（4 + 1），0 失败、0 忽略；相对设计基线 0 增加 5 个。日志：`.autome/output/m01-domain.txt`、`.autome/output/m01-e2e-final.txt`。
- e2e 第二个根因是名称与测试运行器冲突：包装脚本以 `current_exe()` 调用 `render-stream codex`，libtest 将参数当过滤词，原约定测试名包含 `codex`，导致自身递归运行。仅将测试名改为 `cost_rollup_task_get_sums_two_sessions_and_counts_missing_cost`，同步契约名称，Codex 场景和验收断言不变。旧运行日志 `.autome/output/m01-e2e-renderer.txt`；名称修改后先暴露 `list_sessions` 按最新在前返回的顺序差异（`m01-e2e-order.txt`），纠正新测试的顺序期望后通过。以上均为本轮新测试准备修正，没有弱化产品断言。
- `cargo fmt --all -- --check`、`cargo clippy -p autome-domain -p automed --all-targets -- -D warnings`、`git diff --check` 全部通过，前两项日志为 `.autome/output/m01-fmt-check.txt`、`m01-clippy.txt`。它们不是新增测试用例，不计入 5 个验收用例。
- 四项自审已执行完毕，M-01 可标为待审，交独立审计复验。
