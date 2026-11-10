# 教训 · 在-apps-web-建-react（W1 骨架）

复盘轮 #2 覆盖写（复盘 #1 之后又跑了实现 #6、#7 与审计 #5）。依据：Autome 指标（reopen 0、实现缺陷 0、验证缺口 0、协议失败 0、关闭后被推翻 0、人工验收未确认 2、总 tokens 3863359、总 turns 44、费用 3.8733 USD）、`retro.md`、`evidence/` 下 12 份证据、审计与裁决记录、`brief/impl-6.md` 与 `brief/impl-7.md`。

质量指标全为 0，四个里程碑都是一次审计关闭。轮次花在三处：设计定稿后被用户驳回重来（L-03）、实现轮执行环境不能回环监听（L-01、L-02）、与本任务无关的宿主门禁失败（L-04、L-06、L-07）。实现 #2、#6、#7 三轮没有推进任何里程碑，全部在修 `apps/desktop` 与 `crates/` 里的门禁失败，而任务硬性约束 1 禁止碰这两个目录。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1、#3、#4、#5、#6、#7 的 Web 全量验收全部因 `listen EPERM 127.0.0.1` 与文件监听 EMFILE 失败（每轮 server 套件 7 个用例跳过，38 个只跑 31 个），复盘 #1 写下本条之后的实现 #6、#7 仍然如此，证据写明「当前权限模式禁止提升权限」；审计 #1–#5 在另一环境跑同样命令全部通过'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮会话允许，两个角色执行环境不同；每个实现轮都重新独立复现一次 EPERM 并补写替代检查，且始终拿不到完整门禁全绿证据'
  evidence: 'docs/在-apps-web-建-react/evidence/M-04-r7-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: protocol-format
  symptom: '实现 #3 写「M-02 保留开放，不标待审」（代码已落地，专项 12/12 通过，只有全量验收被环境阻断），随后审计 #2 把同一里程碑直接复验关闭；M-01、M-03、M-04 也是同样的「开放 → 审计关闭」路径'
  root_cause: '协议里的里程碑状态只有「开放 / 待审 / 已完成」，没有一种状态能表达「实现完成、仅验收被执行环境阻断」；实现轮按字面留「开放」，调度却照样派审计，状态与实际流转不一致'
  evidence: 'docs/在-apps-web-建-react/evidence/M-02-r3-impl.md'
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '设计 #1 把 Node 下限定为 22.18.0；评审 #4 以任务 R-02「≥22」为据提出 D4-P01；裁决 #1 采纳，把下限退回 22.0，依赖降到 vite6/vitest3/eslint9+tsx；用户在定稿卡点驳回，设计 #2 改为 ≥22.22.0 + RR8 + vite8/vitest5/eslint10，又多跑了评审 #5 与裁决 #2'
  root_cause: '任务文件写的版本下限「Node ≥ 22」没有对照首选依赖当前的 engines（RR8 要求 22.22.0）；设计与裁决把这个冲突当成可单方裁定的六类问题，没有把它作为二选一交给用户'
  evidence: 'docs/在-apps-web-建-react/在-apps-web-建-react-adjudication.md'
  level: rule
  proposal: '任务文件或设计写运行时版本下限时，必须附上首选主依赖最新版 `engines` 的 `npm view` 实测值；两者冲突时在设计的「争议项」列出两套依赖组合，由用户在定稿卡点选择，裁决轮不单方采纳'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '实现 #2 没有推进任何里程碑，整轮在修任务范围外的宿主门禁：`apps/desktop/src/app-protocol.js` 顶层 `require(''electron'')` 在缺二进制时导致 2 个 Desktop 用例失败，修复后整轮门禁仍因 Rust PID 清理断言 exit=1；修复依赖用户临时授权越出任务硬性约束 1'
  root_cause: '任务开工前没人在基线提交上跑过门禁；基线本来就不绿，失败被算到本任务的实现轮里'
  evidence: 'docs/在-apps-web-建-react/evidence/M-01-r2-impl.md'
  level: protocol
  proposal: '任务进入第一个实现轮前，Autome 在基线提交上跑一次宿主门禁；有失败项时，把失败项写进实现轮简报并标为「基线失败」，不让实现轮在本任务内修复它们'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: '复盘 #1 与复盘 #2 收到的 Autome 指标都是「设计轮 0/15、实现轮 0/0」，而设计文档状态块终值是 `design-round: 2/15`、`implementation-round: 7/20`，`retro.md` 记有设计 #1–#2、实现 #1–#7；复盘 #1 之后又跑了两个实现轮，指标仍不变'
  root_cause: 'Autome 记录轮次指标时取到的不是任务分支上状态块的当前值，指标来源是过期快照或读错了检出'
  evidence: 'docs/在-apps-web-建-react/在-apps-web-建-react.md'
  level: test
  proposal: 'Autome 增加一条测试：任务结束后，指标中的 design_rounds_used 与 impl_rounds_used 分别等于任务分支上设计文档状态块 `design-round`、`implementation-round` 的分子'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```

```yaml
- id: L-06
  domain: process
  symptom: '实现 #2 已经定位 `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 的失败机制（wrapper 先发布退出标记再删 PID，测试见到 Done 就断言 PID 已删，crates/automed/src/gate.rs:300），按范围约束不修；设计文档 `## Backlog` 始终为空，实现 #3–#5 证据里也再没提到它；所有里程碑关闭后它在门禁里再次失败，实现 #7 专门花一整轮修它'
  root_cause: '实现轮诊断出的范围外门禁失败只写在当轮证据文件里，没有进入任何跨轮会读的位置（Backlog、简报、独立任务），下一次门禁失败时又从零复现'
  evidence: 'docs/在-apps-web-建-react/evidence/M-04-r7-impl.md'
  level: rule
  proposal: '实现轮诊断出与本任务改动无关、且按范围约束不修的门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-07
  domain: tooling
  symptom: '实现 #6、#7 是在所有里程碑已关闭后因宿主门禁失败派出的修复轮；两份证据都写「用户提供的宿主摘要没有失败断言，不能声称已核对宿主失败的具体用例与这里相同」，两轮都先在本地原样跑一遍 ci-local（154s、75s）才找到失败用例，而简报 `brief/impl-6.md`、`brief/impl-7.md` 的「预算与用户表态」只有预算一行'
  root_cause: '宿主门禁失败交给修复轮时只传了摘要，没有失败用例名与断言原文；会话只能自己重跑全量门禁猜测是哪一个，而时序类失败在本地不一定复现同一个'
  evidence: 'docs/在-apps-web-建-react/evidence/M-04-r6-impl.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
