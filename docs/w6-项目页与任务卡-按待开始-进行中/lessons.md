# W6 复盘教训（T-58）

本任务 Autome 没有记录指标，下面不引用任何数字指标，只引用本目录里的证据文件。
运行概况（取自 `retro.md` 与状态块）：设计 1 轮（评审 1、裁决 1），实现 6 轮，5 个里程碑审计各一次即通过，reopen 全为 0。
唯一多出来的实现轮是 M-01 的实现轮 2，起因是宿主快检报类型错误（L-01）。

```yaml
- id: L-01
  domain: verification
  symptom: M-01 实现轮 1 报待审后宿主快检的 typecheck 失败（TS2322、TS2352），多花实现轮 2 只为修类型错误；M-01 的验收子集 40 个用例两轮都是全绿
  root_cause: 设计把 lint 与 typecheck 并进「A-07 由合并前宿主门禁统一跑」，没有写进里程碑验收命令，所以实现轮 1 明确不跑这两项；这两项是静态检查，不受 listen EPERM 限制，本可在实现轮当场跑
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-01-r2-impl.md
  level: brief
  proposal: 实现轮证据在报待审之前写出 npm run typecheck 与 npm run lint 两条命令各自的退出码（apps/web 任务）
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: M-02 的负向对照「已完成卡不读 task.get → (8) 与 (10) 失败」审计实跑只有 (10) 红；(8) 测的是纯函数 cardView，不经过 loader，被变异的取数路径根本没被它执行
  root_cause: 这条对照是裁决轮为采纳 D1-P01 新写的；评审轮已对 M-01..M-05 做过同类检查（D1-P02 正是「点名用例不在被变异路径上」），但裁决轮改写后没有对新文字再做一遍同类检查，同类缺陷又被带进定稿；实现轮实跑时也只报了 (10) 红，没有指出与契约文字的出入
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-02-r3-audit.md
  level: brief
  proposal: 裁决轮新增或改写的每条负向对照，裁决记录里都要为每个点名用例写出它会执行到被变异函数的调用链（用例 → 函数 → 被变异的函数）
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: M-05 的 board-live-listen (1) 审计实测「stop 到收到 websocket 事件」4253 ms，上限 4500 ms，余量 247 ms（约 5%），审计自己记了一句「余量不大」；本次还没失败，但这条用例要在宿主门禁并发时跑
  root_cause: 设计用「ticker 3000 ms + 1500 ms 余量」推出上限，没有先实测；前车之鉴「超时不小于实测 2 倍」只管超时设置，管不到写在断言里的耗时上限，这条就漏过去了
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-05-r6-audit.md
  level: rule
  proposal: 设计里每个写进断言的耗时上限都要附一次实测值；上限小于实测值 2 倍时，设计必须在争议项写出「放宽上限」或「改测量起点」的备选
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
