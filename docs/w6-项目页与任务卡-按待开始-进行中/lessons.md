# W6 复盘教训（T-58，复盘轮 #2）

Autome 没有为本任务记录指标，下面不引用任何数字指标，只引用本目录里的证据文件。
运行概况（取自 `retro.md` 与设计文档状态块）：设计 1 轮（评审 1、裁决 1），实现 7 轮，审计 6 轮；5 个里程碑各被审计一次即关闭，里程碑表 reopen 全为 0。
里程碑之外多出来的两次实现轮都是宿主在里程碑子集之外的检查发现问题后补的：实现 #2 修宿主快检报的类型错误（L-01）；实现 #7 是五个里程碑全部关闭后，修合并门禁全量 Web 测试报的 2 个失败（L-03、L-04），随后审计 #6 抽检通过。
复盘轮 #1 写过 L-01..L-03。本轮覆盖重写：L-01、L-02 不变；L-03 的预警已在合并门禁上应验，按实现 #7 查明的根因改写；新增 L-04。

```yaml
- id: L-01
  domain: verification
  symptom: M-01 实现轮 1 报待审后，宿主快检的 typecheck 失败（TS2322、TS2352），多花实现轮 2 只为修类型错误；M-01 验收子集的 40 个用例两轮都全绿
  root_cause: 设计把 lint 与 typecheck 并进「A-07 由合并前宿主门禁统一跑」，没写进里程碑验收命令，所以实现轮 1 明确不跑这两项；这两项是静态检查，不受 listen EPERM 限制，本可在实现轮当场跑
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-01-r2-impl.md
  level: brief
  proposal: 实现轮证据在报待审之前写出 npm run typecheck 与 npm run lint 两条命令各自的退出码（apps/web 任务）
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: M-02 的负向对照「已完成卡不读 task.get → (8) 与 (10) 失败」审计实跑只有 (10) 红；(8) 测的是纯函数 cardView，不经过 loader，被变异的取数路径根本没被它执行
  root_cause: 这条对照是裁决轮为采纳 D1-P01 新写的；评审轮已对 M-01..M-05 做过同类检查（D1-P02 正是「点名用例不在被变异路径上」），但裁决轮改写后没有对新文字再查一遍，同类缺陷又进了定稿；实现轮实跑时只报了 (10) 红，没有指出与契约文字的出入
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-02-r3-audit.md
  level: brief
  proposal: 裁决轮新增或改写的每条负向对照，裁决记录里都要为每个点名用例写出它执行到被变异函数的调用链（用例 → 函数 → 被变异的函数）
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: M-05 审计 #5 实测 board-live-listen (1)「stop → websocket 事件」4253 ms，上限 4500 ms，只记了句「余量不大」就关闭；里程碑全部关闭后，宿主合并门禁上这条用例以「task.updated timeout 4500 ms」失败，追加了实现 #7 和审计 #6；改成从命令回复起计时后，审计 #6 实测 1898 ms
  root_cause: 设计给上限的理由只算了 ticker（3000 ms + 1500 ms 余量），但计时从命令启动开始，区间里还有 task.stop 提交状态前固定等的 3000 ms（launcher.rs stop_session），最坏情况约 6000 ms，超过 4500 ms，能不能过取决于 ticker 相位；设计轮实测过取数和附件，却没有按这个口径实测过这段耗时
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-05-r7-impl.md
  level: rule
  proposal: 设计里每个写进断言的耗时上限都要逐项列出计时起点到终点之间代码中的固定等待（sleep、ticker 周期、轮询间隔）及出处，并写出它们的最坏情况之和，上限必须大于这个和
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: M-04 新建页 projects.$id.tasks.new.tsx 把「待开始」写成字面量，审计 #4 通过、里程碑关闭；直到五个里程碑都关闭后，宿主合并门禁跑全量 Web（461 例）才被 labels.test.ts 的「views do not duplicate status, column or stage Chinese literals」拦下，修复挤进实现 #7
  root_cause: labels.test.ts 是扫描路由源码的全局守卫，新增任何视图文件都会落进它的检查范围，但 M-04 的验收命令（task-new、task-new-real）和实现轮自选的相关子集（styles、live-routes）都没包含它；全量测试被 A-07 一并交给 before_merge 门禁，所以 M-04 带进来的缺陷要等到所有里程碑都关闭后才暴露
  evidence: docs/w6-项目页与任务卡-按待开始-进行中/evidence/M-05-r7-impl.md
  level: brief
  proposal: apps/web 任务的实现轮报待审之前跑一次 cd apps/web && npx vitest run 全量，证据逐条列出失败用例并标明是否为 listen EPERM，有非 EPERM 失败就不报待审
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
