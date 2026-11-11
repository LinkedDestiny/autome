# autome status 最近五个会话定位

status: 已定位
fix-round: 1/3
复现命令: cd crates/autome-cli && cargo test --offline --test cli status_shows_latest_five_sessions_in_chronological_order -- --exact
根因: 内核按 started_at 降序返回会话，CLI 使用 sessions.iter().rev().take(5)，先反转全部记录再截取，选中了最早五个。
复核: 待复核
repos: .

## 现象

任务详情标题为「会话（最近 5 个）」，实际展示最早五个。七个会话按日期编号 1～7 时，期望展示 3、4、5、6、7，实际展示 1、2、3、4、5。

定位基线为 `74ae4d142a691419b26831d9d966ceac6810791a`，不是任务卡所述旧版本 `fd519b2`；当前代码仍存在同样问题。未发现补充信息文件。

## 复现

从本 worktree 根目录执行状态块中的命令。此项目是单 Git 仓库的 Cargo workspace，`repos: .` 指当前仓库，命令自行进入成员 `crates/autome-cli`。

新增 `crates/autome-cli/tests/cli.rs` 中的 `status_shows_latest_five_sessions_in_chronological_order`：

- 复用 Sandbox，在临时目录隔离账本和项目，通过真实 CLI 注册项目、创建草稿任务，避免启动模型或调度器。
- 通过真实 Store 写入七个已结束会话，日期固定为 2026-01-01～07，插入顺序为 4、1、7、2、6、3、5，不依赖墙钟、sleep 或数据库插入顺序。
- 先运行真实 `status <任务> --json`，断言完整返回 7～1，验证存储和内核数据链路。
- 再运行真实文本 `status <任务>`，检查标题，严格断言显示的会话 ID 恰好为 3～7 且按时间升序。此断言同时覆盖数量、最新集合、排除旧记录和展示顺序。

最终测试连续两次退出 101，均失败于文本展示断言，JSON 前置断言通过。实际数组为 `[history-1, history-2, history-3, history-4, history-5]`，期望数组为 `[history-3, history-4, history-5, history-6, history-7]`。这是业务断言失败，不是编译、超时或缺失命令。

原始日志位于 `.autome/output/regression-final-1.log` 和 `.autome/output/regression-final-2.log`，不纳入提交。使用 Cargo 离线模式，无网络请求；需要宿主已有 Rust 工具链和依赖缓存。

## 根因分析

1. `crates/automed/src/store.rs:1373` 的 `list_sessions` 明确约定 newest first，SQL 使用 `ORDER BY started_at DESC`。
2. `crates/automed/src/dispatch.rs:1596` 获取列表，`:1608` 按原顺序映射为 JSON，没有提前裁剪或遗漏新会话；回归测试的 JSON 断言验证了这一点。
3. `crates/autome-cli/src/main.rs:696` 使用 `sessions.iter().rev().take(5)`。输入 7～1 被反转为 1～7，再取前五条，结果必然是 1～5。

因此根因在文本展示层截取与反转的先后次序。后续修复应从新的在前的列表先选最新五个，再按旧到新展示；不能只去掉反转，否则展示顺序仍违反任务卡。无需改变内核的历史排序契约。

## 修复说明

修复第 1/3 轮：仅将 `crates/autome-cli/src/main.rs` 的 `sessions.iter().rev().take(5)` 改为 `sessions.iter().take(5).rev()`，先取最新五个，再按时间升序展示。存储和 JSON 历史排序契约不变；定位轮回归测试未修改、删除、跳过或弱化。

- 原复现命令修复前退出 101，实际为 history-1～5，期望为 history-3～7；修复后同一命令退出 0，1 个测试通过。
- 已运行配置门禁 `sh scripts/ci-local.sh`，退出 1，用时 165 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过。
- Web test 未通过：3 个测试文件失败、19 个通过；11 个测试失败、198 个通过、16 个跳过。日志包含 `listen EPERM: operation not permitted 127.0.0.1`、文件监听器 `EMFILE: too many open files, watch` 以及服务启动等待超时。当前环境阻止本地服务正常启动，不能宣称完整门禁通过；需由宿主在允许监听端口的环境重跑原门禁。本轮未更改门禁或测试来绕过限制。
- 原始日志：`.autome/output/fix-1-before.log`、`.autome/output/fix-1-after.log`、`.autome/output/fix-1-gate.log`，不纳入提交。未运行可选 `--package` 打包步骤。

## 复核意见

等待后续独立复核，本轮未启动下一会话。
