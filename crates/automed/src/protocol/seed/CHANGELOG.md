# 协议改动记录

每个版本一节，每条改动一个块。字段的含义见 `docs/plans/2026-09-17-loop-self-improvement-proposal.md` §3.4：

- `kind` —— `behavioral` 改变会话的行为，必须带一个「改前红、改后绿」的 eval 用例；
  `clarify` 只是把同一件事说清楚，不需要用例，靠评审轮签字；
  `retire` 移除条文，只能以移除实验的形式提出，`direction` 必须是 `flat`。
- `evidence` —— 至少两个任务的证据。一个任务只是一次倒霉。
- `predicted_impact.metric` —— 只能取内核真的记录的那些指标（`TaskMetrics::METRIC_NAMES`）。
- `realized_impact` —— 由 Autome 在 `horizon` 个任务之后回填，不由提出改动的人填。

## protocol/v1

v1 是把 1.x 跑了几个月得到的规则原样出仓，加上 2026-09-16 那次协议审计当天落地的
七条（S1–S7），再加上出仓这件事本身带来的两条：证据文件名加角色后缀（C-09，
原来同一个 `k` 的实现轮和审计轮会写到同一个路径）、读文件的开销（C-10）、
里程碑验收契约（C-11）和每轮简报（C-12）。证据来自三次真实运行：`dashboard-mvp`（2026-08，1.x）、
`island-workbench`（1.x）、`voice-schedule`（2026-09-16，2.0 内核 + v1 协议）。
这些条目的 `realized_impact` 永远是 `null`：它们落地时内核还不记录任何指标，
补不出一个诚实的数来。这本身就是把观测层排在第一位的理由。

```yaml
- id: C-01
  kind: behavioral
  clause: loop-protocol.md#文件/证据不写进设计文档
  evidence: [dashboard-mvp S1, island-workbench S1, voice-schedule S1]
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
  eval: evals/impl-writes-evidence-not-design/
  realized_impact: null
- id: C-02
  kind: behavioral
  clause: loop-protocol.md#文件/运行记录只有一行
  evidence: [dashboard-mvp S2, island-workbench S2, voice-schedule S2]
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
  eval: evals/retro-is-one-line/
  realized_impact: null
- id: C-03
  kind: behavioral
  clause: loop-protocol.md#实现循环/标 `待审` 之前的自审清单
  evidence: [voice-schedule S3, island-workbench S3]
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
  eval: evals/impl-self-check-before-pending/
  realized_impact: null
- id: C-04
  kind: behavioral
  clause: loop-protocol.md#实现循环/审计造的检查归谁
  evidence: [voice-schedule S4, island-workbench S4]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: evals/audit-no-history-rerun/
  realized_impact: null
- id: C-05
  kind: clarify
  clause: loop-protocol.md#实现循环/轮次与预算
  evidence: [voice-schedule S5, dashboard-mvp S5]
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 5}
  eval: null
  realized_impact: null
- id: C-06
  kind: behavioral
  clause: loop-protocol.md#里程碑/验收必须是会话自己能跑的
  evidence: [voice-schedule S6, island-workbench S6]
  predicted_impact: {metric: manual_items_open, direction: down, scope: task, horizon: 3}
  eval: evals/plan-manual-checks-are-not-milestones/
  realized_impact: null
- id: C-07
  kind: clarify
  clause: loop-protocol.md#实现循环/审计轮
  evidence: [voice-schedule S7, dashboard-mvp S7]
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
- id: C-11
  kind: behavioral
  clause: loop-protocol.md#里程碑/M-xx 验收契约
  evidence: [voice-schedule M-07 关不上, island-workbench 验收不明确]
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
  eval: evals/plan-manual-checks-are-not-milestones/
  realized_impact: null
- id: C-12
  kind: behavioral
  clause: loop-protocol.md#文件
  evidence: [voice-schedule 每轮上下文, dashboard-mvp 每轮上下文]
  predicted_impact: {metric: mean_request_input, direction: down, scope: task, horizon: 3}
  eval: evals/impl-writes-evidence-not-design/
  realized_impact: null
- id: C-08
  kind: behavioral
  clause: session-protocol.md#状态块
  evidence: [voice-schedule 状态块空表头, island-workbench 设计文档体积]
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 5}
  eval: evals/plan-milestone-section-holds-one-table/
  realized_impact: null
- id: C-09
  kind: behavioral
  clause: loop-protocol.md#文件
  evidence: [voice-schedule 证据撞名, island-workbench 证据撞名]
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
  eval: evals/impl-writes-evidence-not-design/
  realized_impact: null
- id: C-10
  kind: behavioral
  clause: session-protocol.md#读文件的开销
  evidence: [voice-schedule 每轮上下文, dashboard-mvp 每轮上下文]
  predicted_impact: {metric: mean_request_input, direction: down, scope: task, horizon: 3}
  eval: evals/impl-writes-evidence-not-design/
  realized_impact: null
- id: C-13
  kind: behavioral
  clause: loop-protocol.md#审计轮
  evidence: [voice-schedule M-07 空转十三轮, island-workbench S6]
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
  eval: evals/audit-manual-only-milestone-goes-to-checklist/
  realized_impact: null
- id: C-14
  kind: clarify
  clause: loop-protocol.md#里程碑/验收必须是会话自己能跑的
  evidence: [voice-schedule M-07 空转十三轮, island-workbench S6]
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

## 关于 C-13 / C-14

C-06 已经写过「要真人才能看到的结果不作为里程碑验收条件」，但它只对设计轮说。
`voice-schedule` 的 M-07 是在那条规则落地之前就立好的里程碑，后面每一轮都照着它跑，
没有任何角色被授权去改它——规则到得太晚，对在途的任务不回填。

于是审计轮落在了三选一之外：既拿不到真机证据关闭 M-07，又没有产品缺陷可以退回它，
只好保持 `待审`。下一个实现轮发现没有 `开放` 的里程碑，无事可做，就去给已经通过的
东西加检查。这一对连续跑了十三轮，烧掉三分之二预算，里程碑表一个字没动。

C-13 把出口写进审计轮的结论一节并点明「没有第四种结论」，C-14 把 C-06 的适用范围
从设计轮扩到任何轮次。配套的内核守卫是 `guards::audit_moved_the_table` 与
`guards::impl_had_a_target`：条文靠自觉的那一半，这次由代码兜住。
