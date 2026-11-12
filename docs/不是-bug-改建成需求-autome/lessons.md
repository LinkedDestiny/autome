# 教训 · 「不是 Bug」改建成需求：`task.convert` 与 `autome task convert`（T-40）

复盘轮 #2 覆盖写。依据：任务文件、`retro.md`（11 行）、`evidence/` 下 8 份证据、审计与裁决记录、设计文档状态块与里程碑表、`git log`，以及 `scheduler.rs`、`task_metrics.rs`、`status_block.rs` 的当前源码。

**指标仍与实际不符，而且比复盘 #1 时更明显。** 本轮收到的指标和复盘 #1 一字不差：设计轮 0/0、实现轮 0/0、里程碑数 0、协议失败 1、人工验收未确认 0、总 turns 19、总 tokens 825031、费用 1.9009 USD。可是两次复盘之间又跑了复盘 #1、实现 #4、审计 #4 三个会话，tokens、turns、费用却一点没变。设计文档现在是 `implementation-round: 4/15`，`## 人工验收清单` 里的 H-01 也还没确认（`status_block.rs` 第 735–742 行把不带勾选框的 `- H-01 …` 计为未确认）。下文的轮次、里程碑数与人工项都取自状态块、`retro.md` 和证据文件，不取这组指标（见 L-02）。turns、tokens、费用无法核对，不引用。

**运行概况。** 设计 1 轮定稿：评审 #1 提了两条阻塞（D1-P01、D1-P02），裁决 #1 都采纳。M-01..M-03 都是一次审计关闭，reopen 0，实现缺陷 0，验证缺口 0。唯一的协议失败出在任务整理轮（L-01）。复盘 #1 之后进入 rebase 节点，`crates/automed/tests/end_to_end.rs` 的文件尾发生冲突，内核为此派了实现 #4 和审计 #4（L-06）。四个实现轮都在沙箱里撞上 `listen EPERM 127.0.0.1`，Web 全量验收都是交给别的环境补跑的（L-03、L-04、L-05）。

