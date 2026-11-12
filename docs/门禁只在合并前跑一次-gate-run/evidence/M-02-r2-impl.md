# M-02 · 实现 #2

## 范围与实现

仅推进 M-02；implementation-round 为 2/40。上一轮 M-01 审计通过，无退回缺陷、无须搬入的审计缺陷检查。

run_gate 在需求任务的 Gate 节点检测有门禁且 BeforeMerge：先 discard_node 杀包装进程组并清理该节点作业，再记 gate.skipped（reason 为「门禁只在合并前运行」），最后按 Passed 进入既有后继。清理错误沿现有 SchedulerError 路径传播；Bug、MergeGate、MainCheck 与 each_round 分支不变。

新增四个 feature_before_merge_* 端到端用例，使用现有 World、真实 Git/worktree、包装进程、账本与门禁作业；原有 11 个 gate 用例及断言未改。四个用例内额外覆盖暂停、熔断后的两种恢复入口、反向切换，不增加顶层用例数量。

## 验收命令与总数

- `cargo test -p automed --test end_to_end gate`：最终 15/15 通过，失败 0、忽略 0、过滤 101；相对上一轮该过滤器基线 11，增加 4，来自本轮四个 feature_before_merge_* 用例。end_to_end 可发现总数 116（112 + 4），本轮只执行 gate 子集，未声称全 116 个通过。原始输出：`.autome/output/m02-final-gate.txt`（75.54 秒）。首次未补充扩展情形时亦 15/15，通过日志 `.autome/output/m02-gate.txt`。
- `cargo test -p autome-domain before_merge`：6/6 通过，相对 M-01 基线 6 增量 0；另一个集成目标命中 0、过滤 1。原始输出：`.autome/output/m02-domain.txt`。
- 去重执行验收及相关域用例共 21 个，相应基线 17，净增 4；未删旧测试、未加 ignore。
- `cargo fmt --all -- --check`、`git diff --check`：通过；日志 `.autome/output/m02-final-fmt.txt`、`.autome/output/m02-diff-check.txt`。
- 未运行完整 ci-local、全量 lib/e2e、clippy，完整宿主门禁由 Autome 执行。本轮不声明这些通过。

## 标待审之前的自审清单

### 1. 情形表逐行

- §2 两里程碑：逐轮实现/审计到 AwaitMerge；计数文件严格为 merge_gate 一行，gate.started 严格只有 merge_gate，task.updated 不含 gate。合并完成后计数严格为 merge_gate、main_check 两行。用例①通过。
- §2 MergeGate 失败：用例②实际失败一次，修复 prompt 含原命令和失败尾部；失败后 task.updated 节点严格为 implement、audit、retro、rebase、merge_gate、await_merge，随后合并 Done，无 Gate 启动。通过。
- §3 正在跑：用例③清理阻塞 Gate 后到 Audit，检查包装 PID 与整个进程组不存在，四种作业文件均删除；gate.skipped 节点/原因准确，没有旧 Gate 结果入账，之后 MergeGate 真正启动。通过。
- §3 已暂停恢复：用例③再运行一次 task.pause→改配置→task.resume，执行同一组清理、事件、Audit、MergeGate 断言。通过。
- §3 已停止恢复：用例④ task.stop→改配置→task.resume，执行同一组断言。通过。
- §3 同因失败熔断恢复：用例④分别真实制造三次同因失败，切换配置后通过 task.rerun_from(gate) 和 task.extend_budget(extra_rounds=1) 恢复；实现轮仍为 3，Gate 启动仍为原来的 3 次，新增 skipped，最终结果来自 MergeGate。两种入口均通过。
- §3 反向切换：用例①第二个独立 World 在 Implement 阻塞时由 BeforeMerge 改 EachRound；释放后确实出现 Gate 的 gate.started，无 gate.skipped，最终到 AwaitMerge。通过。
- 审计关闭的后继：相关域用例比较完整 Transition，覆盖 Implement/Retro/Rebase、预算耗尽、有纳入决议；已有 Gate Passed 后继等价用例通过。MergeGate/MainCheck 原有相关 e2e 全部保持通过。

### 2. 失败分支

- 设计点名的 MergeGate 失败→修复→审计→复盘→rebase→MergeGate：用例②实际执行并通过，包含 GateFailed 命令/尾部验证。
- 同因失败熔断及恢复：用例④两个恢复入口均实际执行并通过。
- 旧阻塞作业取消：用例③④验证杀进程和文件清理，且不把被取消结果记成新门禁结果。
- 拒绝授权、读盘失败、门禁超时、空输入、外部服务不可达：本里程碑设计未点名这些失败分支，新增配置域由 M-01 负责，未增加授权/网络/超时机制；本轮不额外注入这些故障，不声称已验证。discard_node 的错误传播沿现有处理方式。

### 3. 全部迁移

已执行本里程碑情形表全部迁移：Implement→Audit（两轮）；MergeGate Failed→Implement→Audit→Retro→Rebase→MergeGate Passed→AwaitMerge→合并/MainCheck→Done；Active Gate→Audit；Paused Gate→恢复→Audit；Stopped Gate→恢复→Audit；同因失败 Gate→重跑/追加预算→Audit；BeforeMerge→EachRound 后 Implement→Gate。四种跳过路径均继续到 MergeGate。域子集补证审计关闭分支和 MergeGate 成败/已请求合并组合，未以正路径替代全部迁移。

### 4. 输入域边界

- 单里程碑、两里程碑、修复已全部完成里程碑：分别在切换、正常全流程、失败修复用例执行。
- 两种合法模式及双向运行中切换：实际执行；复用无作业文件（停止后先丢弃）、仍有活进程（活动/暂停）两个清理边界。
- 同因失败恰达阈值 3、追加预算最小正值 1：实际执行。
- 空串、非法配置、日期/闰日/跨年/前导零/数值最大值：不适用；M-02 未新增输入字段，封闭模式解析属于已完成 M-01，未变更数值或日期域。

## 负向对照与修正

1. 临时把 task.rs 的 gates_each_round() 恢复为 gate.is_some()，执行 `cargo test -p automed --test end_to_end feature_before_merge_`：3 通过、1 失败，失败回路节点多出 gate。日志 `.autome/output/m02-negative-state.txt`。
2. 上一步揭示设计负向预期有误：新增 run_gate 跳过分支会屏蔽脚本调用，错误进入 Gate 不一定让计数文件出现 gate。因此给用例①补 task.updated 不含 gate 的断言，保留错误变异再执行 `cargo test -p automed --test end_to_end feature_before_merge_two_milestones`：1 个失败（退出 101），节点断言捕获错误。日志 `.autome/output/m02-negative-node.txt`。设计验收契约仅就地修正这一判据，不追加轮次正文。
3. 恢复 task.rs 后临时禁用本轮 run_gate 跳过分支，执行 `cargo test -p automed --test end_to_end feature_before_merge_switching_`：2 个均失败（退出 101），90 秒有界等待后仍停在 Active Gate，无法到 Audit。日志 `.autome/output/m02-negative-skip.txt`。
4. 所有变异已还原；task.rs 与 HEAD 无差异，scheduler.rs 与保留的正确实现逐字节相同。最终验收在恢复后的代码上 15/15 绿。最初编译时的模块路径与错误转换问题已修正为 crate::gate 和现有 SchedulerError 映射，没有遗留编译错误。
