# autome 输出管道提前关闭时 panic

status: 已定位
fix-round: 2/3
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

第 2/3 轮：保留上一轮 stdout BrokenPipe 修复，产品代码和定位轮回归测试均未修改。宿主提供的日志末尾只标明 Rust Test 失败，没有具体失败用例或断言；本轮按同一命令重跑后 Rust Test 全部通过，尚不能确定宿主上次失败的根因，不能将重跑通过表述为修复了该未知失败。

本轮执行 `( cargo test --offline -p autome-cli --test cli broken_pipe -- --nocapture ) && ( sh scripts/ci-local.sh )`：断管回归 2 passed；项目门禁用时 639 秒、退出 1。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过；其中 Rust Test 包含 CLI 集成测试 13 条、domain 测试 345 条、内核单元测试 614 条、端到端测试 81 条，均无失败。唯一失败步骤为 Web test：4 failed / 29 passed 文件，11 failed / 271 passed / 19 skipped 测试，日志包含 `listen EPERM: operation not permitted 127.0.0.1` 和 `EMFILE: too many open files, watch`。Desktop 的 Electron DOM 测试因二进制缺失且下载失败由原测试逻辑跳过，不能视为真实 Electron 验收通过。未修改、删除、跳过或弱化测试及门禁；未运行可选 package。

完整门禁日志为 `.autome/output/fix-2-gate.log`，Web 失败详情摘录为 `.autome/output/fix-2-gate-errors.log`，再次执行原复现命令的日志为 `.autome/output/fix-2-repro.log`。本轮完整门禁仍未通过，需由宿主在允许本地服务监听的环境复验；若 Rust Test 再次失败，需要保留具体失败用例及断言，才能继续定位。以下保留第 1/3 轮记录。

本轮在 `crates/autome-cli/src/main.rs` 为 CLI 的 `print!` / `println!` 添加局部输出封装，直接检查 stdout 写入及 flush 的错误。仅在 `ErrorKind::BrokenPipe` 时安静退出 0；其它输出错误继续 panic，其它业务错误处理不变。未改变 SIGPIPE 处置，也未修改内嵌驱动的子进程 stdin 写入行为。

定位轮两条回归测试保持原样。原复现命令在修复前为 1 passed、1 failed，修复后为 2 passed、0 failed，分别验证管道关闭后的安静退出和 SIGPIPE 仍被忽略。日志为 `.autome/output/fix-1-before.log`、`.autome/output/fix-1-after.log`。`cargo fmt --all -- --check` 和 `git diff --check` 通过。

已运行配置门禁 `sh scripts/ci-local.sh`，833 秒后退出 1。Format、Clippy、Rust 全量 Test、End to end、Adapter flags、Desktop、Contract、Web install/lint/typecheck/build 步骤通过；Web test 为 4 failed / 29 passed 文件、11 failed / 271 passed / 19 skipped 测试。失败日志明确包含 `listen EPERM: operation not permitted 127.0.0.1`，以及文件监听 `EMFILE: too many open files, watch`，当前受限环境未能完成服务监听验收。Desktop 步骤虽通过，但 Electron DOM 测试因缺少 Electron 二进制被原测试跳过，不能视为真实 Electron 验收通过。未修改或跳过任何测试，未运行可选 package。完整日志为 `.autome/output/fix-1-gate.log`；整体门禁尚未通过，需由宿主在可监听环境重新验证。

## 复核意见

第 2/3 轮：待独立复核。以下为上一轮复核记录，不代表本轮通过。

复核第 1/3 轮：通过。
- 复现命令独立重跑：2 passed、0 failed；`cargo clippy -p autome-cli --all-targets -D warnings` 通过。
- 差异 9e751f8..184cceb 只改 `crates/autome-cli/src/main.rs`（+23 行）和 bug 文档，回归测试未删、未跳过、未弱化。
- 根因成立：`println!` 在 BrokenPipe 时 panic。修复用局部 print!/println! 宏，仅 BrokenPipe 时静默退出 0，其它写错误仍 panic；未改 SIGPIPE 处置，信号保护测试继续通过。
- 范围最小。其余 stdout 写入（行 814、890、893）已是 `let _` 或显式处理，没有遗漏的 panic 路径。
- 提醒：修复轮报告的全量门禁 Web 测试失败原因是沙箱的 listen EPERM 和 EMFILE，与本改动无关。宿主应在可监听环境确认。
