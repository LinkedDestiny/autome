# 教训 · 宿主准备命令 [setup]（T-27）

复盘轮 #11 覆盖写。L-01..L-08 的 proposal 原文不动，方便跨任务计数。本轮不新增条目：复盘 #10 之后只多了实现 #16、宿主门禁 e31406289d 和审计 #14，新事实都落在已有条目里。本轮更新了 L-03..L-08 的 symptom，L-05、L-06、L-07 的 evidence 改指新文件。依据如下：
- `retro.md` 中实现 #16 与审计 #14 两行，以及两份新证据：`evidence/M-02-r16-impl.md`、`evidence/M-04-r16-audit-14.md`。更早的证据文件已由复盘 #1..#10 读过，本轮没有重读。
- 简报的 diff：`brief/impl-15.md` 对 `impl-16.md` 只差两行（标题和正文里的轮次号），`impl-16.md` 全文没有「冲突」二字；`brief/audit-13.md` 对 `audit-14.md` 只差轮次号和门禁提交号；`brief/retro-10.md` 对 `retro-11.md` 只差轮次号。
- 宿主门禁日志（主检出 `.autome/output/gates/T-27/`）：`gate-e31406289d48.exit` 的 exit_code=0。日志第 1105 行和第 1245 行，`bug_regression_guard_checks_each_workspace_member ... ok`。
- 本轮在 HEAD 1fd076f 上做的只读模拟 `git merge-tree --write-tree main HEAD`：退出码 1，`CONFLICT (content): Merge conflict in crates/autome-cli/src/render.rs`，输出在 `.autome/output/r11retro-merge-tree.txt`。main 仍是 5a8cd57。
- 设计文档 `## Backlog` 现有 B-01..B-06，没有一条提到 `bug_regression_guard_checks_each_workspace_member`。

**指标与实际仍然不符（第十一次）。** 本轮收到的指标和复盘 #1..#10 一样：「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、协议失败 1、总 turns 16、总 tokens 817294、费用 0.8261 USD」。设计文档状态块现在是 `status: 已完成`、`design-round: 1/15`、`implementation-round: 16/20`、`current-milestone: 无`。下文的数字取自状态块、`retro.md`、证据文件和门禁日志，不取自这组指标（见 L-04）。

**运行概况。** 设计 1 轮就定稿。M-01..M-04 都是一次审计就关闭，reopen 0，实现缺陷 0，验证缺口 0。四个里程碑关闭后，同一处 render.rs rebase 冲突让任务又转了十圈：实现 #6..#12、#14..#16，审计 #5..#14，复盘 #2..#11（实现 #13 是门禁修复轮）。冲突至今未解决。实现预算已用 16/20。

**复盘 #10 之后新增的事实：**
- 实现 #16 从 5eb9f51 开始，证据标题仍是「指定冲突文件复验」，又做了一遍实现 #7..#15 的检查：未合并索引为空、没有冲突标记、没有 rebase 目录、CLI 测试 33 个通过、`cargo fmt --check` 通过。它照样没有实跑 rebase，结论是「本轮无需再次解决冲突或修改代码」。
- 宿主在 e31406289d 上跑门禁，通过。`bug_regression_guard_checks_each_workspace_member` 两处都是 ok。
- 审计 #14 自跑了 `cargo test --test end_to_end`（96/96），并引用门禁 e31406289d 的通过结果，结论是「通过，表不变」。它也没有检查能否 rebase 到 main。

