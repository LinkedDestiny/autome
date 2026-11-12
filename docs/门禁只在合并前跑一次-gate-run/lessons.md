# 教训 · 门禁只在合并前跑一次（`[gate] run = "before_merge"`，T-61）

复盘轮 #2 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 17 份证据（含复盘 #1 之后新增的 `M-08-r9-impl.md`）、审计与裁决记录、任务分支 `git log` / `git show` / `git grep` 的实际输出。Autome 本任务没有记录到指标，下文不引用任何指标数值；文中的数字都取自证据文件或 `git` 的实际输出。

设计 3 轮（设计 #1 在定稿卡点被用户驳回），实现 9 轮，M-01..M-08 都是一次审计通过，没有 reopen。复盘 #1 的四条（L-01..L-04）保持不变：宿主门禁缺席没人发现，负向对照的注入没核对，范围外的读者只按正确性判断，实现轮改了验收契约。复盘 #1 之后，实现 #9 把分支 rebase 到 main，又暴露出两条新教训：rebase 集成轮改了代码、金样和测试前提，但审计 #9 因为「无待审里程碑」没有复验（L-05）；rebase 把冲突标记提交进了 7 个中间提交，冲突标记只在 HEAD 上查过（L-06）。

```yaml
- id: L-01
  domain: verification
  symptom: '本任务 9 个实现轮、8 个有里程碑的审计轮都写「全量测试 / clippy / ci-local 由宿主门禁负责」；M-01 的验收契约甚至写「其余现有用例由宿主门禁在本轮 head 上的结果证明通过」。但 8 份审计简报都没有「## 宿主门禁」段，审计 #8 注意到了，仍写「全量由宿主在合并前门禁跑」并判通过。rebase 之后的实现 #9 写「未运行完整宿主门禁」「未宣称修复后重新跑过整个 721 用例集」。任务验收 6（clippy -D warnings、sh scripts/ci-local.sh 全绿）在任务目录里没有任何一轮的运行证据'
  root_cause: '主检出 .autome/config.toml 的 [gate] 在任务输入提交之后被注释掉，Autome 读的是主检出配置，所以本任务没有宿主门禁（MergeGate 也只看 gate.is_some()，同样不会跑）。worktree 里的 .autome/config.toml 是分支基线的旧副本，仍写着 command = "sh scripts/ci-local.sh"；任务文件 §3 也写「由 Autome 在实现轮之后自己跑」——会话能看到的两处信息都说门禁存在。「没有门禁」和「有门禁但还没出结果」在简报里都是什么也不写，审计分不出来'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-08-r8-audit.md'
  level: brief
  proposal: '项目当前没有生效的 [gate] 时，Autome 在实现轮与审计轮简报里写明「本项目当前没有宿主门禁，全量验证不会由宿主执行」；任务验收标准点名的全量命令（本任务为 clippy -D warnings 与 sh scripts/ci-local.sh），必须在某一轮实现或审计证据里出现并附退出码'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '审计 #5 第一次做负向对照时，用 BSD sed 和错误的行号注入错误，改动实际没落到文件上，被点名的用例仍是绿的。审计核对 git diff --stat 才发现注入没生效，改用 perl 重做后用例才变红'
  root_cause: 'macOS 的 BSD sed 与 GNU sed 的 -i 语义不同，行号又随前序里程碑漂移；sed 静默不改，也退出 0。负向对照只看「测试红没红」，不看错误到底注入进去没有。审计要是没顺手查 diff，就会得出「验收命令拦不住」的错误结论；反过来，一次没生效的注入也可能被记成已覆盖'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-05-r5-audit.md'
  level: rule
  proposal: '每条负向对照在跑验收命令之前，证据里必须记录注入后 git diff --stat 的输出，且被注入的文件出现在输出中；没有这条记录的对照不得写成「拦得住」或「拦不住」'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '设计 #1 经评审 #1、裁决 #1 定稿后，在定稿卡点被用户驳回。设计 #2 按驳回意见把范围扩到 protocol/prompts（prompt 分段渲染、协议 v3、eval）；裁决 #1 为避开 protocol/ 新加的 gate_note 宿主追加通道，在设计 #2 里又被删掉。多花了一整轮设计、评审与裁决'
  root_cause: '设计 #1 已经看到复核 prompt「要求会话自己复验项目门禁」，但只按正确性判断（「措辞差异不会让任何一轮做错事」），当作范围外处理。任务目标是「降低门禁运行次数、避免门禁抢占」，而 audit.md / fix.md / fix-review.md 与 Inject::GateFailed 都让会话自己再跑全量，排除它们会让目标只完成一半。任务卡写「不改会话自己跑测试的方式」，与目标冲突，设计没有把这个冲突写进争议项交给用户'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run.md'
  level: rule
  proposal: '设计轮发现范围外的文件里有直接执行任务目标所要减少的动作的文字时（本任务为 prompt 要求会话全量跑门禁），必须在 `## 争议项` 写一条，列出纳入与不纳入两种方案及各自对目标的影响，由用户在定稿卡点选择，不得只以「不会做错事」为由排除'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: '实现 #1 的提交 6682aa9 把 M-01 验收契约的预期从「集成 1 个通过」改成「集成共 1 个、默认忽略 1 个」，理由是 tests/t14_gate_logs.rs 带 #[ignore]；审计 #1 按改过的预期判通过'
  root_cause: '设计轮写预期时没有核对这个集成测试默认被忽略；实现轮发现不符，就直接改了契约文字。这一次改的是事实订正，但协议没有禁止实现轮改本里程碑的验收契约：被评测的一方改了评测基准，审计拿到的已经是改后的数字'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-01-r1-impl.md'
  level: protocol
  proposal: '实现轮的提交不得改动任何「### M-xx 验收契约」小节的文字；实际用例数与预期不符时，只在实现证据里写差值和每个用例的来源，由审计轮按未改动的契约逐条判断'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 5}
