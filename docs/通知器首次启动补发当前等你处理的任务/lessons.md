# 教训 · 通知器首次启动补发当前「等你处理」的任务，已通知状态落库

复盘轮 #2 覆盖写，覆盖整次运行（设计 #1 → 审计 #4）。依据：任务文件、`retro.md`、`evidence/` 下 8 份证据、评审、裁决与审计记录、设计文档状态块，以及本分支的 `git log` / `git reflog`。

本任务 Autome 没有记录指标，下文不引用任何指标数值。可核对的事实只有设计文档状态块里的这些：`design-round: 1/15`、`implementation-round: 4/15`。M-01..M-03 各用一个实现轮加一个审计轮关闭，reopen 都是 0，审计 #1..#3 没有报实现缺陷，也没有报验证缺口。代价出在三处：设计被评审退回一次（R1-P01，见 L-02）；M-03 的实现轮因为沙箱拒绝监听，拿不到全量证据（L-01）；合并前门禁在变基后的树上发现替身漂移，多跑了实现 #4 和审计 #4（L-05，本轮新增）。

**提交号说明**：Autome 在 11-13 05:26 和 05:42 两次把本分支变基到 main 上（见 `git reflog`），复盘 #1 引用的提交号都变了。下文写的是现在分支上的提交号，括号里是变基前的提交号。

