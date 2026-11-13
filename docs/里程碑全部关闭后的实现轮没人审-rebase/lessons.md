# 教训 · 复盘 #3

本任务（T-68）设计 1 轮。实现 8 轮，审计 7 次，其中两次是集成复验。M-01..M-05 都是一次审计就通过，reopen 都是 0。Autome 没有记录到本任务的指标，下面只引用证据文件里写着的事实。

和复盘 #2 相比的变化：rebase 之后，宿主的合并前门禁在提交 e5cee3d0 上失败了。automed lib 共 899 例，898 例通过，1 例失败：`protocol::eval::tests::a_code_sample_that_is_not_a_status_block_is_not_parsed`。为此多跑了实现 #8（再精简 session-protocol 69 字节）和审计 #7（集成复验通过）。证据见 `evidence/M-05-r8-impl.md`、`evidence/M-05-r8-audit.md`。

- 新增 L-05：这次门禁失败的直接原因。改了 protocol/ 的几轮都没有运行 protocol 模块的单测。
- L-03 在 symptom 里补上了后果：协议正文精简后只剩 30 字节余量，而那条单测需要 38 字节。
- L-01 没有新的出现：实现 #8 只改了 protocol/ 下的 md 文件，没有改 `.rs`。证据仍写着「未运行全量门禁或快检」。
- L-02、L-04 的证据没有变化，原样保留。

不作为教训的事实：

- `retro.md` 里「审计 #4」仍然出现了两次（M-03、M-04），后面的审计编号都错开一位，没有造成退回。
- 审计 #6、#7 都写着「本轮固定协议为 v6 副本，无集成块」，是照审计 #6 的先例自行做的集成复验。本任务的新功能还管不到本任务自己，这是版本门按设计生效，不是缺陷。

