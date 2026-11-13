# M-02 · 实现轮 3/25

## 范围与实现
- 仅推进 M-02。上一轮审计通过 M-01，没有退回缺陷，因此没有必须逐字移植的审计判别检查。
- D2：授权只在用户操作或合并落后时写账本；普通转换不再清除授权。Transition 字段改为 grants_merge_request，Bug Rebase 不再把传入授权当作新授予。
- task.merge 按状态表区分立即合并、预先同意和拒绝；cancel 严格接收布尔值，拒绝合并已经开始/终结/没有授权，持锁撤销同次释放锁。CLI 接受 --cancel 并打印返回结果。
- config::warnings 为无门禁的项目自动合并给出警告，task.get/dashboard.get 返回当前警告；save_project 保留整个手写 merge 表。状态展示与事件文案的其余部分仍属 M-04。
- 测试适配保留既有冲突、注入、门禁失败 SHA、修复轮和主线文件断言；仅按设计取消离开尾段时的撤销期望。
- 当前历史核对：T-51 的 bf399dbf 已在历史中；T-53 最新匹配仍是 adjudicate 简报，没有引入另一套 FIFO 排序。

## 验收命令与结果
- 验收命令（四条串行，整链退出码 0）：
  - `cargo test -p autome-domain merge_auth_`：6。
  - `cargo test -p automed --lib merge_auth_`：6。
  - `cargo test -p autome-cli merge_auth_`：1。
  - `cargo test -p automed --test end_to_end merge_auth_`：4。
- **用例总数 17，全部通过，失败 0、忽略 0**。与上一轮 M-01 验收基线 19 相差 **-2**，因为本轮改验 M-02 的 17 项，并非删除两个测试；M-01 的 19 项另行回归。相同 merge_auth_ 过滤口径在本轮起点为 0，本轮增加 17 个匹配项，其中 7 个既有用例改名/适配、10 个新增用例；项目测试净增 10。
- 实际结果：授权/撤销两条完整需求 Loop 均满足预期；未批准设计仍停在设计批准卡点；三任务用户授权跨重启后门禁调用恰好 3 次，每次 main 都是 HEAD 祖先，MainCheck 均沿用，无 Behind 授权；门禁失败修复后自动合并且 merge.direct 来源 behind 恰好一次。
- 日志：.autome/output/m02-domain.txt、m02-lib.txt、m02-cli.txt、m02-e2e.txt。初始化空筛选测试程序的 0 tests 没有计入 17。
- 相关回归命令（原始日志 .autome/output/m02-regression.txt）：
  - M-01：`cargo test -p autome-domain merge_queue_`、`cargo test -p automed --lib merge_queue_`、`cargo test -p automed --test end_to_end merge_queue_`（19 项，较上一轮同口径基线差 0）。
  - 域层逐个运行 `cargo test -p autome-domain <filter>`，filter 为 `a_requested_rebase_`、`a_passed_merge_gate_on_a_requested_merge`、`without_a_request_rebase`、`a_rebase_entered_from_anywhere`、`backlog_items_included_during_the_automatic_path`、`behind_is_only_accepted_from_merging`、`bug_loop_rebase_always`、`bug_loop_rebase_conflict_and_merge_gate_failure`、`bug_loop_merge_tail_is_shared`（10 项）。
  - 宿主逐个运行 `cargo test -p automed --lib <filter>`，filter 为 `a_requested_rebase_records`、`a_rebase_nobody_asked`、`with_every_slot_taken_the_requested_rebase`、`a_merge_behind_main_is_requeued`（4 项）。
  - e2e 逐个运行 `cargo test -p automed --test end_to_end <filter>`，filter 为 `two_tasks_waiting_to_merge_both_land`、`main_moving_after_the_press`、`main_check_reuse_switched_off_matches_today_event_by_event`（3 项）。
- 相关回归整链退出码 0，36 项全部通过（19 + 10 + 4 + 3），没有重复计数。加上 M-02 验收，本轮最终共 **53 个不同用例通过**，失败 0、忽略 0；较上一轮记录的 19 个验收用例多 34，来源为 M-02 的 17 个验收项及另外 17 个直接相关兼容回归。git diff --check 通过。



## 标待审前自审清单
1. **情形表逐行**：D2 merge_press 全部 Active 节点（含 Bug 节点）、Draft/Queued、每个节点的 Paused/Stopped/Requeued、Failed/Done/Cancelled 分别检查；cancel_press 同表，额外检查无授权、重复授权、重复撤销、项目开关被用户撤销覆盖、重新同意。对应 merge_auth_merge_press_by_state、cancel_overrides_the_project_switch、task_merge_rejects_failed_and_finished_tasks。
2. **失败分支**：拒绝授权/撤销由状态表和 RPC 用例覆盖；Rebase 读盘失败由 domain 冲突用例中的 CoreStepResult::Failed 覆盖；真实 Git 冲突由宿主用例覆盖；门禁失败→修复→自动合并由 e2e 覆盖；准备超时（exit_code 124）及门禁熔断验证不撤销授权；空/缺失/不存在 task_id、非布尔 cancel 拒绝且不写授权。外部服务不可达不适用：M-02 不新增外部服务调用。
3. **全部迁移**：未授权→用户授权→重复同意不重复写→用户撤销→再次同意；项目授权→用户撤销覆盖；Merging Behind→授权；暂停/停止/恢复/冲突回实现/门禁失败/准备超时/熔断/取消均不新增撤销事件；MergeGate 持锁→撤销释放→通过停 AwaitMerge；MergeGate 通过→Merging 后撤销被拒→照常合并；无门禁 Rebase 直接合并；设计批准卡点授权不放行；重启从账本恢复用户授权。
4. **输入域边界**：覆盖 cancel 缺省、false/true、字符串/null；task_id 缺失/空/不存在；全节点枚举、失败与两个终结态；配置 auto false/true × gate 有/无，以及 merge 表注释、未知键、after 空白保留。日期、数字上限、前导零不适用：本里程碑没有此类输入。

四项自审均已执行，通过；未运行设计中的源码缺陷注入负向对照，未将其计为已验收用例。

## 执行中修正
- 初次机械编辑误删 Bug Loop 一段内容，编译发现后从本轮起点恢复该文件并仅删除授权复制行；未改变 Bug 状态转换。
- 补强生命周期测试首次把运行中的 Implement 直接 RerunFrom，转换表正确拒绝。夹具改为先 Stop 再 RerunFrom，保留生产规则。失败日志 m02-domain-fixture-failure.txt。

## 未运行与提交
- 遵循本轮指令，未运行 ci-local、宿主快检或项目全量门禁；只格式化改动 Rust 文件。fmt/clippy/Web 快检由宿主运行。
- 本轮改动提交到当前任务分支；不推送、不启动下一会话。
