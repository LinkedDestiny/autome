# autome status <任务> 的「最近 5 个会话」实际是最早的 5 个

## 现象
任务详情里的「会话（最近 5 个）」列出的是任务最早的 5 个会话（任务整理、设计、评审……），实现跑到第 20 轮时也看不到最近的会话。

## 复现步骤
1. 建一个任务并让它跑出 7 个以上会话（e2e 里用假 CLI 即可）
2. 运行 `autome status <任务>`

## 期望行为
列出时间最新的 5 个会话，按时间顺序显示。

## 实际行为
列出最早的 5 个。

## 环境
- autome-v2 main @ fd519b2，macOS（Darwin 27），已安装的 `~/.local/bin/autome` 由同一提交构建
- 项目 autome-v2 自身由 tmux 里的 `autome run` 驱动

## 线索
`crates/autome-cli/src/main.rs` 约 695 行：`sessions.iter().rev().take(5)`，而内核返回的会话列表本来就是新的在前。
