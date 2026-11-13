# 实现 #20：合并门禁 Rust 失败修复

## 范围

本轮只修宿主门禁失败，不推进里程碑，不修改里程碑表或已关闭里程碑的最新证据指针；implementation-round 为 20/80。上一轮审计通过，没有审计检查需要搬迁；本轮失败检查本身均已在项目测试体系中，先原样运行再修复。

只读宿主日志：主检出 .autome/output/gates/T-95/merge_gate-f0c98afe872f.log，automed 库测试 993 个，980 通过、13 失败。摘录见本 worktree .autome/output/r20-gate-index.txt。

## 根因和修复

- D / M-09：audit-checks-operator-notes 的 3 个源文件存在却未嵌入 protocol/evals.rs；CHANGELOG 引用不存在的 seed 目录，造成 8 个协议静态测试失败。补齐 case、scaffold、grader 清单，133 项变为 136 项。不改协议正文、版本或哈希算法。
- C / M-07 与 E / M-11：4 个调度测试从定位后的节点开始，却缺定位轮的 Bug 文档和保护集；M-07 无快照补建正确拒绝缺失材料。共用夹具现在提交真实 regression.sh、Bug 文档，并通过生产 regression_set 构造非空保护集；重入测试在 Locate 后、Fix 前准备相同产物。保留退出码、超时、预算、冲突熔断及注入断言，没有放松生产守卫。
- G / M-15：kernel_mark_migration_preserves_queue 仅把当前 v9 表版本号回拨到 7，导致重放迁移时 28 值插入 27 列。复用 downgrade_sessions_to_v7 还原真实旧表再迁移；最终版本断言使用 SCHEMA_VERSION，队列内容及顺序断言保留。生产迁移 SQL 未改。

同类检查覆盖协议静态层、共享 repro_gate_world 的 quick/before_merge 用例、Bug 重入和冲突预算、迁移及保护集。中间复跑 30 通过、4 失败，原因是新夹具重复拼接 Bug 文档路径；已改正并最终复验，不把中间结果当作通过。

## 命令与结果

原始输出均在 .autome/output/。测试沿用临时仓库/账本夹具，不接触真实账本。

1. 原样复现及修后复跑：
   cargo test -p automed --lib -- protocol::eval::tests before_merge_fix_gate_records_repro_across_mode_switch_even_on_failure bug_loop_reenter_maps_bug_nodes_to_bug_sessions gate_quick_bug_fix_gate_runs_repro_and_quick_under_the_full_timeout merge_conflict_bug_streak_and_budget_stop_before_launching_another_fix kernel_mark_migration_preserves_queue
   修前 34 个：21 通过、13 失败（r20-red.txt）；修后 34 个全通过（r20-green-final.txt）。中间日志 r20-green.txt。
2. 直接相关子集：
   cargo test -p automed --lib -- protocol::eval::tests gate_quick_ before_merge_ bug_loop_reenter_maps_bug_nodes_to_bug_sessions merge_conflict_bug_streak_and_budget_stop_before_launching_another_fix migration regression_
   98 个全通过（r20-related.txt），包含上述全部 34 个。
3. M-16 原验收：
   cargo test -p automed -- protocol_integrity_ the_version_in_force
   库 2 个、end_to_end 6 个，共 8 个全通过（r20-m16.txt）。
4. 三个修改的 Rust 文件执行 rustfmt；git diff --check 通过。未执行项目快检或全量门禁。

最终去重用例总数 **106 = 98 + 8，106 通过、0 失败、0 跳过**。相比实现 #19 的 82 个为 **+24**，原因是本轮改跑 Rust 协议、调度和迁移范围，不再跑上一轮 Web/domain 范围；不是新增 24 个测试。相同选择范围的总数变化 0，没有新增、删除或跳过测试函数，红绿复跑不重复计数。

## 四项自审

1. **情形表逐行**：合法 seed、缺 eval、非法条款及状态块由 protocol::eval::tests 覆盖；quick 有/无、before_merge/each_round、配置切换和重用由 gate_quick_/before_merge_ 覆盖；迁移及队列顺序由 migration 覆盖，全部通过。
2. **失败分支**：退出码 9/124/126/127、复现失败不执行 quick、超时、协议缺必需短语/用例、保护集为空或修改拒绝、冲突预算耗尽，均通过。授权和外部服务不可达不适用：没有新增授权或网络路径。
3. **全部迁移**：Locate/Fix/FixReview 重入；FixGate 成功到 FixReview、失败到 Fix；冲突回 Fix、预算/冲突熔断停止；模式切换按启动种类收割；v7/v8 到当前账本迁移保留行及队列，全部通过。未新增产品迁移。
4. **输入域边界**：退出码 0/9/124/126/127、无 quick、空保护集、预算 0、冲突计数临界、缺 eval 目录、旧 schema 版本均通过。日期、前导零、闰日不适用：改动没有这类输入。

## 交接

保持既有已完成里程碑及其审计证据，待独立集成复验、宿主快检和合并门禁。H-01 付费模型行为验收仍待人工确认。不运行全量门禁，不启动下一会话。
