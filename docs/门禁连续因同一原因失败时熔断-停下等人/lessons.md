# 教训 · 门禁连续因同一原因失败时熔断、停下等人（T-28）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 11 份证据、审计与裁决记录、`brief/` 下的简报，以及 `crates/automed/tests/end_to_end.rs` 的当前源码。Autome 本任务没有记录到指标，下文不引用任何指标数值；文中的数字都取自证据文件或仓库实际内容。

运行概况（取自状态块与 `retro.md`）：设计 1 轮定稿（评审 2 条全部采纳），实现 6 轮，M-01..M-05 都是一次审计关闭、没有 reopen；M-03 多用了一轮实现（#4），起因是宿主门禁在 `ad5e78d` 上一条端到端用例失败、会话内复现不了。H-01（真实桌面端失败卡片与恢复按钮）仍在人工验收清单里。`retro.md` 只有逐轮行，没有 §终止 要求的总结段；本轮按 prompt 只写本文件，没有补那一段。

六条里有四条与此前任务的教训是同一件事，`proposal` 按原文复用，以便跨任务计数：L-01 同 `内核支持任务类型-需求-bug` 等三份的 L-01；L-04 同 `在-apps-web-建-react` L-07、`服务端每次-tick-后用-events` L-06；L-05 同 `在-plugins-新增-autome-插件` L-02；L-06 同 `用户按合并时如果默认分支已经前进-任务自动` L-04。L-02、L-03 是本任务新出现的。

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
  symptom: '宿主门禁在 ad5e78d 上失败于 end_to_end 的 bug_loop_fix_rounds_run_out_and_extend_adds_more（期望 Some("await_merge")，实际 None）；实现 #4 用了一整轮复查：ci-local 一次加单跑 11 次都通过，没有改代码，结论是「原偶发失败的因果证据仍未闭合」。审计 #1 还遇到同文件另一条 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it 在并发负载下超过 15 秒期限'
  root_cause: 'end_to_end.rs 已有诊断方法 why()（打印任务状态、替身实际跑过的轮次、事件序列与会话日志），是 2026-09-22 因「CI 失败、本机复现不了」加的，但只实现在 Workspace 上；这条用例用的是 World，没有它，断言 assert_eq!(w.node(&id).as_deref(), Some("await_merge")) 失败时只打印 None，分不清是熔断、会话异常还是协议失败'
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
  symptom: '实现 #4 在会话沙箱内跑 sh scripts/ci-local.sh，564 秒后退出 1：Format、Clippy、Test、End to end、Desktop、Web build 等阶段都通过，Web test 因 listen EPERM 127.0.0.1 与 EMFILE 失败 11 项；结论写成「未取得全绿证据」，retro.md 的阻塞栏也记了「Web 监听被沙箱拒绝」'
  root_cause: '会话沙箱不允许回环监听，ci-local 的 Web test 阶段在会话内必然失败，全量跑一遍拿不到全绿证据；brief/impl-4.md 没写全仓门禁以提交后的宿主门禁为准'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-03-r4-impl.md'
  level: brief
  proposal: 宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: '审计 #1 跑 workspace 全量时，end_to_end 的 bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it 超过 15 秒内置期限失败，被判为并发负载导致、与 M-01 无关；实现 #4 对未复现的 bug_loop_fix_rounds_run_out_and_extend_adds_more 写了「后续若 Autome 再现，应采集 task.state、session 生命周期…」。两件事都只留在证据文件里，任务结束时设计文档的 ## Backlog 仍是「（无）」'
  root_cause: '与本任务无关、按范围不修的失败没有固定去处，会话只把它写进本轮证据；证据文件不会被后续任务读到，同一条用例下次失败时要从头再查'
  evidence: 'docs/门禁连续因同一原因失败时熔断-停下等人/evidence/M-01-r1-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
