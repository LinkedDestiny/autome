# 教训 · 在-apps-web-建-react（W1 骨架）

复盘轮 #1 写。依据：Autome 指标（reopen 0、实现缺陷 0、验证缺口 0、协议失败 0、人工验收未确认 2、总 tokens 3863359、总 turns 44）、`retro.md`、`evidence/` 下 9 份证据、审计与裁决记录。质量指标全为 0，所以这里的教训都关于轮次与 token 花在了哪里，不关于缺陷。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1、#3、#4、#5 的 Web 全量验收都因 `listen EPERM 127.0.0.1` 与文件监听 EMFILE 失败（每轮 7 个服务用例跳过），里程碑都留在「开放」；审计 #1–#4 在另一个环境里跑同样的命令，全部通过，并写明「阻塞未复现」'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮会话允许，两个角色执行环境不同；每个实现轮都重新做一次独立 `net.createServer().listen(0,''127.0.0.1'')` 复现（m01/m02/m04-listen.txt），并在证据里补写替代检查'
  evidence: 'docs/在-apps-web-建-react/evidence/M-04-r5-impl.md'
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
  symptom: '实现 #2 没有推进任何里程碑，整轮在修任务范围外的宿主门禁：`apps/desktop/src/app-protocol.js` 顶层 `require(''electron'')` 在缺二进制时导致 2 个 Desktop 用例失败；Rust `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 的 PID 清理断言有时序窗口（crates/automed/src/gate.rs:300），修复后整轮门禁仍 exit=1'
  root_cause: '任务开工前没人在基线提交上跑过门禁；基线本来就不绿，失败被算到本任务的实现轮里，还要用户临时授权越出任务范围（任务硬性约束 1 禁止碰 apps/desktop 与 crates）'
  evidence: 'docs/在-apps-web-建-react/evidence/M-01-r2-impl.md'
  level: protocol
  proposal: '任务进入第一个实现轮前，Autome 在基线提交上跑一次宿主门禁；有失败项时，把失败项写进实现轮简报并标为「基线失败」，不让实现轮在本任务内修复它们'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: '复盘轮收到的 Autome 指标是「设计轮 0/15、实现轮 0/0」，而设计文档状态块终值是 `design-round: 2/15`、`implementation-round: 5/20`，`retro.md` 也记有设计 #1–#2、实现 #1–#5'
  root_cause: 'Autome 记录轮次指标时取到的不是任务终态的状态块（0/0 与设计定稿前的值一致），说明指标来源是过期快照或读错了检出'
  evidence: 'docs/在-apps-web-建-react/在-apps-web-建-react.md'
  level: test
  proposal: 'Autome 增加一条测试：任务结束后，指标中的 design_rounds_used 与 impl_rounds_used 分别等于任务分支上设计文档状态块 `design-round`、`implementation-round` 的分子'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```
