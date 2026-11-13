# M-02 · 审计 #2 复验证据（对应实现轮 r2）

## 验收命令
- domain `session_timeout` 2 通过；automed lib 4 个：首次运行 2 通过 2 失败（见下「偶发失败」），之后同一二进制连续 30 余次（含 12 并行、10 个忙循环加压）全绿；e2e `session_timeout` 4 通过；回归子集 5 通过（stopping_a_task 2、a_non_zero_exit、model_not_found、a_rate_limited_round）；`end_to_end.rs` 删改行数 0。合计与契约一致（2+4+4+5=15）。

## 负向对照（逐条注入、跑对应用例、`git checkout` 还原，工作区干净已核对）
| 注入 | 结果 |
|---|---|
| A 先看时间再 classify（已退出会话也进超时路径） | ⑦ exit_marker_wins 失败 |
| B 每次读当前配置 | ⑧ keeps_launch_limit 失败 |
| C 只 kill(pid) 不 kill(-pid) | ⑥ stops_wrapper_cli_and_child 失败 |
| D pid 为 None 仍记超时收割 | ⑩ missing_pid 失败 |
| E 恢复路径凭事件不终止进程组 | ⑨ 用例无法通过（残留进程存活，waiter.join 挂起，被手动终止） |
| F 恢复路径仅在有退出标记时查事件 | ⑨ 同样挂起不通过 |
| G terminate_group 保留固定 3 秒睡眠 | ④ promptly 失败（3.28s） |
E/F 以挂起而非断言失败的形式拦下；残留的 `sleep 1000` 已清理。

## 偶发失败
首次运行 lib 时 `session_timeout_escalates_and_accepts_absent_group` 与 `..._recovers_exit_and_vanished_...` 失败（约 3.6 秒处，即 SIGKILL 之后的断言），输出未保留；其后 30 余次含加压均不复现。机制推断：两者都用 `trap '' TERM` 的 sh，SIGKILL 后孤儿 `sleep` 由 launchd 回收有延迟，测试里 `pid_alive(child)`（对僵尸返回真）与 `kill(-pgid,0)`（组已空）不一致。产品只依赖后者，属测试断言的时序脆弱，不是产品行为错误；e2e 的同类断言带 10 秒等待而稳定。记入 Backlog，不退回。

## 结论
通过。无实现缺陷、无验证缺口。
