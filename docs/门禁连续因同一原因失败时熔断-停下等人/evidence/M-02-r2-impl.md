# M-02 · 实现 #2

结论：本里程碑验收通过，完成下述自审后报待审；不关闭里程碑。implementation-round 为 2/25。

## 改动与边界

- 新增 FailureReason / Trigger 的 GateSameFailure；特性 Gate / MergeGate、Bug FixGate / MergeGate 熔断时进入 Failed，Action::None，保留 count/summary/log。
- 同节点 rerun 和 extend 使用已有 DiscardGateRuns；extend 增加预算，特性缺失预算拒绝，Bug 缺省使用 bug.fix_rounds。
- ProjectConfig 新增可选上限，resolve 缺省 3；GateConfig / GateDone / GateOutcome / Context 不改。scheduler 仅补 trigger_name 穷举匹配以保持工作区可编译。
- M-03 的配置解析、持久计数和调度接线、M-04 的新 e2e、M-05 的读者未提前实现。
- 上一轮审计 M-01 为通过，没有缺陷判别检查需要逐字迁入测试。
- 初次新增测试编译发现 Rejected 没有 Display，改用现有 reason 字段断言；首轮 13 项运行有 1 项红：Bug 节点先被通用 guard 拒绝，文案不符。将特性熔断触发器放在控制触发器分支、先于该 guard 校验后转绿。未改旧 gate 路径。

## 验收命令与用例数

原始输出均在 .autome/output/，这里不转存日志。

| 命令 | 结果 | 原始输出 |
|---|---|---|
| cargo test -p autome-domain same_failure | 13 通过，0 失败；较上轮该筛选 0 项新增 13 项 | m02-focused.txt |
| cargo test -p autome-domain | 库 374 通过，0 失败；另 1 个真实日志集成测试保持 ignored，doc-test 0 | m02-domain-final.txt |
| cargo clippy --all-targets -- -D warnings | 退出 0，无警告 | m02-clippy.txt |
| cargo fmt --all -- --check | 退出 0 | m02-fmt-check.txt |
| git diff --check | 退出 0 | 无输出 |

domain 库上一轮基线 361，本轮 374，差 +13：task 10、bug_loop 2、config 1；没有删减或改写旧测试。包含 ignored 测试的 domain 测试总数为 375，上一轮为 362，仍为 +13。通过读取 HEAD 的四个源文件、截取各测试模块从第一个旧 #[test] 到文件末尾，断言该整个字符串仍存在于当前文件，确认旧测试逐字保留。唯一删除的旧行是 task.rs 生产代码 ExtendBudget 模式匹配，将 reason,.. 改为 at,reason。

## 负向对照

每次只临时修改 task.rs 一处，运行下述测试，然后从内存保存的完整原文件还原。三次最终有效注入均退出 101、1 项失败；还原后 domain 库 374 项全绿。

1. 在熔断返回前插入 ctx.budget_n == Some(0) 时返回 ImplBudget：cargo test -p autome-domain same_failure_takes_priority_over_exhausted_budget。日志 m02-negative-budget.txt。
2. 精确定位同节点熔断 RerunFrom 分支，将 Action::DiscardGateRuns 改成 Action::RunCoreStep：cargo test -p autome-domain same_failure_rerun_discards_gate_runs。日志 m02-negative-discard-targeted.txt。
3. 熔断 ExtendBudget 的 checked_add(extra) 改成 checked_add(0)：cargo test -p autome-domain same_failure_extend_adds_budget_and_discards_runs。日志 m02-negative-extend.txt。

第二项第一次按文件首个 DiscardGateRuns 替换，误命中既有 Resume 分支，测试仍绿；该结果不计为有效负向证据（m02-negative-discard.txt），已重新精确注入并取得红证据。所有注入均已还原，未留下测试替身。

## 标待审之前的自审清单

### 1. 情形表逐行

