# autome 输出接管道后被提前关闭时 panic

## 现象
`autome status T-10 | head -12` 打印完前几行后，stderr 出现 Rust panic。

## 复现步骤
1. 让 `autome status <任务>` 的输出多于 1 行
2. 运行 `autome status <任务> | head -1`

## 期望行为
下游关闭管道时安静退出（退出码 0 或 141 均可，不打印 panic 与回溯）。**SIGPIPE 必须保持被忽略**：`autome run` 在同一进程里跑内核，内核会经管道给 git 写 stdin（`crates/automed/src/git.rs` 里 `child.stdin` 的 `write_all`），git 提前退出时现在只会得到一个错误；若把 SIGPIPE 恢复成 `SIG_DFL`，整个驱动进程会被无声杀掉。

## 实际行为
```
thread 'main' panicked at library/std/src/io/stdio.rs:1165:9:
failed printing to stdout: Broken pipe (os error 32)
```

## 环境
- autome-v2 main（2026-11-11），macOS（Darwin 27）

## 线索
- 上一次尝试（已取消的 T-22）在 `main` 开头 `libc::signal(SIGPIPE, SIG_DFL)`，因上面的原因不能用。
- 可选做法：只针对写 stdout 失败且错误是 BrokenPipe 的情形安静退出——例如 panic hook 识别这一种 panic 后 `exit(0)`、其它 panic 照常打印；或把命令行输出统一走一个遇 BrokenPipe 就退出的写函数。
- 回归测试除了「关闭读端后 stderr 为空」，还要有一条断言：命令行进程里 SIGPIPE 仍是忽略状态（或：给已退出子进程的 stdin 写入时得到错误而不是进程被杀）。
