# 教训 · Web 通知（W4）：web.sqlite3 面板库、通知器与铃铛

复盘轮 #2 覆盖写，覆盖整次运行。依据：Autome 记录的指标、任务文件、`retro.md`、`evidence/` 下 17 份证据、审计与裁决记录、设计文档状态块与 Backlog/D 项，以及 `git log`（commit 号都可在本分支核对）。

指标（Autome 记录）：设计 2/15 轮，实现 10/35 轮，里程碑 7 个，reopen 合计 0，实现缺陷 2，验证缺口 2，协议失败 1，关闭后被推翻 0，人工验收未确认 7（H-01..H-07），总 tokens 41,394,000，turns 230，费用 11.7057 USD。

十个实现轮里只有 7 个做的是里程碑本身（M-01..M-07 各一个）。多出来的 3 个实现轮和 2 个审计轮都有据可查：实现 #4 修 M-03 的失败夹具（L-02，实现缺陷之一）；M-07 关闭、复盘 #1 结束后，宿主门禁在两个**只改了 docs/** 的提交上先后失败——ab8675e（只写了 lessons.md）上是 Rust End to end 失败，115e351（只写了实现 #9 证据）上是 notifier-real 超时——于是又派了实现 #9、#10 和审计 #8、#9（L-05、L-06）。M-06/M-07 两对轮次没改产品代码（L-04）。协议失败那 1 次，在本任务文件里定位不到是哪一轮：审计 #8 与审计 #9 的简报除了轮次号外逐字相同，两轮都写着「无待审」，但这一点证明不了因果，所以不为它写教训。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1 至 #10 的 Web 全量验收都被 `listen EPERM 127.0.0.1` 与 watcher EMFILE 阻断（M-04 起每轮 11 失败、19 未执行，涉及 gateway-http、eventbus-live、server、notifier-live；实现 #9、#10 的完整门禁依旧如此）；实现 #3 新写的 notifier-live 3 个用例与「安全路由前移」负向对照在实现轮一次都没跑到。审计轮写明「本轮能监听 127.0.0.1」，同样的命令全部通过、0 跳过'
  root_cause: '实现轮会话的沙箱不允许回环监听，审计轮与宿主门禁允许。这是继 在-apps-web-建-react L-01、apps-web-的服务端托管-automed-2 L-02、服务端每次-tick-后用-events L-04 之后第四个出现同一问题的任务；设计「风险与验证安排」已预先写明「实现轮不能监听 127.0.0.1」，但写明只能让实现轮把阻断讲清楚，跑不到这些断言'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-03-r3-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '实现 #3 交 M-03 待审后，宿主门禁在 9fb3d7d 上报 Web 277 项中 3 项失败，全在 notifier-live，于是派了实现 #4。实现 #4 用真实 autome 加退出码 3 的假 CLI 跑同一夹具：账本里出现 session.retry（attempt=1、of=2），任务停在 active、没有走到 failed，而测试只等 15 秒'
  root_cause: '夹具是照「假 CLI 退出 3 → 任务立即 failed」的设想写的，没有对照内核的真实默认配置（crates/autome-domain/src/config.rs 默认重试 2 次、首次等 60 秒）；同仓库的 w4-ledger.sh 已显式关掉重试，notifier-live 的 configureFailureRepo 没有关。断言终态的那几行只在监听套件里，实现轮因 L-01 跑不到，错误直到宿主门禁才暴露'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-03-r4-impl.md'
  level: test
  proposal: '驱动真实内核让任务进入 failed 的测试夹具必须显式写 retry attempts=0 与 backoff_seconds=0，并有一个不需要监听的用例断言账本里 session.retry 为 0、failed 恰为 1'
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '裁决 #1 采纳 D1-P01、D1-P03 时新加了两套机制：旁听 socket（tapSocket），以及 Web Locks 可见性锁加 10 分钟认领记录。裁决记录自己写明「本轮没有再经评审」。用户在定稿卡点驳回第 1 版，设计 #2 把两套都换掉——范围放宽到 app/lib/live-client.ts 加 onMessage 回调；系统通知只看 document.hidden 与权限、靠 tag 合并，接受「多一条横幅、不漏发」。设计轮因此用到 2/15，并多出评审 #2、裁决 #2'
  root_cause: '评审指出的两处阻塞，裁决轮都守着原约束（「不改 app/lib/」「不弹重复」）、靠加机制来解决，没有把「放宽范围」或「接受取舍」作为备选交给用户；用户真正要的恰好是被排除的那种简单方案'
  evidence: 'docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-adjudication.md'
  level: brief
  proposal: '裁决轮采纳的修订若引入设计中原本没有的新机制（新的连接旁听、跨标签页锁或协调、按时间清理的认领记录等），必须在设计文档「## 争议项」写一条，列出「放宽范围或接受取舍」的简化备选及其代价，由用户在定稿卡点选择'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: 'M-06（B-01）与 M-07（B-04）按「实现随 M-04 / M-02，本里程碑独立核对，不重复实现」立成独立里程碑，用掉实现 #7、#8 与审计 #6、#7 两对轮次，产品代码 0 改动。审计 #7 的两项负向对照（补齐前提前更新 states、失败时 save(event.seq)）与审计 #2 关闭 M-02 时做的相同；审计 #6 的两项（onMessage 只放 default 分支、publishLiveMessage 去掉 try/catch）与审计 #4 关闭 M-04 时做的相同'
  root_cause: '设计 #2 把用户纳入的两条 Backlog 各立成一个里程碑，但它们的实现与验收已经写进 M-02、M-04 的验收契约；新里程碑契约里没有一项是前一次审计没跑过的，这两对轮次只是重复复验。与 用户按合并时如果默认分支已经前进-任务自动 L-02（用户纳入后单开 M-06 一整对轮次）是同一形态'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-07-r8-audit.md'
  level: brief
  proposal: '设计轮把用户纳入的 Backlog 条目立为独立里程碑时，若其实现由另一个里程碑交付，它的验收契约必须至少有一项命令或负向对照不在那个里程碑的契约里；做不到就在定稿卡点的「## 争议项」写明它会多占一个实现轮和一个审计轮，由用户确认'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: '宿主门禁在 115e351（只改 docs/）上报 notifier-real 两例失败，于是派了实现 #10。实现 #10 原样复现：第一例在 90232ms 时被 execFile 的 90000ms 超时杀掉，通知库为空，第二例 `toHaveLength(14)` 连带拿到 `[]`，afterAll 再报 ENOTEMPTY。在负载下未改动地测 w4-ledger.sh，用了约 139 秒；把整套夹具的上限改为 300000ms 后，同一条命令 2/2 通过'
  root_cause: 'M-02 定 90 秒上限时没有记录夹具实测耗时（M-02-r2-impl.md 里没有任何耗时数字），只在机器空闲时能过。宿主上的门禁并不是独占运行：scripts/ci-local.sh 头注释写明 .githooks/post-commit 每次提交都在后台跑一遍，实现 #10 的 139 秒就是与另一遍门禁的 Rust 阶段并发时测到的。宿主门禁在 2428d80、58c237a、93bc1412a5 上，notifier-real 都过了，所以这个错到后来才在一个与代码无关的提交上暴露'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-02-r10-impl.md'
  level: test
  proposal: '调用真实 autome/automed 进程的测试，实现证据必须写出该夹具在 `sh scripts/ci-local.sh` 全程运行期间实测的耗时秒数，且测试里的超时上限不小于该实测值的 2 倍'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: '宿主门禁在 ab8675e（只加了复盘 #1 的 lessons.md）上报 End to end 失败，于是派了实现 #9。实现 #9 写明「宿主给出的日志仅包含 End to end 的 FAILED 汇总，没有失败用例、断言或 panic 上下文」，本地两遍 End to end 都是 80/80，复现不了，零修复结束。对照实现 #10：它拿到了宿主给的第二例行号和三段错误结构，一轮就红→绿。后来实现 #10 的门禁里出现的 Rust 截止失败（B-05，30 秒整体截止、停在 MergeGate）也说明这类 End to end 失败与本任务改动无关'
  root_cause: '宿主门禁失败转给实现轮时只带了汇总尾部。失败签名缺失时，实现轮分不清是本任务的缺陷、偶发时序，还是范围外的 crates/ 测试（本任务禁止修改 crates/），只能整条重跑门禁去猜'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-07-r9-impl.md'
  level: brief
  proposal: 'Autome 把宿主门禁失败交给实现轮时，简报必须附失败用例全名与断言或 panic 原文；拿不到这些时，先在同一提交上重跑一次门禁，不派实现轮'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
