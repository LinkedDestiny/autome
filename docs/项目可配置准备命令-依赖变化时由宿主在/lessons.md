# 教训 · 宿主准备命令 [setup]（T-27）

复盘轮 #20 覆盖写。L-01..L-10 的 proposal 原文不动，便于跨任务计数。本轮更新了 L-03..L-08、L-10 的 symptom；L-05、L-06、L-07、L-10 的 evidence 改指新文件；没有新增条目。依据如下：
- `retro.md` 中实现 #25 与审计 #23 两行，以及两份新证据：`evidence/M-02-r25-impl.md`、`evidence/M-04-r25-audit-23.md`。更早的证据由复盘 #1..#19 读过，本轮没有重读。
- 简报的 diff：`brief/impl-24.md` 对 `impl-25.md` 只差两行，第 1 行标题和第 232 行「本轮是第 25 轮，实现预算 N = 20」；`impl-25.md` 里「冲突」出现 0 次。`brief/audit-22.md` 对 `audit-23.md` 只差轮次号和门禁提交号；`brief/retro-19.md` 对 `retro-20.md` 只差轮次号。
- 宿主门禁日志（主检出 `.autome/output/gates/T-27/`）：`gate-e2d7a2265642.exit` 里 exit_code=0；日志第 1107 行和第 1250 行，`bug_regression_guard_checks_each_workspace_member ... ok`。
- 本轮在 HEAD ab6087e 上只读跑了 `git merge-tree --write-tree main HEAD`，退出码 **0**；`git merge-base --is-ancestor main HEAD` 退出码 0。main 仍是 5a8cd57。输出在 `.autome/output/r20retro-merge-tree.txt`。复盘 #4..#19 每次跑这条命令都是退出码 1，render.rs 内容冲突。
- 设计文档 `## Backlog` 仍是 B-01..B-06，没有一条提到 `bug_regression_guard_checks_each_workspace_member`。

**指标与实际仍然不符（第二十次）。** 本轮收到的指标和复盘 #1..#19 一样：「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、协议失败 1、总 turns 16、总 tokens 817294、费用 0.8261 USD」。设计文档状态块现在是 `status: 已完成`、`design-round: 1/15`、`implementation-round: 25/20`、`current-milestone: 无`。下文数字取自状态块、`retro.md`、证据文件、门禁日志和 git，不取这组指标（见 L-04）。

**运行概况。** 设计 1 轮定稿。M-01..M-04 都是一次审计就关闭，reopen 0，实现缺陷 0，验证缺口 0。四个里程碑关闭后，render.rs 的同一处 rebase 冲突让任务又转了十九圈：实现 #6..#12、#14..#25，审计 #5..#23，复盘 #2..#20（实现 #13 是门禁修复轮）。**冲突在实现 #25 解决了**，靠的是用户明确要求实跑 rebase，不是 Autome 的调度或简报。实现轮最终用到 25/20，超出预算五轮（见 L-10）。H-01、H-02 两条人工验收仍未确认。

