# T-70 复盘教训（复盘 #3 覆盖写）

本任务没有记录到 Autome 指标，下面不引用任何数字指标，只引用证据文件和 git 记录里写明的事实。

运行概况：设计 1 轮；实现 8 轮；审计 7 轮；5 个里程碑都是一次审计关闭，reopen 0。

复盘 #3 相对复盘 #2 的变化：复盘 #2 之后，宿主合并前门禁在提交 `2b7669a4` 上失败了。Rust end_to_end 174 例里挂 1 例，Web 535 例里挂 2 例，
摘录见 `evidence/M-04-r8-impl.md`。实现 #8 只改了测试基线和替身（提交 `8697b21f`），审计 #7 复验了那一例。这一段写成新条目 L-07，
L-01..L-06 原样保留。

复盘 #1 之后又跑了两轮：
实现 #7 按用户追加的指令 rebase 到 main `f5a963ad`，在 scheduler.rs、end_to_end.rs、task.rs、dispatch.rs 里手工解决了冲突，
还改了 bug_loop.rs 的一条共享尾段测试、scheduler.rs 的一个旧字段，并就地改写了设计 D4/D8/验证安排；审计 #6 写「无待审里程碑」，
没有写证据文件就结束了。

与复盘 #1 相比：

- L-01 改写。实现 #7 已把设计「验证安排」改成「fmt/clippy 由宿主轮后快检」，和 main 上 T-67 每轮快检的做法一致。
  复盘 #1 的 proposal 要求实现轮证据写 clippy 退出码，和改后的设计、实现轮提示词都冲突了，所以作废。
  能带走的是冲突本身：设计给实现轮安排了提示词禁止它跑的命令。
- L-02..L-04 原样保留。
- L-05 是新条目。proposal 逐字照抄 `docs/w8-实时会话日志/lessons.md` 的 L-06，那条也已经记下「宿主门禁按机器排队」任务里的同一经过，便于跨任务计数。
- L-06 是新条目。

