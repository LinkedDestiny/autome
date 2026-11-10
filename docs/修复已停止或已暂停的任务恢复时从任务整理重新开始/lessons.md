# 教训 · 暂停或停止的任务恢复时回到停下时的节点

```yaml
- id: L-01
  domain: process
  symptom: 实现 #2、#3、#4 三轮都以同一个门禁失败签名「Electron failed to install correctly」（apps/desktop 的 app-protocol 两个用例）结束，三轮都没有改代码，M-01 停在待审；直到实现 #5 用户明确要求修 Desktop，一处改动（把 require('electron') 挪进函数）就转绿
  root_cause: 失败落在任务 §4 禁止改动的 apps/ 下，且下载 Electron 被沙箱 DNS 挡住；实现轮只能记「环境阻塞」后标待审，没有用设计文档的 `## 争议项` 把「授权越界修复或豁免该步」交给用户，于是 Autome 每次重跑门禁都得到同样结果、再派一轮
  evidence: docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/evidence/M-01-r4-impl.md
  level: protocol
  proposal: 门禁修复轮如果因为失败在任务允许改动的目录之外而不改代码就结束，必须在设计文档 `## 争议项` 新增一条，写明失败签名与需要用户决定的事项（授权越界修复或豁免）
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: 实现 #2 记「用户摘要里的 Rust Test 失败本次未复现，摘要没有提供该失败详情」；实现 #8 原样跑两次门禁都通过，记「宿主只提供汇总，未给出 Test 的失败名，因此不能确认宿主那一次的具体失败原因」，只能凭实现 #3 留下的旧日志推测是 driver_lock 用例并去修它
  root_cause: Autome 交给门禁修复轮的门禁失败信息只有步骤级汇总，没有失败的测试名、断言行或宿主门禁日志路径；遇到偶发失败（实现 #3 先后出现 gate PID 时序与 driver_lock 两个不同失败）时，修复轮无法知道宿主那一次到底红在哪
  evidence: docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/evidence/M-03-r8-impl.md
  level: test
  proposal: Autome 派给门禁修复轮的失败信息必须包含宿主门禁完整日志的路径和每个失败步骤里的失败测试名，并由一条 automed 测试断言该文字里出现这两项
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: 实现 #6 的简报在「用户已对以下待决条目作出决定」下只列了 B-01、B-03 的原条目文字，B-01 那一行原文是「……不影响行为，本任务不做」，没有写用户的结论；实现轮实际按「纳入」做了 M-04、M-05，结论只能来自 prompt
  root_cause: 同一批表态有两个渲染器：launcher.rs 的 render_decisions 会加「→ 纳入：作为新的里程碑实现 / 忽略 / 裁定」，scheduler.rs 写简报的那一段只拼 `item_id + text`，丢掉了 disposition；简报与 prompt 对同一条表态给出相反的读法
  evidence: docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/brief/impl-6.md
  level: test
  proposal: 简报里每条用户表态都必须带「→ 纳入 / 忽略 / 裁定」结论且与 prompt 的同一条文字一致，由一条 automed 测试对同一组 DecisionRecord 同时断言简报与 prompt 的输出
  predicted_impact: {metric: reopen_total, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: 复盘轮拿到的指标是「设计轮 0/15、实现轮 0/0、里程碑数 3」，而设计文档状态块写 design-round 1/15、implementation-round 10/25，里程碑表有 M-01..M-05 五行且全部已完成，retro.md 也记了实现 #1..#10
  root_cause: 复盘轮读的是账本 tasks.metrics 列的存量值（scheduler.rs 中 Role::Retro 走 store.task_metrics），而 write_task_metrics 只在进入 cleanup、Failed、Cancelled 或「Done 且尚无指标」时才写；复盘在 cleanup 之前，拿到的是更早某次写入的旧快照（3 个里程碑、design-round 0/15、implementation-round 0/0，与评审 #4 提交 61f1177 时、裁决之前的状态块逐字段一致）
  evidence: docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/修复已停止或已暂停的任务恢复时从任务整理重新开始.md
  level: test
  proposal: 复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```