**复盘 #19 之后新增的事实：**
- 实现 #25 的证据写着「本轮依据用户明确要求，实际执行 `git rebase main`」。它从 1d5b561 开始，rebase 在 141 个提交中的第 19 个（ce7d7d8）停在 render.rs 冲突（`.autome/output/r25-rebase.txt`），保留双方测试后 `git rebase --continue` 返回 0，141 个提交全部重放，HEAD 变为 49b0b47。证据写「最终 render.rs 与本轮开始时逐字一致」（`cmp` 对比 rebase 前备份）：之前十八轮查过的工作区内容一直是对的，冲突只存在于提交历史的重放过程里。
- 这一要求不在简报里：`brief/impl-25.md` 和 `impl-24.md` 只差轮次号，而 L-06 的 proposal 只写在 lessons.md 里。
- rebase 后实现 #25 自跑全量 `cargo test --workspace --all-targets` 1133 通过（比 1124 多 9 个，都来自 main）；宿主在 e2d7a22656 上跑门禁通过；审计 #23 自跑全量 1133 通过，结论「无待审」。
- 实现 #25 证据把 implementation-round 写成 25/20，没提预算已超出；审计 #23 证据也没提。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1（904f057）把设计文档骨架的状态块写在 text 代码围栏里，Autome 判协议失败，任务停下。Autome 随后用 82cc6e3（提交信息：状态块去掉代码围栏，任务整理轮写在了 text 代码围栏里）删掉围栏上下两行，任务才进入设计轮。这是本任务唯一一次协议失败（protocol_failures = 1）'
  root_cause: 'protocol/prompts/intake.md 第 23–31 行把要写进骨架的状态块放在 text 代码围栏里给出，提示语没有说不要加围栏。只有 session-protocol.md 写了「不要把状态块放进代码块」，而它自己展示同一块时也用了 text 代码围栏。任务整理轮照抄了 prompt 里的示例'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加代码围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '本任务出现了两次。第一次：宿主门禁在 ac5aff9 上报 Test 失败，实现 #2 证据写「未获得原 Test 的具体失败用例及断言」，原样跑了 634 秒 ci-local，整轮没改产品代码。第二次：宿主门禁在 b65e21b 上失败（gate-b65e21b20693.log：End to end 95 通过、1 失败，失败用例 bug_regression_guard_checks_each_workspace_member，end_to_end.rs:5069，state=MergeGate）。实现 #13 证据写「宿主提供的汇总没有失败用例名称」，于是改去处理 Backlog 里另一条用例 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it：修掉它的竞态、删掉 B-A2-01，又跑了 663 秒 ci-local。真正失败的那条用例没人看过，下一次门禁 ed9678f 通过只是因为它碰巧没抖'
  root_cause: 'Autome 派门禁修复轮时，简报里没有任何门禁信息：brief/impl-13.md 全文没有「门禁」「ci-local」或提交号，和 impl-12.md 只差轮次号。会话只能从 prompt 的注入段落拿到一句汇总，没有失败用例名、断言，也没有日志路径。同一时期的审计简报（brief/audit-11.md 第 239 行）却写了门禁提交号和完整日志的绝对路径，可见 Autome 手里有这些信息，只是没放进实现简报'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/brief/impl-13.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: 'bug_regression_guard_checks_each_workspace_member 在本任务失败过四次：实现 #1 的宿主门禁（ac5aff9）、审计 #3 的全量 e2e、审计 #5 的全量 cargo test（1100 通过、1 失败，单跑 47 秒通过）、宿主门禁 b65e21b（95 通过、1 失败）。审计 #3 和审计 #5 都判它为「与本任务无关的时序抖动」，但都没写进设计文档 `## Backlog`；而更早记成 B-A2-01 的那条 bug_loop 遗留任务抖动一直在 Backlog 里。第四次失败时，门禁修复轮实现 #13 拿不到失败用例名，就按 Backlog 修了 B-A2-01 那条，并删掉了 B-A2-01。之后十三次门禁（ed9678f、5915a73、43aec45、e31406289d、38e0887dee、1f836c23b5、8bbc0e26fd、1909c96bd9、dfd7a674a9、45d52a0dfe、052444bf10、3e7a3c0ba0、e2d7a22656）它都碰巧通过（gate-e2d7a2265642.log 第 1107、1250 行 ok）。复盘 #20 时 Backlog 只有 B-01..B-06，仍然没有这条用例的任何记录，任务即将合并，这条抖动的诊断信息会随任务结束而丢失'
  root_cause: '审计轮只在本轮结论或证据里写「无关的抖动」，没有落到设计文档的 Backlog，而后续轮次读的是 Backlog，不是某一轮的结论段。所以不在 Backlog 里的抖动每出现一次都要重新诊断（审计 #5 又单跑了一次）；信息不全时，还会被 Backlog 里的另一条抖动顶替（实现 #13）'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-04-r13-impl.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '复盘 #1..#20 收到的指标完全相同：「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、总 turns 16、总 tokens 817294」。复盘 #20 启动时，设计文档状态块是 design-round 1/15、implementation-round 25/20，里程碑表四行都已完成，人工验收清单 H-01、H-02 都未确认。复盘 #1 之后又跑了实现 #6..#25 和审计 #5..#23，指标里的轮次、turns、tokens 都没跟着变；实现轮已经超出预算五轮，指标里却还是 0/0'
  root_cause: 'scheduler.rs 只在第 1610 行（measure_now，即转入 Failed/Cancelled 或进入 Cleanup 时）调用 write_task_metrics。任务整理轮的围栏状态块（L-01）让任务转入 Failed，当时状态块解析失败，status 为 None，task_metrics.rs 的 aggregate 因此写出 0/0 和 0 个里程碑（第 132–143 行只在 Some 时赋值）。此后直到这几次复盘都没有再经过这几个转移，复盘 prompt 渲染的一直是这份旧指标'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-05
  domain: process
  symptom: '四个里程碑关闭、复盘 #1 结束后，合并到 main 时 render.rs 与 main 的 c95ef10（单仓库 merge.rebased 文案修复）冲突，Autome 先后派出实现 #6..#12、#14..#25。brief/impl-6.md..impl-25.md 互相之间只差轮次数字（impl-24.md 对 impl-25.md 只差两行），都写着「还没有里程碑表，或者所有里程碑都已完成」，都没有冲突文件、main 提交号和 rebase 状态（impl-10.md..impl-25.md 里「冲突」都出现 0 次）。实现 #6..#12、#14..#24 开工时找不到冲突现场，都判定没有待解决的冲突，实现 #15..#24 的证据标题是「指定冲突文件复验」。实现 #25 是唯一解决冲突的一轮，它的证据写明是「依据用户明确要求，实际执行 `git rebase main`」：解决冲突所需的信息和做法来自用户，简报里没有'
  root_cause: 'Autome 因合并 rebase 冲突派实现轮时，冲突文件清单只出现在会话 prompt 的注入段落里（launcher.rs 第 425–435 行「本分支 rebase 到最新默认分支时发生冲突，涉及文件……请解决这些冲突并提交」），没写进简报。注入段落也不写 main 的提交号，不写 rebase 已被 git.rs 第 582 行 `rebase --abort` 中止。工作区不留冲突现场，会话不知道冲突发生在重放哪一个提交时（实现 #25 实跑后才知道是 141 个提交中的第 19 个 ce7d7d8）'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/brief/impl-25.md'
  level: brief
  proposal: '因合并 rebase 冲突派出的实现轮，其简报必须列出冲突文件清单、冲突时 main 的提交号，并写明 rebase 是否已中止'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: '实现 #6 在分支顶上追加 e329a0a，把 main 的 merge.rebased 分支和两个测试搬进 render.rs，用「git diff main -- render.rs 无删除行」判定冲突已解决。实现 #7..#12、#14..#24 的证据逐条相同：`git ls-files -u` 为空、render.rs 无冲突标记、`cargo test -p autome-cli` 33 个通过，后几轮多查了 rebase 状态目录不存在；都没有实跑 rebase，都判定冲突已处理或不存在。复盘 #4..#19 各用 git merge-tree 只读模拟一次，退出码都是 1，render.rs 内容冲突。实现 #25 照本条 proposal 做了：实跑 `git rebase main`，停在 ce7d7d8 的 render.rs 冲突，解决后 `rebase --continue` 返回 0，141 个提交全部重放。它的证据写「最终 render.rs 与本轮开始时逐字一致」。也就是说，之前十八轮检查的工作区内容本来就对，这些检查判断的对象错了。复盘 #20 在 ab6087e 上跑 merge-tree，退出码 0，main 是 HEAD 的祖先（.autome/output/r20retro-merge-tree.txt）。十八轮没做到的事，一轮实跑就做到了'
  root_cause: 'git.rs 第 571–583 行的合并前同步，是在任务检出里执行 `git rebase main`，一冲突就 `--abort`。rebase 逐个重放提交，第一个改 render.rs 的 ce7d7d8 就和 c95ef10 冲突，后面追加的修复提交改变不了这一步。十八轮用来判断「已解决」的检查（对 main 的 diff、未合并索引、冲突标记、rebase 目录、CLI 测试）都只看当前工作区，看不到重放时才出现的冲突'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r25-impl.md'
  level: rule
  proposal: '因 rebase 冲突派出的实现轮，结束前必须在任务检出里实跑 `git rebase <默认分支>` 并在证据中记录其退出码为 0；只凭「无冲突标记」「未合并索引为空」或「对默认分支的 diff 无删除行」判定冲突已解决的，不算完成'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-07
  domain: process
  symptom: '实现 #6..#12、#14..#24 每轮都说没有待解决的冲突，Autome 每次都照常派审计轮和复盘轮，十八次都是这样。审计 #5..#22 跑的检查各轮不同：自跑全量 cargo test、end_to_end、setup 过滤，或者什么都没跑。这些检查都不验证能否 rebase 到 main，ci-local 也不验证，所以每一圈都要等下一次合并前同步，宿主才发现冲突还在。而 git merge-tree 几秒钟就能复现冲突（复盘 #4..#19 每次都复现了）。实现 #25 实跑 rebase 后，审计 #23 自跑全量 1133 通过、结论「无待审」，同样没有检查能否干净 rebase。这一次冲突确实已经解决（.autome/output/r20retro-merge-tree.txt 退出码 0），但审计的结论和前十八次一样，审计本身分不清这两种情况'
  root_cause: '宿主只在合并前同步时 rebase 一次。冲突修复轮结束后，宿主不先验证能否干净 rebase，就按普通实现轮的流程派门禁、审计和复盘。于是「冲突是否已解决」交给了看不到重放冲突的门禁和审计轮来判（见 L-06），十九圈下来没有一个 Autome 节点做过真正能证伪的检查'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-04-r25-audit-23.md'
  level: test
  proposal: '因 rebase 冲突派出的实现轮结束后，Autome 必须先对默认分支做一次 rebase 检查（实跑后中止，或 git merge-tree 只读模拟）；仍冲突就不派审计轮和复盘轮，而是再派实现轮或停下等人，并由一条 automed 测试断言这一点'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-08
  domain: process
  symptom: '复盘 #1 之后，retro.md 记了实现 #6..#25 和审计 #5..#23，结果列全是「通过」或「保留已关闭/已完成状态」。复盘 #2..#19 之间跑了十八圈，每圈结束时里程碑表 M-01..M-04 都是已完成，reopen 都是 0，状态块 status 都是已完成，人工验收清单 H-01、H-02 都未确认，render.rs 的 rebase 冲突也都还在。复盘 #3..#19 写下的 L-05..L-10 在后续简报里没有任何体现（brief/impl-10.md..impl-25.md 两两之间只差轮次号），每圈的输入和产出都和上一圈相同。复盘 #14、#15 预言第 20 轮用完后任务会因预算耗尽停下，结果没有停。打破循环的是实现 #25 收到的「用户明确要求」，不是 Autome 的任何判断：如果用户没有介入，没有证据表明第 26 圈会和前十八圈不同'
  root_cause: 'Autome 只按「实现 → 审计 → 复盘」的固定顺序调度，没有一个依据「上一圈是否改变了任何状态」的熔断条件；冲突路径上连预算这道兜底也没有（见 L-10）。同一任务内，复盘轮的教训不会回灌到简报（落地由用户决定），因此一圈无变化之后，下一圈注定重复，只能等用户发现后人工介入'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/retro.md'
  level: test
  proposal: '同一任务内，相邻两次复盘轮之间里程碑表各行状态、reopen 计数与设计文档 status 都没有变化，且合并前同步仍失败时，Autome 不再派下一轮实现，而是停下等人，并由一条 automed 测试断言这一点'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-09
  domain: tooling
  symptom: '本任务出现了两次。实现 #20 定位 rebase 状态目录时，第一次探测把 zsh 的特殊变量 `path` 当普通变量赋值。`path` 在 zsh 里与 `PATH` 绑定，赋值后这次后续的 git 调用都找不到命令；换成 `rebase_state_path` 后整段重跑才拿到结果，证据里注明「以重跑结果为准」。复盘 #15 写下本条之后，实现 #24 做同一项检查时又犯了一次，证据写「首次状态目录检查误用 zsh 特殊变量 path，导致该条命令后续 git 查找失败；改用 state_path 后重新执行成功」'
  root_cause: '会话的 shell 是 macOS 默认的 zsh。zsh 会把小写的 `path`（以及 `cdpath`、`fpath`、`manpath`）与对应的大写环境变量绑定成数组，给它们赋值就是改写命令搜索路径。在 bash 里同名变量没有这个副作用，会话按 bash 的习惯起了变量名。复盘 #15 之后本条只写在 lessons.md 里，brief/impl-24.md 没有带上它，所以实现 #24 照旧踩了一次'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r24-impl.md'
  level: rule
  proposal: '会话在 shell 命令里不得把 `path`、`cdpath`、`fpath`、`manpath` 用作自定义变量名'
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-10
  domain: process
  symptom: '实现 #20 把 implementation-round 写到 20/20 之后，Autome 在合并前同步再次 rebase 冲突时又先后派出实现 #21..#25。brief/impl-25.md 第 232 行写着「本轮是第 25 轮，实现预算 N = 20」，状态块最终是 implementation-round 25/20。协议「轮次与预算」一节写着「`k` 达到 `N` 时任务停下等用户，是否追加由用户决定」，但超出预算的五轮里没有一次停下。实现 #22..#25 的证据都写了「implementation-round 更新为 k/20」，照常做完；实现 #21 的证据和审计 #19..#23 都没提预算已超出。第 25 轮的介入来自用户，不是预算闸门'
  root_cause: 'crates/autome-domain/src/task.rs 第 1223–1231 行，`(Node::Rebase, CoreStepResult::Conflict { .. })` 不看 impl_round 与 budget，直接转到 Node::Implement 并启动实现角色。这一圈后面的 Node::Audit 分支（第 1118–1125 行）只在里程碑未全部完成时判断 `impl_round >= budget`，全部完成时直接 after_the_loop。于是「冲突 → 实现 → 门禁 → 审计 → 复盘 → rebase」这一圈里没有任何一个节点检查预算，冲突修不好就可以无限转下去'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/brief/impl-25.md'
  level: test
  proposal: 'Rebase 节点报告冲突时，若设计文档状态块的 implementation-round 已达到预算 N，Autome 不派实现轮，而是转入预算耗尽、停下等用户，并由一条 autome-domain 测试断言 impl_round == budget 时 (Node::Rebase, Conflict) 不产出 StartRole'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 1}
```
