# 合并后钩子 `[merge] after` · 复盘教训

复盘 #1。本任务没有 Autome 记录的指标，下文不引用指标数值，只引用证据文件里的事实。依据：`retro.md`、`evidence/` 下 9 份证据、评审 / 裁决 / 审计记录、`brief/impl-2.md`、`brief/impl-5.md`、`brief/audit-4.md` 与 git 记录。

运行概况：设计 1 轮（评审 #1 两项问题均在裁决 #1 采纳），实现 5 轮、审计 4 轮，M-01..M-04 每个里程碑都一次审计通过，没有退回，reopen 都是 0。五个实现轮里有两轮没有推进产品：实现 #2 处理宿主门禁失败，只改了一条测试的诊断消息；实现 #5 文档已完成，但因沙箱内 ci-local 失败把 M-04 留在「开放」。两轮卡住的原因都是执行环境（Electron 启动即 SIGABRT、`listen EPERM`），这两个现象在之前的任务里也出现过。下面 L-01..L-03 与之前任务的教训用同一句 proposal，方便累计。

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
  symptom: '实现轮在会话沙箱里跑了两次全量 sh scripts/ci-local.sh，两次都失败在与本任务无关的环境上：实现 #2 用时 723 秒、实现 #5 用时 429 秒，都是 Desktop 的 Electron 子进程 SIGABRT（实现 #5 单独跑 Electron --version 退出 134）、Web test 因 listen EPERM 127.0.0.1 与 EMFILE 失败 11 项、跳过 19 项。同样的命令在宿主门禁 6f1d610 上通过，审计 #4 在自己的环境里跑也是 ci exit 0，Format / Clippy / Test / Desktop / Contract / Web 全部 ok'
  root_cause: '实现轮的沙箱不允许 Electron 启动和回环监听，宿主门禁与审计轮的环境允许；M-04 的验收命令仍要求实现轮在会话里跑 ci-local，这种运行在实现轮必然失败，也给不出结论'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r5-impl.md
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: protocol-format
  symptom: '实现 #5 写完 README 与技术设计文档，cargo test --workspace 1219 个用例全部执行（1218 通过、1 个原有忽略），也有不依赖应用代码的独立复现（Electron --version 退出 134），却因 ci-local 被环境阻断写「M-04 保持开放，不标待审」；brief/audit-4.md 里 M-04 的状态仍是「开放」，Autome 照样派出审计 #4，审计复验通过后直接关闭'
  root_cause: '协议的里程碑状态只有「开放 / 待审 / 已完成」，表达不了「实现完成、只有验收被执行环境阻断」；实现轮按字面留「开放」，调度却照常派审计，状态与实际流转不一致'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r5-impl.md
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-04
  domain: design
  symptom: '设计「背景与现状」的基线一节已经写明 autome-domain tests/t14_gate_logs 有 1 个用例默认忽略，M-04 验收契约的预期却写成「各目标通过数之和为 1219」；实现 #5 实跑得到总数 1219、1218 通过、1 个忽略，只好在同一轮把契约改成「总数 1219，其中 1218 通过、1 忽略」（提交 2a338a2 的设计文档 diff）'
  root_cause: '设计把基线「总数」直接当成「通过数」写进预期，没有把默认忽略的用例单列；只要 t14_gate_logs 还带 #[ignore]，每个跑全工作区的任务都会遇到这个差 1'
  evidence: docs/合并后钩子-merge-after/evidence/M-04-r5-impl.md
  level: rule
  proposal: '验收契约里对 cargo test 用例数的预期必须分别写出总数、通过数和忽略数，三者满足总数 = 通过 + 忽略'
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```
