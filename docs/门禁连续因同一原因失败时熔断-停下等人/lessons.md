# 教训 · 门禁连续因同一原因失败时熔断、停下等人（T-28）

复盘轮 #4 覆盖写。第 3 轮复盘截止在审计 #7，本轮补入实现 #10 和审计 #8。依据：任务文件、`retro.md`、`evidence/` 下 18 份证据、审计与裁决记录、`brief/` 下的简报、主仓库 `.autome/output/gates/T-28/` 下 10 份宿主门禁日志及其退出码，以及本轮实跑的 `git merge-base HEAD main`、`git merge-base --is-ancestor main HEAD` 和 `git merge-tree --write-tree HEAD main`。Autome 没有记录本任务的指标，所以下文不引用任何指标数值。文中的数字都来自证据文件、日志或仓库内容。

运行概况（取自状态块、`retro.md` 和门禁日志）：
- 设计 1 轮定稿，实现 10 轮，审计 8 轮。M-01..M-05 都在第一次审计时关闭，没有 reopen。状态 `已完成`，next-action 为「无」。
- 宿主门禁共跑 10 次，2 次退出码为 1：`ad5e78d` 和 `6c143f5`。两次都失败在 end_to_end 里 World 用例的 `assert_eq!(w.node(&id)…)` 状态断言上，不是墙钟期限断言。其余 8 次退出 0，最后一次是 `1289c2e`。
- 实现 #7 到 #10 都不对应开放里程碑。#7、#8、#10 处理与默认分支的 render.rs 冲突，#9 修 `6c143f5` 的门禁失败。
- H-01（真实桌面端的失败卡片和恢复按钮）仍在人工验收清单里。
- `retro.md` 仍然只有逐轮记录，没有 §终止 要求的总结段。

与第 3 轮复盘相比的改动：
- **L-08：冲突已经解决，但用了三轮。** 实现 #10 执行 `git rebase main`，复现冲突后按双方并集解决。本轮实跑结果：merge-base 是 main 的 `5a8cd57`，`git merge-base --is-ancestor main HEAD` 退出 0，`git merge-tree --write-tree HEAD main` 退出 0。第 3 轮「冲突仍在」的判断被实现 #10 证实，随后也被它消除。实现 #10 证明冲突已解决用的是 `--is-ancestor`，没有用 merge-tree，所以 proposal 改为两者都认。
- **L-04：补入冲突轮次。** brief/impl-7、8、9、10 四份简报两两之间只差轮次号，都不含「冲突」「rebase」字样。实现 #10 的证据自称在做「用户明确要求的 rebase 冲突修复」，但这个要求不在它的简报里。proposal 保留原文。
- **L-05：补入一条照做的对照。** 实现 #10 没有在会话内全量跑 ci-local.sh，证据写「合并后的宿主门禁由 Autome 复验」，宿主门禁在 `1289c2e` 随即通过，整轮没有花篇幅解释与产品无关的失败。proposal 保留原文。
- **L-09：补入审计 #8。** 审计 #8 再次以宿主门禁通过（`1289c2e`）作为依据。`6c143f5` 上真正失败的 legacy_bug 用例仍然没人查，Backlog 里也仍然没有它。第 3 轮写的「next-action 仍写『确认 Test 超时修复』」已经不成立：实现 #10 把它改成了「无」。
- L-01、L-02、L-03、L-06 不变。

跨任务复用：L-01 与 `内核支持任务类型-需求-bug` 等三份任务的 L-01 相同；L-04 与 `在-apps-web-建-react` 的 L-07、`服务端每次-tick-后用-events` 的 L-06 相同；L-05 与 `在-plugins-新增-autome-插件` 的 L-02 相同；L-06 与 `用户按合并时如果默认分支已经前进-任务自动` 的 L-04 相同。这四条的 `proposal` 一字未改，方便跨任务计数。L-02、L-03、L-08、L-09 是本任务新出现的。

