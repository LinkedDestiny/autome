# 教训 · 复盘 #4

本任务（T-68）设计 1 轮，实现 9 轮，审计 8 次，其中三次是集成复验。M-01..M-05 都是一次审计就通过，reopen 都是 0。Autome 没有记录本任务的指标，下面只引用证据文件里写明的事实。

和复盘 #3 相比，有这些变化：宿主在提交 eef466bd 上跑的合并前门禁又失败了一次。automed lib 899 例全部通过；end_to_end 187 例里 186 例通过，1 例失败：`every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`，失败在 `end_to_end.rs:1951`，期望 `protocol/v7@`，实际是 `protocol/v8@…`。为此多跑了实现 #9（只改这一行字面量）和审计 #8（集成复验通过）。证据见 `evidence/M-05-r9-impl.md`、`evidence/M-05-r9-audit.md`。宿主随后对 51542801 跑的快检（fmt、clippy、web lint、typecheck）退出 0。复盘 #4 写本文件时，51542801 的合并前门禁还没有结果。

- 新增 L-06：实现 #7 把版本顺延到 v8 时，只同步了版本单测和契约计数，没有在全仓库搜索旧版本字面量。
- 新增 L-07：第一次门禁失败时，cargo 在 lib 失败后就停了，end_to_end 根本没有运行，这个缺陷被藏了一整个修复周期。
- L-01 加上了实现 #9 的情况：本轮又改了 `.rs`，同样没有跑 fmt/clippy。宿主快检通过，没有因此多出轮次。
- L-04 加上了后果：顺延规则写了，但要同步的断言没有列全。
- L-02、L-03、L-05 的证据没有变化，原样保留。

下面几条是事实，但不作为教训：

- `retro.md` 里「审计 #4」仍然出现了两次（M-03、M-04），后面的审计编号都错开一位，没有造成退回。
- 审计 #6、#7 写着「本轮固定协议为 v6 副本，无集成块」，是照先例自行做的集成复验，审计 #8 也是这样。本任务的新功能还管不到本任务自己，这是版本门按设计生效，不是缺陷。