```yaml
- id: L-01
  domain: protocol-format
  symptom: '任务整理 #1（904f057）把设计文档骨架的状态块写在 text 代码围栏里，Autome 判协议失败，任务停下；Autome 随后用 82cc6e3（提交信息：状态块去掉代码围栏，任务整理轮写在了 text 代码围栏里）删掉围栏上下两行，任务才进入设计轮。这是本任务唯一一次协议失败（protocol_failures = 1）'
  root_cause: 'protocol/prompts/intake.md 第 23–31 行把要写进骨架的状态块放在 text 代码围栏里给出，提示语里没有说不要加围栏；只有 session-protocol.md 写了「不要把状态块放进代码块」，而它自己展示同一块时也用了 text 代码围栏。任务整理轮照抄了 prompt 里的示例'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/protocol/prompts/intake.md'
  level: protocol
  proposal: 'prompts/intake.md 给出的状态块骨架不放在代码围栏里，并且紧接着骨架写一句「这几行原样写成裸行，不要加代码围栏」'
  predicted_impact: {metric: protocol_failures, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '本任务里出现了两次。第一次：宿主门禁在 ac5aff9 上报 Test 失败，实现 #2 证据写「未获得原 Test 的具体失败用例及断言」，原样跑了 634 秒 ci-local，整轮没有改产品代码。第二次：宿主门禁在 b65e21b 上失败（gate-b65e21b20693.log：End to end 95 通过 / 1 失败，失败用例 bug_regression_guard_checks_each_workspace_member，end_to_end.rs:5069，state=MergeGate）。实现 #13 证据写「宿主提供的汇总没有失败用例名称」，于是去重复 Backlog 里另一条用例 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it，修掉了它的竞态并删掉 B-A2-01，又跑了 663 秒 ci-local。真正失败的那条用例没有被看过，下一次门禁 ed9678f 是因为它没抖才通过的'
  root_cause: 'Autome 派门禁修复轮时，简报里没有任何门禁信息：brief/impl-13.md 全文没有「门禁」「ci-local」或提交号，和 impl-12.md 只差轮次号。会话只能从 prompt 注入段落里拿到一句汇总，里面没有失败用例名、断言，也没有日志路径。同一时期的审计简报（brief/audit-11.md 第 239 行）倒是写了门禁提交号和完整日志的绝对路径，可见 Autome 手里有这些信息，只是没有放进实现简报'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/brief/impl-13.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: 'bug_regression_guard_checks_each_workspace_member 在本任务里失败过四次：实现 #1 的宿主门禁（ac5aff9）、审计 #3 的全量 e2e、审计 #5 的全量 cargo test（1100 通过 / 1 失败，单跑 47s 通过）、宿主门禁 b65e21b（95 通过 / 1 失败）。审计 #3 和审计 #5 都把它判为「与本任务无关的时序抖动」，但从没写进设计文档 `## Backlog`；而更早记成 B-A2-01 的那条 bug_loop 遗留任务抖动一直留在 Backlog 里。第四次失败时，门禁修复轮实现 #13 拿不到失败用例名，就按 Backlog 修了 B-A2-01 那条，并删掉了 B-A2-01。之后四次门禁（ed9678f、5915a73、43aec45、e31406289d）它都碰巧通过（gate-e31406289d48.log 第 1105、1245 行 ok）。复盘 #11 时 Backlog 只有 B-01..B-06，仍然没有这条用例的任何记录'
  root_cause: '审计轮只在本轮结论或证据里写「无关的抖动」，没有落到设计文档的 Backlog。后续轮次读的是 Backlog，不是某一轮的结论段。所以不在 Backlog 里的抖动每出现一次都要重新诊断（审计 #5 又单跑了一次）；信息不全时，还会被 Backlog 里的另一条抖动顶替（实现 #13）'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-04-r13-impl.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '复盘 #1..#11 收到的指标完全相同：「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、总 turns 16、总 tokens 817294」。复盘 #11 启动时，设计文档状态块是 design-round 1/15、implementation-round 16/20，里程碑表四行都已完成，人工验收清单 H-01、H-02 都未确认。复盘 #1 之后又跑了实现 #6..#16 和审计 #5..#14，指标里的轮次、turns、tokens 都没有跟着变'
  root_cause: 'scheduler.rs 只在第 1610 行（measure_now，即转入 Failed/Cancelled 或进入 Cleanup 时）调用 write_task_metrics。任务整理轮的围栏状态块（L-01）让任务转入 Failed，那时状态块解析失败，status 为 None，task_metrics.rs 的 aggregate 因此写出 0/0 与 0 个里程碑（第 132–143 行只在 Some 时赋值）。之后直到这几次复盘都没有再经过这几个转移，复盘 prompt 渲染的一直是这份旧指标'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-05
  domain: process
  symptom: '四个里程碑关闭、复盘 #1 结束后，合并到 main 时 render.rs 与 main 的 c95ef10（单仓库 merge.rebased 文案修复）冲突，Autome 先后派出实现 #6..#12、#14..#16。brief/impl-6.md..impl-16.md 互相之间只有轮次数字不同（impl-15.md 对 impl-16.md 只差两行），都写着「还没有里程碑表，或者所有里程碑都已完成」，都没有冲突文件、main 提交号和 rebase 状态（impl-10.md..impl-16.md 全文都没有「冲突」二字）。实现 #6 开工时没有进行中的 rebase，只能自己对比本地 main，推断出缺的是 merge.rebased 分支和两个测试；实现 #7..#12、#14..#16 开工时同样找不到冲突现场，都判定没有待解决的冲突。实现 #10..#12、#14..#16 把这项工作叫作「用户指定的」或「指定冲突文件」的兼容复验，实现 #15、#16 的证据标题就是「指定冲突文件复验」'
  root_cause: 'Autome 因合并 rebase 冲突派实现轮时，冲突文件清单只出现在会话 prompt 的注入段落里（launcher.rs 第 425–435 行「本分支 rebase 到最新默认分支时发生冲突，涉及文件……请解决这些冲突并提交」），没有写进简报。注入段落也不写 main 的提交号，不写 rebase 已被 git.rs 第 582 行 `rebase --abort` 中止。工作区里不留冲突现场，会话不知道冲突发生在重放哪一个提交时'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/brief/impl-16.md'
  level: brief
  proposal: '因合并 rebase 冲突派出的实现轮，其简报必须列出冲突文件清单、冲突时 main 的提交号，并写明 rebase 是否已中止'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: '实现 #6 在分支顶上追加 e329a0a，把 main 的 merge.rebased 分支和两个测试搬进 render.rs，用「git diff main -- render.rs 无删除行」判定冲突已解决。实现 #7..#12、#14..#16（最后一轮从 5eb9f51 开始）的证据逐条相同：`git ls-files -u` 为空、render.rs 无冲突标记、`cargo test -p autome-cli` 33 个通过；实现 #10..#12、#14..#16 只多查了「rebase-merge/rebase-apply 目录不存在」。结论都是冲突已处理或不存在，都没有实跑 rebase，实现 #15 明写「不启动 rebase」，实现 #16 写「本轮无需再次解决冲突或修改代码」。复盘 #4..#11 各用 git merge-tree 只读模拟一次，最近一次在 1fd076f 上：整条分支与 main（5a8cd57）三方合并，render.rs 仍是内容冲突（.autome/output/r11retro-merge-tree.txt）'
  root_cause: 'git.rs 第 571–583 行的合并前同步，是在任务检出里执行 `git rebase main`，一冲突就 `--abort`。rebase 逐个重放提交，第一个改 render.rs 的 ce7d7d8 就和 c95ef10 冲突，后面追加的修复提交改变不了这一步。十轮用来判断「已解决」的检查（对 main 的 diff、未合并索引、冲突标记、rebase 目录、CLI 测试）都只看当前工作区，看不到重放时才会出现的冲突'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r16-impl.md'
  level: rule
  proposal: '因 rebase 冲突派出的实现轮，结束前必须在任务检出里实跑 `git rebase <默认分支>` 并在证据中记录其退出码为 0；只凭「无冲突标记」「未合并索引为空」或「对默认分支的 diff 无删除行」判定冲突已解决的，不算完成'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-07
  domain: process
  symptom: '每次冲突修复轮声称没有待解决的冲突后，Autome 都照常派审计轮和复盘轮。最近一圈是实现 #16 → 宿主门禁 e31406289d 通过 → 审计 #14 → 复盘 #11。审计 #14 自跑 end_to_end 96/96，再引用门禁通过，结论「通过，表不变」；审计 #13 同样自跑 96/96，审计 #12 只引用门禁，审计 #11 重跑了全量 cargo test，审计 #8 什么也没跑。这些检查都不验证能否 rebase 到 main，ci-local 也不验证。同样的组合已经出现十次（实现 #6..#12、#14..#16 之后各有审计和复盘），每一圈都要等到下一次合并前同步，宿主才发现冲突还在；而 git merge-tree 几秒钟就能复现冲突（.autome/output/r11retro-merge-tree.txt）'
  root_cause: '宿主只在合并前同步时才 rebase 一次。冲突修复轮结束后，宿主没有先验证能否干净 rebase，就按普通实现轮的流程派门禁、审计和复盘。所以「冲突是否已解决」由看不到重放冲突的门禁和审计轮来判（见 L-06），十圈下来没有一个节点做过真正能证伪的检查'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-04-r16-audit-14.md'
  level: test
  proposal: '因 rebase 冲突派出的实现轮结束后，Autome 必须先对默认分支做一次 rebase 检查（实跑后中止，或 git merge-tree 只读模拟）；仍冲突就不派审计轮和复盘轮，而是再派实现轮或停下等人，并由一条 automed 测试断言这一点'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-08
  domain: process
  symptom: '复盘 #1 之后，retro.md 记了实现 #6..#16 和审计 #5..#14，结果列全是「通过」或「保留已关闭状态」。复盘 #2..#11 之间跑了十圈，每圈结束时里程碑表 M-01..M-04 都是已完成，reopen 都是 0，状态块 status 都是已完成，人工验收清单 H-01、H-02 都未确认，render.rs 的 rebase 冲突也都还在。复盘 #3..#10 写下的 L-05..L-08 在后续简报里没有任何体现（brief/impl-10.md..impl-16.md 两两之间只差轮次号），所以每圈的输入和产出都与上一圈相同；唯一的产品侧改动（实现 #13 的测试屏障）来自一次门禁失败，与冲突无关。实现预算已用到 16/20'
  root_cause: 'Autome 只按「实现 → 审计 → 复盘」的固定顺序调度，没有一个依据「上一圈是否改变了任何状态」的熔断条件。在同一任务内，复盘轮的教训不会回灌到简报（落地由用户决定），因此一圈无变化之后，下一圈注定重复'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/retro.md'
  level: test
  proposal: '同一任务内，相邻两次复盘轮之间里程碑表各行状态、reopen 计数与设计文档 status 都没有变化，且合并前同步仍失败时，Autome 不再派下一轮实现，而是停下等人，并由一条 automed 测试断言这一点'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
