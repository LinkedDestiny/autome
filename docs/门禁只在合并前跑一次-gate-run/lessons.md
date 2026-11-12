# 教训 · 门禁只在合并前跑一次（`[gate] run = "before_merge"`，T-61）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 16 份证据、审计与裁决记录、评审记录、任务分支 `git log`，以及对账本 `~/.autome/state/automed.sqlite3` 的只读查询。Autome 本任务没有记录到指标，下文不引用任何指标数值；文中的数字都取自证据文件、`git` 或账本的实际输出。

设计 3 轮（设计 #1 在定稿卡点被用户驳回），实现 8 轮，M-01..M-08 都是一次审计通过，没有 reopen。表面上收敛得很干净，但有一个没人发现的缺口：本任务从头到尾没有跑过一次宿主门禁，而 16 份实现与审计证据都把全量测试与 clippy 交给了它（L-01）。另外三条：负向对照的注入没生效、差点被当成「拦不住」（L-02）；设计 #1 只按正确性、没按任务目标判断范围外读者，被用户在定稿卡点驳回（L-03）；实现轮改了验收契约的预期（L-04，与 `apps-web-的服务端托管-automed-2` 的 L-01 同一句，按原文复用以便跨任务计数）。

```yaml
- id: L-01
  domain: verification
  symptom: '本任务 8 个实现轮、8 个审计轮全部写「全量测试 / clippy / ci-local 由宿主门禁负责」，M-01 的验收契约甚至写「其余现有用例由宿主门禁在本轮 head 上的结果证明通过」；但账本里 T-61 没有任何 gate.* 事件，节点序列 8 次都是 implement → audit，不经 gate；8 份审计简报都没有「## 宿主门禁」段，审计 #8 注意到了这一点，仍写「全量由宿主在合并前门禁跑」并判通过。任务验收标准第 6 条（clippy -D warnings、sh scripts/ci-local.sh 全绿）在任务目录里没有任何一轮的运行证据'
  root_cause: '用户在任务输入提交 19d1653（22:37:36）之后 36 秒提交 400f9f5，把主检出 .autome/config.toml 的 [gate] 注释掉；Autome 读主检出配置，所以本任务没有宿主门禁。worktree 里的 .autome/config.toml 是分支基线的旧副本，仍写着 command = "sh scripts/ci-local.sh"，任务文件 §3 也写「由 Autome 在实现轮之后自己跑」——会话能看到的两处信息都说门禁存在。简报对「没有门禁」与「有门禁但还没结果」一样什么都不写，审计看不出区别'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-08-r8-audit.md'
  level: brief
  proposal: '项目当前没有生效的 [gate] 时，Autome 在实现轮与审计轮简报里写明「本项目当前没有宿主门禁，全量验证不会由宿主执行」，并且任务验收标准里点名的全量命令（本任务为 clippy -D warnings 与 sh scripts/ci-local.sh）必须在某一轮实现或审计证据里出现并带退出码'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '审计 #5 的第一次负向对照用 BSD sed 和错误的行号注入，改动实际没有落到文件上，被点名的用例仍是绿的；审计核对 git diff --stat 才发现注入没生效，改用 perl 重做后用例才变红'
  root_cause: 'macOS 的 BSD sed 与 GNU sed 的 -i 语义不同，行号又随前序里程碑漂移，sed 静默不改也退出 0；负向对照的结论只看「测试红没红」，不看「错误到底注入进去没有」。如果审计没顺手查 diff，会得出「验收命令拦不住」的错误结论，反向也可能把一次没生效的注入记成已覆盖'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-05-r5-audit.md'
  level: rule
  proposal: '每条负向对照在跑验收命令之前，证据里必须记录注入后 git diff --stat 的输出，且被注入的文件出现在输出中；没有这条记录的对照不得写成「拦得住」或「拦不住」'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '设计 #1 经评审 #1、裁决 #1 定稿后，在定稿卡点被用户驳回；设计 #2 按驳回意见把范围扩到 protocol/prompts（prompt 分段渲染、协议 v3、eval），裁决 #1 为避开 protocol/ 而新加的 gate_note 宿主追加通道在设计 #2 里被删掉，多花了一整轮设计、评审与裁决'
  root_cause: '设计 #1 已经看到复核 prompt「要求会话自己复验项目门禁」，但只按正确性判断（「措辞差异不会让任何一轮做错事」），把它当范围外处理；任务目标是「降低门禁运行次数、避免门禁抢占」，而 audit.md / fix.md / fix-review.md 与 Inject::GateFailed 都让会话自己再跑全量，排除它们会让目标只完成一半。任务卡「不改会话自己跑测试的方式」与目标冲突，设计没有把这个冲突写进争议项交给用户'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run.md'
  level: rule
  proposal: '设计轮发现范围外的文件里有直接执行任务目标所要减少的动作的文字时（本任务为 prompt 要求会话全量跑门禁），必须在 `## 争议项` 写一条，列出纳入与不纳入两种方案及各自对目标的影响，由用户在定稿卡点选择，不得只以「不会做错事」为由排除'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: '实现 #1 的提交 6682aa9 把 M-01 验收契约的预期从「集成 1 个通过」改成「集成共 1 个、默认忽略 1 个」，理由是 tests/t14_gate_logs.rs 带 #[ignore]；审计 #1 按改过的预期判通过'
  root_cause: '设计轮写预期时没有核对该集成测试默认忽略；实现轮发现不符后直接改了契约文字。本次改动是订正事实，但协议没有禁止实现轮改本里程碑验收契约，被评测方改了评测基准，审计拿到的已经是改后的数字'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-01-r1-impl.md'
  level: protocol
  proposal: '实现轮的提交不得改动任何「### M-xx 验收契约」小节的文字；实际用例数与预期不符时，只在实现证据里写差值和每个用例的来源，由审计轮按未改动的契约逐条判断'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 5}
```
