# 教训 · T-45 文档补齐（复盘轮 #1）

本任务没有记录到指标；下面只引用证据文件里写明的事实。运行概况（取自 `retro.md` 与里程碑表）：设计 1/15、实现 3/15，三个里程碑 reopen 均为 0，审计无实现缺陷、无验证缺口。可带走的教训因此不多，只写有证据、能在别的任务里复现的四条。

```yaml
- id: L-01
  domain: design
  symptom: 评审 #1 提出 D1-P01——设计 T-27/R-13 把 start_fix 里的修复轮预算检查写成所有修复启动路径的硬上限，而 apply_for 的手动 RerunFrom(fix) 分支直接开会话、不经该检查
  root_cause: 设计轮从守卫函数本身（start_fix）出发写陈述，没有列出能到达「开修复会话」这一动作的全部调用路径，把一条路径上的检查推广到了所有路径
  evidence: docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始-review.md
  level: rule
  proposal: 设计文档中凡写「达到上限即拒绝/失败」类守卫陈述，必须在同一条陈述或其出处里列出到达被守卫动作的每个调用点，并逐个注明经过或不经过该守卫
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 审计 #1 的第一次负向对照用了 BSD sed 不支持的 0,/re/ 地址，文档实际没被改动，检查仍 PASS；评审 #1 也有一次 cargo test 用未限定模块名的 --exact 筛选命中 0 个用例。两次都是验证动作静默空转、结果看起来像「通过」
  root_cause: 负向对照与定向测试只看检查结果，不先确认验证动作本身生效（改动确实落盘、筛选确实选中用例）
  evidence: docs/文档补齐-bug-loop-任务卡-待开始/evidence/M-01-r1-audit.md
  level: brief
  proposal: 每条负向对照在跑检查前记录 git diff --stat 非空、每条定向测试记录选中用例数不为 0，任一为空的结果在证据里标为作废并重做，不得用作通过或失败的依据
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: 实现 #3 跑全量门禁 sh scripts/ci-local.sh 退出 1（418s）——Electron 最小启动 SIGABRT、Web 测试 listen EPERM，M-03 留开放；紧接着的审计 #3 在另一执行环境同一命令 exit 0（605s），阻塞未复现
  root_cause: 实现轮会话的执行环境不允许启动 Electron、不允许监听本地端口，而 M-03 的验收命令要求在实现轮内跑含 Desktop/Web 测试层的全量门禁；简报没有告诉实现轮这一环境限制
  evidence: docs/文档补齐-bug-loop-任务卡-待开始/evidence/M-03-r3-impl.md
  level: brief
  proposal: 实现轮的全量门禁失败若只带 Electron SIGABRT 或 listen EPERM 这类环境签名，证据须逐条列出签名与受影响的测试层，并把全量门禁复验明确交给审计轮或宿主门禁，不以此为由要求再开实现轮
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: 实现 #1、#2 证据都写「此前没有该测试子集的计数基线」，无法按协议比对用例总数；实现 #3 只能从 .autome/output/audit-ci.txt 里补出 Desktop 166 的基线。审计 #1 证据只写「全部通过 423s」，审计 #2、#3 只记了 cargo（及 web）的分包数，三次审计都没写 Desktop 用例数
  root_cause: 审计证据只记门禁结论，不记各测试层的用例总数，下一实现轮拿不到可比对的上一轮基线
  evidence: docs/文档补齐-bug-loop-任务卡-待开始/evidence/M-01-r1-impl.md
  level: brief
  proposal: 每份跑过全量门禁的审计证据都写出 Rust（分包）、Desktop、Web 三个测试层各自的用例总数与通过数
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```
