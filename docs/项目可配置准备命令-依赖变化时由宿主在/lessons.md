# 教训 · 宿主准备命令 [setup]（T-27）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 9 份证据、审计与裁决记录、`brief/impl-2.md`、`protocol/prompts/intake.md`、任务分支 `git log`，以及主检出 `.autome/output/gates/T-27/` 下 5 份宿主门禁日志（只读）。

**指标与实际不符，先说清楚。** Autome 给本轮的指标是「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、协议失败 1」，而设计文档状态块终值是 `design-round: 1/15`、`implementation-round: 5/20`，里程碑表四行全部已完成，`## 人工验收清单` 里 H-01、H-02 两条都未确认。`target/debug/doccheck` 解析终稿得到 `OK status=Done design=1/15 … manual=2`。设计上限是 `0/0`，只有 `task_metrics::aggregate` 拿到 `status = None` 时才会出现这个值（`crates/automed/src/task_metrics.rs:130-143`）。指标只在任务转入 Failed/Cancelled 或进入 Cleanup 时写入（`crates/automed/src/scheduler.rs:1601-1612`），而本任务唯一一次读不出状态块的时刻，就是任务整理轮把状态块写进 ```` ```text ```` 围栏、触发协议失败的那一次（之后 Autome 用 `82cc6e3` 去掉了围栏）。所以这组指标是那一刻的快照，tokens/turns/费用大概率也只覆盖任务整理一个会话（这一点是推断，没有核对）。下文的轮次数字都取自状态块、`retro.md` 与证据文件，不取自这组指标。

运行概况：设计 1 轮定稿（评审 3 项全部采纳），M-01..M-04 都是一次审计关闭，reopen 0，审计没有记实现缺陷，也没有记验证缺口。真正多出来的成本有两处：一是任务整理轮的状态块围栏导致一次协议失败（L-01），它同时冻结了指标（L-04）；二是实现 #2 整轮没有改产品代码，花在两条基线就有的抖动测试上，因为简报里没有失败用例名（L-02）；其中一条测试此后又出现过、却没有进 Backlog（L-03）。

L-02、L-03、L-04 的 proposal 原文照抄先前任务中的同类教训，方便跨任务计数。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1（904f057）把设计文档骨架的状态块写在 ```text 围栏里，Autome 判协议失败，任务停下；Autome 随后用 82cc6e3「状态块去掉代码围栏（任务整理轮写在了 ```text 里）」删掉围栏上下两行，任务才进入设计轮。这是本任务唯一一次协议失败（protocol_failures = 1）'
  root_cause: 'protocol/prompts/intake.md 第 23–31 行把要写进骨架的状态块放在 ```text 围栏里给出，提示语里没有说不要加围栏；只有 session-protocol.md 第 84 行写了「不要把状态块放进代码块」，但它自己在第 41 行也用 ```text 展示同一块。任务整理轮照抄了 prompt 里的示例'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加 ``` 围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '实现 #1 把 M-01 标待审后，宿主门禁在 ac5aff9 上报 Test 失败；实现 #2 是门禁修复轮，证据写「未获得原 Test 的具体失败用例及断言，不能推断根因」，在沙箱里原样跑了 634 秒 ci-local，Test 全绿，最后被 Web listen EPERM 挡住，整轮没有改产品代码。宿主日志 gate-ac5aff94443f.log 第 1070、1111 行其实写明了两个失败用例：bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it 与 bug_regression_guard_checks_each_workspace_member。两者都是基线就有的抖动（审计 #2 在基线上 20 次失败 2 次；审计 #3 判为负载竞速），下一次宿主门禁（b433bfe，只改文档）就通过了；brief/impl-2.md 的「预算与用户表态」只有预算一行'
  root_cause: 'Autome 派门禁修复轮时只给会话一句「宿主失败在 Test」，没有给失败用例名、断言和日志路径；实现轮拿不到这些，就没法几分钟内去基线 2553923 上单跑这两个用例确认它们是抖动，只能重跑整套门禁'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-01-r2-impl.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: 'bug_regression_guard_checks_each_workspace_member 先让实现 #1 的宿主门禁变红（gate-ac5aff94443f.log:1111），审计 #3 全量 e2e 时又失败一次；审计 #3 判为「全量并发负载下 30 秒轮询超时、与本里程碑无关」，单跑 3 次通过，但设计文档 `## Backlog` 始终没有它的条目。同类的 bug_loop 遗留任务抖动直到审计 #2 才记为 B-A2-01，gate.rs 超时抖动到审计 #4 才记为 B-06，那时实现 #2 已经白跑'
  root_cause: '审计轮只在本轮结论里写「无关的负载竞速」，没有落到设计文档的 Backlog；Backlog 才是后续轮次与下一个任务会读到的地方，结论段不是，于是同一条抖动下次出现时又要重新诊断一遍'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-03-r4-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '复盘轮收到的指标是「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0」，而设计文档状态块终值是 design-round 1/15、implementation-round 5/20，里程碑表 M-01..M-04 四行全部已完成，人工验收清单 H-01、H-02 都未确认；retro.md 也记有设计 #1、实现 #1..#5、审计 #1..#4。doccheck 解析终稿得到 design=1/15、manual=2'
  root_cause: 'scheduler.rs:1601-1612 只在转入 Failed/Cancelled 或进入 Cleanup 时调用 write_task_metrics；任务整理轮的围栏状态块（L-01）让任务转入 Failed，那一刻 read_status 解析失败返回 None（scheduler.rs:1771-1775），aggregate 因此写出 0/0 与 0 个里程碑（task_metrics.rs:130-143）。任务恢复后直到复盘都没有再经过这几个转移，复盘 prompt 渲染的是这份旧指标'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```
