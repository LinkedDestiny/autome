# 教训 · 宿主准备命令 [setup]（T-27）

复盘轮 #7 覆盖写。L-01..L-08 的 proposal 原文不动，方便跨任务计数。本轮不新增条目，只把 L-04..L-08 的 symptom 更新到复盘 #7 时的状态，并把 L-05、L-06、L-07 的 evidence 改指实现 #11、审计 #10 的文件。依据如下：
- `retro.md` 和本轮新增的两份证据 `evidence/M-02-r11-impl.md`、`evidence/M-04-r11-audit.md`；此前的证据文件由复盘 #1..#6 读过，本轮没有重读。
- `brief/impl-10.md` 与 `impl-11.md`、`brief/audit-9.md` 与 `audit-10.md`、`brief/retro-6.md` 与 `retro-7.md` 的 diff。
- 本轮在 HEAD aaa753c 上做的只读模拟 `git merge-tree --write-tree main HEAD`：退出码 1，`CONFLICT (content): Merge conflict in crates/autome-cli/src/render.rs`，输出在 `.autome/output/r7retro-merge-tree.txt`。main 仍是 5a8cd57。
- `git diff dcc33c0 HEAD -- crates apps` 为空：复盘 #6 之后没有任何产品代码改动，L-04、L-06 引用的 scheduler.rs、task_metrics.rs、git.rs 行号仍然有效（git.rs 第 571–582 行已复核）。

**指标与实际仍然不符（第七次）。** 本轮收到的指标是「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、协议失败 1、总 turns 16、总 tokens 817294、费用 0.8261 USD」，和复盘 #1..#6 收到的一样。设计文档状态块现在是 `status: 已完成`、`design-round: 1/15`、`implementation-round: 11/20`、`current-milestone: 无`。下文的数字取自状态块、`retro.md` 和证据文件，不取自这组指标（见 L-04）。

**运行概况。** 设计 1 轮就定稿。M-01..M-04 都是一次审计就关闭，reopen 0，实现缺陷 0，验证缺口 0。四个里程碑关闭后，任务因同一处 render.rs rebase 冲突又转了六圈：实现 #6..#11、审计 #5..#10、复盘 #2..#7。冲突至今未解决。

**复盘 #6 之后新增的事实：**
- `brief/impl-11.md` 和 `impl-10.md` 只差轮次号，全文没有「冲突」二字。
- `brief/audit-10.md` 和 `audit-9.md` 只差轮次号和门禁提交号（abf94aa0d5）。
- 实现 #11 的检查和实现 #10 逐项相同：`git ls-files -u` 为空、没有冲突标记、rebase-merge 和 rebase-apply 目录都不存在、`cargo test -p autome-cli` 33 个用例通过。它仍然没有实跑 rebase，结论还是「本轮没有待解决的冲突」，还把这项工作称作「用户指定的冲突文件兼容性」。
- 审计 #10 在只改了文档的 abf94aa 上重跑了全量 `cargo test --workspace --all-targets`，1124 个通过、0 个失败，同样看不到重放时才出现的冲突。
- 实现 #11 在 `retro.md` 里写的证据路径是相对路径 `evidence/M-02-r11-impl.md`，其它行都写全路径。这不影响调度，所以不单独成条。

