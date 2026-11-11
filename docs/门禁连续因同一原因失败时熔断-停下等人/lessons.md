# 教训 · 门禁连续因同一原因失败时熔断、停下等人（T-28）

复盘轮 #3 覆盖写。第 2 轮复盘截止在审计 #6，本轮补入实现 #8、实现 #9 和审计 #7。依据：任务文件、`retro.md`、`evidence/` 下 16 份证据、审计与裁决记录、`brief/` 下的简报、主仓库 `.autome/output/gates/T-28/` 下 9 份宿主门禁日志及其退出码、`crates/automed/tests/end_to_end.rs` 当前源码，以及本轮对 `git merge-tree --write-tree HEAD main` 的实跑（输出存在 `.autome/output/retro3-merge-tree.txt`）。Autome 没有记录本任务的指标，所以下文不引用任何指标数值，出现的数字都来自证据文件、日志或仓库内容。

运行概况（取自状态块、`retro.md` 和门禁日志）：
- 设计 1 轮就定稿，实现用了 9 轮，审计 7 轮。M-01..M-05 都在第一次审计时关闭，没有 reopen。
- 9 次宿主门禁里有 2 次退出码为 1：`ad5e78d` 和 `6c143f5`。两次都是 end_to_end 里的 World 用例，都失败在 `assert_eq!(w.node(&id)…)` 这句状态断言上，不是墙钟期限断言。
- 实现 #7、#8、#9 都不对应开放里程碑：#7 处理 render.rs 冲突，#8 复核冲突，#9 修 `6c143f5` 的门禁失败。
- H-01（真实桌面端失败卡片和恢复按钮）还在人工验收清单里。
- `retro.md` 仍然只有逐轮记录，没有 §终止 要求的总结段。

本轮核对出两件与前几轮结论相反的事实：

1. **`6c143f5` 的门禁失败不是实现 #9 修的那条用例。** 完整门禁日志 `gate-6c143f56cbe2.log` 第 1171 行记录的失败是 `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it`，位置 `end_to_end.rs:4284:5`，断言为 `left: Some("retro")`、`right: Some("implement")`。实现 #9 只拿到日志尾部，于是借审计 #6 沙箱日志里另一条用例（workspace_bug，30s 期限）推断根因，把那条和同类的一条从 30s 改成了 90s。随后 `62d15bf` 的门禁通过，审计 #7 据此写下「宿主门禁全绿」，结论是通过。但 4284 行这条用例本身一行没改，它在本任务里已经失败过两次（另一次是审计 #1），至今不在 Backlog。
2. **render.rs 与默认分支的冲突仍然存在。** 本分支与 main 的 merge-base 仍是 `70cafab`。对 main `5a8cd57` 跑 `git merge-tree --write-tree HEAD main`，输出 `CONFLICT (content): Merge conflict in crates/autome-cli/src/render.rs`，冲突块落在合并结果第 381–407 行：本分支新增的两条 gate_same_failure 测试，对面 main 一侧为空。实现 #7 只是把 main 的修复按内容搬了过来，并没有合入 main 的提交。实现 #8 检查的是 `git ls-files -u` 和工作区冲突标记，据此得出「不能将过去发生的冲突当作当前仍未解决的冲突」，这个结论不成立。

与第 2 轮复盘相比的改动：
- 删去 L-07（端到端用例的墙钟期限）。本任务两次宿主门禁失败都不是期限断言。期限失败只出现在会话内全量运行里（审计 #1、审计 #6），这部分事实已并入 L-05。
- L-03、L-04、L-05、L-06 补入实现 #9 和 `6c143f5` 门禁的事实。L-04、L-05、L-06 的 `proposal` 保留原文。
- L-08 按「冲突仍在」改写了 proposal。第 2 轮的 L-08 只要求核对内容差异（`git diff main -- render.rs` 无删除行），而这一项实现 #7 已经做到，冲突照样留下了。
- 新增 L-09：修门禁失败的轮次，要先从失败提交的门禁日志里点名失败用例。

