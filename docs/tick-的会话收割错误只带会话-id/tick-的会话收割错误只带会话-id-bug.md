# tick 会话收割错误缺少任务 ID

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p autome-cli --test cli run_json_tick_error
根因: scheduler::tick_locked 的会话收割错误分支只用 s.id 包装错误，遗漏已有的 s.task_id；CLI 原样输出该错误。
复核: 通过

## 现象

已退出的整理会话在 tick 中被收割时，非法项目配置导致错误，第一条 JSON 错误仅含 `ses-…: … TOML parse error`，无法直接识别任务。任务卡已明确要求任务 ID 在前、会话 ID 在后，因此这是产品错误上下文缺失，不是需要弱化断言的测试问题。

基线为 `acbc590`（包含任务输入与协议，产品基线 `d9a8cc1`）。本项目是当前根目录的单 Git 仓库，Cargo crates 是仓库内包，不是 Autome 工作区成员；复现命令直接在当前 worktree 根目录执行，无需成员目录切换。

## 复现

新增 `crates/autome-cli/tests/cli.rs` 中的 `run_json_tick_error_reaped_session_identifies_task`，与既有草稿任务用例共用运行和 SIGINT 检查流程，使用不同沙箱目录，允许并行执行。

1. 通过真实 CLI 注册隔离项目并新建非草稿任务，使用假模型 CLI，不访问外部服务。
2. 确认账本中恰有一个运行中的会话；有界轮询退出文件，直到 `ExitMarker::parse` 完整解析成功（最多 10 秒），固定“进程已退出但尚未收割”的状态。
3. 将任务置为 `Active { node: Gate }`，写入非法配置 `invalid = [`。
4. 执行真实 `autome run --json --interval 0.05`，读取第一行，发 SIGINT 并等待退出。
5. 要求第一条错误以该任务和该会话的精确前缀 `T-… ses-…:` 开头，并包含 `TOML parse error`；保留正常退出、stderr 为空及没有 idle 尾行的检查。原草稿用例仍要求 `T-…:` 前缀，保护内核步骤原格式。

连续两次执行状态块命令均返回 101：各 1 个通过、1 个失败；失败均为新用例的错误前缀断言，而非超时或构建失败。第一次实际首行为 `{"error":"ses-3a934ff2ce29432f9fb5a8d16a4d8bcb: … config.toml 格式错误：TOML parse error …"}`。原用例两次均通过，新用例在前缀断言前的 SIGINT 正常退出与 stderr 空检查均通过。

完整日志在 `.autome/output/tick-reap-regression-1.log`、`.autome/output/tick-reap-regression-2.log`。`cargo fmt --all -- --check` 通过，日志在 `.autome/output/tick-reap-fmt.log`。这些为本地验证产物，不提交。未运行完整门禁；本轮按项目 before_merge 配置只进行定向验证。

## 根因分析

- `crates/automed/src/scheduler.rs:174–188` 先遍历运行会话，`reap_session` 返回错误时使用 `format!("{}: {e}", s.id)`；同一个 Session 已有 `task_id`，但包装时没有保留。
- 收割通过退出标记确认结束，随后 `deliver_outcome` 调用 `advance`；`advance` 在读取项目配置时传播非法 TOML 错误。错误最终由上述 tick 分支包装，丢失任务上下文。
- 同文件后续任务内核步骤使用任务 ID 包装错误。先收割、后推进的顺序使本场景的首条错误必然来自收割，不能通过重排错误规避。
- `crates/autome-cli/src/main.rs:854–861` 逐条输出同一 errors 数组：JSON 写入 error 字段，可读模式写入 stderr 的 `  ! ` 行。两种输出共同继承内核的上下文缺失；本轮动态回归覆盖 JSON，可读模式结论来自该共享输出链路的静态检查。
- 原草稿用例刻意没有运行会话，只能验证内核步骤错误，无法保护收割分支。新增用例固定退出标记而非固定睡眠，不再依赖第一次 tick 与会话退出之间的竞态。

## 修复说明

修复第 1/3 轮：仅修改 `scheduler::tick_locked` 的会话收割错误包装分支。查询所属任务成功时使用 `任务 ID 会话 ID: 原因`；查询失败时回退为 `会话 ID: 原因`，保留原始收割错误。内核步骤格式与错误输出顺序不变，CLI 两种输出继续使用同一报告。未修改定位轮回归测试。

验证结果：

- 修复前执行原复现命令，退出码 101，1 通过、1 失败，仍为收割错误前缀断言失败；日志 `.autome/output/fix-1-before.log`。
- 修复后执行同一复现命令，退出码 0，2 通过、0 失败；日志 `.autome/output/fix-1-after.log`。
- `cargo test --offline -p automed --test end_to_end a_session_that_finished_during_a_restart_is_consumed_on_recovery`：1 通过，覆盖重启后的已退出会话收割；日志 `.autome/output/fix-1-recovery.log`。
- `cargo test --offline -p automed --test end_to_end a_tick_skips_while_another_driver_holds_the_ledger`：1 通过，覆盖 tick 驱动锁；日志 `.autome/output/fix-1-driver.log`。
- `cargo fmt --all -- --check` 通过；日志 `.autome/output/fix-1-fmt.log`。

未运行全量门禁 `sh scripts/ci-local.sh`，留给宿主合并前执行。所属任务不存在的回退与可读输出链路仅静态检查，未动态验证。

## 复核意见

复核通过（第 1/3 轮）。
- 独立复验：复现命令 `cargo test --offline -p autome-cli --test cli run_json_tick_error` 退出正常，2 通过、0 失败（含新用例 `run_json_tick_error_reaped_session_identifies_task` 与原草稿用例）。
- 差异检查（4dd3653..860c4bb，crates/ 下）：仅 `crates/automed/src/scheduler.rs` 收割错误分支 +7/-1；`crates/autome-cli` 无任何改动，回归测试未删除、跳过或弱化，仍断言精确前缀 `T-… ses-…:`。
- 根因成立：该分支只用 `s.id` 包装错误，丢了已有的 `s.task_id`；修复在此处补任务 ID，查询任务失败时回退为原 `会话 ID: 原因`，不吞原错误。
- 范围最小：未触及内核步骤格式、错误顺序和 CLI 输出链路。
- 未跑全量门禁，留给宿主合并前执行。
