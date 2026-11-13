# 协议改动记录

每个版本一节，每条改动一个块。字段的含义见 `docs/plans/2026-09-17-loop-self-improvement-proposal.md` §3.4：

- `kind` —— `behavioral` 改变会话的行为，必须带一个「改前红、改后绿」的 eval 用例；
  `clarify` 只是把同一件事说清楚，不需要用例，靠评审轮签字；
  `retire` 移除条文，只能以移除实验的形式提出，`direction` 必须是 `flat`。
- `evidence` —— 至少两个任务的证据。一个任务只是一次倒霉。
- `predicted_impact.metric` —— 只能取内核真的记录的那些指标（`TaskMetrics::METRIC_NAMES`）。
- `realized_impact` —— 由 Autome 在 `horizon` 个任务之后回填，不由提出改动的人填。

## protocol/v1

基线。协议从这一版起只以 autome-v2 的 `protocol/` 为准、随
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

## protocol/v2

Bug Loop 的独立协议、定位、修复与复核模板。

```yaml
- id: B-1
  kind: behavioral
  clause: prompts/locate.md
  evidence: [web-dashboard-via-loop §4.2, 按计划-4-2-节实现-bug-修复-loop 任务文件]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: evals/locate-proves-the-bug-with-a-failing-test/
  realized_impact: null
```

```yaml
- id: B-2
  kind: behavioral
  clause: prompts/fix.md
  evidence: [web-dashboard-via-loop §4.2, 按计划-4-2-节实现-bug-修复-loop 任务文件]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: evals/fix-leaves-the-regression-test-alone/
  realized_impact: null
```

```yaml
- id: B-3
  kind: behavioral
  clause: prompts/fix-review.md
  evidence: [web-dashboard-via-loop §4.2, 按计划-4-2-节实现-bug-修复-loop 任务文件]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: evals/fix-review-rejects-a-weakened-test/
  realized_impact: null
```

```yaml
- id: B-4
  kind: behavioral
  clause: bug-protocol.md
  evidence: [web-dashboard-via-loop §4.2, 按计划-4-2-节实现-bug-修复-loop 任务文件]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: evals/locate-proves-the-bug-with-a-failing-test/
  realized_impact: null
```

## protocol/v3

审计、复盘、复核模板声明文档写入边界；验证缺口仅补文档验收命令或不入库临时检查，项目门禁失败交回实现轮。

```yaml
- id: S-1
  kind: behavioral
  clause: prompts/audit.md
  evidence: [审计-prompt Bug 定位回归, T-47 审计越界事故线索]
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
  eval: evals/audit-keeps-project-files-unchanged/
  realized_impact: null
```

```yaml
- id: S-2
  kind: clarify
  clause: prompts/retro.md
  evidence: [审计-prompt Bug 定位回归, T-48 会话范围守卫]
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

```yaml
- id: S-3
  kind: clarify
  clause: prompts/fix-review.md
  evidence: [审计-prompt Bug 定位回归, T-48 会话范围守卫]
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```


`[gate] run = "before_merge"` 下会话不再自己跑全量门禁。

```yaml
- id: G-1
  kind: behavioral
  clause: prompts/impl.md
  evidence: [T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: evals/impl-before-merge-runs-only-the-milestone-checks/
  realized_impact: null
```

```yaml
- id: G-2
  kind: behavioral
  clause: prompts/audit.md
  evidence: [T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: evals/audit-before-merge-concludes-without-the-full-gate/
  realized_impact: null
```

```yaml
- id: G-3
  kind: behavioral
  clause: prompts/plan.md
  evidence: [T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: evals/plan-before-merge-keeps-the-full-gate-out-of-milestones/
  realized_impact: null
```

```yaml
- id: G-4
  kind: behavioral
  clause: prompts/fix.md
  evidence: [T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: evals/fix-before-merge-runs-only-the-repro-and-related-tests/
  realized_impact: null
```

```yaml
- id: G-5
  kind: behavioral
  clause: prompts/fix-review.md
  evidence: [T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: evals/fix-review-before-merge-concludes-without-the-full-gate/
  realized_impact: null
```

```yaml
- id: G-6
  kind: clarify
  clause: bug-protocol.md
  evidence: [T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]
  predicted_impact: {metric: total_turns, direction: flat, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

## protocol/v4

Bug 三个步骤列出任务附件副本与参考文档的位置。

```yaml
- id: I-1
  kind: clarify
  clause: prompts/locate.md
  evidence: [web-dashboard-via-loop §3.1, T-41 任务卡]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

```yaml
- id: I-2
  kind: clarify
  clause: prompts/fix.md
  evidence: [web-dashboard-via-loop §3.1, T-41 任务卡]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

```yaml
- id: I-3
  kind: clarify
  clause: prompts/fix-review.md
  evidence: [web-dashboard-via-loop §3.1, T-41 任务卡]
  predicted_impact: {metric: protocol_failures, direction: flat, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

## protocol/v5

复核说明按宿主实际运行的快检或完整门禁结果渲染。

```yaml
- id: Q-1
  kind: clarify
  clause: prompts/fix-review.md
  evidence: [T-61 L-07, T-56, T-65]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

```yaml
- id: Q-2
  kind: clarify
  clause: bug-protocol.md
  evidence: [T-61 L-07, T-56, T-65]
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

## protocol/v6

明确状态块必须写在标题下、首个二级标题之前且在代码围栏之外；样例围栏仅用于展示。
这是对已有 session/bug 协议的澄清，不改变合法文档格式。

```yaml
- id: F-1
  kind: clarify
  clause: prompts/intake.md
  evidence: [T-27 围栏状态块事件（用户任务卡提供，历史未独立核实）, T-40 围栏状态块事件（用户任务卡提供，历史未独立核实）, 状态块写在代码围栏里时报-缺少状态字段 回归测试]
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

同样的说明同步到 `prompts/plan.md`、`prompts/locate.md` 与 `loop-protocol.md`。

## protocol/v7

明确定位提交前的格式化检查及修复轮的纯格式化例外，独立复核仍只写任务文档；已固定的旧协议不迁移。

```yaml
- id: F-2
  kind: clarify
  clause: prompts/locate.md
  evidence: [T-64 格式化阻塞（用户任务卡提供，历史未独立核实）, 修复轮不敢格式化回归测试-定位轮写的测试过不了 回归测试]
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
  eval: null
  realized_impact: null
```

同样的边界同步到 `prompts/fix.md`、`prompts/fix-review.md` 与 `bug-protocol.md`。

## protocol/v8

里程碑之外的实现改动进入独立的集成复验；普通审计保留原有行为。

```yaml
- id: J-1
  kind: behavioral
  clause: prompts/audit.md
  evidence: [T-61 L-05（docs/门禁只在合并前跑一次-gate-run/lessons.md）, T-61 L-06 实现 #9/#10 之后的空转审计, T-68 任务卡]
  predicted_impact: {metric: impl_defects, direction: up, scope: task, horizon: 3}
  eval: evals/audit-integration-review/
  realized_impact: null
```
