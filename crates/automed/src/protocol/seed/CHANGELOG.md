# 协议改动记录

每个版本一节，每条改动一个块。字段的含义见 `docs/plans/2026-09-17-loop-self-improvement-proposal.md` §3.4：

- `kind` —— `behavioral` 改变会话的行为，必须带一个「改前红、改后绿」的 eval 用例；
  `clarify` 只是把同一件事说清楚，不需要用例，靠评审轮签字；
  `retire` 移除条文，只能以移除实验的形式提出，`direction` 必须是 `flat`。
- `evidence` —— 至少两个任务的证据。一个任务只是一次倒霉。
- `predicted_impact.metric` —— 只能取内核真的记录的那些指标（`TaskMetrics::METRIC_NAMES`）。
- `realized_impact` —— 由 Autome 在 `horizon` 个任务之后回填，不由提出改动的人填。

## protocol/v1

基线。协议从这一版起只以 autome-v2 的 `crates/automed/src/protocol/seed/` 为准、随
Autome 发布（ADR-0003），此前在 `~/.autome/protocol/` 仓库里的版本号和改动记录随那个
仓库一起清空，从这里重新起算。条文本身没有回退：它们是此前各轮迭代的结果，包括工作区
项目的文档路径由内核按会话工作目录填写、状态块的 `repos:` 行。

```yaml
- id: C-01
  kind: clarify
  clause: README.md
  evidence: [voice-schedule 三次真实运行的协议审计, offchat T-6 工作区任务跑通到合并]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

以后的改动在本文件末尾加新的 `## protocol/vN` 一节，按上面的字段写，一条改动一个块。
