# 教训 · 门禁连续因同一原因失败时熔断、停下等人（T-28）

复盘轮 #2 覆盖写（第 1 轮复盘在审计 #5 之后，本轮补入实现 #7 与审计 #6）。依据：任务文件、`retro.md`、`evidence/` 下 13 份证据、审计与裁决记录、`brief/` 下的简报、审计 #6 的全量日志 `.autome/output/audit6-ci.log`，以及 `crates/automed/tests/end_to_end.rs`、`crates/autome-cli/src/render.rs` 的当前源码。Autome 本任务没有记录到指标，下文不引用任何指标数值；文中的数字都取自证据文件、日志或仓库实际内容。

运行概况（取自状态块与 `retro.md`）：设计 1 轮定稿（评审 2 条全部采纳），实现 7 轮，审计 6 轮，M-01..M-05 都是一次审计关闭、没有 reopen。M-03 多用了一轮实现（#4），起因是宿主门禁在 `ad5e78d` 上一条端到端用例失败，会话内复现不了。实现 #7 不对应任何开放里程碑：默认分支在本任务分叉后合入了 `c95ef10 fix(cli): render single-repo rebase without empty label`，同样改了 `render.rs`，两边产生冲突；实现 #7 把默认分支的修复和测试搬进本分支，审计 #6 全量复验后通过。H-01（真实桌面端失败卡片与恢复按钮）仍在人工验收清单里。`retro.md` 仍然只有逐轮行，没有 §终止 要求的总结段。

与第 1 轮复盘相比的改动：
- L-05 的根因改写。第 1 轮写的是「沙箱内 Web test 必然失败」，但审计 #6 在会话内跑同一脚本时 Web test 通过（22 个文件、225 项）。这条根因被证据推翻，已经改掉；proposal 不变。
- L-03、L-06 补入审计 #6 的事实。
- 新增 L-07（端到端用例的墙钟期限）和 L-08（与默认分支冲突时的修复方法）。

跨任务复用：L-01 同 `内核支持任务类型-需求-bug` 等三份的 L-01；L-04 同 `在-apps-web-建-react` L-07、`服务端每次-tick-后用-events` L-06；L-05 同 `在-plugins-新增-autome-插件` L-02；L-06 同 `用户按合并时如果默认分支已经前进-任务自动` L-04。这四条的 `proposal` 按原文保留，便于跨任务计数。L-02、L-03、L-07、L-08 是本任务新出现的。

