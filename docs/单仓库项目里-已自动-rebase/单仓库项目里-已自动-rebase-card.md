# 单仓库项目里「已自动 rebase」那一行缺仓库名

## 现象
`autome run` 打印 `merge.rebased` 事件时，单仓库项目显示成「T-14 已自动 rebase： → main @ 5c8631f6a5」，箭头前是空的。

## 复现步骤
1. 单仓库项目里让任务停在等你合并，再往 main 提交一次
2. `autome merge <任务>`，看 `autome run` 的输出

## 期望行为
单仓库显示「T-14 已自动 rebase 到 main @ 5c8631f」之类没有空位的句子；工作区项目仍按「仓库 → 分支 @ 提交」逐个列出。

## 实际行为
`14:50  T-14 已自动 rebase： → main @ 5c8631f6a5`

## 环境
- autome-v2 main @ fd519b2，macOS（Darwin 27），已安装的 `~/.local/bin/autome` 由同一提交构建
- 项目 autome-v2 自身由 tmux 里的 `autome run` 驱动

## 线索
`crates/autome-cli/src/render.rs` 的 `"merge.rebased"` 分支；单仓库时事件里的 `repo` 为空串（`scheduler.rs` 写事件处用的是 `slot.name`）。