```

```yaml
- id: L-05
  domain: process
  symptom: '实现 #9 在 M-01..M-08 全部关闭之后把分支 rebase 到 main，解决冲突时改了 launcher.rs、scheduler.rs、两份 before_merge 金样，并改了两个测试的前提：end_to_end 的 feature_before_merge_two_milestones 显式加 reuse_for_main_check = false，旧模板兼容测试改成先选出 each_round 模板。它还把 M-08 验收契约的「最新证据」从 M-08-r8-audit.md 改指向自己的 M-08-r9-impl.md。随后审计 #9 以「无待审里程碑（M-01..M-08 均已完成），未改表」结束，没有复验任何一项'
  root_cause: '实现轮简报和审计轮简报都按里程碑表决定「审什么」：表里没有开放或待审的里程碑，审计简报就写「没有待审的里程碑」，审计会话没有对象可验。rebase 集成轮不属于任何里程碑，它的改动（包括为让旧断言继续成立而改的测试配置）落在审计的盲区，M-08 的「已完成」也改由一份未经审计的实现证据支撑'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-08-r9-impl.md'
  level: protocol
  proposal: '所有里程碑已完成之后，只要某个实现轮又产生了任务文档目录以外的提交，紧随其后的审计轮必须对这些提交的范围逐项复验并写出审计证据文件，不得以「无待审里程碑」结束'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: implementation
  symptom: '实现 #9 证据写「源码、协议与两份产品文档扫描无冲突标记」，但 rebase 后的 607e967（docs: explain before-merge gate tradeoffs）到 d0dbb75 共 7 个提交里，README.md 与 docs/development/technical-design.md 都带 <<<<<<< HEAD / >>>>>>> b0a360a 冲突标记，其中包括审计 #8 判「通过」的那个提交 a4e72e7；标记直到修复提交 2e411a2 才删掉。合并用 merge --no-ff，这 7 个提交会原样进入 main 历史。M-08 的验收命令（两组 grep + cargo fmt --check）在 607e967 上照样全部命中，拦不住冲突标记'
  root_cause: '冲突解决时带着标记 git add 后执行 GIT_EDITOR=true git rebase --continue，再在末尾另开提交清理；冲突标记扫描只在最终 HEAD 上做，没有逐个检查 main..HEAD 的提交。文档类里程碑的验收命令是「关键句 grep 命中」，冲突两侧都保留时关键句依然存在'
  evidence: 'docs/门禁只在合并前跑一次-gate-run/evidence/M-08-r9-impl.md'
  level: rule
  proposal: 'rebase 完成后，证据里必须记录对 main..HEAD 每个提交执行 git grep -E "^(<<<<<<<|>>>>>>>) " 的结果，且结果为空；不为空时须把清理并入对应提交，不得只在末尾另开提交清理'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```
