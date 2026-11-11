# 项目配置 parallel 未作用于调度器

status: 已定位
fix-round: 2/3
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

第 2/3 轮：在起点 `452f2fdf5156998de207caf26cf54e6f3580f6ba` 执行宿主指定的同一完整命令 `( cargo test --offline -p automed --lib dispatch::tests::project_file_parallel_override_queues_the_second_task -- --exact ) && ( sh scripts/ci-local.sh )`。原复现测试通过（1 passed / 0 failed / 0 ignored），配置门禁退出码 1，用时 374 秒，原始门禁输出见 `.autome/output/fix-2-gate.log`。

此次 Rust Test 步骤全部通过，包括 607 项 automed 库测试、80 项端到端测试；独立 End to end、Format、Clippy、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过。宿主报告的 Rust Test 失败未在本轮复现；提供的宿主日志只有步骤汇总，缺少具体失败用例与 panic 信息，尚不能确定其根因。

本地唯一失败步骤为 Web test：3 个文件失败、19 个文件通过；11 项失败、198 项通过、16 项因初始化失败跳过。日志包含 `listen EPERM: operation not permitted 127.0.0.1`、`EMFILE: too many open files, watch` 和服务初始化超时。当前沙箱禁止权限提升，无法在本会话解除监听限制。未修改、删除、跳过或弱化任何测试，也未修改门禁或产品代码；只更新本轮状态及验证记录，避免在没有失败证据时引入无关改动。完整门禁仍未通过，需宿主保留 Rust 失败的具体用例及错误段落，并在允许本地监听的环境复验同一命令。

本轮 `git diff --check` 通过，定位轮回归测试保持不变。复核重置为待复核；以下第 1/3 轮复核意见仅为历史记录，不代表本轮已通过。未启动后续会话。

第 1/3 轮：`fill_slots` 改为通过现有 `resolve_config` 读取最新全局配置与项目覆盖，以合并后的 `loop_defaults.parallel` 计算启动名额，不再使用注册时的账本缓存。保留原有最小值和槽位占用判断。

旧调度器及端到端测试中直接写账本的并行数准备步骤改为保存项目配置或调用 `config.set_loop`，全部行为断言保留。定位轮新增的 `project_file_parallel_override_queues_the_second_task` 已与本轮起点逐字比较，未修改、删除、跳过或弱化。

原复现命令修复前失败（0 passed / 1 failed，退出码 101），修复后通过（1 passed / 0 failed / 0 ignored，退出码 0）。日志分别为 `.autome/output/fix-1-before.log`、`.autome/output/fix-1-after.log`。

配置的项目门禁 `sh scripts/ci-local.sh` 已对最终代码执行，退出码 1：Format、Clippy、Rust workspace 测试（含 607 项 automed 库测试及 80 项端到端测试）、独立 End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 全部通过；仅 Web test 未通过。日志为 `.autome/output/fix-1-gate-final.log`。

Web 测试结果为 3 个文件失败、19 个文件通过；11 项失败、198 项通过、16 项因初始化失败跳过。环境报错包括监听 `127.0.0.1` 被拒绝（`listen EPERM`）、文件监听 `EMFILE` 及服务启动等待超时。本轮没有跳过或修改这些测试，完整门禁不能声明通过；需宿主在允许本地监听的环境重跑配置门禁。首轮门禁日志 `.autome/output/fix-1-gate.log` 中的两项端到端准备步骤问题已修正，最终门禁中均通过。

`git diff --check` 通过。未进行真实 tmux/Agent 会话验收；后续独立复核与调度交由 Autome。

## 复核意见


**通过**（第 1/3 轮，独立复核）。

- 复现命令复验：`dispatch::tests::project_file_parallel_override_queues_the_second_task` 1 passed / 0 failed / 0 ignored。该测试在定位提交 f5bada6 与修复提交 ba4e3e7 间无差异，未被删除、跳过或弱化。
- 门禁复验：automed 库测试 607 通过、端到端 80 通过、clippy `-D warnings` 与 `cargo fmt --check` 均通过。Web 测试因沙箱禁止监听端口失败，修复未触及 Web，属环境问题，留给宿主门禁。
- 根因成立：`fill_slots` 原先读注册时缓存的 `project.parallel_limit`，而 `config.get` 读合并后的配置；修复改为 `resolve_config(...).loop_defaults.parallel.max(1)`，数据来源与展示一致，仅一行产品代码改动。
- 范围：既有测试中直接写账本缓存的准备步骤改为写项目配置或 `config.set_loop`（否则新来源下这些测试失去意义），断言全部保留，改动合理且最小。
