# W5 复盘教训（复盘 #3，覆盖复盘 #2）

Autome 没有为本任务记录指标，下面不引用任何指标数值。所有事实都来自 `retro.md`、`evidence/`、`brief/`、设计文档和 git 提交。

运行概况（可核对）：设计 1 轮，实现 11 轮，审计 8 轮。M-01..M-05 都是第一次审计就通过，reopen 0。
多出的实现轮有两段：
- 实现 #4：M-03 的宿主门禁复现轮。
- 实现 #7–#11：全部里程碑在审计 #6 关闭以后，宿主门禁又失败了 5 次，每次补一轮实现。审计 #7 在 `5b394e1` 上确认门禁全绿。复盘 #2 的提交 `b44b444` 只改了 lessons.md 和 retro.md，门禁却在 `crates/automed/src/gate.rs` 的用例上失败，于是有了实现 #11。实现 #11 只提交文档（`9ff58fe`），门禁全绿，由审计 #8 记录。

复盘 #2 的 L-01..L-06 仍然成立。L-01、L-02、L-05 补上了实现 #11 的新证据，L-07 是本次新增。

```yaml
- id: L-01
  domain: tooling
  symptom: "实现 #1–#11 每轮都报 `listen EPERM 127.0.0.1`，另有 watcher EMFILE，实现 #6–#11 还有 Electron 启动 SIGABRT。Web 和 Desktop 全量在实现轮从来没跑绿过。可每轮照样跑 `sh scripts/ci-local.sh`：实现 #4 用了 706 秒，#5 用了 828 秒，#7 790 秒，#8 678 秒，#9 517 秒，#10 跑了两次（737 秒和 359 秒），#11 674 秒。每次结果都一样：Web 12 条失败、19 条未执行，Desktop 少 60 条。审计都在能监听的环境里复验全绿"
  root_cause: "实现会话的沙箱不允许监听回环端口，也启动不了 Electron，和项目代码无关。实现 #4 用一个不加载项目源码的 listen 探针复现了 EPERM，实现 #9 单独跑 Electron 也拿到了 SIGABRT。但是简报和规则都没说「探针失败就别跑全量」，所以每轮仍按能监听的前提去跑全量，再花篇幅解释失败原因"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r11-impl.md"
  level: brief
  proposal: "实现轮跑 Web 全量或 scripts/ci-local.sh 之前，先跑 listen 探针，并在证据里引用探针日志。探针报 EPERM 时，本轮 scripts/ci-local.sh 至多跑一次"
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: process
  symptom: "因宿主门禁失败安排的实现轮里，简报都没有失败信息。brief/impl-7.md 到 impl-11.md 两两比较，只有轮号不同，「本里程碑」一节写的是「所有里程碑都已完成」，「上一轮对手轮的结论」写的是「没有对应的里程碑」。实现 #8 只能自己去只读查询本机账本里的 `gate.result`，才找到失败 SHA 的日志路径。实现 #11 也是自己找到 `.autome/output/gates/T-35/merge_gate-b44b4449a404.log`，才知道失败的是 `gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files`。之前实现 #4 拿到的也只有 `FAILED Test` 汇总，最后整轮都没复现出来"
  root_cause: "Autome 生成门禁修复轮的简报时，套用的是里程碑轮的模板。失败步骤、失败用例名、断言原文和宿主日志路径都没有写进简报"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r8-impl.md"
  level: brief
  proposal: "Autome 因宿主门禁失败安排实现轮时，简报里必须写出失败提交 SHA、宿主日志路径、失败用例全名，以及该用例的断言或超时原文"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: "设计按 `vitest list` 的静态枚举数写 Web 预期（287/316/329/343/350），`npm test` 报的却是运行时数，多出 14 个参数化展开（如 330/343/357/364）。实现 #1、#2、#3、#5、#6 每轮都要把两种口径重新对一遍。设计还写死 desktop「应保持 104」，实际上 Electron 能启动时是 165，不能启动时是 104 或 105"
  root_cause: "设计写的用例数口径和验收命令实际输出的口径不一致，也没写明这个数依赖哪些环境条件。desktop 的 `electron dom harness` 只有在 Electron 能启动时，才会通过 `t.test` 动态注册子用例（apps/desktop/test/dom.test.js:83）"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-02-r2-impl.md"
  level: rule
  proposal: "设计文档里每个用例总数预期都写验收命令运行时报告的数；如果这个数依赖环境（Electron 能否启动、能否监听端口），要在同一句里写出各环境下的数"
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: verification
  symptom: "审计 #6 报 desktop 165/165，比设计的 104 多 61。审计的解释是「设计里的 104 是静态数，同理」。可实现轮实跑报的都是运行时 104，差额其实来自 Electron 能启动时 dom harness 动态注册的子用例，跟静态和运行时口径无关。实现 #7–#11 后来把这个差额记成「相对 165 差 -60」，这才说清"
  root_cause: "审计照搬了 Web 那 14 个差额的解释，套到 desktop 上，没有核对差额到底来自哪个文件"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r6-audit.md"
  level: rule
  proposal: "审计报告的用例总数和设计预期不一致时，必须写出差额来自哪个测试文件、第几行，并核对差额数目；不能只用类比来解释"
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-05
  domain: process
  symptom: "M-05 的产品提交在审计 #6 时门禁已经通过，之后又补了 5 个实现轮（#7–#11）。#7 和 #8 对应的失败提交 59d2677、6b46563 只改了 docs/ 下的文件，门禁仍然失败（CLI 用例 30 分钟超时；notifier-live 重启用例 30000ms 超时）。#9 对应的 8d0c0ff 只改了 apps/web/test/notifier-live.test.ts，失败的却是 desktop 的 dom harness 60 秒超时。#10 对应的 755af52 只改了 apps/desktop/test，失败的又是 notifier-live。#11 对应的 b44b444 只改了 lessons.md 和 retro.md，失败的是 crates/automed/src/gate.rs:548 的断言，而任务文件明确禁止改 crates/。实现 #11 没改源码，只提交了文档 9ff58fe，审计 #8 就记录到门禁全绿。实现 #7 在 git diff 59d2677 HEAD -- crates scripts apps 结果为空的情况下，还记录到宿主 load average 137.68"
  root_cause: "Autome 遇到门禁红灯就安排一个实现轮。它不区分这次提交引入的失败，和宿主高负载下、或任务范围外代码里与本次改动无关的偶发失败。所以纯文档提交、只改另一个测试的提交都消耗了实现轮，失败在本任务禁止改动的目录里时也是一样"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r11-impl.md"
  level: protocol
  proposal: "宿主门禁失败时，如果失败用例所在的文件和它依赖的产品代码，相对上一次门禁全绿的提交都没有改动，或失败用例位于任务文件禁止修改的目录，Autome 先在同一 SHA 上原样重跑一次门禁，仍失败才安排实现轮"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-06
  domain: verification
  symptom: "实现 #7 只是原样复跑，没复现，也没留下诊断，下一次宿主失败时依旧无从定位。实现 #8 在原用例里给 6 个 await 阶段加了耗时输出（断言和 30000ms 超时都不动）。下一次宿主日志里，读旧通知 5 ms，关服务器 28057 ms。实现 #10 据此把范围缩到关闭阶段，改用 production 模式启动，提交 5b394e1 的门禁就转绿了"
  root_cause: "本地沙箱复现不了宿主超时（listen EPERM、Electron SIGABRT），能拿到失败现场的只有下一次宿主运行。只有在原用例里留下分阶段耗时，那次运行才产出可以定位的数据"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r10-impl.md"
  level: rule
  proposal: "宿主门禁的超时失败在本轮无法本地复现时，本轮提交必须在原失败用例里加上分阶段开始/结束耗时输出（不改断言和超时），并在证据里写明下一次宿主日志该看哪个前缀的行"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-07
  domain: process
  symptom: "本任务的实现轮点名了三个 crates/ 里的偶发失败：实现 #5 的 `bug_regression_guard_checks_each_workspace_member`（end_to_end.rs:5125），实现 #7 的 `bug_regression_guard_survives_retry_and_extend`（end_to_end.rs:5231），实现 #11 的 `gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files`（gate.rs:548）。实现 #11 已经用确定性探针证明了空 PID 发布竞态。任务文件「宿主门禁在基线上可能本来就不是全绿」一条要求，这类范围外失败要在 `## Backlog` 写一条，含用例全名、断言原文和机制。可设计文档的 Backlog 只有 B-01..B-06，三条一条都没有。gate.rs 的结论只写进了 next-action 和证据文件的「交接」段，next-action 下一轮就会被覆盖"
  root_cause: "这条规则只写在任务文件末尾的经验列表里，实现轮简报的四项自审清单里没有它。实现轮就把范围外失败当成本轮证据的一部分来交代，没有转成设计文档里能留下来的 Backlog 条目"
  evidence: "docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r11-impl.md"
  level: brief
  proposal: "实现轮证据里点名任务范围外目录的失败用例时，同一提交必须在设计文档 `## Backlog` 新增一条，写明该用例全名、文件行号和断言原文"
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