```yaml
- id: L-01
  domain: implementation
  symptom: '任务验收 7 要求实现轮在会话里跑 cargo fmt --all -- --check 与 cargo clippy --all-targets -- -D warnings，并记录退出码。提交里改了 .rs 的实现轮有 6 个（#1–#5、#7），只有 #2 记录了 clippy 退出码。#1 写「本轮最新用户指令优先：未跑宿主快检」，之后宿主快检在 integration.rs:203 报 clippy::type_complexity，实现 #2 整轮只用来把 &dyn Fn 改成 &impl Fn。#3 与 #7 只对改动文件跑了 rustfmt --check。#4 写「未运行全量 ci-local、宿主快检」。#5 写「按本轮用户指令…也未执行宿主负责的 fmt/clippy/Web 快检」'
  root_cause: '这是 T-61 L-07（docs/门禁只在合并前跑一次-gate-run/lessons.md）在本任务里的再次出现。任务文件把它写成了验收 7，会话 prompt 却要求「宿主快检不要跑」，实现轮按「最新用户指令优先」处理。两处要求互相冲突，任何一处都没有点明，结果几秒就能跑完的 clippy 每次都留给了宿主；宿主一失败，就要多花一个实现轮'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-01-r2-impl.md
  level: brief
  proposal: '改了 .rs 文件的实现轮（包括 rebase 集成轮），证据里必须记录本轮提交上 cargo clippy --all-targets -- -D warnings 与 cargo fmt --all -- --check 的退出码，不得以「由宿主门禁负责」跳过这两条'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: verification
  symptom: '审计 #4 原样执行了 M-04 契约里的负向对照：把集成简报段 short() 的 take(7) 改成 take(12)。结果 brief_before_merge_audit_brief_never_shows_a_stale_result 和 4 个 integration_e2e 仍然全部通过，这条对照拦不住任何东西。审计 #5 原样执行了 M-05 契约里的「改成 #### 集成复验 时两个金样失败」，prompt_rendering 仍然 35 个全过，因为金样用的是空协议文本。这两条都由审计就地改写契约，记为验证缺口。实现轮这边，M-04 只跑了契约三条对照中的第一条（Round.integration 置 None），M-05 一条负向对照都没跑；M-01..M-03 的实现轮逐条跑了契约里的对照，审计也没有发现那几份契约的描述有错'
  root_cause: '负向对照是设计轮在还没有代码的时候推理出来的。实现轮是第一个能实际执行它们的会话，但自审清单不要求它照契约原样逐条执行，于是各轮改跑自己挑的注入。没跑的那几条恰好就是写错的那几条，一直到审计才暴露'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-04-r5-audit.md
  level: protocol
  proposal: '实现轮证据的负向对照部分必须逐条列出本里程碑验收契约「负向对照」里的每一项注入、照原样执行的结果与输出路径；任何一项没有变红，同一轮就要改写契约并注明实测结果，然后才能标待审'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: 'M-03 实现证据已经写到「协议正文当前接近 24 KiB 上限」。M-05 第一次跑验收就变红：正文 26590 字节，超过上限 24576；改动前基线 24533，只剩 43 字节余量。实现 #6 只好精简设计范围之外的 Loop 引言、检查表引言和 session-protocol 三节，补记一条 J-3 clarify，并回头修改设计范围说明；中途还因为删掉四个固定短语被静态检查打红过一次。精简后正文 24546 字节，只剩 30 字节余量。protocol 模块的既有单测要往正文追加 38 字节，合并前门禁因此失败，实现 #8 又得再精简 69 字节（evidence/M-05-r8-impl.md）'
  root_cause: '设计 §10 规划在 loop-protocol 里新增一个二级「集成复验」节，另加检查表行与 CHANGELOG 条目，但没有核对协议正文的字节上限和当前余量。M-05 验收契约里也没有字节预算，超限要等实现轮第一次跑验收才看得到，删哪些文字、留多少余量也只能由实现轮临场决定'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r6-impl.md
  level: rule
  proposal: '要往 protocol/ 正文新增文字的里程碑，设计的验收契约必须写明当前正文字节数、上限和本里程碑预计新增的字节数；预计会超限时，设计里必须列出要删减的小节'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: design
  symptom: 'M-05 验收契约把 CHANGELOG 标题数写死为 7，版本单测把期望写死为 protocol/v7。任务运行期间，main 先发布了 protocol/v7（rebase 时保留了上游 v7/F-2），实现 #7 只好在 rebase 轮里把本任务顺延为 v8：版本单测的精确期望从 v7 改成 v8，M-05 契约计数从 7 改成 8，设计里的版本号也一起同步。设计 R4 事先写好了「若 main 先发布了 v7，本任务顺延为 v8 并相应修改 M-05 契约里的标题计数（这是事实变化，不是放宽验收）」，所以审计 #6 复验时，没有把改测试期望、改契约计数当作放宽验收，集成复验一次通过，reopen 0'
  root_cause: '本仓库 parallel = 3，几个并行任务都可能发布 protocol 新版本。版本号和 CHANGELOG 标题数是全仓库共用、单调递增的值，一旦写进验收契约，rebase 时就必然要改。设计在风险节预先写明了顺延规则，rebase 轮改断言才有条款可依；没有这条规则，改测试期望就和「修改测试前提」分不开'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r7-impl.md
  level: rule
  proposal: '设计的验收契约或测试里写死了协议版本号、CHANGELOG 标题数这类全仓库单调递增的值时，风险节必须写明 main 先发布同号时的顺延规则，并列出顺延时要同步的断言与契约计数'
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}

- id: L-05
  domain: verification
  symptom: 'M-05 只改了 protocol/ 下的正文，验收契约只列了 protocol eval 与 prompt_rendering 两条测试命令。实现 #6 把正文压到 24546 字节（上限 24576，余 30）。之后的实现 #6、审计 #5、实现 #7、审计 #6 四轮都没有运行 cargo test -p automed --lib protocol::，审计 #6 只跑了 --lib integration 的 34 例。protocol::eval 里的既有单测 a_code_sample_that_is_not_a_status_block_is_not_parsed 会往种子正文追加 38 字节的 shell 样例，再要求静态检查通过。这条单测直到宿主合并前门禁才第一次变红：提交 e5cee3d0，automed lib 899 例里 898 例通过、1 例失败。任务因此多跑了实现 #8 与审计 #7；实现 #8 跑了 protocol:: 的 70 例，全绿'
  root_cause: 'protocol/ 正文经 include_str! 编进 automed，读种子正文的不只有 protocol eval 和金样，protocol 模块的单测也直接拿种子正文做前提。验收契约只挑了里程碑本身要证明的命令，没有把被改文件的其余消费者测试列进去。会话里又不跑全量门禁，所以这类测试的第一次运行就落在了宿主的合并前门禁上'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r8-impl.md
  level: rule
  proposal: '改了 protocol/ 下任何被跟踪文件的实现轮与审计轮，都必须运行 cargo test -p automed --lib protocol:: 并在证据里记录用例数与退出码，不论验收契约是否列出这条命令'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