```yaml
- id: L-01
  domain: design
  symptom: 设计「验证安排」与任务验收 8 原本要求每个实现轮跑 cargo fmt --check 与 clippy -D warnings 并把退出码写进证据；实现 #1、#3、#4、#5、#6 的证据都写明按本轮指令没有运行宿主快检，审计 #1..#5 也没有把缺这两条退出码记成验证缺口。M-01 的 clippy::nonminimal_bool 由宿主轮后快检抓到，用实现 #2 修掉。实现 #7 最后把这一条改写成「由宿主在轮后快检」
  root_cause: 设计轮写验证安排时没有对照实现轮提示词里「不要自己跑项目门禁命令」，给实现轮安排了它被禁止跑的命令；之后的轮次都按提示词执行，设计里的这一条一直没人满足，也没人提出来，直到 rebase 轮顺手改掉
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r7-impl.md
  level: brief
  proposal: 设计「验证安排」里分配给实现轮的命令不得包含 cargo fmt --check、cargo clippy、sh scripts/ci-local.sh 这类宿主门禁命令；这些检查一律写成由宿主轮后快检或合并前门禁承担
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-02
  domain: verification
  symptom: 审计 #3 第一次用 sed 注入负向对照，在 macOS 上语法错误、实际没注入，跑出“全绿”；实现 #6 第一次在副本上用 sed 删行，表达式没删中；两次都是事后才发现注入没生效
  root_cause: 负向对照只看测试结果，不先确认缺陷真的注入了，所以“注入失败”和“测试没拦住”看起来一样，都是全绿
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-audit.md
  level: rule
  proposal: 每条负向对照在跑测试之前，证据里要记下注入后的 `git diff --stat`（副本则记 `diff` 行数），且不为空；为空的那次不算对照
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-03
  domain: design
  symptom: 实现 #5 按设计 D7 去测 `task.list`，得到 `UnknownMethod task.list` 报错，才发现 `autome ls` 实际调用的是 `project.get`，只好在实现轮里就地改写设计 D7 和 R4
  root_cause: 设计轮写了一个当前源码里根本没有的 RPC 方法名，又没标成“新增”；评审轮和裁决轮都没有拿它去对 dispatch.rs 的方法表
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-04-r5-impl.md
  level: rule
  proposal: 设计文档里点名的每个 RPC 方法名，要么在当前 `crates/automed/src/dispatch.rs` 里能 grep 到，要么在设计里写明“新增”
  predicted_impact: {metric: impl_defects, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-04
  domain: tooling
  symptom: M-03 的 e2e 修复会话以 exit 141 退出，修复脚本没把文档写完整，表现为回归测试保护或复核字段报错、复盘重跑，调了好几次夹具才定位
  root_cause: 替身模型 CLI 的 stdout 就是事件流，脚本里直接调用的 git rebase/merge 把进度输出写进了 stdout，引发 SIGPIPE
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-impl.md
  level: test
  proposal: 在 crates/automed/tests/end_to_end.rs 里，替身 CLI 脚本调用的每条 git 命令都要把输出重定向开，不写进 stdout
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-05
  domain: process
  symptom: '审计 #5 关闭全部里程碑以后，实现 #7 按用户追加要求 rebase 到 main f5a963ad，在 rebase 的 continue 提交里手工合并了 scheduler.rs、end_to_end.rs、task.rs、dispatch.rs 的冲突，又在收尾提交 59c9f3d4 里把 scheduler.rs 的旧字段 merge_requested 改成 grants_merge_request，改写 bug_loop.rs 的共享尾段测试（让 MergeGate 冲突的 Bug 与需求转换不再要求相同，改为单独断言 start_fix），并就地把设计 D4/D8 的 Merging→Behind 从 Requeued{Rebase} 改成 Active{Rebase}。实现 #7 写「不自行关闭或重开已审计项」，五个里程碑保持已完成；审计 #6 写「无待审里程碑」，表未动、没有证据文件。冲突改动分散在被改写的 continue 提交里，只看最后一个提交的 diff 看不到。这与 w8-实时会话日志任务的 L-06 经过一致'
  root_cause: '协议只有「实现轮标待审、审计轮复验待审」这一条路径。任务完成之后再改产品代码和测试断言（这里是解决 rebase 冲突），没有对应的里程碑状态可标。实现轮自己宣布保留关闭，审计轮看到表里没有待审项，就把空转当成了结论。合并进 main 之前最后一次改动产品行为、测试断言和设计条款，都没有经过对手复验'
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r7-impl.md
  level: protocol
  proposal: '实现轮在已关闭的里程碑之后改动了产品源码（含解决 rebase 冲突）时，必须把受影响的里程碑重新标为 `待审`，并在证据里列出改动的文件；紧接着的审计轮必须对这些文件写出证据文件，不得以「无待审里程碑」结束'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: '任务文件 §7.6 只点名 T-51、T-53、T-44，要求「每个实现轮开头重新核对」；设计 D4 也只写核对 T-51/T-53，实现 #5 证据也只核对了这两项（T-51 已合入、T-53 未命中）。这期间 main 上先后合入了 T-44 费用可见、T-67 每轮快检、T-57 rerun 修复预算和 T-73 合并核心步骤不占会话槽，四个都改了本设计要改的文件。按 author date，T-73（7f31316f，09:37）比 M-01 实现提交（e167a35e，11:19）早约 1 小时 40 分合入，它把 Merging→Behind 改成直接 Active{Rebase}，和设计 D4 表行、D8 的 Requeued{Rebase} 相反。五个里程碑按旧语义审计关闭，一直到实现 #7 rebase 时才暴露：scheduler.rs 的 E0560 编译错误、bug_loop_merge_tail_is_shared_with_the_feature_loop 测试失败、设计 D4/D8 被就地改写'
  root_cause: '主线核对是按任务编号白名单做的，只看任务文件整理时想到的那几个任务，不看 main 上真正改了本设计目标文件的提交。分支基线 ce6ec5f 在任务整理时就落后 main 30 个提交，会话又被要求不要自己 rebase，所以偏差只能靠只读核对发现，而核对的范围选错了'
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r7-impl.md
  level: brief
  proposal: '实现轮开头的主线核对必须运行 `git log --oneline $(git merge-base HEAD main)..main -- <本里程碑「结果」中要修改的已有文件>`，在证据里列出全部命中提交，并逐个注明是否改变本设计某条款的前提；只核对任务文件点名的任务编号不算照做'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-07
  domain: verification
  symptom: '五个里程碑关闭以后，宿主合并前门禁在 2b7669a4 上失败 3 例。第一例：end_to_end 的 bug_loop_task_get_reports_the_fix_round_for_bugs_only 对 task.get 做严格键集合断言，缺 M-04（D7，cf1c06af）新加的 merge。这条测试 214a5dc4 就有，早在分支起点 ce6ec5f 之前。另两例在 apps/web/test/notifier-backfill-real.test.ts：共享替身 dashboard-double.ts 缺 D2/D7 新加的 config_warnings，第一处断言提前退出，下一例的通知基线跟着坏掉。这个测试文件（902c14af）和替身（c8b23997）都不在 ce6ec5f 里，是实现 #7 rebase 到 f5a963ad 时才进来的。实现 #5、审计 #4 只跑了 merge_view_ 前缀（6 例），实现 #7 也只跑了 merge_、cost_rollup_ 等按名过滤的子集，没有一轮不加名称过滤地运行引用 task.get 或 dashboard.get 的测试文件。为修这个多出实现 #8、审计 #7、复盘 #3 和一次合并前门禁重跑。设计 R5 事先写到了这个风险，处理办法是「合并前门禁会跑它们」'
  root_cause: '验收命令按里程碑前缀过滤，同一个 RPC 已有的严格键断言和替身不在过滤结果里。设计 R5 把发现这类测试变红的时机交给了合并前门禁，没有交给实现轮。rebase 会带进新的同接口消费者测试，rebase 之后的那一轮同样只跑了按名过滤的子集。所以返回字段一变，这类断言注定要等到合并前门禁才红'
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-04-r8-impl.md
  level: rule
  proposal: '改动某个 RPC 返回字段集合的里程碑，以及其后每一个做过 rebase 的实现轮，都要运行 `git grep -l -e <方法名> -- crates/automed/tests apps/web/test apps/desktop/test`（本任务为 task.get、dashboard.get、project.get）。命中的每个测试文件都要不加名称过滤地运行一次，证据里写出命中列表和每个文件的用例总数；少一个文件就不算照做'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 5}
```

L-07 与 `docs/通知器首次启动补发当前等你处理的任务/lessons.md` 的 L-05 是同一个现象：拦下问题的是同一个替身 `dashboard-double.ts`，
是同一处严格键断言，也都是到合并前门禁才红。不同的是责任方：那边是替身所在的任务没跟上 main 新加的字段；这边是改返回形状的任务没去找
已有的消费者测试。两条 proposal 管的是两方各自的动作，所以没有合并成一条。

提交说明：本轮直接提交；若提交被拒绝，提交由 Autome 兜底。
