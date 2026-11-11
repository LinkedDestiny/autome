# 教训 · Web 通知（W4）：web.sqlite3 面板库、通知器与铃铛

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 15 份证据、审计与裁决记录、评审 #2 记录、设计文档状态块与里程碑表。Autome 本任务没有记录到指标，下文不引用任何指标数值；文中的数字都取自证据文件、设计文档或 `git log`。

事实概要：设计 2 轮（第 1 版定稿后被用户在卡点驳回），实现 8 轮，M-01..M-07 全部一次审计关闭、reopen 均为 0；M-03 多用了一个实现轮（实现 #4），起因是宿主门禁失败而不是审计退回。可带走的四条：实现轮不能监听回环、审计轮能，这是第四个撞上的 Web 任务（L-01，按前两个任务原句复用以便跨任务计数）；真实内核失败夹具没关重试，只有监听才跑得到的断言把这个错漏到了宿主门禁（L-02）；裁决轮为满足范围约束加了两套未经评审的新机制，用户在定稿卡点把两套都换成放宽范围的简单方案（L-03）；用户纳入的两条 Backlog 被立为独立里程碑，两对实现/审计轮重跑了已关闭里程碑审计做过的同一组负向对照（L-04）。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1 至 #8 的 Web 全量验收全部被 `listen EPERM 127.0.0.1` 与 watcher EMFILE 阻断（M-04 起每轮 11 个失败、19 个跳过，涉及 gateway-http、eventbus-live、server、notifier-live 四个文件）；实现 #3 新写的 notifier-live 3 个用例与「安全路由前移」负向对照在实现轮一次都没跑到。审计 #3 起写明「本轮能监听 127.0.0.1」，同样的命令全部通过、0 跳过'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮与宿主门禁允许。这是继 在-apps-web-建-react L-01、apps-web-的服务端托管-automed-2 L-02、服务端每次-tick-后用-events L-04 之后第四个出现同一问题的任务；设计「风险与验证安排」第一条已预先写明「实现轮不能监听 127.0.0.1（W1–W3 都撞上）」，但预先写明只能让实现轮把阻断写清楚，不能让它跑到这些断言'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '实现 #3 交 M-03 待审后，宿主门禁在 9fb3d7d 上报 Web 277 项中 3 项失败，全是 notifier-live；Autome 因此派出实现 #4。实现 #4 用真实 autome 与退出 3 的假 CLI 跑同一夹具，账本出现 session.retry（attempt=1、of=2），任务停在 active，没有走到 failed，而测试只等 15 秒'
  root_cause: '夹具按「假 CLI 退出 3 → 任务立即 failed」的设想写成，没有对照内核真实默认配置（crates/autome-domain/src/config.rs 默认重试 2 次、首次等待 60 秒）；同仓库的 w4-ledger.sh 已显式关掉重试，notifier-live 的 configureFailureRepo 没有。断言终态的那几行只在监听套件里，实现轮因 L-01 跑不到，错误直到宿主门禁才暴露'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-03-r4-impl.md'
  level: test
  proposal: '驱动真实内核让任务进入 failed 的测试夹具必须显式写 retry attempts=0 与 backoff_seconds=0，并且有一个不需要监听的用例（如 notifier-live-fixture.test.ts）断言账本里 session.retry 为 0、failed 恰为 1'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '裁决 #1 采纳 D1-P01、D1-P03 时新增了旁听 socket（tapSocket）和 Web Locks 可见性锁 + 10 分钟认领记录两套机制，裁决记录自己写明「本轮没有再经评审」；用户在定稿卡点驳回第 1 版，设计 #2 把两套都换掉：范围放宽到 app/lib/live-client.ts 加 onMessage 回调，系统通知只看 document.hidden + 权限、靠 tag 合并，并接受「多一条横幅、不漏发」的取舍。为此多跑了设计 #2、评审 #2、裁决 #2'
  root_cause: '评审指出的两处阻塞问题，裁决轮都在「不改 app/lib/」「不弹重复」的原约束内用加机制的方式解决，没有把「放宽范围」或「接受取舍」作为备选交给用户选；用户真正想要的恰好是被排除的那种简单方案'
  evidence: 'docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-adjudication.md'
  level: brief
  proposal: '裁决轮采纳的修订若引入设计中原本没有的新机制（新的连接旁听、跨标签页锁或协调、按时间清理的认领记录等），必须在设计文档「## 争议项」写一条，并列出「放宽范围或接受取舍」的简化备选及其代价，由用户在定稿卡点选择'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: 'M-06（B-01）与 M-07（B-04）按「实现随 M-04 / M-02，本里程碑独立核对，不重复实现」立为独立里程碑，花掉实现 #7、#8 与审计 #6、#7 两对轮次，产品代码 0 改动。审计 #7 的两项负向对照（补齐前提前更新 states、失败时 save(event.seq)）与审计 #2 关闭 M-02 时做的两项相同；审计 #6 的两项（onMessage 只放 default 分支、publishLiveMessage 去掉 try/catch）与审计 #4 关闭 M-04 时做的两项相同'
  root_cause: '设计 #2 把用户纳入的两条 Backlog 各立成一个里程碑，但它们的实现与验收都已写进 M-02、M-04 的验收契约；新里程碑的契约没有任何一项是前一个审计没跑过的，所以这两对轮次只是重复复验。与 用户按合并时如果默认分支已经前进-任务自动 L-02（用户纳入后单开 M-06 一整对轮次）是同一形态'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-07-r8-audit.md'
  level: brief
  proposal: '设计轮把用户纳入的 Backlog 条目立为独立里程碑时，若其实现由另一个里程碑交付，该里程碑的验收契约必须至少有一项命令或负向对照不在那个里程碑的验收契约里；做不到时，在定稿卡点的「## 争议项」写明它将额外占用一个实现轮和一个审计轮，由用户确认'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
