# W5 复盘教训

本任务 Autome 没有记录指标，下面不引用任何指标数值；所有事实取自 `retro.md` 与 `evidence/`。
运行概况（可核对）：设计 1 轮，实现 6 轮，审计 5 轮，M-01..M-05 全部首审通过，reopen 0。
唯一多出的实现轮是 M-03 的实现 #4（宿主门禁复现轮）。

```yaml
- id: L-01
  domain: tooling
  symptom: "实现 #1–#6 全部报 `listen EPERM 127.0.0.1`（另有 watcher EMFILE），Web 全量在实现轮从未跑绿；实现 #3 全量跑两次（第二次 267 秒），实现 #4 跑 ci-local.sh 706 秒，实现 #5 跑 ci-local.sh 828 秒，Web test 步骤每次都因同一原因失败；五次审计都在能监听的环境里跑出全量全绿"
  root_cause: "实现会话沙箱不允许监听回环端口，这和项目代码无关（实现 #4 用不加载项目源码的 `net.createServer().listen(0,'127.0.0.1')` 探针复现了 EPERM），但每轮仍按「可监听」的前提去跑全量，再花篇幅解释失败"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-03-r4-impl.md"
  level: brief
  proposal: "实现轮在跑 Web 全量或 scripts/ci-local.sh 前，先跑一次 listen 探针并在证据里引用探针日志；探针报 EPERM 时，本轮 `npm test --prefix apps/web` 全量至多跑一次，且不跑 scripts/ci-local.sh"
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: tooling
  symptom: "M-03 实现 #3 提交后宿主门禁失败，实现 #4 整轮只做复现：原样跑 ci-local.sh，Rust Test 1,095 个全过，未复现，结论只能是「不能判断原失败不存在，也不能声称已修复」；实现 #5 又在独立 End to end 步骤里遇到 `bug_regression_guard_checks_each_workspace_member` 全量失败、单跑通过，与实现 #4 要找的那次失败是否同源无法核实"
  root_cause: "交给实现 #4 的门禁失败信息只有 `FAILED Test` 汇总，没有失败用例名、断言或错误栈（实现 #4 证据原文）；而且 brief/impl-4.md 和 impl-3.md 相比只改了轮号和 M-03 的状态，失败信息不在简报里"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-03-r4-impl.md"
  level: brief
  proposal: "Autome 因宿主门禁失败给实现轮安排一轮时，简报里必须带上失败步骤的失败用例名，以及该用例的断言或错误段原文"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: "设计按 `vitest list` 静态枚举数写 Web 预期（287/316/329/343/350），`npm test` 实际报的是运行时数（多 14 个参数化展开，例如 330/343/357/364）；实现 #1、#2、#3、#5、#6 每轮都要把两种口径重新对一遍。设计还写死 desktop「应保持 104」，但实现 #5 的 desktop 在门禁里报 104（1 失败），审计 #6 报 165/165"
  root_cause: "设计写的用例数口径和验收命令实际输出的口径不一致，也没写明这个数依赖哪些环境条件。desktop 的 `electron dom harness` 只有在 Electron 能启动时才通过 `t.test` 注册子用例（apps/desktop/test/dom.test.js:83），所以总数会随环境变"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-02-r2-impl.md"
  level: rule
  proposal: "设计文档里每个用例总数预期都写验收命令运行时报告的数，若这个数依赖环境（Electron 能否启动、能否监听端口），要在同一句里写明各环境下的数"
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: verification
  symptom: "审计 #6 报告 desktop 165/165，与设计的 104 差 61，审计的解释是「设计里的 104 是静态数，同理」；可实现 #1、#2、#4、#5、#6 实跑 desktop 报的都是运行时 104，差额的真实来源是 Electron 能启动时 dom harness 动态注册的子用例（apps/desktop/test/dom.test.js:83），不是静态和运行时口径的差"
  root_cause: "审计照搬了 Web 那 14 个差额的解释，套到 desktop 上，没有核对差额究竟来自哪个文件"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r6-audit.md"
  level: rule
  proposal: "审计报告的用例总数和设计预期不一致时，必须写出差额来自哪个测试文件和行号，并核对差额数目；不能只用类比来解释"
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```
