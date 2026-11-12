# 教训 · 「不是 Bug」改建成需求：`task.convert` 与 `autome task convert`（T-40）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 6 份证据、审计与裁决记录、设计文档状态块与里程碑表、`git log`。

**指标与实际不符。** 本轮收到的指标是：设计轮 0/0、实现轮 0/0、里程碑数 0、协议失败 1、总 turns 19、总 tokens 825031、费用 1.9009 USD。设计文档状态块是 `status: 已完成`、`design-round: 1/15`、`implementation-round: 3/15`，里程碑表 M-01..M-03 三行都是 `已完成`。下文的轮次与里程碑数取自状态块、`retro.md` 和证据文件，不取这组指标（原因见 L-02）。turns、tokens、费用无法核对，不引用。

**运行概况。** 设计 1 轮定稿：评审 #1 提了两条阻塞（D1-P01 单事务改建、D1-P02 取消完成判据），裁决 #1 都采纳。三个里程碑都是一次审计关闭，reopen 0，实现缺陷 0，验证缺口 0。设计轮预测 M-03 结束时 `cargo test --workspace` 应为 1206（基线 1193 + 13），审计 #3 实测 1206/0/1，预测准确。唯一的协议失败出在任务整理轮（L-01）。三个实现轮都被同一个沙箱限制（`listen EPERM 127.0.0.1`）挡住全量验收，三个审计轮在别的环境里全部补齐（L-03、L-04、L-05）。

L-01、L-02 的 proposal 原样沿用 `项目可配置准备命令-依赖变化时由宿主在` 的 L-01、L-04；L-03 沿用 `在-apps-web-建-react` 与 `实现-web-sqlite3-与通知器` 的 L-01；L-04 沿用 `在-apps-web-建-react` 的 L-02；L-05 沿用 `在-plugins-新增-autome-插件` 的 L-02 和 `门禁连续因同一原因失败时熔断-停下等人` 的 L-05。措辞不改，是为了能跨任务计数。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1 把设计文档骨架的状态块写进了 text 代码围栏，Autome 判协议失败，任务停下（protocol_failures = 1）。用户随后提交 8b4e75b（「T-40 状态块去掉代码围栏（任务整理轮写在了 ```text 里）」），删掉围栏上下两行，任务才进入设计 #1。T-27 出过同样的事，当时由 82cc6e3 修复'
  root_cause: '本任务固定的 protocol/prompts/intake.md 在第 23–31 行把要写进骨架的状态块放在 text 代码围栏里给出，紧接着的 repos 示例也是这样，提示里没有说不要带围栏。任务整理轮照抄了示例。T-27 复盘之后协议副本没有改，所以同一个失败在下一个任务里又出现了'
  evidence: 'docs/不是-bug-改建成需求-autome/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加代码围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '复盘 #1 收到的指标是设计轮 0/0、实现轮 0/0、里程碑数 0、协议失败 1。启动复盘时，设计文档状态块是 design-round 1/15、implementation-round 3/15，里程碑表 M-01..M-03 三行都已完成，retro.md 记了设计 #1、评审 #1、裁决 #1、实现 #1..#3、审计 #1..#3 共九轮。指标和实际对不上，T-27 也是同样的情形'
  root_cause: 'scheduler.rs 第 1778–1789 行只在三种情况下调用 write_task_metrics：转入 Failed 或 Cancelled、进入 Cleanup、或者 Done 时还没有指标。task_metrics.rs 第 132 行起只在状态块解析成功时才填写轮次和里程碑数。任务整理轮的围栏状态块（L-01）让任务转入 Failed，那时状态块解析不出来，于是写下的是 0/0 和 0 个里程碑。复盘发生在 Cleanup 之前，此后没有再经过这几个转移，复盘 prompt 渲染的一直是这份旧指标'
  evidence: 'docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '三个实现轮的 Web 全量都被沙箱挡住。实现 #1、#2 的 ci-local 里 Web 共 301 项，271 通过、11 失败、19 跳过，日志里有 listen EPERM 127.0.0.1 和 EMFILE。实现 #3 正是 Web 网关那个里程碑：npm test 全量一直没有汇总，只好终止（退出 130）；它用 net.createServer().listen(0, 127.0.0.1) 单独复现了 EPERM（bind.txt），lint 也挂住没跑完，typecheck 没有执行。审计 #3 在另一个环境跑同样的命令，33 个文件、302 项全部通过'
  root_cause: '实现轮会话拿到的权限不允许回环监听，审计轮拿到的允许。这个任务的验收 A-07 恰好要求 npm test --prefix apps/web 全绿，所以实现轮从结构上就拿不到这条验收的证据，只能交给审计轮代跑。在本任务之前，至少已有两个任务的复盘写过同一条教训'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: protocol-format
  symptom: '同一任务里三个实现轮的处理不一致，而它们碰到的是同一种环境阻断。实现 #1、#2 都是 Web/Desktop 门禁失败，仍把 M-01、M-02 标成待审；实现 #3 写「M-03 保持开放，不标待审」，retro.md 记为「开放，网关通过但全量验收受阻」。审计 #3 随后照样复验了 M-03 并关闭。状态格写的是开放，实际走的却是待审到审计这条路'
  root_cause: '协议里里程碑只有开放、待审、已完成三种状态，没有一种能表达「代码已落地，只是验收被执行环境挡住」。实现轮只能各自理解「全部验收通过才能标待审」这句话，于是同一个任务里出现了两种做法，调度却不看这一格，照样派审计'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r3-impl.md'
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: process
  symptom: '三个实现轮都在沙箱里跑了 sh scripts/ci-local.sh 全量，没有一次能直接当结论。实现 #1、#2 都完整跑完，退出 1：Desktop 105 项里 1 项失败（Electron DOM harness status=null），Web 11 项失败、19 项跳过；两份证据都写「需宿主允许的环境复跑」。实现 #3 跑到 Rust 测试阶段就终止了（退出 130）。同期宿主门禁在 b27e4c6 等提交上退出 0，审计 #1、#3 都把它当旁证引用'
  root_cause: '宿主门禁在提交后本来就会跑同一条 ci-local.sh，可实现轮简报没写这一点。实现轮于是把全量门禁当成自己的验收义务，在明知会失败的沙箱里反复跑，再花篇幅解释那些与产品无关的失败'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-02-r2-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```
