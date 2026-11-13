# 实现 #6：宿主门禁失败修复（M-02）

## 范围与根因

本轮为修复轮，不推进新里程碑。保留 M-01～M-04 已审计关闭状态、status: 已完成、current-milestone: 无、next-action: 无；implementation-round 更新为 6/20。没有待审项需要改变。

宿主提交 5d796d0fa821 的完整门禁日志位于主工作树 `.autome/output/gates/T-54/merge_gate-5d796d0fa821.log`。第 1137～1138 行定位唯一失败：`launcher::tests::session_timeout_escalates_and_accepts_absent_group` 在 `terminate_group(pid).unwrap()` 返回 EPERM；lib 为 838 通过、1 失败。摘录保存于本 worktree `.autome/output/r6-gate-failures.txt`。

原样单跑同一用例复现。临时区分探测与发信号的错误信息后，再次复现为 `probe group 67777: Operation not permitted (os error 1)`，发生于约 3 秒的 SIGKILL 升级阶段。由此定位为终止后的组探测错误，不是信号发送失败。孤儿进程被回收期间的短暂不可探测是机制解释，未额外采集内核回收轨迹。

修复：`kill(-pid, 0)` 返回 EPERM 表示不能探测/发送信号，不能据此确认组消失；保守按仍存在继续原有 50ms 轮询，直到 ESRCH 或原有期限。只有 ESRCH 返回终止成功；实际 TERM/KILL 的 EPERM 仍返回错误，未放宽终止确认条件。临时诊断信息已还原。同类检查范围为 terminate_group 两处 errno 判断：探测与信号发送，后者保留拒绝行为；pid_alive 的心跳语义按 N-3 不改。

## 红绿与验证

- 红：`cargo test -p automed --lib launcher::tests::session_timeout_escalates_and_accepts_absent_group -- --exact --nocapture`，0 通过、1 失败，3.04 秒；`.autome/output/r6-red.txt`。
- 定位：同一命令，0 通过、1 失败，3.06 秒；`.autome/output/r6-probe.txt`，明确是 probe 的 EPERM。
- 绿：修复后同一命令，1 通过，3.32 秒；`.autome/output/r6-green.txt`。原检查已在项目测试体系中，无需搬入或复制，未更改测试内容、断言、夹具，也未增加跳过。
- `cargo test -p autome-domain session_timeout`：2 通过；`.autome/output/r6-domain.txt`。
- `cargo test -p automed --lib session_timeout`：4 通过；`.autome/output/r6-lib.txt`。
- `cargo test -p automed --test end_to_end -- session_timeout timeout_retry stopping_a_task a_non_zero_exit model_not_found a_rate_limited_round`：11 通过、0 失败、0 忽略，28.26 秒；`.autome/output/r6-e2e.txt`。

按不同用例计，本轮通过总数 17，上一实现轮基线 54，差 -37：本轮是 domain 2 + lib 4 + e2e 11；上一轮是 e2e 18 + prompt 36。减少来自取消本轮无关的 prompt 36、格式化回归 5、既有暂停回归 2，增加 domain/lib 6（-36-5-2+6=-37）。新增测试 0，红绿及重复执行不重复计数。

未跑全量门禁、宿主快检；最终结果由宿主在提交后/合并前重跑决定。人工 H-01/H-02 未执行。

## 四项自审

1. **情形表逐行**：本轮影响终止方式。TERM 正常退出（promptly）、忽略 TERM 后升级 KILL（escalates）、已消失组（同用例）、恢复时有退出标记/无标记但组成员仍在（recovers 两分支）均通过。e2e 覆盖超时终止、正常退出优先、启动时冻结上限、缺少 pid、完整重试耗尽、暂停后恢复六种情形；其它会话类型的结果分派未变，本轮不重复全量验证。
2. **失败分支**：探测 EPERM 原样红绿；真正发送信号失败仍返回 Err（代码核对，不把权限错误当不存在）；超时重试用尽、缺失 pid 保持 running、非零退出、模型不存在、限流均由子集验证。读盘失败、空日志、外部服务不可达不适用：未改相关路径。持续不可探测不会成功确认：保留原有期限与错误返回，代码核对，未模拟跨用户权限。
3. **全部迁移**：组存在→TERM→消失、组存在→TERM 宽限→KILL→消失、已消失→立即成功均实跑；新增 EPERM→继续等待由原失败用例红绿覆盖。会话 running→timed_out→retry→failed、paused→结果挂起→resume→retry、running→exited 由 e2e 通过。未确认时不收割的缺 pid 对照通过；持续 EPERM 的上限退出为代码核对，未声称实跑。
4. **域边界**：原 escalates 用例覆盖 i32::MIN、-1、0、1 拒绝，以及已不存在组重复终止；domain 覆盖 limit*60-1 与 limit*60。未新增配置输入域；其余日期、空串边界不适用。

补充稳定性检查：上述 exact 单用例连续额外运行 3 次均通过，输出 `.autome/output/r6-repeat-{1,2,3}.txt`。这不证明所有调度时序，但原失败已在不改断言下重复转绿。
