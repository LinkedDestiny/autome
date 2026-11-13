# 实现 #7：合并前 rebase 兼容修复

## 范围与状态

简报没有开放里程碑；上一轮审计已关闭 M-01—M-05，且没有退回缺陷。本轮执行用户追加的 rebase 指令，不新增里程碑、不自行关闭或重开已审计项。implementation-round 更新为 7/25，current-milestone 与 next-action 保持「无」。本文件以 M-01 为合并队列集成证据归档，不表示重新推进多个里程碑。

`git rebase main` 完成，默认分支基线为 `f5a963ad`；`git merge-base --is-ancestor main HEAD` 返回 0。未 merge、未切分支、未推送、未启动下一会话。

## 冲突与修复

- scheduler.rs：同时保留主线 reusable_main_check（准入与执行共享沿用条件）及本分支 merge_gate_up_to_date；保留主线快检和不占会话槽的 Git 步骤。
- end_to_end.rs：保留主线费用汇总、格式保护测试，以及 merge_queue_cases.inc 入口。
- task.rs：合并主线直接 Active Rebase 的落后恢复和本分支 grants_merge_request、MergeGate 冲突回 Implement；保留主线 gate_quick 测试和槽位断言。
- dispatch.rs：保留费用汇总和配置警告；CLI 同时显示费用、合并状态，保留双方渲染测试。
- rebase 后编译检查发现主线新增 Transition 构造仍写旧字段 merge_requested，改为 grants_merge_request。全 crates 扫描剩余同名项均为函数入参，非旧结构字段。
- 旧共享尾段测试把 MergeGate 冲突也视作 Bug/需求完全相同；D6 明确要求分别回 Fix/Implement。保留原测试并添加该事件的完整 start_fix 转换断言（含预算、注入、动作），其余事件仍逐一对比。
- 设计就地对齐主线 Active Rebase 和宿主快检安排，没有追加轮次正文。

## 红绿记录

上一轮审计通过，没有需要搬入的审计临时检查。本轮发现的问题均使用项目现有测试复现：

1. `cargo test -p autome-domain merge_`：退出 101，42 例中 41 通过、1 失败；失败为 bug_loop_merge_tail_is_shared_with_the_feature_loop，实际 Bug 为 Fix、需求为 Implement。原始输出 `.autome/output/r7-domain.txt`。修复后同命令输出 `.autome/output/r7-domain-green.txt`，42 例通过。
2. `cargo test -p automed --lib merge_`：退出 101，编译 E0560（scheduler.rs 的旧授权字段），未执行测试。原始输出 `.autome/output/r7-lib-red.txt`。改名后同命令输出 `.autome/output/r7-lib.txt`，85 例通过。

## 自审清单

1. **情形表逐行**：本轮冲突落法逐项核查：共享 MainCheck 沿用与出队祖先检查同时存在；费用与警告同时存在；CLI 费用与状态同时存在；主线 e2e 与任务 include 同时存在；Behind 使用主线 Active Rebase、授权授予字段使用本分支新语义；MergeGate 冲突分别回需求 Implement/Bug Fix。源码函数留存对照 `.autome/output/r7-source-retention.txt`，缺少的名称仅为设计明确移除的 in_merge_stretch 和已改名授权测试。对应 merge_*、gate_quick_*、cost_rollup_*、regression_format_only_* 测试覆盖。
2. **失败分支**：merge_queue_every_exit_releases_the_section 覆盖门禁失败、熔断、准备失败、冲突、暂停/停止/取消及 Merging 的 Ok/Blocked/Behind/Failed；merge_auth_* 覆盖终态/失败拒绝与合并开始后拒绝撤销；merge_conflict_* 覆盖需求/Bug 冲突与次数/预算边界。读盘失败、外部服务不可达、超时不是本轮新增或冲突改变的分支，不适用本轮兼容修复；不将此子集宣称为全量故障验收。
3. **全部迁移**：保留共享尾段测试对 MergeGate、AwaitMerge、Merging、MainCheck、Cleanup × triggers × 是否授权的遍历；D6 的冲突例外改为完整转换断言。merge_auth_only_a_behind_merge_grants_a_request 遍历节点与触发器，Behind→Active Rebase 仍授予授权；队列出入、释放、重启和成功合并由 merge_queue_* 验证。
4. **域边界**：merge_auth_merge_press_by_state 覆盖授权/撤销状态域；merge_conflict_* 覆盖空/单个/多个冲突文件、两种授权、连续冲突和预算；merge_queue_auto_rejects_non_booleans 覆盖配置类型边界。前导零、闰日、跨年不适用：本轮无日期或数值解析改动。

## 验收

仅执行与冲突和兼容修复相关的测试子集；未执行 ci-local、全量测试或宿主 fmt/clippy/Web 快检。格式仅对冲突解决文件及修改文件执行 rustfmt。人工 H-01/H-02 未取证，仍由合并前人工确认。

最终命令与结果（均退出 0，失败/忽略均为 0）：

| 命令 | 通过用例 | 原始输出 |
|---|---:|---|
| `cargo test -p autome-domain merge_` | 42 | `.autome/output/r7-domain-green.txt` |
| `cargo test -p automed --lib merge_` | 85 | `.autome/output/r7-lib.txt` |
| `cargo test -p autome-cli merge_` | 5（单测 4 + CLI 集成 1） | `.autome/output/r7-cli.txt` |
| `cargo test -p automed --test end_to_end merge_` | 44 | `.autome/output/r7-e2e.txt` |
| `cargo test -p autome-domain gate_quick_` | 6 | `.autome/output/r7-quick-domain.txt` |
| `cargo test -p automed --test end_to_end cost_rollup_` | 1 | `.autome/output/r7-cost-green.txt` |
| `cargo test -p automed --test end_to_end regression_format_only_` | 5 | `.autome/output/r7-regression.txt` |
| `cargo test -p automed --test end_to_end gate_quick_` | 6 | `.autome/output/r7-quick-e2e.txt` |

用例总数：194 次成功执行、192 个不同用例（过滤器重叠重复 2 例；不把红轮重试累加）。按既有验收前缀核对，merge_queue_ 19、merge_auth_ 17、merge_conflict_ 6、merge_view_ 6，共 48，与各自上次审计基线 19/17/6/6 相比差为 0。本轮总范围较这 48 例增加 144 个既有兼容用例，来自更宽的 merge_ 过滤以及主线快检、费用汇总、格式保护；没有新增测试函数，没有用减少断言换通过。

M-05 文档命令原样执行：`grep -q '^auto = ' README.md && grep -q 'Merge queue' README.md && grep -q 'merge cart --cancel' README.md && grep -q '合并队列' docs/development/technical-design.md && grep -q 'merge.locked' docs/development/technical-design.md && grep -q 'MergeGate 出队冲突' docs/development/technical-design.md && echo docs-ok`，输出 docs-ok、退出 0，6 项与上一轮基线相比差 0；文档检查不混入测试函数计数。

`git diff --check` 退出 0。状态字段逐项校验及 5 行已审计里程碑保留检查输出 status-and-milestones-ok；无未解决冲突。合并前宿主门禁尚未运行，本轮证据不替代它。
