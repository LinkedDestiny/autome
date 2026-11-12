# 教训 · 网关放行任务卡的创建与开始（T-37）

复盘轮 #1。Autome 本任务没有记录到指标，下文不引用 tokens、turns、费用等数字。依据：设计文档状态块（`status: 已完成`，`design-round: 2/15`，`implementation-round: 6/20`）、`retro.md`、`evidence/` 下 10 份证据、审计与裁决记录、`brief/impl-3.md` 与 `brief/impl-6.md`、git 提交记录。

M-01..M-04 都只经过一次审计就关闭，reopen 都是 0。四次审计结论都是「无实现缺陷、无验证缺口」，各自在可监听的环境里实跑了负向对照，其中包括往真实 `dispatch.rs` 的 `READ_METHODS` 注入一个方法，A-04 由此直接取证。6 个实现轮里有 4 轮各推进一个里程碑（#1、#2、#4、#5）。另外 2 轮（#3、#6）是门禁修复轮，没有改任何代码就结束了。两次触发它们的都是同一个与本任务无关的不稳定用例 `notifier-live.test.ts`「停服期间新增失败，重启只补一条且保留旧已读」（L-02、L-03、L-04）。6 个实现轮全部遇到 `listen EPERM 127.0.0.1`，没有一轮跑到真实 HTTP 断言（L-01）。

跨任务复用：下面 4 条都是前几个任务记过的教训，本任务再次出现，`proposal` 一字未改，方便跨任务计数。L-01 与 `服务端每次-tick-后用-events` L-04、`在-apps-web-建-react` L-01 相同；L-02 与 `实现-web-sqlite3-与通知器` L-06 相同；L-03 与 `服务端每次-tick-后用-events` L-06 等相同；L-04 与 `在-plugins-新增-autome-插件` L-02 等相同。本任务没有新出现、又能写成可检验条目的教训。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1..#6 全部遇到 `listen EPERM: operation not permitted 127.0.0.1`，常伴有文件监听 EMFILE。M-01..M-04 的真实 HTTP 验收用例（A-01/A-02 真实内核、A-03 probe、65536 字节逐字读回、替身 CLI 生命周期）在实现轮一次都没跑到业务断言，所以 4 个里程碑没有一个被实现轮标待审。实现 #5 新写的 gateway-task-lifecycle 两条用例与四项负向对照第一次真正执行是在审计 #4。审计 #1..#4 在另一环境跑同样的命令，全部通过'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮允许，两个角色的执行环境不同。这是继 在-apps-web-建-react、apps-web-的服务端托管-automed-2、服务端每次-tick-后用-events 等任务之后，同一问题再次出现；任务文件没有提这一点，每个实现轮都要自己重新发现一次'
  evidence: 'docs/网关放行任务卡的创建与开始-write/evidence/M-04-r5-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: process
  symptom: '宿主门禁在 ecd9edf（M-02 实现）和 992f546（M-04 实现）上各报 1 个 Web 用例失败，两次都是 apps/web/test/notifier-live.test.ts 第 44 行「停服期间新增失败，重启只补一条且保留旧已读」，于是各派了一个门禁修复轮（实现 #3、#6）。两轮都只能复现出 beforeAll 的监听 EPERM，没有修改任何代码就结束了。两轮之后的提交 4a6392a、64047b4 相对 ecd9edf、992f546 只改了 docs/，宿主门禁都通过了。审计 #4 单跑这条用例两次，一次失败一次通过，并确认 `git diff <merge-base> -- apps crates` 不含该文件及其被测代码，记为 B4-01。6 个实现轮里有 2 个花在这条与本任务无关的不稳定用例上'
  root_cause: 'Autome 只凭「门禁失败」就派实现轮，既没附失败签名，也没核对失败用例是否落在任务可改范围内（任务 §3 的可改清单不含 notifier-live）。同一份产品代码重跑一次门禁就会通过，派出的修复轮只是把同样的不确定性再记一遍'
  evidence: 'docs/网关放行任务卡的创建与开始-write/evidence/M-04-r6-impl.md'
  level: brief
  proposal: '宿主门禁失败时，若拿不到失败用例全名与断言原文，或失败用例位于任务文件禁止修改的路径（本任务为 crates/），Autome 先在同一提交上重跑一次门禁、不派实现轮；只有重跑仍以同一可改范围内的用例失败时才派实现轮，并在简报里附该用例全名与断言原文'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: '实现 #6 证据写「失败片段对应 notifier-live.test.ts 第 44 行……错误正文未包含在片段中，不能据此断定是关闭、CLI 执行或通知补偿超时」，因此「无法在停服、CLI 超时与重启补偿之间作有依据的选择」。brief/impl-3.md 与 brief/impl-6.md 里都没有出现 notifier-live、失败用例名或断言原文，失败信息只通过用户在 prompt 里粘贴的片段传到实现轮。断言原文（30s 超时）直到审计 #4 自己复跑才拿到'
  root_cause: '宿主门禁日志没有随简报交给门禁修复轮，实现轮沙箱又跑不到监听用例，本地重跑替代不了宿主日志'
  evidence: 'docs/网关放行任务卡的创建与开始-write/evidence/M-04-r6-impl.md'
  level: brief
  proposal: '因宿主门禁失败派出的实现轮，其简报必须包含门禁日志中每个失败用例的全名与断言原文，以及门禁运行所在的提交号'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '两个门禁修复轮都在沙箱内全量跑了 sh scripts/ci-local.sh：实现 #3 用时 817 秒，实现 #6 用时 641 秒，都退出 1。失败项都是 Web test（listen EPERM / EMFILE）和 Desktop（Electron DOM harness 退出码 null），都没跑到宿主上失败的那条断言，两份证据都写「以 Autome 对后续提交的重跑为准」。宿主门禁随后在 4a6392a、64047b4 上都通过了'
  root_cause: '沙箱禁止本机监听，实现轮跑全仓门禁永远不可能变绿，只能重复记录已知的环境失败。真正有效的全仓门禁由 Autome 在提交后在宿主上运行'
  evidence: 'docs/网关放行任务卡的创建与开始-write/evidence/M-02-r3-impl.md'
  level: brief
  proposal: '宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```
