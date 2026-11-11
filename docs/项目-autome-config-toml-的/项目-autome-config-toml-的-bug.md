# 项目配置 parallel 未作用于调度器

status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p automed --lib dispatch::tests::project_file_parallel_override_queues_the_second_task -- --exact
根因: 项目注册时将全局 parallel 缓存为账本 parallel_limit，直接编辑项目配置不会同步该缓存，而 fill_slots 使用缓存而非合并后的项目配置。
复核: 待复核
repos: autome-v2

## 现象

任务卡报告全局并行数为 3、项目文件覆盖后仍同时启动多个任务。本轮在当前仓库基线 `1998ad10a3ea9fba55c10271daddac770b0d64a4` 复现：项目文件设置为 1，`config.get` 显示 1，连续创建两个任务时第二个仍立即启动，返回 `started=true`、`queue_position=null`、状态 `active/intake`。

## 复现

当前检出是 autome-v2 单仓库 Cargo workspace，以上命令从本 worktree 根目录执行，无需切换成员仓库。需要本地 Git、Rust 工具链及已缓存的 Cargo 依赖；`--offline` 禁止网络访问。

新增测试位于 `crates/automed/src/dispatch.rs` 的 `dispatch::tests::project_file_parallel_override_queues_the_second_task`，步骤为：

1. 建立隔离临时目录、真实 Git 仓库和内存 SQLite 账本，将隔离全局配置的并行数设为 3。
2. 通过真实 `project.add` 注册项目，然后直接写入临时项目文件 `[loop] parallel = 1`，不调用 `config.set_loop`。
3. 调用真实 `config.get` 并断言生效值为 1。
4. 依次通过真实 `task.create` 创建两个任务，走 `scheduler::tick`；先断言首个任务成功启动，再断言第二个应排队、队列位置为 1、占用槽位为 1。

使用现有 dry 测试上下文替代外部 Agent 进程，无真实模型调用、网络或 sleep；不修改运行中项目的配置、账本或用户全局配置。缺少 Git 会显式失败，不会静默跳过。

两次实跑均在“第二个任务不应启动”断言处失败（实际 true，期望 false），退出码 101；测试运行耗时分别约 4.82 秒、4.70 秒。解析断言和首个任务启动断言均已通过，失败不是编译错误、超时或命令不可用。原始输出见 `.autome/output/parallel-regression-1.log` 和 `.autome/output/parallel-regression-2.log`。

对照命令 `cargo test --offline -p automed --lib parallel_limit` 的 4 个现有测试全部通过（0 ignored），包括通过 `config.set_loop` 更新缓存以及调度器遵守账本限制的测试，日志见 `.autome/output/parallel-controls.log`。`git diff --check` 通过。未运行全量测试；新增回归测试按定位轮要求保持失败。

## 根因分析

- `crates/automed/src/config_io.rs:238` 的 `parse_project` 正常读取 `loop.parallel`；`crates/autome-domain/src/config.rs:382` 合并项目覆盖与全局默认。因此不是 TOML 字段不受支持。
- `crates/automed/src/dispatch.rs:694` 的项目注册逻辑把全局默认写入 `Project.parallel_limit`。
- `crates/automed/src/dispatch.rs:1004` 的 `config_get` 每次加载项目文件并合并配置，展示值能够更新。
- `crates/automed/src/dispatch.rs:1182` 的 `config_set_loop` 会保存配置并更新账本缓存；直接编辑文件不经过该入口。
- `crates/automed/src/scheduler.rs:1344` 的 `fill_slots` 用 `project.parallel_limit.max(1)` 决定启动名额。虽然后续 `advance` 会解析项目配置，但名额判定已经使用旧值。

问题是配置读取和调度决策的数据来源不一致。回归测试只约束外部行为，不要求未来修复必须保留或更新缓存；正确修复后应在同一命令下通过。本轮未修改任何产品逻辑，未进行真实 tmux/Agent 运行验收。

## 修复说明

定位轮未修复，交由后续修复轮处理。

## 复核意见

待独立复核。
