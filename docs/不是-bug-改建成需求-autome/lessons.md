# 教训 · 「不是 Bug」改建成需求：`task.convert` 与 `autome task convert`（T-40）

复盘轮 #3 覆盖写。依据：任务文件、`retro.md`（13 行）、`evidence/` 下全部 10 份证据（先读实现 #5、审计 #5）、审计与裁决记录、设计文档状态块与里程碑表、`git log`，以及 `autome-domain/src/task.rs`、`automed/src/gate.rs`、`automed/src/scheduler_gate_queue_tests.rs` 的当前源码。

**指标仍与实际不符，这是第三次。** 本轮收到的指标和复盘 #1、#2 一字不差：设计轮 0/0、实现轮 0/0、里程碑数 0、协议失败 1、人工验收未确认 0、turns 19、tokens 825031、费用 1.9009 USD。复盘 #2 之后又跑了复盘 #2、实现 #5、审计 #5 三个会话，tokens 和 turns 依旧没变。状态块现在是 `design-round: 1/15`、`implementation-round: 5/15`，里程碑表三行都已完成，`## 人工验收清单` 里的 H-01 还没确认。下文的轮次、里程碑数与人工项都取自状态块、`retro.md` 和证据文件，不取这组指标（见 L-02）。指标里只有「协议失败 1」可以核对，而且是对的（L-01）。

**运行概况。** 设计 1 轮定稿：评审 #1 提了两条阻塞，裁决 #1 都采纳。M-01..M-03 都是一次审计关闭，reopen 0，实现缺陷 0，验证缺口 0。唯一的协议失败出在任务整理轮（L-01）。审计 #3 关闭全部里程碑之后，任务又经过两次 rebase 冲突。每次冲突都多花一对实现轮和审计轮，外加一个复盘轮：实现 #4、审计 #4、复盘 #2，以及实现 #5、审计 #5、复盘 #3（L-06、L-08）。第二次冲突从 1 个文件变成 3 个文件，全都是在文件尾追加测试造成的。实现轮的 Web 全量验收五次都没能在会话里跑完（L-03）。实现 #5 还撞上一个上游测试第一次跑失败、重试通过（L-07）。

