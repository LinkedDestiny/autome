# 教训 · 合并提交的树与合并前门禁通过的树相同时跳过验证

```yaml
- id: L-01
  domain: design
  symptom: "实现 #3 跑全量 ci-local 时 apps/web 的 notifier-real 两项失败——夹具 apps/web/test/fixtures/w4-ledger.sh 依赖 MainCheck 真跑失败产生 main_gate_failed 报警，新缺省复用后通知少 1 条（14 → 13）；M-02 改了缺省，但其验收命令只跑 Rust 测试，直到最后一个里程碑才暴露，实现轮只好在 M-03 里补改夹具并改写里程碑说明"
  root_cause: "设计轮按验收 4 核对「受新缺省影响的现有用例」时，grep 只覆盖 end_to_end.rs、scheduler.rs 测试模块与 autome-cli，没有覆盖 apps/web、apps/desktop 的测试与夹具（设计「背景与现状」第 8 条只 grep 了按 gate.* 事件分支的代码，没搜 main_gate_failed）"
  evidence: docs/合并提交的树与合并前门禁通过的树相同时跳过验证/evidence/M-03-r3-impl.md
  level: rule
  proposal: "改变缺省行为的设计，在列「受影响的现有用例」时必须写出对整个仓库（不限语言，含 apps/*/test 与 fixtures）实际执行的 grep 命令及其全部命中文件"
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: "M-03 契约要求 `sh scripts/ci-local.sh` 退出码 0；实现 #3 实跑退出 1（Electron SIGABRT、listen EPERM、EMFILE），里程碑保持开放；审计 #3 实跑仍有 Desktop 一步 60 秒超时，改为单独复跑 `npm test --prefix apps/desktop` 166/166 通过，判为负载抖动并关闭 M-03，retro.md 审计 #3 一行的阻塞段写「无」。两轮证据里都没有整条命令退出 0 的运行"
  root_cause: "整条门禁命令因环境非零时协议没有判法，审计把「失败步骤单独复跑通过」当成了契约里的整条退出码，且没有把未达成的那一步记为阻塞"
  evidence: docs/合并提交的树与合并前门禁通过的树相同时跳过验证/evidence/M-03-r3-audit.md
  level: protocol
  proposal: "预期写「退出码 0」的验收命令，审计证据里若没有整条命令退出 0 的运行日志，retro.md 本轮一行的阻塞段不得写「无」，必须写出未取得退出 0 的步骤与替代证据"
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: "评审 #1 R1-P01（裁决 D1-P01 采纳）——设计初稿以 gate.result.command 认定「实际执行过的命令」，但该字段在收集结果的 tick 取当时的配置；作业运行期间把命令从 A 改成 B，结果里记 B，MainCheck 会据此误跳从未执行过的 B"
  root_cause: "设计把账本事件字段当作执行凭据时，没有核对该字段在源码中的赋值时刻（scheduler.rs 写 gate.result 时用本 tick resolve_config 的值，gate.started 才是启动时的值）"
  evidence: docs/合并提交的树与合并前门禁通过的树相同时跳过验证/合并提交的树与合并前门禁通过的树相同时跳过验证-adjudication.md
  level: rule
  proposal: "设计把账本事件的某个字段当作「实际执行了什么」的凭据时，必须写明该字段的赋值源码位置，并写明它取自作业启动时还是结果收集时"
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: "评审 #1 N1 实测多文件 `grep -c` 在一个文件 0 匹配时退出码仍为 0，M-03 负向对照「README 或技术设计没补时 && 链中断」不成立；裁决只把它记进 Backlog（D1-B01），设计定稿时验收命令没改，直到实现 #3 才由实现轮自己改写 M-03 验收契约"
  root_cause: "裁决把「负向对照实测不成立」当作非阻塞建议处理，而协议规定负向对照是验收命令有鉴别力的唯一证明"
  evidence: docs/合并提交的树与合并前门禁通过的树相同时跳过验证/合并提交的树与合并前门禁通过的树相同时跳过验证-adjudication.md
  level: protocol
  proposal: "评审实测某条负向对照不成立时，裁决轮必须当轮改写该验收命令或负向对照，不得只记入 Backlog"
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: "实现 #3 结束时 M-03 仍为「开放」（next-action 要求换环境复跑 ci-local），Autome 仍在 14 秒后生成审计简报、启动审计 #3，简报里 M-03 一行状态为「开放」，审计随后把这个从未标过「待审」的里程碑关闭"
  root_cause: "autome-domain 的 task.rs 在 Node::Implement 完成（或 Gate 通过）后无条件转 Node::Audit，不检查里程碑表里有没有「待审」行；协议规定审计「独立复验待审的里程碑」"
  evidence: docs/合并提交的树与合并前门禁通过的树相同时跳过验证/brief/audit-3.md
  level: test
  proposal: "autome-v2 增加一个测试，断言实现轮结束后里程碑表中没有「待审」行时，下一节点不是 Audit"
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
