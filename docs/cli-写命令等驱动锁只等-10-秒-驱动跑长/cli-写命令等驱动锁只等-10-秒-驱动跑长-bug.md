# CLI 写命令在长 tick 持锁期间提前失败

status: 已定位
fix-round: 1/3
复现命令: cd crates/autome-cli && cargo test --offline -p autome-cli --test cli write_command_waits_for_a_driver_lock_held_for_fifteen_seconds -- --exact --nocapture > ../../.autome/output/driver-lock-regression.log 2>&1
根因: 写命令统一只等待驱动锁 10 秒，而 scheduler 在整个 tick（包括同步清理等长步骤）期间持锁，正常竞争超过预算即返回 Internal 并退出。
复核: 通过
repos: crates/autome-cli, crates/automed, apps/web

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

第 1/3 修复轮采用延长等待方案：`dispatch` 的写命令驱动锁等待上限从 10 秒提高到 120 秒。保留整个 tick/写命令持锁的生命周期，避免拆锁改变任务状态与会话启动的原子性。超过 120 秒仍会返回原有竞争错误。

锁等待支持可选提示回调，竞争达到 2 秒后只调用一次。CLI 仅在非 `--json` 模式启用，向 stderr 输出「等待驱动释放状态库…」；桌面调用默认不提示。同步修正了 tick 最多几百毫秒的过时注释。

门禁发现 Web 的调用超时需大于核心锁等待时间，已将 `CALL_TIMEOUT_MS` 从 15 秒同步提高到 125 秒，保留现有契约测试。

验证：

- 修改前运行状态块原命令，退出 101，出现同一锁竞争错误；日志 `.autome/output/driver-lock-before-fix.log`。
- 修改后运行同一命令通过（1 passed，16.16 秒），JSON stderr 为空，创建记录可读；日志 `.autome/output/driver-lock-regression.log`。定位轮回归测试未修改、删除或弱化。
- 新增锁提示单元测试，验证立即拿锁无提示、持续竞争只提示一次、释放后可重新拿锁；已通过。
- 隔离临时仓库的实际非 JSON CLI 验证通过：前 1.5 秒无提示，随后收到一次提示，释放锁后写命令成功且没有重复提示；日志 `.autome/output/fix-notice-smoke.log`。
- 配置门禁 `sh scripts/ci-local.sh` 已完整执行，383 秒后退出 1；Format、Clippy、Rust 全量测试、Contract、Web lint/typecheck/build 通过。Desktop 的 Electron DOM harness 状态为 null，独立启动确认信号为 SIGABRT、stdout/stderr 为空（`.autome/output/fix-electron-probe.log`）。Web 测试包含监听 `127.0.0.1` 被拒绝（EPERM）、文件监听 EMFILE 和超时失败；未修改或跳过这些测试。完整日志 `.autome/output/fix-gate.log`。
- 同次门禁还抓到了 Web 调用超时小于核心等锁预算的契约失败；同步修复后，`cd apps/web && npx vitest run test/core-supervisor.test.ts` 全部 10 项通过（`.autome/output/fix-web-timeout.log`）。该一行修复后未重跑完整门禁，整体门禁仍记为未通过，需宿主在支持 Electron/本地监听的环境复跑。
- `git diff --check` 通过；定位轮 `crates/autome-cli/tests/cli.rs` 无差异。

未重新驱动用户 T-27/T-36 的真实清理/合并流程，未运行可选打包门禁。

## 复核意见

结论：通过（第 1/3 轮复核）。

- 复现命令独立复跑：`write_command_waits_for_a_driver_lock_held_for_fifteen_seconds` 1 passed，17.27 秒，退出 0（`.autome/output/review-repro.log`）。
- 回归测试：定位提交 9ee0769 到修复提交 fc7feae 之间 `crates/autome-cli/tests/cli.rs` 无差异，未删除、跳过或弱化；该测试以 15 秒真实持锁驱动真实 CLI 子进程，断言成功、JSON 可解析、记录可读，有意义。
- 根因站得住：`DRIVER_LOCK_WAIT` 固定 10 秒，而 tick 全程持锁（含同步清理），与现象一致；延长到 120 秒后同一测试由失败转通过。
- 范围：产品改动仅为等待上限、可选提示回调（仅非 `--json` 的 CLI 启用）和 Web 调用超时 125 秒（需大于核心等锁预算，合理）。新增提示单元测试；`cargo test -p automed driver_lock` 5 项通过，Web `core-supervisor` 10 项通过，clippy `-D warnings` 无告警。
- 说明：修复轮记录的完整门禁 Desktop/Web 失败来自 Electron、本地监听 EPERM 等沙箱环境限制，与本改动无关，本轮未重跑完整门禁；120 秒仍是上限，超出才返回原错误，属已声明的取舍。