跨任务复用：L-01 同 `内核支持任务类型-需求-bug` 等三份任务的 L-01；L-04 同 `在-apps-web-建-react` L-07、`服务端每次-tick-后用-events` L-06；L-05 同 `在-plugins-新增-autome-插件` L-02；L-06 同 `用户按合并时如果默认分支已经前进-任务自动` L-04。这四条的 `proposal` 一字未改，方便跨任务计数。L-02、L-03、L-08、L-09 是本任务新出现的。

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
  symptom: '本任务两次宿主门禁失败都失败在 World 用例的 assert_eq!(w.node(&id).as_deref(), …) 上，日志里只有左右两个值：ad5e78d 的 bug_loop_fix_rounds_run_out_and_extend_adds_more（end_to_end.rs:4427，实际 None），6c143f5 的 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it（end_to_end.rs:4284，实际 Some("retro")，期望 Some("implement")）。前一次花掉实现 #4 一整轮，ci-local 一次加单跑 11 次都没复现，结论是「因果证据仍未闭合」；后一次的根因到任务结束也没人查。对照：审计 #6 遇到的 Workspace 用例失败带有 w.why(&id) 输出，日志里直接给出状态、跑过的步骤和事件序列，审计当轮就定位出了原因'
  root_cause: 'end_to_end.rs 已有诊断方法 why()（打印任务状态、替身实际跑过的轮次、事件序列与会话日志），是 2026-09-22 为「CI 失败、本机复现不了」加的，但只实现在 Workspace 上；World 的状态断言失败时只打印一个节点名，分不清是熔断、会话异常、协议失败还是调度顺序问题，偶发失败也就无法事后分析'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r4-impl.md'
  level: test
  proposal: 'crates/automed/tests/end_to_end.rs 的 World 提供与 Workspace::why 内容相同的诊断方法，文件里每一条 assert_eq!(w.node(…), …) 断言都把它作为失败消息'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: '两次因宿主门禁失败派出的实现轮，简报里都没有失败信息。brief/impl-4.md 与 impl-3.md 只差轮次号和 M-03 状态；brief/impl-9.md 与 impl-8.md 只差轮次号（diff 只有第 1 行和第 232 行），「上一轮对手轮的结论」写的是「没有对应的里程碑」。实现 #9 证据自述「用户提供的 6c143f5 门禁尾部只显示 Test 失败，没有失败用例栈」，于是用审计 #6 日志里另一条用例推断根因，修了一条并没有在 6c143f5 失败的用例'
  root_cause: '简报只从里程碑表和审计文件取材，没有门禁失败这一栏；门禁失败触发的轮次拿到的简报与普通实现轮一样，而 ci-local 的汇总尾部只有阶段名，不含 panicked 行'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/brief/impl-9.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: '会话内全量跑 sh scripts/ci-local.sh 共 4 次，另有审计 #1 跑过 1 次 cargo test --workspace，没有一次能当作结论，每次都要另花篇幅解释与产品无关的失败。实现 #4：564 秒，退出 1，Web test listen EPERM/EMFILE 失败 11 项。审计 #1：end_to_end 与另一 worktree 并发，legacy_bug 用例超过 15 秒期限。审计 #6：Test 阶段 workspace_bug 用例超过 30 秒期限，同一次运行的 End to end 阶段又通过了。实现 #9 跑了两次（修复前 515 秒、修复后 407 秒），两次都退出 1，都因 Web test listen EPERM，证据结论是「未取得完整门禁通过证据，最终以 Autome 提交后宿主复验为准」；宿主门禁随后在 62d15bf 上 653 秒全部通过'
  root_cause: '宿主门禁本来就会在提交上跑同一个脚本，会话内再全量跑一遍得不到新信息，只会把会话环境的差异（沙箱回环监听权限、与其它 worktree 并发时的墙钟期限）当成失败带进证据；简报没写「全仓门禁以宿主门禁为准」'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r9-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: 'bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it 在本任务里失败过两次：审计 #1 的全量运行中超过 15 秒期限；宿主门禁 6c143f5 中失败在 end_to_end.rs:4284，断言为 Some("retro") 对 Some("implement")。两次都只留在证据文件或门禁日志里。设计文档 ## Backlog 到任务结束只有 B-01，记的是 workspace_bug 用例期限已改为 90s，既没有这条 legacy 用例，也没有 ad5e78d 上 fix_rounds_run_out 那条用例'
  root_cause: '与本任务无关、按范围不修的失败没有固定去处，写不写进 Backlog 取决于当轮会话自己的判断；只写进证据的失败，后续轮次读不到，这里的后果是第二次失败被归到了另一条用例名下（见 L-09）'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-01-r1-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-08
  domain: verification
  symptom: '默认分支在本任务分叉（70cafab）后合入了 c95ef10，同样改了 render.rs。实现 #7 把对面的 42 行修复和 2 条测试按内容搬进本分支，证据写的是「git diff main -- render.rs 没有删除行」，并没有合入 main 的提交。实现 #8 被再次派来处理同一冲突，只查了 git ls-files -u 和工作区冲突标记，结论是「不能将过去发生的冲突当作当前仍未解决的冲突」。复盘轮 #3 实跑 git merge-tree --write-tree HEAD main，仍输出 CONFLICT (content): Merge conflict in crates/autome-cli/src/render.rs，冲突块位于合并结果第 381–407 行（本分支新增的 gate_same_failure 两条测试，对 main 一侧为空），merge-base 仍是 70cafab'
  root_cause: '三方合并比较的是双方相对 merge-base 的改动，不是最终内容。只要本分支没有合入默认分支的提交，同一位置两边的新增就仍算冲突，内容一致也无济于事；工作区和索引里的冲突状态只在合并进行中存在，查它们证明不了与默认分支可以合并'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r8-impl.md'
  level: rule
  proposal: '处理与默认分支冲突的轮次，证据必须包含结束时 git merge-tree --write-tree HEAD <默认分支> 的输出，且其中 CONFLICT 行数为 0；只检查 git ls-files -u 或工作区冲突标记的轮次不得写「冲突已解决」'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-09
  domain: verification
  symptom: '实现 #9 修的是 6c143f5 的宿主门禁失败，但没有读该提交的完整门禁日志 gate-6c143f56cbe2.log（实现 #4 读过同目录下的 ad5e78d 日志）。它根据审计 #6 沙箱日志里的 workspace_bug 30s 超时，把两条 Workspace 用例的期限改成 90s；真正失败的是 legacy_bug 用例在 end_to_end.rs:4284 的状态断言，这条用例没有改动。审计 #7 以「62d15bf 宿主门禁全绿」为依据，写下「这同时消除了…悬挂项」，结论是通过；状态块 next-action 至今仍写「确认 Test 超时修复」'
  root_cause: '偶发失败的修复，下一次门禁通过证明不了修对了：没修的那条用例本来也是偶尔才失败。协议没有要求修复轮先把「失败提交的门禁日志里哪条用例、哪一行失败」写进证据，也没有要求审计核对改动是否落在这条用例上'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-05-r9-impl.md'
  level: rule
  proposal: '以宿主门禁失败为起因的实现轮，证据必须逐字引用失败提交门禁日志中的 panicked 行（用例全名与文件:行号）；审计只在本轮改动触及该用例或该行断言所测的代码路径时，才可把这次门禁失败记为已修复'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
