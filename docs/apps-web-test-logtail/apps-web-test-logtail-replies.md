## 第 1 条（2026-11-13T11:15:55Z）

宿主已补动态证据（2026-11-13 19:17，macOS，Node v26.5.0，在本任务 worktree 根目录、准备命令已完成的前提下）：连续两次执行状态块里的复现命令 npm --prefix apps/web test -- test/logtail-live-cleanup.test.ts，两次都退出 1，失败的是预期的那条断言：AssertionError: Leaked logtail-live processes after child suite exited: expected [ Array(7) ] to deeply equal []（第 58 行），不是环境错误、超时或权限失败；没有出现 listen EPERM 或 spawnSync ps EPERM。每次运行后 finally 清理生效，本次临时目录下没有残留进程。日志在主检出 .autome/output/t86-host-repro-1.log 与 t86-host-repro-2.log。结论：稳定复现，回归测试按预期在修复前失败，可以进入修复轮。会话沙箱里无法监听 127.0.0.1 与执行 ps，修复轮同样跑不了这条命令——修复后的通过证据由宿主的复现检查与门禁给出，修复轮请如实写明未能在会话内运行，不要因此改动回归测试。