L-01..L-05 的 proposal 原样沿用复盘 #1，复盘 #1 的 proposal 又分别沿用了更早任务的条目（出处见复盘 #1 的版本，提交 `ccd11f5`）。措辞不改，是为了能跨任务计数。这次只在 symptom 里补了实现 #4 的复现。L-06 是本轮新增的。另外，main 上已经有 `b3a9c19`（「defer full-suite session checks to before-merge gate」），从标题看和 L-05 方向一致，但它不在本任务 HEAD 里，也不在固定的 protocol/v2 里，本次运行没有受它影响。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1 把设计文档骨架的状态块写进了 text 代码围栏，Autome 判协议失败，任务停下（protocol_failures = 1）。用户随后提交 23b73b9（「T-40 状态块去掉代码围栏」），删掉围栏上下两行，任务才进入设计 #1。T-27 出过同样的事，当时由 82cc6e3 修复'
  root_cause: '本任务固定的 protocol/prompts/intake.md 在第 23–31 行把要写进骨架的状态块放在 text 代码围栏里给出，提示里没有说不要带围栏。任务整理轮照抄了示例。T-27 复盘之后协议副本没有改，所以同一个失败在下一个任务里又出现了'
  evidence: 'docs/不是-bug-改建成需求-autome/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加代码围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '复盘 #1 和复盘 #2 收到的指标一字不差：设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、turns 19、tokens 825031、费用 1.9009。两次复盘之间又跑了复盘 #1、实现 #4、审计 #4 三个会话，tokens 和 turns 却没有增加。复盘 #2 启动时，状态块是 design-round 1/15、implementation-round 4/15，里程碑表三行都已完成，H-01 未确认，retro.md 有 11 行。T-27 也出现过同样的不符'
  root_cause: 'scheduler.rs 第 1882–1893 行只在三种情况下调用 write_task_metrics：转入 Failed 或 Cancelled、进入 Cleanup、或者 Done 时还没有指标。task_metrics.rs 的 aggregate（第 122 行起）只在状态块解析成功时才填写轮次、里程碑数和人工项。任务整理轮的围栏状态块（L-01）让任务转入 Failed，那时状态块解析不出来，于是写下了一份全零快照。之后任务一直没有走到 Cleanup，复盘 prompt 渲染的就始终是这份快照，包括其中的 tokens 和 turns'
  evidence: 'docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '四个实现轮的 Web 全量都被沙箱挡住。实现 #1、#2：Web 共 301 项，271 通过、11 失败、19 跳过，日志里有 listen EPERM 127.0.0.1 和 EMFILE。实现 #3 正是 Web 网关那个里程碑，npm test 一直没有出汇总，只好终止（退出 130），然后用 net.createServer().listen(0, 127.0.0.1) 单独复现了 EPERM。实现 #4（rebase 后）：365 项，334 通过、12 失败（全部是 listen EPERM）、19 跳过。审计 #3 在另一个环境跑同样的命令，302 项全部通过'
  root_cause: '实现轮会话拿到的权限不允许回环监听，审计轮拿到的允许。本任务的验收 A-07 恰好要求 npm test --prefix apps/web 全绿，所以实现轮从结构上就拿不到这条验收的证据，只能交给审计轮或宿主代跑。在本任务之前，至少已有两个任务的复盘写过同一条教训'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: protocol-format
  symptom: '三个实现轮碰到的是同一种环境阻断，处理却不一致。实现 #1、#2 都是 Web/Desktop 门禁失败，仍把 M-01、M-02 标成待审。实现 #3 写的是「M-03 保持开放，不标待审」，retro.md 记为「开放，网关通过但全量验收受阻」。审计 #3 随后照样复验了 M-03 并关闭。状态格写的是开放，实际走的却是待审到审计这条路'
  root_cause: '协议里里程碑只有开放、待审、已完成三种状态，没有一种能表达「代码已落地，只是验收被执行环境挡住」。实现轮只能各自理解「全部验收通过才能标待审」这句话，于是同一个任务里出现了两种做法，而调度并不看这一格，照样派了审计'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r3-impl.md'
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: process
  symptom: '四个实现轮都在会话里跑了 sh scripts/ci-local.sh 全量，没有一次的结果能直接当结论。实现 #1、#2 跑完，退出 1：Desktop 1 项失败（Electron DOM harness），Web 11 项失败。实现 #3 跑到 Rust 测试阶段就终止了（退出 130）。实现 #4 的 Rust 1221/0/1 全部通过，脚本却以 143 结束，接着又单独跑了 Web 全量（12 项 EPERM 失败）和 Desktop 全量（1 项 SIGABRT），证据里留下的结论是「完整宿主门禁未通过，需宿主复验」。同期宿主门禁在提交后跑的是同一条脚本，审计轮把它当旁证引用'
  root_cause: '宿主门禁在提交后本来就会跑同一条 ci-local.sh，可实现轮简报没写这一点，设计文档还写着「宿主门禁 sh scripts/ci-local.sh 每个实现轮都会跑」。实现轮于是把全量门禁当成自己的验收义务，在明知会失败的沙箱里反复跑，再花篇幅解释那些与产品无关的失败'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-02-r2-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: implementation
  symptom: '复盘 #1 之后的 rebase 节点出了冲突，唯一的冲突文件是 crates/automed/tests/end_to_end.rs：本任务和 main 都在文件尾追加了测试。内核为此派了实现 #4 解决冲突（implementation-round 3/15 → 4/15），又跟了一个审计 #4。解决时还得手工「补齐末尾函数闭合」。设计 R3 只预计到与 T-37 在 write-gate.ts 相邻行冲突，没料到这里。解决之后，本任务的改建用例仍然在文件尾（第 6423 行，文件共 6481 行），同主题的 bug_loop_not_a_bug_waits_for_a_decision… 在第 4571 行。而 main 此时已经领先 200 个提交，这个文件长到了 7522 行'
  root_cause: 'automed 的端到端用例全部集中在 end_to_end.rs 这一个文件里，各任务习惯把新用例追加到文件尾。并行的 worktree 都改同一处文件尾，rebase 时文本冲突几乎必然发生。scheduler.rs 第 1658–1709 行让每次冲突都消耗一个实现轮，连续 3 次冲突文件集相同，任务就转入 Failed'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r4-impl.md'
  level: rule
  proposal: 'automed 新增的端到端用例不追加在 crates/automed/tests/end_to_end.rs 的文件尾，而是放在同主题既有用例之后，或者新建一个 tests/*.rs 文件；判据是本任务对 end_to_end.rs 的 diff 不触及基线上该文件的最后一个函数'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```
