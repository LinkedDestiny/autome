# 教训 · 宿主准备命令 [setup]（T-27）

复盘轮 #2 覆盖写（复盘 #1 的 4 条保留，proposal 原文不动，方便跨任务计数；只按复盘 #1 之后的新证据更新 symptom/evidence，并新增 L-05）。依据：任务文件、`retro.md`、`evidence/` 下全部 11 份证据（新增 `M-02-r6-impl.md`、`M-02-r6-audit.md`）、审计与裁决记录、`brief/impl-6.md`、`protocol/prompts/intake.md`、任务分支 `git log`。

**指标与实际仍然不符。** 本轮收到的指标还是「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、协议失败 1、总 turns 16、总 tokens 817294」。设计文档状态块终值是 `design-round: 1/15`、`implementation-round: 6/20`，里程碑表 M-01..M-04 四行都已完成，`## 人工验收清单` 里 H-01、H-02 都未确认；`retro.md` 记有设计 #1、实现 #1..#6、审计 #1..#5。复盘 #1 之后又跑了实现 #6 与审计 #5，指标里的轮次数却没有变。下文的轮次数字都取自状态块、`retro.md` 与证据文件，不取自这组指标；tokens/turns/费用覆盖哪些会话，本轮也没有核对。

运行概况：设计 1 轮定稿，M-01..M-04 都是一次审计关闭，reopen 0，实现缺陷 0，验证缺口 0。多出来的成本有这几处：任务整理轮的状态块围栏导致一次协议失败（L-01），这次失败同时冻结了指标（L-04）；实现 #2 整轮没有改产品代码（L-02）；同一条抖动测试三次出现都没有进 Backlog（L-03）；实现 #6 处理合并 rebase 冲突时，简报里没有冲突信息，只能自己去重建冲突现场（L-05）。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1（904f057）把设计文档骨架的状态块写在 ```text 围栏里，Autome 判协议失败，任务停下；Autome 随后用 82cc6e3「状态块去掉代码围栏（任务整理轮写在了 ```text 里）」删掉围栏上下两行，任务才进入设计轮。这是本任务唯一一次协议失败（protocol_failures = 1）'
  root_cause: 'protocol/prompts/intake.md 第 23–31 行把要写进骨架的状态块放在 ```text 围栏里给出，提示语里没有说不要加围栏；只有 session-protocol.md 写了「不要把状态块放进代码块」，而它自己展示同一块时也用了 ```text 围栏。任务整理轮照抄了 prompt 里的示例'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加 ``` 围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '实现 #1 把 M-01 标为待审后，宿主门禁在 ac5aff9 上报 Test 失败。实现 #2 是门禁修复轮，证据里写「未获得原 Test 的具体失败用例及断言，不能推断根因」；它在沙箱里原样跑了 634 秒 ci-local，Test 全绿，最后被 Web listen EPERM 挡住，整轮没有改产品代码。两个失败用例（bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it、bug_regression_guard_checks_each_workspace_member）都是基线就有的抖动，下一次宿主门禁（b433bfe，只改了文档）就通过了'
  root_cause: 'Autome 派门禁修复轮时只告诉会话「宿主失败在 Test」，没有给失败用例名、断言和日志路径；实现轮拿不到这些，就没法几分钟内去基线 2553923 上单跑这两个用例确认是抖动，只能重跑整套门禁'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-01-r2-impl.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: 'bug_regression_guard_checks_each_workspace_member 在本任务里失败过三次：实现 #1 的宿主门禁（ac5aff9）、审计 #3 的全量 e2e、审计 #5 的全量 cargo test（1100 通过 / 1 失败，单跑 47s 通过）。审计 #3 和审计 #5 都在结论里把它判为「负载下的时序抖动、与本任务无关」，但到任务结束，设计文档 `## Backlog` 仍然没有它的条目。同类抖动里，bug_loop 遗留任务那条到审计 #2 才记为 B-A2-01，gate.rs 超时那条到审计 #4 才记为 B-06'
  root_cause: '审计轮只在本轮结论或证据里写「无关的抖动」，没有落到设计文档的 Backlog。后续轮次和下一个任务读的是 Backlog，不是某一轮的结论段，所以同一条抖动每出现一次都要重新诊断（审计 #5 又单跑了一次、又写了一遍因果证据）'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r6-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '复盘 #1 与复盘 #2 收到的指标都是「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0」。复盘 #2 启动时，设计文档状态块是 design-round 1/15、implementation-round 6/20，里程碑表四行都已完成，人工验收清单 H-01、H-02 都未确认。复盘 #1 之后又跑了实现 #6 与审计 #5，指标里的轮次数没有跟着变'
  root_cause: 'scheduler.rs 只在第 1610 行（measure_now 时，即转入 Failed/Cancelled 或进入 Cleanup）调用 write_task_metrics。任务整理轮的围栏状态块（L-01）让任务转入 Failed，那一刻状态块解析失败，status 为 None，task_metrics.rs 的 aggregate 因此写出 0/0 与 0 个里程碑（第 132–143 行只在 Some 时赋值）。之后直到两次复盘都没有再经过这几个转移，复盘 prompt 渲染的一直是这份旧指标'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-05
  domain: process
  symptom: '四个里程碑关闭、复盘 #1 结束后，合并到 main 时 render.rs 与 main 的 c95ef10（单仓库 merge.rebased 文案修复）冲突，Autome 派出实现 #6。brief/impl-6.md 除轮次行外和 impl-5 的通用部分一样，写的是「还没有里程碑表，或者所有里程碑都已完成」，没有冲突文件、main 提交号和 rebase 状态。实现 #6 开工时发现没有进行中的 rebase（无 REBASE_HEAD、无冲突标记），只能自己对比本地 main，推断出缺的是 merge.rebased 分支和两个测试，然后搬入、跑红再跑绿。随后又跑了一轮审计 #5 专门复验这次合并'
  root_cause: 'Autome 因合并 rebase 冲突派实现轮时，冲突信息只在会话 prompt 里一句「rebase 冲突处理」，没有写进简报；rebase 已被中止，工作区里也不留冲突现场，会话无从得知冲突在哪些文件、对的是 main 的哪个提交'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r6-impl.md'
  level: brief
  proposal: '因合并 rebase 冲突派出的实现轮，其简报必须列出冲突文件清单、冲突时 main 的提交号，并写明 rebase 是否已中止'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
