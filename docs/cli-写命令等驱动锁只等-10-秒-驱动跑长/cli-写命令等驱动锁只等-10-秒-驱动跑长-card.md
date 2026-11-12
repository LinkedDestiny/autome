## 现象
驱动（`autome run`）在 tick 里持有驱动锁，一个 tick 可能很长（启动 `_verify` 检出、清理 worktree、跑守卫）。CLI 的写命令只等 10 秒（`crates/automed/src/dispatch.rs:214` `DRIVER_LOCK_WAIT`），等不到就报「另一个 Autome（桌面端或命令行）正占着状态库，稍后再试」直接退出。11-12 按 `autome merge T-36` 时连续两次失败，第三次才成功；T-27 当时正在清理。
## 复现步骤
1. 让驱动跑一个长 tick（例如一个任务进入清理，或合并后验证 main 起检出）。
2. 同时执行任意写命令（`autome merge <t>`、`autome approve <t>`）。
3. 10 秒内 tick 没结束 → 命令失败。
## 期望行为
- 写命令等待锁的时间与 tick 的实际长度相称（例如最多 60–120 秒），等待超过 2 秒时在 stderr 打一行「等待驱动释放状态库…」，`--json` 模式不打。
- 或者（更好）tick 在长步骤之间释放并重新获取锁，让写命令插队；两者选其一并在证据里说明理由。
- 补 e2e：锁被另一个持有者占 15 秒时，写命令仍成功。
## 环境
main，`crates/automed/src/dispatch.rs:225-250`、`crates/automed/src/driver_lock.rs`、`crates/automed/src/scheduler.rs:127-140`。
