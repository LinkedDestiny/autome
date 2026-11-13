# 教训：W8 实时会话日志（复盘 #3）

Autome 没有为本任务记录指标，下文不引用任何指标数值。所有事实都来自 `retro.md`、`evidence/`、`brief/`、评审、裁决、审计记录、设计文档、任务文件，以及 `.autome/output/` 下的 rebase 记录（`r8-rebase.txt`、`r8-conflicts.txt`）。

运行概况（可核对）：
- 设计 1/15 轮。评审 #1 提出 3 项问题，裁决 #1 全部采纳，没有复审。
- 实现 8/25 轮，审计 8 轮。M-01..M-05 全部 `已完成`。M-03 reopen 1（领域 logtail-live），其余里程碑 reopen 0。
- 复盘 #1 之后的合并前全量门禁失败（Rust 1 条、Web 2 个文件）。实现 #7 修复，审计 #7 无待审里程碑。
- 复盘 #2 之后，实现 #8 按用户追加要求 `git rebase main`（main = 7f29e3dd，即 T-58 W6 合并提交），重放第 34/44 个提交 4f18bf3b 时在 `bell-targets.ts`、`routes.ts`、`app.css`、`bell-view.test.ts` 四处冲突。实现 #8 手工解决冲突，跑了 8 个文件、64 个用例，加上 lint、typecheck、build，退出码都是 0，并保留 M-05 `已完成`。审计 #8 写「无待审里程碑」，没有证据文件。审计文件记录宿主快检在 8a04951c5b 上通过，全量门禁待宿主合并前运行。
- 人工验收 H-01–H-05 仍待用户在合并前确认。

和复盘 #2 相比的变化：
- L-05 改写了，proposal 换成 `docs/w6-项目页与任务卡-按待开始-进行中/lessons.md` L-04 的原文。两个任务被同一个全局守卫（labels.test.ts）以同样的方式拦下，统一措辞后可以跨任务计数。
- 新增 L-06。proposal 与 `docs/宿主门禁按机器排队-gate/lessons.md` L-06 逐字相同，本任务实现 #8 和审计 #8 是它的第二次出现。
- 新增 L-07（并行任务的冲突文件没有预判全）。
- L-01..L-04 原样保留。L-01 的 proposal 与 `docs/bug-loop-的定位-修复-复核/lessons.md` L-04 逐字相同。

