# 守卫 audit_edited_the_design 把状态块更新也当成审计改设计

## 现象
几乎每个审计轮之后都出现「守卫提醒：audit_edited_the_design」，但审计只改了设计文档开头的状态块（`status`、`current-milestone`、`next-action`）和验收契约里的「最新证据」行——这些正是协议要求审计改的地方。

## 复现步骤
1. 审计轮把里程碑从「待审」改为「已完成」，同时按协议更新状态块与对应契约下的「最新证据：…」行
2. 看守卫结果

## 期望行为
只有改了里程碑表、状态块、「最新证据」行以外的内容才提醒。

## 实际行为
T-12、T-13、T-15 的审计轮都触发了提醒，例如 T-12 审计 #5 的差异只有状态块 5 行与「最新证据」1 行。

## 环境
- autome-v2 main @ fd519b2，macOS（Darwin 27），已安装的 `~/.local/bin/autome` 由同一提交构建
- 项目 autome-v2 自身由 tmux 里的 `autome run` 驱动

## 线索
`crates/automed/src/guards.rs` 的 `design_changed_outside_milestones` 的计算处，把状态块与「最新证据：」行加进允许范围。
