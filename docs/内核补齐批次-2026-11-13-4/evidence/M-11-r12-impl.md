# M-11 实现证据（实现 #12）

结论：验收通过，申请待审。实现轮 12/80，仅推进 M-11。上一轮审计 #10 对 M-10 的结论是通过，无退回缺陷、无需要逐字搬入的审计检查。

## 实现与范围

- 按实际 Launch 种类划分车道：full / setup / 未知旧作业为 heavy，quick / repro 为 light。占位读取启动时 `.kind`，等待次序读取事件 `lane`，旧事件无 lane 默认 heavy。
- 全局及项目 quick_concurrency 默认 1、范围 1–10，项目值由全局封顶。只写并发键不启用门禁；save_project / save_global 保留手写 gate 表。
- 同车道 FIFO 与占位计数；等待期间重算种类改变时写新 gate.queued、进入目标车道队尾。准备命令的连续重车道名额不能直接转给轻车道检查；无法确认在途占位时两车道均保守等待。
- task.get / dashboard.get 增加 gate_queue_lane，位置只数同车道；CLI status / run 区分完整门禁与快检或复现检查。保留原 gate.queued.job 的 gate / setup 含义；gate.started 额外记录 lane。
- 更新 README 与技术设计。未修改配置、协议副本、门禁脚本、前端页面；未运行全量门禁或宿主快检。

## 命令、用例总数与基线

1. 里程碑验收命令原样执行：
   `cargo test -p autome-domain gate_lane_ && cargo test -p automed -- gate_lane_ gate_queue_ && cargo test -p autome-cli gate_lane_`
   退出 0，**47 个用例**：domain 5，automed 库 32 + end_to_end 9，CLI 1；0 失败、0 忽略。日志 `.autome/output/m11-acceptance.txt`。
2. `cargo test -p autome-cli -- gate_queue_ gate_lane_`：4 个通过（与主验收重叠 1）；日志 `m11-cli-related.txt`。
3. `cargo test -p autome-domain -- gate_queue_ gate_concurrency_ && cargo test -p automed --lib gate_concurrency_`：9 + 7 = 16 个通过；日志 `m11-config-related.txt`。
4. `cargo test -p automed --lib gate_quick_host_`：10 个通过；日志 `m11-quick-related.txt`。覆盖启动种类冻结、运行中改模式、删除 quick、排队时使用最新配置、超时与复用边界。
5. `cargo fmt --all` 格式化成功；`git diff --check` 通过。没有运行 `cargo fmt --all -- --check`、全项目 clippy、npm lint/typecheck 或 `sh scripts/ci-local.sh`，这些留给宿主。

**计数口径：** 主验收相对轮初同过滤范围的 30 个既有用例新增 **17** 个（domain 5、automed 库 8 / e2e 3、CLI 1），成为 47。设计原预测 44，实际多 3：解析越界放在拥有 TOML I/O 的 automed 层独立测试，另补 setup→light 禁止转槽与 light 读盘失败各 1；domain 保留独立的缺省/序列化测试。上述所有命令去重合计 **76 个用例**，相对相同范围轮初 59 个增加 17；重复验证不叠加计数。上一轮 M-10 验收是 14 个，本轮主验收数值差 **+33**，来自切换到 M-11 的过滤范围，不能解释为新增 33 个测试。

## 旧夹具调整与调试结果

- `gate_queue_bug_loop_repro_check_waits_without_spending_a_fix_round` 原先让 repro 等 full，与 E-R1 冲突；现使用真实 quick 作业阻塞同轻车道，保留全部排队、修复轮次、释放后启动断言。完整门禁之间的既有队列用例仍通过。
- `gate_quick_host_queued_jobs_select_current_config_and_head` 同理将合成占位标记为 repro，仍检查配置变更、命令、head 和删除 quick。
- 既有 `gate_queue_fix_round_is_counted_once_after_setup_not_while_waiting` 最初超时：`start_session` 的 M-07 守卫在缺少 bug 文档及 regression_snapshot 时已拒绝进入 Fix，准备命令没有启动（调试记录 exists=false、waiters=[]、held=0）。补齐 Locate→Fix 必备保护快照后原断言通过，未放松产品守卫。
  - 红：`cargo test -p automed --lib gate_queue_fix_round_is_counted_once_after_setup_not_while_waiting`，0 通过 / 1 失败，日志 `m11-budget-rerun.txt`、诊断 `m11-budget-debug2.txt`。
  - 绿：同命令，1 通过，日志 `m11-budget-green.txt`；最终主验收也通过。