不写成教训、只记录的事实：
- `docs/不是-bug-改建成需求-autome/lessons.md` L-08 认为「rebase 冲突修复后再派复盘轮」是浪费。本任务的复盘 #3 正是这样派出来的（`brief/retro-3.md`），但 L-06、L-07 恰恰来自冲突修复这一轮，所以本轮不能证明再派一次复盘没有产出，不附议这一条。
- 设计文档第 71 行「`bell-system.test.ts` 自己注入 `targets`，不读默认值，不受影响（已核对）」仍然和第 311 行矛盾。本轮不改设计文档。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #3 和实现 #4 的 listen 探针都报 EPERM，M-03 验收命令两次都是 21 条里 18 条通过、3 条 live 用例 skipped。实现 #3 交待审时，测试文件里有两处测试自身的缺陷：beforeAll 里调用了 expect.poll，第 3 条重复执行了 project.add。beforeAll 卡在监听这一步，所以实现轮看不到这两处缺陷。到审计 #3 它们才暴露，M-03 因此 reopen 1。实现 #4 修复前的红灯和修复后的绿灯都没跑出来；main.ts 不调 logtail.close 的负向对照也只得到「环境阻塞，非有效负向证据」。三项负向对照最后全部由审计 #4 完成'
  root_cause: '实现轮会话的沙箱不允许监听回环端口，审计轮的环境允许。需要真实 /ws 的验收在实现轮从来没有执行到断言这一步，实现轮只能交一份没跑过的测试文件'
  evidence: 'docs/w8-实时会话日志/evidence/M-03-r4-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '审计 #3 在可以监听的环境里跑 M-03 验收，退出码 1，报错 `Error: expect.poll() must be called inside a test`（test/logtail-live.test.ts:66，位于 beforeAll 里；第 34、77 行同类）。审计把这三处换成手写循环后，第 3 条又失败了：第二次 startTask() 对同一个仓库重复执行 project.add，报「该目录已经是一个项目」。产品行为在真实内核上符合设计，失败全部来自测试的准备代码'
  root_cause: '设计 M-03 第 2 条写的是「用 expect.poll 调 scheduler.tick 推进收割」。实现把调用了 expect.poll 的 startTask 放进了 beforeAll，又把注册项目和建任务写进同一个辅助函数。Vitest 的 expect.poll 只能在测试体里用。这段准备代码在实现轮从来没执行过（见 L-01），只能等审计来发现'
  evidence: 'docs/w8-实时会话日志/evidence/M-03-r3-audit.md'
  level: rule
  proposal: 'vitest 测试文件中，beforeAll/afterAll 以及它们调用的辅助函数里不出现 `expect.poll` 或 `expect(`，等待一律用 `vi.waitFor`'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '设计原稿只处理了 bell-view.test.ts 第 62 行的路由一致性守卫，评审 #1 D1-P03 用探针实测补出了第 28 行。裁决 #1 和设计第 71 行随后写下「bell-system.test.ts 自己注入 targets，不读默认值，不受影响（已核对）」。合并前全量门禁里，bell-system「列表缺失、刷新失败、空正文均降级；无任务不跳转，项目路由可解析」失败了：同一用例第 83 行 `showSystemNotification(notifier, true, message, [item])` 没传 targets，走的是默认参数 `targets = TARGETS`，却仍断言 `href: null`。实现 #7 把它改成 `/tasks/T-3`'
  root_cause: '设计、评审、裁决三轮都靠读代码判断改一个导出常量会波及哪些测试。默认参数让调用点不出现这个符号名；裁决又按文件下结论（「它自己注入」），漏掉了同一文件里不传参的那次调用。`git grep -n TARGETS -- apps/web/test` 只命中 bell-view 和 task-page，所以 grep 的做法同样会漏。评审 #1 唯一抓准的那一处，是把改动临时施加后真跑出来的'
  evidence: 'docs/w8-实时会话日志/evidence/M-05-r7-impl.md'
  level: brief
  proposal: '设计或裁决写「某已有测试不受改动影响」时，必须附上把该改动临时施加后运行所在包全量测试得到的失败用例清单；只凭阅读代码写的「已核对」不得作为依据'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-04
  domain: design
  symptom: '设计原稿让日志模型从头裁掉超过 N=256 KiB 的内容，把「历史会话展开能看到完整内容」降成了「不超过 256 KiB 时完整」。理由是本机历史样本最大只有 34,832 字节，读取更早内容的能力被放进 Backlog B-01。评审 #1 D1-P01 给出反例：`HEADER\n` 加 262,144 字节正文，展开后永远看不到 HEADER，违反 A-04。A-04 本身没有大小豁免'
  root_cause: '设计用本机样本的大小代替了任务的输入范围，又把 DOM 渲染上限同时当成了内容保留上限，等于默默收窄了验收条款。任务文件里没有任何条款允许这种收窄'
  evidence: 'docs/w8-实时会话日志/w8-实时会话日志-review.md'
  level: brief
  proposal: '设计对任务要求或验收标准加大小、数量或场景上的限定（如「不超过 N 时」）时，必须引用任务文件或任务卡中允许该限定的原文；引用不出原文的，不得加这种限定，也不得把满足原要求所需的能力放进 Backlog'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-05
  domain: verification
  symptom: 'M-05 新增的任务页文案「读取任务失败」含状态标签「失败」，日志组件文案「已暂停滚动」含「已暂停」。审计 #6 只跑了 M-05 验收的 4 个文件、29 个用例和 lint、typecheck、build，就关闭了 M-05。直到合并前全量门禁跑 Web 全量，labels.test.ts「7 views do not duplicate status, column or stage Chinese literals」才拦下这两处文案，同一次门禁还拦下了 bell-system（见 L-03）。实现 #6、#7、#8 的证据都写明没有跑 Web 全量。实现 #8 把分支 rebase 到带着 W6 新视图的 main 之后，也只跑了 8 个文件、64 个用例'
  root_cause: 'labels.test.ts 是扫描路由源码的全局守卫，新增任何视图文件都会落进它的检查范围，但 M-05 的验收命令不包含它。设计验收约定写「全量回归由合并前门禁负责」，任务文件 §4 第 11 条又要求「不要每轮都跑全量」，于是所有全量运行都推给了所有里程碑关闭之后的宿主门禁。这两处确定性失败和监听无关，实现轮环境里跑一次 Web 全量就会变红，结果多花了实现 #7、审计 #7 和一次全量门禁。W6 任务在同一个守卫上以同样的方式出过同样的事'
  evidence: 'docs/w8-实时会话日志/evidence/M-05-r6-impl.md'
  level: brief
  proposal: 'apps/web 任务的实现轮报待审之前跑一次 cd apps/web && npx vitest run 全量，证据逐条列出失败用例并标明是否为 listen EPERM，有非 EPERM 失败就不报待审'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: '审计 #6 关闭全部里程碑以后，实现 #8 按用户追加要求 rebase 到 main 7f29e3dd，在 4 个文件里手工合并了冲突：bell-targets.ts 任务目标和项目目标都改为开放；routes.ts 三条路由并存；app.css 两套样式并存；bell-view.test.ts 默认断言改成任务和项目都有链接，「两类目标都关闭」的断言改用显式传入的 targets。其中 bell-targets.ts 的冲突改变了 M-05 已关闭时的行为（原设计是「项目目标仍保持未开放」）。实现 #8 写「保留已关闭状态」，没有把 M-05 标成待审。审计 #8 写「无待审里程碑，无可复验对象」，没有证据文件，也没有复验这次冲突合并'
  root_cause: '协议只有「实现轮标待审、审计轮复验待审」这一条路径。任务完成之后再改产品代码（这里是解决 rebase 冲突），没有对应的里程碑状态可标。实现轮自己宣布保留关闭，审计轮看到表里没有待审项，就把空转当成了结论。结果是合并进 main 之前最后一次改动产品行为，从头到尾没有经过对手复验。这与「宿主门禁按机器排队」任务实现 #7 和审计 #7 的经过一致'
  evidence: 'docs/w8-实时会话日志/evidence/M-05-r8-impl.md'
  level: protocol
  proposal: '实现轮在已关闭的里程碑之后改动了产品源码（含解决 rebase 冲突）时，必须把受影响的里程碑重新标为 `待审`，并在证据里列出改动的文件；紧接着的审计轮必须对这些文件写出证据文件，不得以「无待审里程碑」结束'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-07
  domain: design
  symptom: '任务文件 §5 写明 W6 与 W8 并行，并点名 `app/routes.ts`、`server/main.ts`、`app/load-context.ts` 是两边都可能碰的文件。设计风险表的「与并行的 W6 冲突」一行只列了 `routes.ts`、`main.ts`。实际 rebase 到 W6 合并后的 main 时冲突的是 `bell-targets.ts`、`routes.ts`、`app.css`、`bell-view.test.ts` 四个文件，其中三个不在风险行里。bell-targets.ts 两边改的是同一行，而且方向相反：W8 开放任务目标并保留「项目目标未开放」，W6 开放项目目标并保留「任务目标未开放」。设计 M-05 和评审 #1 写下的「项目目标仍保持未开放」的断言，合并后不再成立'
  root_cause: '设计自己已经推出「加路由会触发铃铛守卫，必须同时改 bell-targets.ts 和 bell-view.test.ts」（风险表「加路由触发铃铛守卫」一行），M-05 的「结果」一栏也列了要修改 routes.ts、bell-targets.ts、bell-view.test.ts、app.css。可是写并行冲突风险时，设计照抄了任务文件给的文件名，没有拿这份已有文件的修改清单去逐个对照 W6 会不会也改。W6 同样要加路由（projects/:id），同一个推理当时就能预见这几处冲突，以及冲突背后的语义对立'
  evidence: 'docs/w8-实时会话日志/evidence/M-05-r8-impl.md'
  level: brief
  proposal: '任务文件声明有并行任务时，设计的并行冲突风险行必须逐个列出本设计各里程碑「结果」中将修改的每个已有文件，并逐个注明并行任务是否也会修改它及依据；合并前 rebase 的冲突文件中出现未列入该行的已有文件，即判未照做'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```

提交说明：本轮直接提交；若提交被拒绝，提交由 Autome 兜底。