L-01..L-05 的 proposal 原样沿用复盘 #2，措辞不改，这样才能跨任务计数，这次只在 symptom 里补了实现 #5 的情况。L-06 只在本任务出现过，这次按第二次冲突把范围从 `end_to_end.rs` 一个文件放宽到所有测试文件；它还没有跨任务计数，改措辞不损失什么。L-07、L-08 是新增条目。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1 把设计文档骨架的状态块写进了 text 代码围栏，Autome 判协议失败，任务停下（protocol_failures = 1，这是本任务指标里唯一能核对上的一项）。用户随后提交 0f89e92（「T-40 状态块去掉代码围栏」），删掉围栏上下两行，任务才进入设计 #1。T-27 出过同样的事，当时由 82cc6e3 修复'
  root_cause: '本任务固定的 protocol/prompts/intake.md 在第 23–31 行把要写进骨架的状态块放在 text 代码围栏里给出，提示里没有说不要带围栏。任务整理轮照抄了示例。T-27 复盘之后协议副本没有改，所以同一个失败在下一个任务里又出现了'
  evidence: 'docs/不是-bug-改建成需求-autome/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加代码围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '复盘 #1、#2、#3 收到的指标一字不差：设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、turns 19、tokens 825031、费用 1.9009。复盘 #1 之后又跑了 6 个会话（复盘 #1、实现 #4、审计 #4、复盘 #2、实现 #5、审计 #5），tokens 和 turns 一点没增加。复盘 #3 启动时状态块是 design-round 1/15、implementation-round 5/15，里程碑表三行都已完成，H-01 未确认，retro.md 有 13 行。T-27 也出现过同样的不符'
  root_cause: 'scheduler.rs 只在三种情况下调用 write_task_metrics：转入 Failed 或 Cancelled、进入 Cleanup、或者 Done 时还没有指标。task_metrics.rs 的 aggregate 只在状态块能解析时才填写轮次、里程碑数和人工项。任务整理轮的围栏状态块（L-01）让任务转入 Failed，那时状态块解析不出来，于是写下了一份全零快照。之后任务一直没有走到 Cleanup，三次复盘 prompt 渲染的都是这份快照，包括其中的 tokens 和 turns'
  evidence: 'docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '五个实现轮的 Web 全量都没能在会话里得到全绿结论。实现 #1、#2：301 项，271 通过、11 失败、19 跳过，日志里有 listen EPERM 127.0.0.1 和 EMFILE。实现 #3：npm test 一直没有出汇总，只好终止（退出 130），然后单独复现了 EPERM。实现 #4：365 项，334 通过、12 失败（全部是 listen EPERM）、19 跳过。实现 #5：live/server 用例失败或跳过，并出现 EMFILE，之后长时间没有进展，主动中止（退出 130），没有汇总。只有审计 #3 在另一个环境里跑出 302 项全部通过。两次 rebase 之后的审计 #4、#5 都写的是「沙箱监听受限，未重跑 Web 全量」，所以 rebase 之后的 Web 全量没有任何会话取到证据，next-action 只能写「宿主复验」'
  root_cause: '实现轮会话拿到的权限不允许回环监听。本任务的验收 A-07 恰好要求 npm test --prefix apps/web 全绿，所以实现轮从结构上就拿不到这条验收的证据，只能交给审计轮或宿主代跑。rebase 之后连审计轮也拿不到。在本任务之前，至少已有两个任务的复盘写过同一条教训'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r5-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: protocol-format
  symptom: '三个实现轮碰到的是同一种环境阻断，处理却不一致。实现 #1、#2 都是 Web/Desktop 门禁失败，仍把 M-01、M-02 标成待审。实现 #3 写的是「M-03 保持开放，不标待审」，retro.md 记为「开放，网关通过但全量验收受阻」。审计 #3 随后照样复验了 M-03 并关闭。状态格写的是开放，实际走的却是待审到审计这条路。实现 #4、#5 都没有开放的里程碑，所以没有新的样本'
  root_cause: '协议里里程碑只有开放、待审、已完成三种状态，没有一种能表达「代码已落地，只是验收被执行环境挡住」。实现轮只能各自理解「全部验收通过才能标待审」这句话，于是同一个任务里出现了两种做法，而调度并不看这一格，照样派了审计'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r3-impl.md'
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: process
  symptom: '实现 #1..#4 都在会话里跑了 sh scripts/ci-local.sh 全量，没有一次的结果能直接当结论：实现 #1、#2 退出 1（Desktop 1 项、Web 11 项失败），实现 #3 在 Rust 阶段终止（退出 130），实现 #4 Rust 全过，脚本却以 143 结束。实现 #5 是第一个没跑 ci-local.sh 全量的实现轮，证据里写「本轮未重跑整个 sh scripts/ci-local.sh、Electron DOM 与 npm ci」。可它仍然跑了 cargo test --workspace 和 Web 全量，后者又中止了。这时 HEAD 里已经有 9626bc1（宿主门禁改为 run = before_merge），不过本任务固定的 protocol/v2 和 impl-5 简报都没有提它'
  root_cause: '宿主门禁本来就会跑同一条 ci-local.sh，可实现轮简报没写这一点，设计文档还写着「宿主门禁 sh scripts/ci-local.sh 每个实现轮都会跑」。实现轮于是把全量门禁当成自己的验收义务，在明知会失败的沙箱里反复跑，再花篇幅解释那些与产品无关的失败'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-02-r2-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: implementation
  symptom: '两次 rebase 都出了冲突，冲突全部出在文件尾追加的测试上。第一次（实现 #4）只有 crates/automed/tests/end_to_end.rs，要手工「补齐末尾函数闭合」。第二次（实现 #5）是三个文件：end_to_end.rs、crates/autome-cli/src/render.rs（文件内测试模块的末尾）、apps/web/test/write-gate.test.ts，要「补齐两侧函数闭合与测试属性」。每次冲突都消耗一个实现轮（implementation-round 3/15 → 5/15），后面再跟一个审计轮和一个复盘轮。设计 R3 预计到会和 T-37 冲突，但只预计在 write-gate.ts 的相邻行，没想到它的测试文件尾也会撞'
  root_cause: '这几个测试文件都被多个并行任务同时修改，各任务又习惯把新用例追加到文件尾或测试模块末尾。并行 worktree 都改同一处末尾，rebase 时文本冲突几乎必然发生。冲突的形态是两侧的最后一个函数缠在一起，所以每次解决都要手工补闭合括号和测试属性'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r5-impl.md'
  level: rule
  proposal: '新增测试不追加在既有测试文件（含 Rust 源文件里的 mod tests 与 TS 的 describe 块）的末尾，而是放在同主题既有用例之后，或者新建一个测试文件；判据是本任务对每个既有测试文件的 diff 都不触及基线上该文件（或该测试模块）的最后一个测试函数/用例'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-07
  domain: tooling
  symptom: '实现 #5 的 cargo test --workspace 第一次跑退出 101：automed 单元 729 通过、1 失败，失败的是上游用例 scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them（scheduler_gate_queue_tests.rs 第 225 行，C 的 gate.started 期望 0、实得 1）。单独重跑 1/1 通过，源码与 main 相同。实现轮因此不能说「全量全绿」，next-action 里多了一项「宿主复验 Rust 门禁队列偶发失败」'
  root_cause: '这条用例用 queue_job 伪造正在跑的门禁作业：只写 started 时间戳（秒），不写 pid 文件。gate.rs 的 running() 在没有 pid 时，只把 started 之后 10 秒墙钟时间内的作业算作还在跑。在整个 workspace 一起跑的负载下，从伪造 B 的作业到第 224 行 queue_run(C) 之间如果超过 10 秒，B 就被当成已经结束，名额空出来，C 的门禁随即启动。单独跑时远远用不了 10 秒，所以重试会通过。这个机制是读源码推出来的，本轮没有复现'
  evidence: 'docs/不是-bug-改建成需求-autome/evidence/M-03-r5-impl.md'
  level: test
  proposal: 'scheduler_gate_queue_tests.rs 里依赖「作业仍在跑」的用例不再依赖 started 之后 10 秒的墙钟窗口（改为写一个存活进程的 pid 或可注入的时钟），判据是把 gate.rs 的启动窗口临时改成 0 秒时这些用例仍然通过'
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-08
  domain: process
  symptom: '本任务跑了三个复盘轮（brief/retro-1.md、retro-2.md、retro-3.md），后两个都是被 rebase 冲突引出来的：审计 #3 → 复盘 #1 → rebase 冲突 → 实现 #4 → 审计 #4 → 复盘 #2 → rebase 冲突 → 实现 #5 → 审计 #5 → 复盘 #3。每个复盘轮都要重读任务文件、全部证据和审计记录，再覆盖写 lessons.md。复盘 #2、#3 的大部分条目沿用上一版，只补了冲突轮的事实'
  root_cause: 'autome-domain/src/task.rs 第 720 行的 after_the_loop：只要审计结束时没有开放的里程碑，并且复盘角色启用，就进入 Retro，不管这个任务是否已经复盘过。rebase 冲突把任务送回实现轮，修完后审计又得出「无开放里程碑」，于是每次冲突都会再派一次复盘。复盘结束（第 1187 行起）总是回到 Rebase，所以 main 每前进一次并造成冲突，就多出一轮完整的复盘'
  evidence: 'docs/不是-bug-改建成需求-autome/brief/retro-3.md'
  level: test
  proposal: '已经有过复盘轮的任务，rebase 冲突修复后的审计通过时直接回到 Rebase，不再派复盘轮，由一条 autome-domain 测试断言这条迁移；判据是这类任务的 brief/ 下只有一份 retro-*.md'
  predicted_impact: {metric: total_tokens, direction: down, scope: project, horizon: 3}
```
