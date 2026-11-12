# 合并后钩子 `[merge] after` · 复盘教训

复盘 #3，覆盖复盘 #2 的版本。本任务没有 Autome 记录的指标，下文不引用指标数值，只引用证据文件里的事实。依据：`retro.md`、`evidence/` 下 13 份证据、评审 / 裁决 / 审计记录、`brief/impl-2.md`、`brief/impl-6.md`、`brief/impl-7.md`、`brief/audit-4.md`、`brief/audit-5.md`、`brief/audit-6.md`、git 记录，以及审计 #6 的原始日志 `.autome/output/m04a-all.txt`（只用来核对 L-07）。

运行概况：设计 1 轮（评审 #1 提的两项问题，裁决 #1 全部采纳），实现 7 轮，审计 6 轮。M-01..M-04 的 reopen 都是 0，没有审计退回。复盘 #2 之后又跑了两轮：实现 #7 应用户追加要求再次 rebase 到 main（1d06101），解决了 README 冲突，又把已关闭的 M-04 改回「开放」；审计 #6 复验通过，第三次关闭 M-04。

实现 #5、#6、#7 三轮都是在沙箱里跑 ci-local 失败后把 M-04 留在「开放」，失败全部出在 Electron / Web 的执行环境上。审计 #4、#5、#6 在各自环境里跑同一条命令，每次都是 exit 0。

L-01..L-06 沿用复盘 #1 / #2 的 proposal 原句，方便跨任务累计。L-02、L-03 在实现 #7 / 审计 #6 又各出现一次，症状里已补上。L-07 是本次新增。