```yaml
- id: L-01
  domain: verification
  symptom: '设计 M-03 验收契约列了 5 条负向对照，实现 #3 只实跑了 4 条（范围 0..=10、查询不分节点、关闭重放、忽略 gate.reused），没有跑「计数只存内存时『重开数据库接着数』用例失败」这一条；审计 #3 也写「未单独注入」，用「实现从账本 last_event_in 取值、无内存状态」的推理代替了这条对照'
  root_cause: '这条对照不是改一两行就能完成的变异，要另写一份只存内存的实现，于是实现轮和审计轮都凭推理放过了它；协议也不要求每条对照在证据里留下退出码，所以漏跑一条不会被发现'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '实现轮对验收契约中的每条负向对照，必须对契约点名的那条测试实跑该变异并在证据里写出它的退出码；点名的测试没有变红时，在同一轮改写契约里的这条表述'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '负向对照「重跑门禁只给 RunCoreStep 不给 DiscardGateRuns」在两个会话里各被注入错位置一次。实现 #2 第一次替换了 task.rs 里第一个 DiscardGateRuns，那是既有的 Resume 分支，用例仍绿，作废后重新定位才变红；审计 #4 在 M-04 做同一类对照时又先改了 Resume 与 extend 分支，用例仍绿，改到 RerunFrom 分支才变红'
  root_cause: '设计 M-02、M-04 的负向对照只写行为（「重跑门禁不丢弃旧作业」），没写注入点；DiscardGateRuns 在 task.rs 出现 7 次、在 bug_loop.rs 出现 6 次，两个会话都按文本搜索命中了不在用例路径上的分支'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-02-r2-impl.md'
  level: rule
  proposal: '验收契约里的每条负向对照都必须写出注入点（文件名加函数名或 match 分支的模式），只写行为描述的对照不得进入定稿设计'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '本任务两次宿主门禁失败都失败在 World 用例的 assert_eq!(w.node(&id).as_deref(), …) 上，日志里只有左右两个值。ad5e78d 失败的是 bug_loop_fix_rounds_run_out_and_extend_adds_more（end_to_end.rs:4427，实际 None），6c143f5 失败的是 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it（end_to_end.rs:4284，实际 Some("retro")，期望 Some("implement")）。前一次花掉实现 #4 一整轮：跑了一次 ci-local，又单跑 11 次，都没复现，结论是「因果证据仍未闭合」；后一次的根因到任务结束也没人查。作为对照，审计 #6 遇到的 Workspace 用例失败带有 w.why(&id) 输出，日志里直接给出了状态、跑过的步骤和事件序列，审计当轮就定位出了原因'
  root_cause: 'end_to_end.rs 已有诊断方法 why()，能打印任务状态、替身实际跑过的轮次、事件序列与会话日志。它是 2026-09-22 为「CI 失败、本机复现不了」加的，但只实现在 Workspace 上。World 的状态断言失败时只打印一个节点名，看不出是熔断、会话异常、协议失败还是调度顺序问题，偶发失败也就没法事后分析'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r4-impl.md'
  level: test
  proposal: 'crates/automed/tests/end_to_end.rs 的 World 提供与 Workspace::why 内容相同的诊断方法，文件里每一条 assert_eq!(w.node(…), …) 断言都把它作为失败消息'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: '为门禁失败、默认分支冲突或用户要求派出的实现轮，简报里都没有派出原因。brief/impl-4.md 与 impl-3.md 只差轮次号和 M-03 状态；brief/impl-7、8、9、10 四份两两之间只差第 1 行和第 232 行的轮次号，都不含「冲突」「rebase」字样，「上一轮对手轮的结论」写的都是「没有对应的里程碑」。实现 #9 的证据自述「用户提供的 6c143f5 门禁尾部只显示 Test 失败，没有失败用例栈」，于是借另一条用例推断根因，修了一条并没有在 6c143f5 失败的用例；实现 #8 没有拿到冲突的具体情况，只查了工作区状态，就得出冲突已不存在的错误结论；实现 #10 自述做的是「用户明确要求的 rebase 冲突修复」，但这个要求不在它的简报里'
  root_cause: '简报只从里程碑表和审计文件取材，没有「本轮为何被派出」这一栏。门禁失败、合并冲突触发的轮次拿到的简报与普通实现轮一样；而 ci-local 的汇总尾部只有阶段名，不含 panicked 行'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/brief/impl-9.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: '会话内全量跑 sh scripts/ci-local.sh 共 4 次，另有审计 #1 跑过 1 次 cargo test --workspace。没有一次能直接当作结论，每次都要另花篇幅解释与产品无关的失败。实现 #4：564 秒，退出 1，Web test 因 listen EPERM/EMFILE 失败 11 项。审计 #1：end_to_end 与另一 worktree 并发，legacy_bug 用例超过 15 秒期限。审计 #6：Test 阶段 workspace_bug 用例超过 30 秒期限，同一次运行的 End to end 阶段却通过了。实现 #9 跑了两次（修复前 515 秒，修复后 407 秒），都因 Web test listen EPERM 退出 1，证据结论是「未取得完整门禁通过证据，最终以 Autome 提交后宿主复验为准」，宿主门禁随后在 62d15bf 上 653 秒全部通过。对照：实现 #10 没有在会话内全量跑，只跑 cargo test -p autome-cli（34 个全过）和 fmt，证据写「合并后的宿主门禁由 Autome 复验」，宿主门禁随即在 1289c2e 通过'
  root_cause: '宿主门禁本来就会在提交上跑同一个脚本，会话内再全量跑一遍得不到新信息，反而把会话环境的差异（沙箱回环监听权限、与其它 worktree 并发时的墙钟期限）当成失败带进证据；简报也没写「全仓门禁以宿主门禁为准」'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r9-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: 'bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it 在本任务里失败过两次：一次在审计 #1 的全量运行中超过 15 秒期限，一次在宿主门禁 6c143f5 中失败于 end_to_end.rs:4284，断言为 Some("retro") 对 Some("implement")。两次都只留在证据文件或门禁日志里。设计文档的 ## Backlog 到任务结束只有 B-01，记的是 workspace_bug 用例的期限已改为 90s；这条 legacy 用例不在里面，ad5e78d 上 fix_rounds_run_out 那条用例也不在里面'
  root_cause: '与本任务无关、按范围不修的失败没有固定去处，写不写进 Backlog 全看当轮会话自己的判断。只写进证据的失败，后续轮次读不到；本任务里的后果是，这条用例的第二次失败被算到了另一条用例名下（见 L-09）'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-01-r1-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-08
  domain: verification
  symptom: '默认分支在本任务分叉（70cafab）之后合入了 c95ef10，它也改了 render.rs。同一个冲突前后用了三个实现轮。实现 #7 把对面的 42 行修复和 2 条测试按内容搬进本分支，证据写的是「git diff main -- render.rs 没有删除行」，但没有合入 main 的提交。实现 #8 只查了 git ls-files -u 和工作区冲突标记，结论是「不能将过去发生的冲突当作当前仍未解决的冲突」；复盘轮 #3 对 main 实跑 git merge-tree，仍然输出 CONFLICT (content): Merge conflict in crates/autome-cli/src/render.rs。直到实现 #10 执行 git rebase main，在重放 47f892b 时复现了同一冲突，解决后 git merge-base --is-ancestor main HEAD 退出 0，冲突才真正消除。实现 #10 解决后的 render.rs 与它开始时 50212c0 上的内容逐字一致（git diff 50212c0 -- render.rs 为空），可见内容早就对了，缺的只是 main 的提交'
  root_cause: '三方合并比较的是双方相对 merge-base 的改动，不是最终内容。只要本分支没有包含默认分支的提交，同一位置两边的新增就仍算冲突，内容一致也没用。工作区和索引里的冲突状态只在合并进行中才存在，查它们证明不了本分支能与默认分支合并'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r8-impl.md'
  level: rule
  proposal: '处理与默认分支冲突的轮次，证据必须包含结束时 git merge-base --is-ancestor <默认分支> HEAD 退出 0，或 git merge-tree --write-tree HEAD <默认分支> 无 CONFLICT 行；只检查 git ls-files -u、工作区冲突标记或内容差异的轮次不得写「冲突已解决」'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-09
  domain: verification
  symptom: '实现 #9 是为修 6c143f5 的宿主门禁失败派出的，但它没有读这个提交的完整门禁日志 gate-6c143f56cbe2.log（实现 #4 读过同目录下 ad5e78d 的日志）。它根据审计 #6 沙箱日志里 workspace_bug 的 30s 超时，把两条 Workspace 用例的期限改成了 90s；而真正失败的是 legacy_bug 用例在 end_to_end.rs:4284 的状态断言，这条用例没有改动。审计 #7 以「62d15bf 宿主门禁全绿」为依据，写下「这同时消除了…悬挂项」，结论是通过；审计 #8 又以「宿主门禁在 1289c2e 通过」收尾。两次审计都没有核对改动是否落在 6c143f5 失败的那条用例上，这条用例的失败原因到任务结束都没有被查过'
  root_cause: '对偶发失败来说，下一次门禁通过证明不了修对了，因为没修的那条用例本来也是偶尔才失败。协议没有要求修复轮先在证据里写明「失败提交的门禁日志里是哪条用例、哪一行失败」，也没有要求审计核对改动是否落在这条用例上'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r9-impl.md'
  level: rule
  proposal: '以宿主门禁失败为起因的实现轮，证据必须逐字引用失败提交门禁日志中的 panicked 行（用例全名与文件:行号）；审计只在本轮改动触及该用例或该行断言所测的代码路径时，才可把这次门禁失败记为已修复'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
