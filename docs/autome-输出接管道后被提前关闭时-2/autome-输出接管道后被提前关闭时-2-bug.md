# autome 输出管道提前关闭时 panic

status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p autome-cli --test cli broken_pipe -- --nocapture
根因: status 的 task_detail 使用 println! 写 stdout，读端关闭后产生 BrokenPipe，打印宏直接 panic，绕过 CLI 的 Result 错误处理。
复核: 待复核
repos: autome-v2

## 现象

正常读取 `autome status <任务>` 时成功输出多行且 stderr 为空；stdout 管道读端已关闭时，真实 CLI 退出码为 101，stderr 打印 `failed printing to stdout: Broken pipe (os error 32)`，开启 RUST_BACKTRACE 时还会打印回溯。期望是退出码 0 或 141（或被 SIGPIPE 终止的等价状态）且 stderr 为空，同时 CLI 必须继续忽略 SIGPIPE，不能靠恢复 SIG_DFL 修复。

## 复现

在当前 worktree 根目录执行状态块中的命令。这里是单一 Git 仓库的 Cargo workspace，Cargo 通过 `-p autome-cli` 选择成员，无需切换成员目录。离线运行依赖本机已缓存的 Cargo 依赖；依赖缺失或编译失败不算复现。

环境：macOS Darwin 27.0.0，rustc 1.93.1。无补充信息文件。

新增 `crates/autome-cli/tests/cli.rs` 中两条真实进程回归测试：

- `broken_pipe_status_exits_quietly`：复用隔离 Sandbox 注册本地临时仓库、创建 draft 任务，先验证正常 status 成功、多行、stderr 为空；再在启动同一个 status 命令前关闭 pipe 的唯一读端，断言 stderr 为空及允许的退出状态。无需模型、网络或 sleep。相比直接 `| head -1`，不受短输出可能在 head 关闭前全部写完的竞态影响。
- `broken_pipe_sigpipe_remains_ignored_in_cli_process`：运行真实 CLI 的 render-stream 入口，通过首条输出握手确认已经进入运行阶段，再发送 SIGPIPE；断言它继续输出第二条消息、成功退出且 stderr 为空。覆盖任务卡提及的 main 开头恢复 SIG_DFL 的错误修法，不改变测试进程的信号处置。

连续两次执行均为：1 passed、1 failed；信号保护测试通过，关闭 stdout 测试因真实 CLI 的 BrokenPipe panic 失败。原始日志保存在 `.autome/output/broken-pipe-first.log`，重复执行日志在 `.autome/output/broken-pipe-repeat.log`。测试失败是预期的定位结果，并非超时或命令缺失。`cargo fmt --all -- --check` 与 `git diff --check` 均通过；未运行全量测试。

## 根因分析

`crates/autome-cli/src/main.rs:494` 的 `status` 成功获取任务后，在非 JSON 路径调用 `task_detail`；后者从第 595 行起使用多个 `println!`。Rust 标准输出遇到无读端的管道返回 BrokenPipe，打印宏在 `std::io::stdio` 内触发 panic。`main` 第 76 行仅匹配 `run` 返回的 Result，无法将此 panic 转换为安静退出。正常输出对照排除了任务查询或数据构造错误，回溯指向 task_detail 的打印调用。

当前 CLI 的 SIGPIPE 忽略行为已由真实进程信号测试验证。`crates/automed/src/git.rs:156-161` 会在同进程线程中向子进程 stdin 执行 `write_all`；把 SIGPIPE 全局恢复默认处置会使这类写入有机会终止整个驱动进程，不能采用。后续修复应只处理 stdout 的 BrokenPipe，保留其它错误和 panic 的可见性。

## 修复说明

定位轮未修改产品代码；仅新增回归测试与本文档。后续修复轮处理。

## 复核意见

待独立复核。