```yaml
- id: L-01
  domain: process
  symptom: '实现 #2 因宿主门禁在 5746d2e 上失败而派出。证据写「Autome 报告提交 5746d2ecb7 的 ci-local 仅 Desktop 失败；提供的末尾没有 Desktop 失败详情，不能将本轮的具体错误直接等同于该次错误」；brief/impl-2.md 里没有提交号、没有失败用例名，也不含「Desktop」字样。这一轮跑了 723 秒的 ci-local（退出 1），只给 apps/desktop/test/dom.test.js 加了 signal 诊断，证据自述「不是红转绿或修复完成」；宿主门禁随后在 6f1d610 上通过（审计 #1 引用），产品代码没有改动'
  root_cause: '门禁修复轮的简报不带门禁日志里的失败用例与断言原文，实现轮只能在自己的沙箱里重新复现，而沙箱里的 Electron 连 --version 都 SIGABRT，复现出来的未必是宿主上那次失败'
  evidence: docs/合并后钩子-merge-after/evidence/M-01-r2-impl.md
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '实现轮在会话沙箱里跑了四次全量 sh scripts/ci-local.sh，四次都失败在与本任务无关的环境上：实现 #2（723 秒）、实现 #5（429 秒，单独跑 Electron --version 也退出 134）、实现 #6 和实现 #7（718 秒），后两次都是 Desktop 105 项里 1 项 SIGABRT，Web 385 项里 18 项失败、19 项跳过，报错 listen EPERM 127.0.0.1 与 EMFILE。同样的命令在宿主门禁 6f1d610 上通过；审计 #4、#5、#6 在各自环境里跑都是 exit 0。审计 #5 证据写「Electron 与 Web 在本会话环境可跑，实现轮 #6 的环境阻塞不复现」（626 秒），审计 #6 写「实现 #7 的 Electron SIGABRT / Web EPERM 属环境问题，本轮未复现」（680 秒）'
  root_cause: '实现轮的沙箱不允许 Electron 启动和回环监听，宿主门禁与审计轮的环境允许；M-04 的验收命令仍要求实现轮在会话里跑 ci-local，这种运行在实现轮必然失败，也给不出结论'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r7-impl.md
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: protocol-format
  symptom: '三次同样的情况。实现 #5 写完文档，cargo test 1219 个用例全部执行，也有独立复现（Electron --version 退出 134），却因 ci-local 被环境阻断写「M-04 保持开放，不标待审」；brief/audit-4.md 里 M-04 仍是「开放」，Autome 照样派出审计 #4，审计复验通过后关闭。实现 #6 在 rebase 后又因 Electron SIGABRT、Web listen EPERM 把 M-04 改回「开放」、status 改为「实现中」；brief/audit-5.md 里 M-04 仍是「开放」，审计 #5 复验通过后关闭。实现 #7 在 rebase 并解决 README 冲突后，cargo test 1385 项全过，README 冲突专项检查 4/4 通过，只因 Desktop SIGABRT、Web EPERM/EMFILE，又把 M-04 改回「开放」，证据原文「未取得 M-04 完整通过证据，所以不标待审」；brief/audit-6.md 里 M-04 仍是「开放」，审计 #6 复验通过后关闭'
  root_cause: '协议的里程碑状态只有「开放 / 待审 / 已完成」，表达不了「实现完成、只有验收被执行环境阻断」；实现轮按字面留「开放」，调度却照常派审计，状态与实际流转不一致'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r7-impl.md
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-04
  domain: design
  symptom: '设计「背景与现状」的基线一节已经写明 autome-domain tests/t14_gate_logs 有 1 个用例默认忽略，M-04 验收契约的预期却写成「各目标通过数之和为 1219」；实现 #5 实跑得到总数 1219、1218 通过、1 个忽略，只好在同一轮把契约改成「总数 1219，其中 1218 通过、1 忽略」（rebase 后为提交 7411661 的设计文档 diff；复盘 #1 引用的是 rebase 前的 2a338a2）'
  root_cause: '设计把基线「总数」直接当成「通过数」写进预期，没有把默认忽略的用例单列；只要 t14_gate_logs 还带 #[ignore]，每个跑全工作区的任务都会遇到这个差 1'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r5-impl.md
  level: rule
  proposal: '验收契约里对 cargo test 用例数的预期必须分别写出总数、通过数和忽略数，三者满足总数 = 通过 + 忽略'
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: 'rebase 到 main（90802f6）后第一次全量 CI，end_to_end 142 项里 2 项失败：E1 找 MainCheck 的 gate.result 时 unwrap(None)，E4 期望 1 条 main_gate_failed 警报、实际 0 条。原因是 main 新增的 [gate] reuse_for_main_check 缺省为 true，树一致时直接复用合并前的成功结论，不再跑 main-should-fail 脚本。两个用例原来都调用 run_gated_task，靠缺省值保证 MainCheck 会实际执行；修复提交 c1a2c15 改成 run_gated_task_with(..., false, Some(false)) 显式关闭复用，并在设计验收契约里补写这个前置条件'
  root_cause: '用例断言的是「MainCheck 实际执行了」，但夹具没有写出决定它是否执行的配置，只依赖当时的缺省值；main 改了缺省值以后，两个判别检查不再判别任何东西，而且断言方式是报错而不是给出可读原因'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r6-impl.md
  level: rule
  proposal: '用例的断言依赖某一配置项的取值（例如断言某个门禁阶段实际执行）时，夹具必须显式写出该配置项的值，不得依赖配置缺省值'
  predicted_impact: {metric: impl_defects, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: '实现 #6 第二次全量 Rust 在 automed lib 有 1 项失败：main 的 gate_queue_upgrade_collects_two_old_jobs_without_killing_them（本轮没有改这段代码）。单独重试 1 次通过，实现轮没有判定原因，把 M-04 留在「开放」，next-action 写「核对门禁队列用例后取证待审」。审计 #5 第一次全量也撞上同一个失败（scheduler_gate_queue_tests.rs:225），于是在 git archive main 的干净副本里单跑 4+5 次，失败 2 次，判为 main 上原有的时序不稳定，记为 B-08 后关闭 M-04'
  root_cause: '实现轮遇到不属于本任务的失败用例，只做了「在本分支单跑一次」，这种做法区分不了「本任务引入」和「main 原本就不稳定」；能区分两者的干净 main 副本对照，直到审计轮才做'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r6-audit.md
  level: rule
  proposal: '全量回归中失败的用例若不在本分支相对 main 的 diff 所改动的文件里，实现轮必须在 main 的干净副本上重复运行该用例至少 5 次，并在证据里写出运行次数与失败次数，再决定里程碑状态'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-07
  domain: verification
  symptom: '审计 #6 的证据写「cargo test --workspace --all-targets：1 个失败 gate_queue_upgrade_collects_two_old_jobs_without_killing_them（B-08）……其余 1206 通过、1 忽略」，没有写总数，也没有和基线 1385 比较。原始日志里这次运行在 automed lib（744 通过、1 失败）之后就停了，退出 101；1206 + 1 + 1 = 1208 = 35 + 16 + 411 + 1 + 745，end_to_end 142、prompt_rendering 28、real_cli 7 共 177 个用例没有执行，本任务的钩子 e2e 都在 end_to_end 里。实现 #6 碰到同样的中断时写明了「第二次在 lib 失败后尚未执行 e2e/prompt_rendering/real_cli」。审计 #6 最终靠 ci-local 退出 0 补上了全量结论，证据里却没有说明这次 cargo test 是一次不完整的运行'
  root_cause: 'cargo test 默认 fail-fast，一个目标失败后就不再跑排在后面的目标。B-08 是 automed lib 里已知的间歇失败用例，而 automed lib 排在 end_to_end 前面，所以 B-08 每触发一次，全部 e2e 就跟着被跳过；证据只报通过数和失败数，不对总数，这种缺口从证据上看不出来'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r7-audit.md
  level: rule
  proposal: '全工作区 cargo test 一律加 --no-fail-fast；证据必须写出实际执行的用例总数，与基线不一致时逐个列出没有执行的测试目标'
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```
