# M-03 · 审计 #3 独立复验

结论：通过。

## 验收命令（本会话自跑）
- `cargo build -p automed`、lint、typecheck 无错误。
- Web：13 文件 138 用例全过，无跳过（契约 113 + 25）。实现轮受沙箱阻塞的 7 个监听用例本会话已一并通过。
- desktop：104 用例，103 通过 0 失败 1 跳过（缺 electron，与基线相同）。

## 负向对照（契约逐条，均变红后 `git checkout server` 还原）
1. `MAX_QUICK_FAILURES` 3→4：supervisor「连续三次快速失败 offline」「缺失二进制」、core-restart 两条失败。
2. ticker 去掉 `pending`：「上次未返回不叠加，也不排队」失败。
3. ticker 去掉 `isBusy()`（本地超时即视为空闲）：「本地超时仍 busy 时不叠加」失败。
4. `skipped` 也记日志：「skipped 安静但仍回调 onTick」失败。
5. `CALL_TIMEOUT_MS` 15000→5000：「大于 Rust DRIVER_LOCK_WAIT」失败。
6. `stop()` 不清重启定时器：「重启延迟期间 stop 取消下一次 spawn」失败。
7. `isTickBusy()` 恒 false：「对账超时保持 starting 和 busy」失败。
还原后 `git status` 干净。

## 新造检查
即上面第 7 项（契约未列、对账超时侧的 isTickBusy 注入），结果为红，验收能拦住。无实现缺陷、无验证缺口。
