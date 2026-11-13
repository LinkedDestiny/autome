# 教训：W8 实时会话日志（复盘 #2）

Autome 没有为本任务记录指标，下文不引用任何指标数值。所有事实都来自 `retro.md`、`evidence/`、评审、裁决、审计记录、设计文档，以及 `.autome/output/gates/T-59/` 下的宿主门禁日志。

运行概况（可核对）：
- 设计 1/15 轮。评审 #1 提出 3 项问题，裁决 #1 全部采纳，没有复审。
- 实现 7/25 轮，审计 7 轮。M-01..M-05 全部 `已完成`。M-03 reopen 1（领域 logtail-live），其余里程碑 reopen 0。
- 复盘 #1 之后的合并前全量门禁 `merge_gate-01e4ff7c3cd2`（kind full）退出码 1：Rust `end_to_end` 163 通过、1 失败（`gate_queue_order_survives_a_core_restart`）；Web 55 个测试文件中 2 个失败（`bell-system.test.ts`、`labels.test.ts`）。
- 实现 #7 修掉这三处，不改里程碑状态。之后宿主只跑了快检 `gate-352b58148f6c`（kind quick：fmt、clippy、lint、typecheck），退出码 0。本轮复盘时合并前全量门禁还没重跑，A-05 仍未取证。
- 审计 #7 没有待审里程碑，未改表。人工验收 H-01–H-05 留待合并前由用户确认。

和复盘 #1 相比的变化：
- L-03 改写了。本次门禁失败证明原 proposal（`git grep` 符号名）抓不到这次的漏网测试（见 L-03 root_cause）。
- 新增 L-05。
- L-01、L-02、L-04 原样保留。

三件不成为教训、只记录的事实：
- Rust `gate_queue_order_survives_a_core_restart` 的失败，实现 #7 只能用「回拨 61 秒后稳定变红」从机制上推断，宿主日志没有留下失败时的时序。只出现一次，根因没有证实，所以不写成教训。
- 该修复改了 `crates/automed/tests/end_to_end.rs`。任务文件「前车之鉴」原本要求范围外失败只记 Backlog，实现 #7 的证据写明这是按最新用户指令做的。
- 另外，设计文档第 71 行仍写着「`bell-system.test.ts` … 不受影响（已核对）」，和实现 #7 在第 311 行新加的说明矛盾。留给后续设计轮或用户处理，本轮不改设计文档。

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
  symptom: '设计原稿只处理了 bell-view.test.ts 第 62 行的路由一致性守卫；评审 #1 D1-P03 用探针实测补出第 28 行。裁决 #1 和设计第 71 行接着写下「bell-system.test.ts 自己注入 targets，不读默认值，不受影响（已核对）」。合并前全量门禁里 bell-system「列表缺失、刷新失败、空正文均降级；无任务不跳转，项目路由可解析」失败：同一用例第 83 行 `showSystemNotification(notifier, true, message, [item])` 没传 targets，走的是默认参数 `targets = TARGETS`，仍断言 `href: null`。实现 #7 把它改成 `/tasks/T-3`'
  root_cause: '设计、评审、裁决三轮都靠读代码判断改一个导出常量会波及哪些测试。默认参数让调用点不出现这个符号名，裁决又按文件下结论（「它自己注入」），漏掉了同一文件里不传参的那次调用。`git grep -n TARGETS -- apps/web/test` 只命中 bell-view 和 task-page，所以复盘 #1 原 L-03 的 grep 做法同样会漏。评审 #1 唯一抓准的那一处，是把改动临时施加后真跑出来的'
  evidence: 'docs/w8-实时会话日志/evidence/M-05-r7-impl.md'
  level: brief
  proposal: '设计或裁决写「某已有测试不受改动影响」时，必须附上把该改动临时施加后运行所在包全量测试得到的失败用例清单；只凭阅读代码写的「已核对」不得作为依据'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
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

```yaml
- id: L-05
  domain: verification
  symptom: '审计 #6 关闭最后一个里程碑 M-05 时只跑了验收命令（4 个文件、29 个用例）和 lint、typecheck、build，没跑 `npm --prefix apps/web test`；六个实现轮的证据也都写明未跑全量。合并前全量门禁的 Web 部分 55 个文件里 2 个失败：一个是 bell-system（见 L-03），另一个是 labels「7 views do not duplicate status, column or stage Chinese literals」——任务页新文案「读取任务失败」含状态标签「失败」，日志组件「已暂停滚动」含「已暂停」。两处都是确定性失败，在审计环境里跑一次 Web 全量就会变红，结果多花了实现 #7、审计 #7 和一次全量门禁'
  root_cause: '任务文件 §4 第 11 条要求 A-05 全绿「由合并前门禁与最后的审计给出」。设计目标表 A-05 行只写了「合并前宿主门禁」，验收约定第 376 行写「全量回归由合并前门禁负责」，把最后一次审计负责的那一半丢了。协议审计复验范围里的「项目测试体系（全量跑一次即可）」也没有执行。审计环境能监听回环（审计 #4 的 listen 探针 ok），并没有环境上的理由不跑'
  evidence: 'docs/w8-实时会话日志/evidence/M-05-r6-audit.md'
  level: brief
  proposal: '关闭最后一个开放里程碑的审计轮，证据文件必须写出任务验收「全绿」条款中全量测试命令（本任务为 `npm --prefix apps/web test`）运行时报告的用例总数与退出码'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

L-01 的 proposal 原文与 `docs/bug-loop-的定位-修复-复核/lessons.md` L-04 逐字相同，方便跨任务计数。

提交说明：本轮直接提交；若提交被拒绝，提交由 Autome 兜底。
