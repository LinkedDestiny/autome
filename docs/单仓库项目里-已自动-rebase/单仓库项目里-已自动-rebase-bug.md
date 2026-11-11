# 单仓库自动 rebase 输出空仓库名

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p autome-cli rebased_event_
根因: 单仓库 RepoSlot.name 按约定为空，merge.rebased 渲染无条件使用“仓库 → 分支”模板，导致箭头前空位。
复核: 待复核

## 现象

任务卡记录 `autome run` 输出 `T-14 已自动 rebase： → main @ 5c8631f6a5`，箭头前没有仓库名。
以任务卡正文为准，单仓库期望输出自然的“已自动 rebase 到 main @ 提交”；工作区仍逐个列出“仓库 → 分支 @ 提交”。无需给单仓库补造仓库名。

## 复现

定位基线：`bf07e7f`。当前为单仓库 Rust workspace，非 Autome 多仓库项目，无需成员目录切换或 `repos` 状态键。
从当前 worktree 根目录执行上述命令，无需网络、tmux、daemon 或并发 Git 操作。

在 `crates/autome-cli/src/render.rs` 的测试模块新增两个回归测试，直接调用实际事件渲染函数 `event_line`：

- `rebased_event_single_repo_has_no_empty_repository_label`：输入与调度器单仓库事件结构一致的空 `repo`，断言完整输出没有空仓库前缀。
- `rebased_event_workspace_preserves_repository_labels`：覆盖一个具名成员及两个具名成员，断言仓库名、目标分支、短 SHA 和分隔符全部保留，避免以仓库数量误判单仓库。

实跑退出码 `101`，结果为 1 passed、1 failed：单仓库用例因输出断言失败，工作区对照通过。

```text
实际：T-14 已自动 rebase： → main @ 5c8631f6a5
期望：T-14 已自动 rebase 到 main @ 5c8631f6a5
```

完整原始日志：`.autome/output/rebased-event-regression.log`（本地产物，不提交）。
`cargo fmt --all -- --check` 通过。未运行真实 tmux/合并流程；本次稳定复现直接覆盖发生错误的生产渲染函数。

## 根因分析

1. `crates/automed/src/layout.rs:75` 的 `TaskLayout::single` 明确设置 `name: String::new()`，空仓库名是合法单仓库表示。
2. `crates/automed/src/scheduler.rs:2654` 将 `slot.name` 原样写入事件的 `repo` 字段；现有调度器测试 `a_requested_rebase_records_merge_rebased_and_merges_without_stopping` 同样断言单仓库事件的 `repo` 为空。
3. `crates/autome-cli/src/render.rs:298` 的 `merge.rebased` 分支统一使用 `{} → {} @ {}` 模板，没有处理合法空仓库名，最终产生 `： → main`。
4. 原有渲染测试只覆盖空列表和具名成员，没有覆盖单仓库真实使用的“列表内一项且 repo 为空”。因此既有测试无法捕获该问题。

短 SHA 当前约定为 10 位，任务卡中的 7 位属于示例；本测试保留当前截断规则，仅锁定空仓库名导致的句式问题。

## 修复说明

在 `merge.rebased` 渲染分支中，当 repos 恰有一项且 repo 为空字符串时，输出“已自动 rebase 到 分支 @ 短 SHA”。其它输入继续使用原格式，保留具名工作区成员、分隔符和 10 位短 SHA。产品代码仅新增 9 行，定位轮回归测试未修改。

原复现命令 `cargo test --offline -p autome-cli rebased_event_` 修复前退出码 101（1 passed、1 failed），最终代码退出码 0（2 passed、0 failed）。日志分别为 `.autome/output/fix-round-1-before.log` 和 `.autome/output/fix-round-1-after.log`。

配置的项目门禁 `sh scripts/ci-local.sh` 已实跑。首次运行发现新增嵌套条件触发 Clippy，合并条件后针对最终代码重跑完整门禁：Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过；Web test 因文件监听限制 `EMFILE` 及本地监听权限 `listen EPERM 127.0.0.1` 失败，门禁最终退出码 1，耗时 221 秒。完整日志为 `.autome/output/fix-round-1-gate-final.log`。未修改、删除、跳过或弱化任何测试；门禁仍需宿主在允许本地监听的环境复验。

`git diff --check` 通过。未运行真实 tmux/合并流程或可选 package 门禁。等待 Autome 调度后续节点，本轮不启动下一会话。

## 复核意见

待独立复核。
