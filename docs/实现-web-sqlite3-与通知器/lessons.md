# 教训 · Web 通知（W4）：web.sqlite3 面板库、通知器与铃铛

复盘轮 #3 覆盖写，覆盖整次运行（到审计 #10 为止）。依据：Autome 记录的指标、任务文件、`retro.md`、`evidence/` 下 19 份证据、审计与裁决记录、设计文档状态块与 Backlog/D 项，以及 `git log`（commit 号都可在本分支核对）。

指标（Autome 记录）：设计 2/15 轮，实现 10/35 轮，里程碑 7 个，reopen 合计 0，实现缺陷 2，验证缺口 2，协议失败 1，关闭后被推翻 0，人工验收未确认 7（H-01..H-07），总 tokens 41,394,000，turns 230，费用 11.7057 USD。设计文档状态块写的是 `implementation-round: 11/35`（实现 #11 自填），比 Autome 记录多 1；本文以 Autome 的数字为准，轮次编号沿用 retro.md。

里程碑本身只用了 7 个实现轮（M-01..M-07 各一个），其中 M-06/M-07 两对轮次没改产品代码（L-04）。实现 #4 修的是 M-03 的失败夹具（L-02）。M-07 关闭后，宿主门禁在三个**只改了 docs/** 的提交上各失败一次，各派了一个实现轮：ab8675e 上 End to end 失败，派实现 #9，零修复；115e351 上 notifier-real 超时，派实现 #10，真修复（L-05）；11cd3af 上 End to end 又失败，派实现 #11，零修复。之后门禁在 2773a92 上通过，而它与 11cd3af 的产品代码完全相同（L-06）。协议失败那 1 次，本任务文件里定位不到是哪一轮，所以不为它写教训。

```yaml
- id: L-01
  domain: tooling
  symptom: '实现 #1 至 #11 的 Web 全量都被 `listen EPERM 127.0.0.1` 与 watcher EMFILE 阻断（M-04 起每轮 11 失败、19 未执行，涉及 gateway-http、eventbus-live、server、notifier-live；实现 #9、#10、#11 的完整门禁依旧如此）；实现 #3 新写的 notifier-live 3 个用例与「安全路由前移」负向对照在实现轮一次都没跑到。审计轮写明「本轮能监听 127.0.0.1」，同样的命令全部通过、0 跳过'
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
  symptom: 'M-06（B-01）与 M-07（B-04）按「实现随 M-04 / M-02，本里程碑独立核对，不重复实现」立成独立里程碑，用掉实现 #7、#8 与审计 #6、#7 两对轮次，产品代码 0 改动。审计 #7 的两项负向对照（补齐前提前更新 states、失败时 save(event.seq)）与审计 #2 关闭 M-02 时做的相同；审计 #6 的两项（onMessage 只放 default 分支、publishLiveMessage 去掉 try/catch）与审计 #4 关闭 M-04 时做的相同。任务文件写明「计划 §7.4 止损：设计文档的里程碑不超过 6 个」，裁决 #1 还核对过「里程碑数量仍为 5，未超过计划 §7.4 的 6 个」，而设计 #2 加了这两个之后是 7 个，此后没有任何一轮再核对这个上限'
  root_cause: '设计 #2 把用户纳入的两条 Backlog 各立成一个里程碑，但它们的实现与验收已经写进 M-02、M-04 的验收契约；新里程碑契约里没有一项是前一次审计没跑过的，这两对轮次只是重复复验。只有改设计的那一轮核对过里程碑上限，后续修订没有再查。与 用户按合并时如果默认分支已经前进-任务自动 L-02（用户纳入后单开 M-06 一整对轮次）是同一形态'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-07-r8-audit.md'
  level: brief
  proposal: '设计轮把用户纳入的 Backlog 条目立为独立里程碑时，若其实现由另一个里程碑交付，它的验收契约必须至少有一项命令或负向对照不在那个里程碑的契约里；做不到、或里程碑表行数因此超过任务文件写明的上限，就并入那个里程碑的验收契约，不单立里程碑'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: '宿主门禁在 115e351（只改 docs/）上报 notifier-real 两例失败，于是派了实现 #10。实现 #10 原样复现：第一例在 90232ms 时被 execFile 的 90000ms 超时杀掉，通知库为空，第二例 `toHaveLength(14)` 连带拿到 `[]`，afterAll 再报 ENOTEMPTY。实现 #10 在同一会话里并行跑着整条门禁的 Rust 阶段，在这种负载下不改动地测 w4-ledger.sh，用了约 139 秒；把整套夹具的上限改为 300000ms 后，同一条命令 2/2 通过'
  root_cause: 'M-02 定 90 秒上限时没有记录夹具实测耗时（M-02-r2-impl.md 里没有任何耗时数字），这个上限只有机器空闲时才够。宿主门禁在 2428d80、58c237a、93bc1412a5 上 notifier-real 都过了，所以这个问题拖到后来，才在一个与代码无关的提交上暴露'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-02-r10-impl.md'
  level: test
  proposal: '调用真实 autome/automed 进程的测试，实现证据必须写出该夹具在 `sh scripts/ci-local.sh` 全程运行期间实测的耗时秒数，且测试里的超时上限不小于该实测值的 2 倍'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: process
  symptom: '宿主门禁在 ab8675e 与 11cd3af（都只加了复盘轮的 lessons.md）上两次报 End to end 失败，各派了一个实现轮（#9、#11）。两轮都写明宿主只给了 End to end 步骤的 FAILED 汇总、没有失败用例与断言，本地 End to end 都是 80/80，复现不了，零修复结束；它们复跑门禁时撞上的失败都在 crates/（B-05 的 end_to_end.rs 30 秒整体截止、B-06 的 gate.rs wrapper 回收时序），而任务 §4.1 禁止修改 crates/。实现 #11 之后，门禁在 2773a92 上通过，这个提交与 11cd3af 的产品代码完全相同（门禁此前在 fbbca2a 上也通过过）。对照实现 #10：宿主给了用例行号与断言，在可改范围内，一轮就红→绿'
  root_cause: 'Autome 只凭「门禁失败」就派实现轮，既没附失败签名，也没核对失败是否落在任务可改范围内。同一份代码重跑一次门禁就会通过，说明这类失败是 crates/ 测试的时序偶发；实现轮按任务文件只能写 Backlog，派它只是花一整轮把同样的不确定性再记一遍'
  evidence: 'docs/实现-web-sqlite3-与通知器/evidence/M-07-r11-impl.md'
  level: brief
  proposal: '宿主门禁失败时，若拿不到失败用例全名与断言原文，或失败用例位于任务文件禁止修改的路径（本任务为 crates/），Autome 先在同一提交上重跑一次门禁、不派实现轮；只有重跑仍以同一可改范围内的用例失败时才派实现轮，并在简报里附该用例全名与断言原文'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
