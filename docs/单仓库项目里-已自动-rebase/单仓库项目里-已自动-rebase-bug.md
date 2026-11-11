# 单仓库自动 rebase 输出空仓库名

status: 已定位
fix-round: 0/3
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

定位轮未修改产品逻辑，仅新增回归测试。待后续修复轮处理空仓库名的输出，保留工作区成员格式。

## 复核意见

待独立复核。