对照设计 §3 转换表，以下全部通过，检查在新增 13 个测试中执行：

| 设计行 | 检查与结果 |
|---|---|
| 特性 Active Gate / MergeGate × 同节点熔断 | 两个独立用例检查 Failed 的完整字段、Action::None；MergeGate 还检查 merge_requested 清除 |
| Bug Active FixGate / MergeGate × 同节点熔断 | 两个节点逐一检查，fix_round=u32::MAX 仍优先熔断 |
| 其它节点 / 节点不符 | 特性 MainCheck/ReproCheck/Implement/FixGate、Bug Gate/MainCheck/ReproCheck/Fix 与错配节点全部拒绝；文案符合 |
| 熔断后 rerun 同一门禁 | 两种 Loop 各两个节点均 Active + DiscardGateRuns |
| 其它失败或其它门禁 rerun | Infeasible、Stopped 的特性门禁重跑拒绝；Bug 其它失败和错节点重跑拒绝 |
| 熔断后 rerun Implement / Fix | 特性启动 Impl 角色；Bug 启动 Fix 会话 |
| 特性 extend 正数 | 两节点预算 25+5=30，原节点 Active + DiscardGateRuns；缺预算报读不到实现预算 |
| Bug extend 正数 | 两节点分别检查显式 25 和缺省 bug.fix_rounds，加 5 并丢弃作业 |
| extend 0 | 两种 Loop 都拒绝 |

### 2. 失败分支

已跑：错误节点、错配节点、停止状态、其它失败原因、无特性预算、追加 0、预算溢出拒绝。熔断与耗尽同时成立优先熔断也已跑。授权拒绝、读盘失败、外部服务不可达不适用：本里程碑是无 I/O 的纯状态机；超时指纹和空日志属于已完成 M-01，本轮 domain 全量包含其原测试，调度器读盘和超时接线在 M-03。

### 3. 全部迁移

本次新增的全部状态×事件已在上一表逐一执行：4 个 Active 门禁熔断到 Failed；4 个 Failed 同节点 rerun 回 Active；4 个 Failed extend 回 Active；两个 Loop 回原有角色会话；所有列明拒绝迁移不产出 Transition。未达阈值仍使用未改的 GateDone，门禁通过、失败回实现/修复与预算耗尽由原有 domain 测试全量复验。计数重置和崩溃重放属于 M-03，本轮不声称已接线。

### 4. 输入域边界

已跑：项目值缺省 None、上限 1/3/10；extra_rounds=0、1、5、u32::MAX，预算 0、25、u32::MAX、None；0+MAX 成功、MAX+1 拒绝。FailureReason JSON 四字段精确全等并反序列化往返。TOML 的 0/11/-1/2.5/字符串越界验证属于 M-03；本轮没有引入解析入口。日期、前导零、闰日、跨年不适用：没有日期或字符串数字输入域。

## 全量补充验证

- cargo test --workspace：退出 0，1113 通过、0 失败、1 ignored，总数 1114（m02-workspace.txt）。组成：CLI 19+11，domain 374，automed 606，end_to_end 80，prompt_rendering 16，real_cli 7，既有 T-14 集成测试 ignored 1；doc-test 0。上一轮为 1100 通过口径 / 总数 1101，本轮差 +13，全部来自本轮 domain 测试，无套件缺失。相对设计初始基线 1084，累积增加 M-01 的 16 和本轮 13。
- 原有 80 个端到端用例全过，包括门禁失败退回、通过继续、停止门禁再恢复，以及 Bug 门禁路径。未运行 sh scripts/ci-local.sh，依任务要求由 Autome 后续执行；未运行桌面端测试，本轮没有桌面端修改。
- clippy 在负向注入前通过；注入后逐字还原生产文件，重新跑 domain 全量、fmt 与 diff --check 均通过。
- 无环境阻塞。提交按本轮要求在当前分支执行，不推送、不启动下一会话。