```yaml
- id: L-01
  domain: verification
  symptom: '设计 M-03 验收契约列了 5 条负向对照，实现 #3 只实跑了 4 条（范围 0..=10、查询不分节点、关闭重放、忽略 gate.reused），「计数只存内存时『重开数据库接着数』用例失败」一条没有跑；审计 #3 也写「未单独注入」，用「实现从账本 last_event_in 取值、无内存状态」的推理代替了这条对照'
  root_cause: '这条对照不是改一两行就能完成的变异（要另写一份只存内存的实现），实现轮和审计轮都按推理放过；协议没有要求每条对照都在证据里留下退出码，漏跑一条不会被发现'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '实现轮对验收契约中的每条负向对照，必须对契约点名的那条测试实跑该变异并在证据里写出它的退出码；点名的测试没有变红时，在同一轮改写契约里的这条表述'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '同一条负向对照「重跑门禁只给 RunCoreStep 不给 DiscardGateRuns」被两个会话各自注入错位置一次：实现 #2 第一次替换了 task.rs 里第一个 DiscardGateRuns（既有 Resume 分支），用例仍绿，作废后重新定位才变红；审计 #4 在 M-04 做同一类对照时又先改了 Resume 与 extend 分支，用例仍绿，改到 RerunFrom 分支才变红'
  root_cause: '设计 M-02、M-04 的负向对照只写行为（「重跑门禁不丢弃旧作业」），没写注入点；DiscardGateRuns 在 task.rs 出现 7 次、bug_loop.rs 出现 6 次，两个会话都按文本搜索命中了不在用例路径上的分支'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-02-r2-impl.md'
  level: rule
  proposal: '验收契约里的每条负向对照都必须写出注入点（文件名加函数名或 match 分支的模式），只写行为描述的对照不得进入定稿设计'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '宿主门禁在 ad5e78d 上失败于 end_to_end 的 bug_loop_fix_rounds_run_out_and_extend_adds_more（期望 Some("await_merge")，实际 None）；实现 #4 用了一整轮复查：ci-local 一次加单跑 11 次都通过，没有改代码，结论是「原偶发失败的因果证据仍未闭合」。对照：审计 #6 遇到的 bug_loop_a_workspace_bug_checks_out_every_member_… 失败用的是 Workspace，断言消息是 w.why(&id)，日志里直接给出 state=Active { node: MergeGate }、跑过的步骤=1 2 3 和完整事件序列，审计在同一轮内就定位成轮询期限问题'
  root_cause: 'end_to_end.rs 已有诊断方法 why()（打印任务状态、替身实际跑过的轮次、事件序列与会话日志），是 2026-09-22 因「CI 失败、本机复现不了」加的，但只实现在 Workspace 上；fix_rounds_run_out 这条用例用的是 World，没有它，断言 assert_eq!(w.node(&id).as_deref(), Some("await_merge")) 失败时只打印 None，分不清是熔断、会话异常还是协议失败'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r4-impl.md'
  level: test
  proposal: 'crates/automed/tests/end_to_end.rs 的 World 提供与 Workspace::why 内容相同的诊断方法，文件里每一条 assert_eq!(w.node(…), …) 断言都把它作为失败消息'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: '实现 #4 是宿主门禁失败触发的，但 brief/impl-4.md 与 brief/impl-3.md 只差轮次号和 M-03 的状态（开放→待审），没有失败用例名、断言原文或门禁提交号；「上一轮对手轮的结论」一栏写的是「审计文件里没有提到 M-03」。证据里的失败信息来自会话自己读主仓库 .autome/output/gates/T-28/gate-ad5e78d0cf*.log'
  root_cause: '简报只从里程碑表和审计文件取材，没有门禁失败这一栏；门禁失败触发的轮次拿到的简报和普通实现轮一样'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/brief/impl-4.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: '会话内全量跑 sh scripts/ci-local.sh 两次，两次都报出与产品无关的失败，需要另花篇幅解释。实现 #4：564 秒后退出 1，Web test 因 listen EPERM 127.0.0.1 与 EMFILE 失败 11 项，结论写成「未取得全绿证据」。审计 #6：Web test 通过（225 项），但 Test 阶段的 bug_loop_a_workspace_bug_checks_out_every_member_… 超过 30 秒轮询期限失败，同一次运行里 End to end 阶段又让这条用例通过了；而 brief/audit-6.md 已经写明宿主门禁在同一提交 8ca459e868 上 ci-local.sh 退出码 0'
  root_cause: '宿主门禁已经在同一提交上跑过同一个脚本，会话内再全量跑一遍拿不到新信息，只会把会话环境的差异（沙箱回环监听权限因会话而异、并行负载下的墙钟期限）当成失败带进证据；简报没写「全仓门禁以宿主门禁为准」'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r4-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: '审计 #1 跑 workspace 全量时 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it 超过 15 秒期限失败；实现 #4 对未复现的 bug_loop_fix_rounds_run_out_and_extend_adds_more 写了「后续若 Autome 再现，应采集 task.state、session 生命周期…」。这两件事都只留在证据文件里。审计 #6 是本任务第一次把这类失败写进设计文档 ## Backlog（B-01，只记了 workspace_bug 那条，没写断言原文）；前两条到任务结束仍不在 Backlog'
  root_cause: '与本任务无关、按范围不修的失败没有固定去处，写不写进 Backlog 取决于当轮会话自己的判断；只写进证据文件的失败不会被后续任务读到，同一条用例下次失败要从头再查'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-01-r1-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-07
  domain: tooling
  symptom: '本任务有两条 end_to_end 用例在全量并行负载下撞到自己的墙钟轮询期限：审计 #1 的 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it（15 秒），审计 #6 的 bug_loop_a_workspace_bug_checks_out_every_member_…（end_to_end.rs:4690，30 秒）。后一条单跑要 27.4 秒，只比期限少 2.6 秒；它在 audit6-ci.log 的 Test 阶段失败（该阶段 83 项共 281.61 秒），在同一次运行的 End to end 阶段通过（190.76 秒）'
  root_cause: 'end_to_end.rs 的轮询期限写死在各用例里（10 秒 3 处、15 秒 1 处、30 秒 2 处、90 秒 3 处），按单跑耗时定，没有给 ci-local Test 阶段与其它 crate 并行时的减速留余量；失败与否取决于机器负载，与被测改动无关'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r7-audit.md'
  level: test
  proposal: 'crates/automed/tests/end_to_end.rs 里每个墙钟轮询期限都不小于该用例单跑耗时的 3 倍；新增或修改这类期限的提交，在证据里写出该用例的单跑耗时'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-08
  domain: verification
  symptom: '默认分支在本任务分叉（70cafab）之后合入 c95ef10，修了 render.rs 里单仓库 merge.rebased 的空仓库名，并加了 2 条测试；本分支也改了 render.rs，两边冲突。实现 #7 先把这 2 条测试原样搬进本分支再跑，1 过 1 败（实际输出「T-14 已自动 rebase： → main @ 5c8631f6a5」）；恢复默认分支的 42 行后 2 条都通过，git diff main -- render.rs 没有删除行'
  root_cause: '冲突解决如果只看冲突块、取本分支一侧，默认分支的修复会被静默丢掉，而本分支已有的测试全绿，看不出来；只有把默认分支在同一文件上新增的测试搬过来先跑一次红，才能证明这次冲突修复没有覆盖掉对方的改动'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r7-impl.md'
  level: rule
  proposal: '处理与默认分支冲突的轮次，证据必须包含：默认分支自分叉点以来在冲突文件上新增的测试搬入本分支后、修复前的运行结果，以及修复后 git diff <默认分支> -- <冲突文件> 的删除行数（非 0 时逐行说明）'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
