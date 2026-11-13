# 教训：W9 项目管理与环境（复盘 #1）

Autome 没有为本任务记录指标，所以下文不引用任何指标数值。所有事实都来自 `retro.md`、`evidence/`、`brief/`、评审、裁决、审计记录、设计文档和任务文件。

运行概况（可核对）：
- 设计 1/15 轮。评审 #1 提出 2 项问题（R1-P01、R1-P02），裁决 #1 全部采纳，没有复审。
- 实现 8/30 轮，审计 7 轮。M-01..M-06 全部 `已完成`。M-02 reopen 1 次（领域 add-project），其余里程碑都是 0 次。
- 8 个实现轮都在 `listen EPERM` 下结束，没有一轮把里程碑标成 `待审`。7 个审计轮都在允许监听的环境里，从 `开放` 直接复验并关闭里程碑。
- 实现 #6 是快检修复轮：M-04 实现 #5 引入了两处 TS1543，被宿主快检拦下。
- 人工验收 H-01..H-04 还没确认。

跨任务复用：
- L-01 的 proposal 逐字沿用 `docs/w8-实时会话日志/lessons.md` 的 L-01，与 `服务端每次-tick-后用-events`、`通知器首次启动补发当前等你处理的任务` 中的同名条目也一字不差。
- L-03 的 proposal 逐字沿用 `docs/通知器首次启动补发当前等你处理的任务/lessons.md` 的 L-02。
- L-02 是新写的，没有沿用 `docs/w6-项目页与任务卡-按待开始-进行中/lessons.md` 的 L-01。那一条的触发条件是「报待审之前」，本任务没有任何实现轮报过待审，照它做也拦不住这次的问题。

只记录、不写成教训的事实：
- M-06 契约的负向对照写的是「处理函数改用请求体里的 `command` 时，env-ops 第 2 条与 env-real 第 2 条失败」（设计第 531 行）。审计 #7 单独注入这一处，没有任何用例失败：write-gate 先把额外键拒掉了。实现 #8 也没有按契约原文注入，换成了「移除额外键拒绝」。审计判为纵深防御、契约措辞偏宽，不是缺陷（`evidence/M-06-r8-audit.md`）。只出现了一次，也没有多花轮次，所以不单列。
- 审计 #2 的四项负向对照只注入了一项，另外三项写的是「时间所限」（`evidence/M-02-r2-audit.md`）。审计 #3 补做了其中两项。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1..#8 的验收命令全部被 `listen EPERM: operation not permitted 127.0.0.1` 拦住，依次是 M-01 14 条、M-02 4 条（两轮）、M-03 5 条、M-04 6 条、M-05 7 条、M-06 9 条。审计 #2 换到能监听的环境，才发现 w9-http 新用例的 `post` 辅助函数对页面路由返回的纯文本 403 无条件 `JSON.parse`，抛出 SyntaxError，M-02 因此 reopen 1。实现 #3 修复前后跑的都是同一个 EPERM，修复前的红灯和修复后的绿灯都没拿到，只能把 M-02 留在开放、等审计 #3 复验'
  root_cause: '实现轮会话的沙箱不允许监听回环端口，审计轮的环境允许。需要真实 HTTP 服务的用例在实现轮从来没有跑到断言这一步，测试自身的缺陷只能等审计来发现。这和 W8 M-03 reopen 的机制相同'
  evidence: 'docs/w9-项目管理与环境-网页上添加项目-2/evidence/M-02-r2-audit.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: implementation
  symptom: 'M-04 实现 #5 在 settings-config.test.ts、settings-real.test.ts 里静态导入 JSON fixture 时没写 `with { type: ''json'' }`。证据写的是「未执行 ... 前端 lint/typecheck，交给宿主快检」。宿主快检报出两处 TS1543，于是多派了实现 #6 专门修这两行。实现 #6 在同一个实现轮沙箱里原样跑了快检（cargo fmt、clippy、lint、typecheck），退出码 0，说明这几项静态检查不受 EPERM 影响'
  root_cause: '任务文件「前车之鉴」写的是「npm run typecheck 与 npm run lint 是静态检查，不受 EPERM 限制：实现轮报待审之前当场跑」。设计文档第 351 行却改写成「lint、typecheck 由宿主每轮快检负责；实现轮……不重复执行宿主门禁」。实现 #1、#4、#5、#7、#8 的证据都按设计写明没跑。另外，任务文件这条建议的触发条件是「报待审之前」，而本任务所有实现轮都因 EPERM 没有报待审，就算照建议做也不会触发'
  evidence: 'docs/w9-项目管理与环境-网页上添加项目-2/evidence/M-04-r6-impl.md'
  level: brief
  proposal: 'apps/web 任务中，凡是改动了 .ts 或 .tsx 文件的实现轮，结束前都在证据里写出 npm run typecheck --prefix apps/web 与 npm run lint --prefix apps/web 的退出码，不论本轮是否报待审，也不论设计是否写明由宿主快检负责'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '设计原稿 §7 规定：admin 在第一次 tick 只记录 env 的 (generation, probing) 基线，不发布任何消息。评审 #1 R1-P02 用真实内核实测（`.autome/output/review-1-env-first-tick.txt`）：首次 env.get 读到 `(probed=false, probing=true, generation=0)`，之后两次 tick 都是 `(generation=1, probing=false)`，events.since 是 0 条。探测如果在第一次 tick 之前就结束，页面会一直显示「正在检测」。裁决 #1 把规则改成首份有效报告必须发布一次 env.changed，并同步改了 env-ops 第 7 条与 env-real 第 3 条'
  root_cause: 'loader 的 env.get 和 ticker 的 scheduler.tick 是两次独立读取。dispatch.rs 里的后台探测线程会在两次读取之间推进 generation。设计默认第一次 tick 看到的就是页面看到的状态，没有列出「先读 env.get，探测完成，再第一次 tick」这种交错'
  evidence: 'docs/w9-项目管理与环境-网页上添加项目-2/w9-项目管理与环境-网页上添加项目-2-review.md'
  level: rule
  proposal: '设计中凡是在两次及以上读取（RPC、快照、游标）之间外部状态可能前进的流程，设计轮必须附一个穷举交错的最小模型脚本，并在设计文档里写出交错总数和与真值不一致的数量（必须为 0），否则不得提交评审'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

提交说明：本轮直接提交；若提交被拒绝，提交由 Autome 兜底。