- 新 heavy/light e2e 初次因夹具未提供复盘脚本停在 retro，未到 MergeGate；补齐第 7 步并按实际 session.kind.role 检查审计启动后通过。最终新增 e2e 等待任务安静、确认无运行会话后结束；宿主作业由退出标记收割。

## 标「待审」之前的自审清单

1. **情形表逐行**（设计 E 小节各落法）：
   - full→heavy、quick/repro→light、setup/空/未知→heavy：domain kind 映射测试；e2e Bug 的 before_merge（含无 quick / 有 quick）、each_round 与 MergeGate 实际事件均符合。
   - 重门禁在跑、快检完成进入审计：`gate_lane_e2e_quick_runs_beside_a_sleeping_merge_gate` 通过，A 无 gate.result 时 B 已完成快检且启动 audit。
   - light 限额 1 FIFO、2 并行：`gate_lane_e2e_two_quick_checks_fifo_at_one_parallel_at_two` 通过，按释放文件控制时序，无固定等待断言。
   - 两车道恢复与旧事件兼容：restart / legacy 测试通过；恢复后的重车道可在轻作业占位时准入。
   - 准备命令后转 full / light：既有 setup_and_gate_keep_one_slot 与新增 setup_cannot_transfer_its_heavy_slot_to_light 分别通过。
   - 配置校验/缺省/封顶/保存、实时位置/CLI：相应 domain、config_io、dispatch、CLI 用例均通过。
2. **失败分支**：
   - 无法读作业目录且存在在途事件：既有 heavy 用例与新增 light 用例均等待；恢复目录后可以准入。
   - 超时、进程消失、暂停时占位、暂停后超时回收、过期作业、队首出错退队：既有 gate_queue_ 用例及 quick_host_queue_reclaims_with_launch_timeout 通过。
   - 非法/空配置输入：解析测试拒绝，见下列域边界。拒绝授权、外部服务不可达不适用：本项无新增授权/远端服务分支。
3. **全部迁移**：
   - 无队列→等待→准入（gate.started / setup.started）分别由两车道 FIFO、setup 用例验证；等待不消耗轮次或重试。
   - heavy→light、light→heavy 等待项均重新排目标队尾；同车道重复请求不重复入队：config_change_while_waiting_requeues_into_the_new_lane 逐方向通过。
   - 无队列/已有队列 × gate.queued、gate.started、setup.started、session.started、session.deferred、task.updated、gate.queue_dropped、无关事件：domain gate_queue_event_transitions 枚举通过；dispatch 的失效事件测试与暂停/停止/恢复用例通过。
   - 运行中切模式按启动种类收割、删除 quick 作废、head 变更作废重跑：10 个 gate_quick_host_ 用例通过；双车道重启恢复通过。
4. **域边界**：
   - 并发缺省、1..=10 全域、0/11/-1、浮点、布尔、字符串、空字符串、数组、i64 最大值、整数溢出、前导零、缺值：解析测试逐项通过；项目覆盖 None/1/2/10 × 全局 1/2/10 封顶通过。
   - 空队列、单车道单元素、交错两车道、未知 task、usize::MAX 占位；CLI 位置 0/1/2/u64::MAX 均通过。日期、闰日和跨年不适用，本项没有日期输入域。

## 负向对照（临时注入均已还原）

- 将 position 改回统计所有车道：`cargo test -p autome-domain gate_lane_position_counts_only_the_same_lane` 为红（0/1），日志 `m11-negative-position.txt`。
- 将准入占位改回 `held_in_lane(ctx, None)`：`cargo test -p automed --lib gate_lane_restart_recovers_both_queues` 为红（0/1），日志 `m11-negative-held.txt`；轻作业错误堵住重车道。
- 还原后原两条命令各 1 个通过，日志 `m11-negative-restored.txt`；还原文件与注入前备份逐字一致。

## 提交与后续

本轮代码、文档与证据提交当前分支；独立审计复验 M-11 后才能关闭。无人工验收阻塞；宿主快检与合并前全量门禁尚未运行。