**为什么不加新条目：** 这一圈的现象都已被 L-05..L-08 覆盖。按「宁可少写」，只更新这几条的 symptom，好让它们在别的任务里出现时更容易对上。

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
  symptom: 'bug_regression_guard_checks_each_workspace_member 在本任务里失败过三次：实现 #1 的宿主门禁（ac5aff9）、审计 #3 的全量 e2e、审计 #5 的全量 cargo test（1100 通过 / 1 失败，单跑 47s 通过）。审计 #3 和审计 #5 都在结论里把它判为「负载下的时序抖动、与本任务无关」，但到复盘 #4，设计文档 `## Backlog` 仍然没有它的条目。同类抖动里，bug_loop 遗留任务那条到审计 #2 才记为 B-A2-01，gate.rs 超时那条到审计 #4 才记为 B-06'
  root_cause: '审计轮只在本轮结论或证据里写「无关的抖动」，没有落到设计文档的 Backlog。后续轮次和下一个任务读的是 Backlog，不是某一轮的结论段，所以同一条抖动每出现一次都要重新诊断（审计 #5 又单跑了一次、又写了一遍因果证据）'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r6-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '复盘 #1..#7 收到的指标完全相同：「设计轮 0/0、实现轮 0/0、里程碑数 0、人工验收未确认 0、总 turns 16、总 tokens 817294」。复盘 #7 启动时，设计文档状态块是 design-round 1/15、implementation-round 11/20，里程碑表四行都已完成，人工验收清单 H-01、H-02 都未确认。复盘 #1 之后又跑了实现 #6..#11 和审计 #5..#10，指标里的轮次、turns、tokens 都没有跟着变'
  root_cause: 'scheduler.rs 只在第 1610 行（measure_now 时，即转入 Failed/Cancelled 或进入 Cleanup）调用 write_task_metrics。任务整理轮的围栏状态块（L-01）让任务转入 Failed，那一刻状态块解析失败，status 为 None，task_metrics.rs 的 aggregate 因此写出 0/0 与 0 个里程碑（第 132–143 行只在 Some 时赋值）。之后直到这几次复盘都没有再经过这几个转移，复盘 prompt 渲染的一直是这份旧指标'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在.md'
  level: test
  proposal: '复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-05
  domain: process
  symptom: '四个里程碑关闭、复盘 #1 结束后，合并到 main 时 render.rs 与 main 的 c95ef10（单仓库 merge.rebased 文案修复）冲突，Autome 先后派出实现 #6..#11。brief/impl-6.md..impl-11.md 只有轮次数字不同，都写着「还没有里程碑表，或者所有里程碑都已完成」，都没有冲突文件、main 提交号和 rebase 状态（impl-10.md、impl-11.md 全文都没有「冲突」二字）。实现 #6 开工时没有进行中的 rebase（无 REBASE_HEAD、无冲突标记），只能自己对比本地 main，推断出缺的是 merge.rebased 分支和两个测试；实现 #7..#11 开工时同样找不到冲突现场，都判定「不存在正在等待解决的 Git 冲突」，实现 #10、#11 还把这项工作称作「用户指定的」render.rs 范围'
  root_cause: 'Autome 因合并 rebase 冲突派实现轮时，冲突文件清单只出现在会话 prompt 的注入段落里（launcher.rs 第 425–435 行「本分支 rebase 到最新默认分支时发生冲突，涉及文件……请解决这些冲突并提交」），没有写进简报；注入段落也不写 main 的提交号，不写 rebase 已被 git.rs 第 582 行 `rebase --abort` 中止。工作区里不留冲突现场，会话不知道冲突发生在重放哪一个提交时'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/brief/impl-11.md'
  level: brief
  proposal: '因合并 rebase 冲突派出的实现轮，其简报必须列出冲突文件清单、冲突时 main 的提交号，并写明 rebase 是否已中止'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: '实现 #6 在分支顶上追加 e329a0a，把 main 的 merge.rebased 分支和两个测试搬进 render.rs，用「git diff main -- render.rs 无删除行」判定冲突已解决。实现 #7（bdc830b 起）、#8（21fa8fa 起）、#9（4e96161 起）、#10（d91accb 起）、#11（939900b 起）的证据逐条相同：`git ls-files -u` 为空、render.rs 无冲突标记、`cargo test -p autome-cli` 33 个通过；实现 #10、#11 只多查了「rebase-merge/rebase-apply 目录不存在」。结论都是「本轮没有实际待解决的冲突」，都没有实跑 rebase。复盘 #4（7744490）、#5（0ab07f5）、#6（324443c）、#7（aaa753c）各用 git merge-tree 只读模拟一次：整条分支与 main（5a8cd57）三方合并，render.rs 都是内容冲突（.autome/output/r7retro-merge-tree.txt）'
  root_cause: 'git.rs 第 571–583 行的合并前同步，是在任务检出里执行 `git rebase main`，一冲突就 `--abort`。rebase 逐个重放提交，第一个改 render.rs 的 ce7d7d8 就和 c95ef10 冲突，后面追加的修复提交改变不了这一步。六轮用来判断「已解决」的检查（对 main 的 diff、未合并索引、冲突标记、rebase 目录、CLI 测试）都只看当前工作区，看不到重放时才会出现的冲突'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-02-r11-impl.md'
  level: rule
  proposal: '因 rebase 冲突派出的实现轮，结束前必须在任务检出里实跑 `git rebase <默认分支>` 并在证据中记录其退出码为 0；只凭「无冲突标记」「未合并索引为空」或「对默认分支的 diff 无删除行」判定冲突已解决的，不算完成'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-07
  domain: process
  symptom: '实现 #11 声称没有待解决的冲突后，Autome 照常派出审计 #10，又派出复盘 #7。审计 #10 在只改了文档的 abf94aa 上重跑了全量 cargo test --workspace --all-targets（1124 通过 / 0 失败），引用了同一提交上已通过的 ci-local；审计 #8 在同样处境下什么也没跑。这些检查都不验证能否 rebase 到 main，ci-local 也不验证。同样的组合已经出现六次：实现 #6 → 审计 #5 → 复盘 #2，实现 #7 → 审计 #6 → 复盘 #3，实现 #8 → 审计 #7 → 复盘 #4，实现 #9 → 审计 #8 → 复盘 #5，实现 #10 → 审计 #9 → 复盘 #6，实现 #11 → 审计 #10 → 复盘 #7。每一圈都要等到下一次合并前同步，宿主才发现冲突还在；而 git merge-tree 几秒钟就能复现冲突（.autome/output/r7retro-merge-tree.txt）'
  root_cause: '宿主只在合并前同步时才 rebase 一次。冲突修复轮结束后，宿主没有先验证能否干净 rebase，就按普通实现轮的流程派审计、派复盘。所以「冲突是否已解决」由一个看不到重放冲突的审计轮来判（见 L-06），六圈下来没有一个节点做过真正能证伪的检查'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-04-r11-audit.md'
  level: test
  proposal: '因 rebase 冲突派出的实现轮结束后，Autome 必须先对默认分支做一次 rebase 检查（实跑后中止，或 git merge-tree 只读模拟）；仍冲突就不派审计轮和复盘轮，而是再派实现轮或停下等人，并由一条 automed 测试断言这一点'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-08
  domain: process
  symptom: '复盘 #1 之后，retro.md 记了实现 #6..#11 和审计 #5..#10，结果列全是「通过」或「复验通过，保留已关闭状态」。复盘 #2..#7 之间跑了六圈「实现 → 审计 → 复盘」，每圈结束时里程碑表 M-01..M-04 都是已完成，reopen 都是 0，状态块 status 都是已完成，人工验收清单 H-01、H-02 都未确认，render.rs 的 rebase 冲突也都还在。复盘 #3..#6 写下的 L-05..L-08 在后续简报里没有任何体现（brief/impl-11.md 与 impl-10.md 只差轮次号），所以每圈的输入和产出都与上一圈相同'
  root_cause: 'Autome 只按「实现 → 审计 → 复盘」的固定顺序调度，没有一个依据「上一圈是否改变了任何状态」的熔断条件。在同一任务内，复盘轮的教训不会回灌到简报（落地由用户决定），因此一圈无变化之后，下一圈注定重复'
  evidence: 'docs/项目可配置准备命令-依赖变化时由宿主在/retro.md'
  level: test
  proposal: '同一任务内，相邻两次复盘轮之间里程碑表各行状态、reopen 计数与设计文档 status 都没有变化，且合并前同步仍失败时，Autome 不再派下一轮实现，而是停下等人，并由一条 automed 测试断言这一点'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