```yaml
- id: L-01
  domain: implementation
  symptom: '任务验收 7 要求实现轮在会话里跑 cargo fmt --all -- --check 与 cargo clippy --all-targets -- -D warnings，并记录退出码。提交里改了 .rs 的实现轮有 7 个（#1–#5、#7、#9），只有 #2 记录了 clippy 退出码。#1 写的是「本轮最新用户指令优先：未跑宿主快检」，之后宿主快检在 integration.rs:203 报 clippy::type_complexity，实现 #2 整轮只用来把 &dyn Fn 改成 &impl Fn。#3 与 #7 只对改动文件跑了 rustfmt --check。#4 写「未运行全量 ci-local、宿主快检」，#5、#9 都写「按本轮用户指令…不运行…宿主快检」。#9 之后宿主快检退出 0，这一次没有多出轮次'
  root_cause: '这是 T-61 L-07（docs/门禁只在合并前跑一次-gate-run/lessons.md）在本任务里的再次出现。任务文件把 fmt/clippy 写成了验收 7，会话 prompt 却要求「宿主快检不要跑」，实现轮按「最新用户指令优先」处理。两处要求互相冲突，任何一处都没有点明，结果几秒就能跑完的 clippy 每次都留给了宿主；宿主一失败，就要多花一个实现轮'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-01-r2-impl.md
  level: brief
  proposal: '改了 .rs 文件的实现轮（包括 rebase 集成轮和门禁修复轮），证据里必须记录本轮提交上 cargo clippy --all-targets -- -D warnings 与 cargo fmt --all -- --check 的退出码，不得以「由宿主门禁负责」为由跳过这两条'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: verification
  symptom: '审计 #4 原样执行了 M-04 契约里的负向对照：把集成简报段 short() 的 take(7) 改成 take(12)。结果 brief_before_merge_audit_brief_never_shows_a_stale_result 和 4 个 integration_e2e 仍然全部通过，这条对照拦不住任何东西。审计 #5 原样执行了 M-05 契约里的「改成 #### 集成复验 时两个金样失败」，prompt_rendering 仍然 35 个全过，因为金样用的是空协议文本。这两条都由审计就地改写契约，记为验证缺口。实现轮这边，M-04 只跑了契约三条对照中的第一条（Round.integration 置 None），M-05 一条负向对照都没跑。M-01..M-03 的实现轮逐条跑了契约里的对照，审计也没有发现这几份契约写错'
  root_cause: '负向对照是设计轮在还没有代码的时候推理出来的。实现轮是第一个能实际执行它们的会话，但自审清单不要求实现轮照契约原样逐条执行，于是各轮改跑自己挑的注入。没跑的那几条恰好就是写错的那几条，一直到审计才暴露'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-04-r5-audit.md
  level: protocol
  proposal: '实现轮证据的负向对照部分必须逐条列出本里程碑验收契约「负向对照」里的每一项注入、照原样执行的结果和输出路径；任何一项没有变红，同一轮就要改写契约并注明实测结果，然后才能标待审'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: 'M-03 的实现证据已经写到「协议正文当前接近 24 KiB 上限」。M-05 第一次跑验收就变红：正文 26590 字节，超过上限 24576；改动前的基线是 24533，只剩 43 字节余量。实现 #6 只好去精简设计范围以外的 Loop 引言、检查表引言和 session-protocol 三节，补记一条 J-3 clarify，并回头修改设计的范围说明；中途还因为删掉了四个固定短语，被静态检查打红过一次。精简后正文 24546 字节，只剩 30 字节余量。protocol 模块的一条既有单测要往正文追加 38 字节，合并前门禁因此失败，实现 #8 又得再精简 69 字节（evidence/M-05-r8-impl.md）'
  root_cause: '设计 §10 规划在 loop-protocol 里新增一个二级「集成复验」节，另加检查表行和 CHANGELOG 条目，但没有核对协议正文的字节上限和当前余量。M-05 验收契约里也没有字节预算，超限要等实现轮第一次跑验收才看得到；删哪些文字、留多少余量，也只能由实现轮临场决定'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r6-impl.md
  level: rule
  proposal: '要往 protocol/ 正文新增文字的里程碑，设计的验收契约必须写明当前正文字节数、上限和本里程碑预计新增的字节数；预计会超限时，设计里必须列出要删减的小节'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: design
  symptom: 'M-05 验收契约把 CHANGELOG 标题数写死为 7，版本单测把期望写死为 protocol/v7。任务运行期间，main 先发布了 protocol/v7（rebase 时保留了上游的 v7/F-2），实现 #7 只好在 rebase 轮里把本任务顺延为 v8：版本单测的精确期望从 v7 改成 v8，M-05 契约里的计数从 7 改成 8，设计里的版本号也一起同步。设计 R4 事先写好了顺延规则，所以审计 #6 没有把这些改动当作放宽验收，reopen 0。但 R4 只点名了「M-05 契约里的标题计数」，没有列出 end_to_end.rs:1951 里写死的 protocol/v7@。这一处漏改，后来让合并前门禁失败了一次（见 L-06）'
  root_cause: '本仓库 parallel = 3，几个并行任务都可能发布 protocol 的新版本。版本号和 CHANGELOG 标题数是全仓库共用、只增不减的值，一旦写进验收契约或测试，rebase 时就必然要改。设计在风险节预先写明了顺延规则，rebase 轮改断言才有条款可依；但设计只列出了自己新写的断言，没有列出仓库里原有的、写死当前版本号的测试'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r7-impl.md
  level: rule
  proposal: '设计的验收契约或测试里写死了协议版本号、CHANGELOG 标题数这类全仓库只增不减的值时，风险节必须写明 main 先发布同号版本时的顺延规则，并列出顺延时要同步的全部断言（含 crates/ 下原有测试里的版本字面量）与契约计数'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}

- id: L-05
  domain: verification
  symptom: 'M-05 只改了 protocol/ 下的正文，验收契约只列了 protocol eval 与 prompt_rendering 两条测试命令。实现 #6 把正文压到 24546 字节（上限 24576，余 30）。之后的实现 #6、审计 #5、实现 #7、审计 #6 四轮都没有运行 cargo test -p automed --lib protocol::，审计 #6 只跑了 --lib integration 的 34 例。protocol::eval 里有一条既有单测 a_code_sample_that_is_not_a_status_block_is_not_parsed，它会往种子正文追加 38 字节的 shell 样例，再要求静态检查通过。这条单测直到宿主的合并前门禁才第一次变红：提交 e5cee3d0，automed lib 899 例里 898 例通过、1 例失败。任务因此多跑了实现 #8 和审计 #7；实现 #8 跑了 protocol:: 的 70 例，全绿'
  root_cause: 'protocol/ 正文经 include_str! 编进 automed。读种子正文的不只有 protocol eval 和金样，protocol 模块的单测也直接拿种子正文做前提。验收契约只挑了里程碑本身要证明的命令，没有把被改文件的其余消费者测试列进去；会话里又不跑全量门禁，所以这类测试第一次运行就是在宿主的合并前门禁上'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r8-impl.md
  level: rule
  proposal: '改了 protocol/ 下任何被跟踪文件的实现轮与审计轮，都必须运行 cargo test -p automed --lib protocol:: 并在证据里记录用例数与退出码，不论验收契约是否列出这条命令'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-06
  domain: implementation
  symptom: '实现 #7 在 rebase 轮里把本任务的协议从 v7 顺延为 v8，只改了 crates/automed/src/protocol/mod.rs 里的版本单测期望（提交 152e2425，只改了这一个 .rs 文件），没有搜索仓库里其余写死 protocol/v7 的地方。end_to_end.rs:1951 的 assert!(protocol_ref.starts_with("protocol/v7@")) 从此一直是红的。之后的审计 #6、实现 #8、审计 #7 都只跑了 end_to_end 的子集（integration_e2e、before_merge 等），没有跑到这条用例，直到宿主在 eef466bd 上跑合并前门禁才报出来（186 通过 / 1 失败）。实现 #9 补做了搜索，把命中结果存成 r9-v7-occurrences.txt，逐条判断后只有这一处该改，改完一行即转绿'
  root_cause: '顺延版本号，相当于改一个全仓库共享的常量，影响范围应该靠搜索来确定，而不是靠记忆或设计里列出的那几处。实现 #7 只改了已知的那一条单测；自审清单「域边界」一项只核对了 v1–v8 标题连续、版本单测精确返回 v8，没有要求搜索旧字面量还剩哪些。同一行字面量在 v6→v7 时也漏改过：main 上的提交 470aa59a「align frozen protocol expectation with v7」只改了 end_to_end.rs 这一行，出自另一个任务（修复轮不敢格式化回归测试-定位轮写的测试过不了-bug）。这次是第二个任务里再次出现'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r9-impl.md
  level: brief
  proposal: '改变协议版本号（发布新版或 rebase 时顺延）的实现轮，证据里必须附上 git grep 在 crates/ 下搜索旧版本字面量（如 protocol/v7）的输出，逐条注明「已改」或「不应改」及理由，并运行每一处已改断言所在的测试用例'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-07
  domain: tooling
  symptom: '宿主在 e5cee3d0 上跑的合并前门禁，日志停在「error: test failed, to rerun pass `-p automed --lib`」（.autome/output/r8-host-failure.txt）：lib 898/899 失败以后，cargo 没有再运行 end_to_end。end_to_end.rs:1951 的 v7 字面量那时已经是红的（自 152e2425 起），但门禁日志里看不到。实现 #8 只能修日志里报出的那一条 lib 失败，修完宿主在 eef466bd 上重跑门禁，才第一次跑到 end_to_end 并报出第二个失败。两个各自独立的缺陷就这样分两次暴露，各花了一对实现轮加审计轮（#8/#7、#9/#8），e2e 那一次还要再等 477 秒的门禁'
  root_cause: 'scripts/ci-local.sh 的测试步骤是 cargo test --workspace --all-targets -- --nocapture，没有加 --no-fail-fast。cargo 默认在第一个失败的测试二进制之后停下，后面的二进制一个都不跑。门禁修复轮按日志修复，只能看到第一个失败的二进制，看不到全部失败'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r9-impl.md
  level: test
  proposal: 'scripts/ci-local.sh 的 cargo test 步骤加上 --no-fail-fast，使宿主门禁失败日志里每个测试二进制都有一行 test result'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
