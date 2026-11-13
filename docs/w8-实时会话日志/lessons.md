# 教训：W8 实时会话日志（复盘 #1）

Autome 没有为本任务记录指标，下文不引用任何指标数值。所有事实都来自 `retro.md`、`evidence/`、评审、裁决、审计记录和设计文档头部。

运行概况（可核对）：
- 设计 1/15 轮。评审 #1 提出 3 项问题，裁决 #1 全部采纳，没有复审。
- 实现 6/25 轮，审计 6 轮。M-01..M-05 全部 `已完成`。
- M-03 reopen 1（领域 logtail-live），其余里程碑 reopen 0，都是第一次审计就通过。
- 人工验收 H-01–H-05 留待合并前由用户确认。

设计按「前车之鉴」把需要监听端口的用例全部放进 M-03 一个文件，M-01、M-02、M-04、M-05 都不监听，所以这几个里程碑不受实现轮 listen EPERM 影响，一次就过。多花的一对实现轮和审计轮全部出在 M-03。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #3 和实现 #4 的 listen 探针都报 EPERM，M-03 验收命令两次都是 21 条里 18 条通过、3 条 live 用例 skipped。实现 #3 交待审时测试文件里有两处测试自身的缺陷（beforeAll 里调用 expect.poll；第 3 条重复执行 project.add），因为 beforeAll 卡在监听这一步，实现轮看不到这两处缺陷。缺陷到审计 #3 才暴露，M-03 因此 reopen 1。实现 #4 修复前的红灯和修复后的绿灯都没跑出来，main.ts 不调 logtail.close 的负向对照也只得到「环境阻塞，非有效负向证据」，三项负向对照最后全部由审计 #4 完成'
  root_cause: '实现轮会话的沙箱不允许监听回环端口，审计轮的环境允许。需要真实 /ws 的验收在实现轮从来没有执行到断言这一步，实现轮只能交一份没跑过的测试文件'
  evidence: 'docs/w8-实时会话日志/evidence/M-03-r4-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '审计 #3 在可以监听的环境里跑 M-03 验收，退出码 1，报错 `Error: expect.poll() must be called inside a test`（test/logtail-live.test.ts:66，在 beforeAll 里；第 34、77 行同类）。审计把这三处换成手写循环后，第 3 条又因第二次 startTask() 对同一个仓库重复 project.add 失败（「该目录已经是一个项目」）。产品行为在真实内核上符合设计，失败全部来自测试的准备代码'
  root_cause: '设计 M-03 第 2 条写的是「用 expect.poll 调 scheduler.tick 推进收割」，实现把调用了 expect.poll 的 startTask 放进了 beforeAll，并把注册项目和建任务写进同一个辅助函数。Vitest 的 expect.poll 只能在测试体里用。准备代码的结构在实现轮从来没执行过（见 L-01），只能等审计来发现'
  evidence: 'docs/w8-实时会话日志/evidence/M-03-r3-audit.md'
  level: rule
  proposal: 'vitest 测试文件中，beforeAll/afterAll 以及它们调用的辅助函数里不出现 `expect.poll` 或 `expect(`，等待一律用 `vi.waitFor`'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '设计原稿要求把 bell-targets.ts 里任务目标的 available 改为 true，同时在文件清单里写「已有测试文件不改」，M-05 预期 bell-view 8 条照常通过。评审 #1 D1-P03 核对 apps/web/test/bell-view.test.ts:28–31：这条用例用默认 TARGETS 断言任务通知不含 `href=`，开放任务目标后必然变红；临时探针实测 2 条中 1 条按预期失败。设计只考虑了同一文件第 62 行的路由一致性守卫，漏掉了第 28 行'
  root_cause: '设计改的是一个导出常量的默认值，却没有在测试目录里查这个符号的全部引用。它只检查了一个已知的守卫，就宣称已有测试不受影响'
  evidence: 'docs/w8-实时会话日志/w8-实时会话日志-review.md'
  level: brief
  proposal: '设计修改导出常量或默认配置的取值时，必须列出 `git grep -n <符号名>` 在测试目录下的全部命中，并逐条写明该测试是否需要修改'
  predicted_impact: {metric: impl_defects, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-04
  domain: design
  symptom: '设计原稿让日志模型从头裁掉超过 N=256 KiB 的内容，把「历史会话展开能看到完整内容」降成了「不超过 256 KiB 时完整」，理由是本机历史样本最大只有 34,832 字节，并把读取更早内容的能力放进 Backlog B-01。评审 #1 D1-P01 给出反例：`HEADER\n` 加 262,144 字节正文，展开后永远看不到 HEADER，违反 A-04。A-04 本身没有大小豁免'
  root_cause: '设计用本机样本的大小替代了任务的输入范围，把 DOM 渲染上限同时当成了内容保留上限，等于默默收窄了验收条款，而任务文件里没有任何条款允许这种收窄'
  evidence: 'docs/w8-实时会话日志/w8-实时会话日志-review.md'
  level: brief
  proposal: '设计对任务要求或验收标准加大小、数量或场景上的限定（如「不超过 N 时」）时，必须引用任务文件或任务卡中允许该限定的原文；引用不出原文的，不得加这种限定，也不得把满足原要求所需的能力放进 Backlog'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```

L-01 的 proposal 原文与 `docs/bug-loop-的定位-修复-复核/lessons.md` L-04 逐字相同，方便跨任务计数。

提交说明：本轮直接提交；若提交被拒绝，提交由 Autome 兜底。
