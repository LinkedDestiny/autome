# CLI 写命令在长 tick 持锁期间提前失败

status: 已定位
fix-round: 0/3
复现命令: cd crates/autome-cli && cargo test --offline -p autome-cli --test cli write_command_waits_for_a_driver_lock_held_for_fifteen_seconds -- --exact --nocapture > ../../.autome/output/driver-lock-regression.log 2>&1
根因: 写命令统一只等待驱动锁 10 秒，而 scheduler 在整个 tick（包括同步清理等长步骤）期间持锁，正常竞争超过预算即返回 Internal 并退出。
复核: 待复核
repos: crates/autome-cli, crates/automed

## 现象

任务卡报告 main 上执行 `autome merge T-36` 连续两次失败，当时 T-27 正在清理。该现场未重新运行；本轮用隔离状态库复现相同的写命令锁失败，不操作用户任务。

写命令遇到仍在工作的锁持有者时约 10 秒后退出，错误为「另一个 Autome（桌面端或命令行）正占着状态库，稍后再试」。任务卡要求持锁 15 秒后写命令仍能成功。

## 复现

基线：`3ffc06c`。从当前 worktree 根目录运行状态块中的单行命令。使用 Cargo workspace 成员 `crates/autome-cli`，核心锁实现位于 `crates/automed`；无需网络、模型账号或桌面端。

回归测试：`crates/autome-cli/tests/cli.rs` 中的 `write_command_waits_for_a_driver_lock_held_for_fifteen_seconds`。

1. 建立临时 Git 仓库和独立 SQLite 状态库，注册项目；先执行一次合法草稿创建，作为无竞争正对照。
2. 测试进程用生产 `driver_lock::try_acquire` 预先拿锁，并确认第二次获取被拒绝。
3. 启动真实 `autome task new ... --draft --json` 子进程，启动后再计时持锁 15 秒。草稿创建与 merge/approve 走同一写命令锁入口，不需要模型会话或完整业务流程。
4. 等待命令退出，回收持锁线程；确认锁可以重新获得，且释放后另一次草稿创建成功。
5. 断言竞争期间命令成功、没有在释放前返回、输出为可解析 JSON、创建记录实际可读，且 stderr 不输出等待提示。

15 秒是受控竞争输入，不是靠 sleep 猜测两个进程的启动顺序；先持锁后启动消除了通常的抢锁竞态。测试设置 130 秒进程上限并回收子进程；触及上限属于环境/实现异常，不计作本 Bug 复现。严重进程饥饿仍可能影响任何基于真实时间的端到端测试。

实跑两次均在业务断言处失败：Cargo 退出码 101，CLI 退出码 1，stdout 为空，stderr 为 `autome：另一个 Autome（桌面端或命令行）正占着状态库，稍后再试`。测试分别耗时 16.91 秒、17.27 秒（含初始化与持锁线程回收，不是 CLI 等锁时长）；释放前后正对照均通过。不是编译失败、命令缺失或测试上限超时。第二次从状态块指定成员目录执行。

原始日志：`.autome/output/driver-lock-regression-first.log`、`.autome/output/driver-lock-regression.log`。`cargo fmt --all -- --check` 与 `git diff --check` 通过。未运行全量测试，也未重新驱动 T-27/T-36 的真实清理/合并流程。

## 根因分析

- `crates/automed/src/dispatch.rs:214` 将 `DRIVER_LOCK_WAIT` 固定为 10 秒；`handle_command` 在调度前为非只读、非 tick 的命令调用 `driver_lock::acquire`，超时直接返回 `Internal`。`task.create`、`task.approve`、`task.merge` 共用这一入口。
- `crates/automed/src/driver_lock.rs` 的 `acquire` 每 50 毫秒重试，达到 deadline 返回 `None`。这里的注释假定 tick 最多几百毫秒，与长步骤不相符。
- `crates/automed/src/scheduler.rs:127` 的 `tick` 在 `tick_locked` 返回后才释放锁；`tick_locked` 调用 `run_core_step`，其中 `Node::Cleanup` 同步调用 `cleanup`（约第 2979 行），长步骤没有在此期间释放驱动锁。
- `crates/autome-cli/src/client.rs` 直接调用核心 `handle_command`，并非网络请求自身的超时。

后续建议选择任务卡允许的延长等待方案（60–120 秒，并为非 JSON 模式增加超过 2 秒的等待提示）。它保持现有 tick 原子性；拆分锁生命周期需要重新证明跨步骤任务状态与并发启动的一致性，不适合作为本问题的首选小修复。本轮未修改产品代码，也未覆盖非 JSON 等待提示的时间行为。

## 修复说明

定位轮，仅添加失败回归测试和本文档；修复轮尚未开始。

## 复核意见

待独立复核。