跨任务复用：L-01 与 `实现-web-sqlite3-与通知器` 的 L-01 是同一条，`proposal` 一字未改，方便跨任务计数。L-02、L-03、L-04 是复盘 #1 写的，本轮只更新了提交号。L-05 是本轮新增的。它和项目里「测试替身跟错了对象」那类缺陷相邻，但根因不同：那类是替身照着调用方写；这次替身是照着真实接口抄的，抄的是分支起点那一版，而接口在 main 上已经变了。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #3 跑 M-03 验收命令时，notifier-live 的 beforeAll 报 `listen EPERM: operation not permitted 127.0.0.1`。npm test 共 315 条：285 通过、11 失败（gateway-http）、19 跳过（notifier-live 3、eventbus-live 7、server 9）。实现轮因此把 M-03 留在「开放」，没有标待审（902c14a，变基前 f289f10）。审计 #3 写明「本沙箱可监听，实现轮的 EPERM 不复现」，同一组命令跑出定向 9/9、全量 315/315，结论通过'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮的允许。此前已有三个任务因为同一原因留下同一条教训（实现-web-sqlite3-与通知器 L-01 等），本任务是第四个。这次没有多花轮次，是因为 Autome 对一个「开放」的里程碑派出了审计轮（a8fec16，变基前 b30d59d，那份 audit-3 简报里 M-03 的状态就是「开放」）。按协议，实现轮只有拿到通过的证据才标待审；否则下一个实现轮还会落在同样的沙箱里重跑'
  evidence: 'docs/通知器首次启动补发当前等你处理的任务/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: '设计 #1 的 D3 先取游标 L0，再调 dashboard.get，然后把快照里的状态记成 (state, seq=L0)，并从 L0 开始回放。评审 #1 给出最小反例（R1-P01）：两次读取之间，任务从 merge_gate 走到 await_merge（task.rs:1433 允许这一步）；回放时先把状态覆盖成 merge_gate，再把 await_merge 当成一次新的进入，同一次等待被通知 2 次。W1→X→W2 的变体是 3 次。评审用 Node 最小模型加真实 SQLite 唯一索引复现了这个结果。裁决 #1 改成「先取 L0，再取快照，再取 L1，L1 等于 L0 才接受」，再用穷举模型核对：12464 种交错全部与真值一致，旧稿有 5776 种不一致'
  root_cause: '设计轮默认「快照就是 L0 时刻的状态」，没有检查两次 RPC 之间账本可能前进。这个缺陷只有把读取和内核写入的全部交错列举出来才看得见。设计轮的竞态表是手写的六行，没有做列举，所以漏掉了。评审和裁决用的都是最小模型，裁决给出的交错总数和不一致数都可以复核'
  evidence: 'docs/通知器首次启动补发当前等你处理的任务/通知器首次启动补发当前等你处理的任务-review.md'
  level: rule
  proposal: '设计中凡是在两次及以上读取（RPC、快照、游标）之间外部状态可能前进的流程，设计轮必须附一个穷举交错的最小模型脚本，并在设计文档里写出交错总数和与真值不一致的数量（必须为 0），否则不得提交评审'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: '设计 #1 给 M-01 定的第一条负向对照是「播种 INSERT 去掉 WHERE name=notifier 后 S1 失败」。评审 #1 的 R1-N01 指出：新库的 event_cursor 是空的；表里只有 notifier 一条游标时也一样，变异前后结果相同，S1 区分不出来。裁决 #1 采纳，要求 S1 的「无 notifier 游标」分支预置一条名为 other 的游标。之后实现 #1 和审计 #1 各跑了一次这条变异，结果都是 S1 失败 1 条，说明补上这条数据之后对照才真正起作用'
  root_cause: '设计写负向对照时只写了「改哪里、哪条用例失败」，没有核对用例数据能不能让变异和原实现走出不同的结果。这个变异只在特定数据下才和原实现不等价，用例数据恰好没有覆盖到那种情况'
  evidence: 'docs/通知器首次启动补发当前等你处理的任务/通知器首次启动补发当前等你处理的任务-adjudication.md'
  level: rule
  proposal: '验收契约中的每条负向对照都必须写出能让变异与原实现结果不同的那条测试数据（例如「S1 预置 other 游标」）；写不出这条数据的对照不得进入定稿设计'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: '设计把 main 当作验收基线。M-02 的契约是 `git diff main --stat -- apps/web/server apps/web/app` 只列出 store.ts 和 notifier.ts；M-03 的契约是「总数等于 main 基线加 14」，并且用 `git show main:<文件>`。任务进行期间，main 合入了并行任务的 UI 改动，实现 #2 跑 `git diff main --stat` 时列出了不属于本任务的文件，于是在 c8b2399（变基前 df74814）里把 M-02 的契约改成 `git merge-base main HEAD`。实现 #3 写明「当前 main 已前进…不能把移动后的 main 全量数直接加 14」，改用共同祖先核对；审计 #3 也是按「其余文件相对共同祖先未改」得出 301+14'
  root_cause: '本仓库同时跑 3 个并行任务（.autome/config.toml 里 parallel = 3），main 会在任务进行期间前进。拿 main 当基线的契约命令本身就不稳定，实现轮和审计轮只能各自临时换成 merge-base，其中一次还因此改动了验收契约的文字'
  evidence: 'docs/通知器首次启动补发当前等你处理的任务/evidence/M-02-r2-impl.md'
  level: rule
  proposal: '验收契约里用作基线的 git 引用一律写成 `$(git merge-base main HEAD)`，定稿设计的验收契约中不得出现 `git diff main`、`git show main:` 或「main 基线」这类以 main 当前头部为基线的写法'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: '审计 #3 自跑 notifier-backfill-real 4/4、全量 315/315，关闭了 M-03。之后 Autome 把分支变基到 main（reflog 记录了 05:26 和 05:42 两次 rebase），宿主门禁在变基后的树上跑出 notifier-backfill-real 4 条里失败 2 条：真实 dashboard.get 的 waiting 项多了 `gate_queue_position`，被 notifier-backfill-real.test.ts:59 的严格键集合断言拦下（门禁日志 merge_gate-1679ccc4da5c.log）。实现 #4 只给共享替身 dashboard-double.ts 补了 `gate_queue_position: null`（dd712ae），审计 #4 复验相关子集 39/39'
  root_cause: '分支起点是 404b481（11-12 19:39），那一版的 crates/automed/src/dispatch.rs 里没有 gate_queue_position；并行任务 T-47 在 11-13 00:52 把这个字段合进了 main（77750fc，字段来自 b6f326d）。M-02 写替身（c8b2399，05:00）和审计 #3 关闭 M-03（05:23）的时候，这个字段在 main 上已经存在 4 个多小时；但 M-01..M-03 的全部验证都是对着分支起点编译的 automed 跑的，所以全绿。没有哪一轮去查 main 在替身模仿的那个接口上改了什么，漂移一直拖到变基后的合并前门禁才暴露，多花了一个实现轮和一个审计轮。严格键集合断言本身是对的，正是它在门禁里拦下了问题'
  evidence: 'docs/通知器首次启动补发当前等你处理的任务/evidence/M-03-r4-impl.md'
  level: rule
  proposal: '测试替身模仿 crates/ 下 RPC 返回形状的任务，最后一个里程碑的审计轮必须运行 `git diff $(git merge-base main HEAD) main -- <被模仿接口的源文件>`，在审计证据里写出这条命令和输出是否为空；输出不为空时，逐个字段说明替身是否已经同步'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
