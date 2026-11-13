# 教训 · 复盘 #1

本任务（T-68）设计 1 轮（`design-round: 1/15`），实现 6 轮（`implementation-round: 6/25`），M-01..M-05 都是一次审计就通过，reopen 都是 0。Autome 没有记录到本任务的指标，下面只引用证据文件里写着的事实。

实现轮数比里程碑数多 1，多出的是实现 #2：它只修了宿主快检报出的一处 Clippy 错误（L-01）。审计在 M-04、M-05 两次就地改正了验收契约里的负向对照描述，都按验证缺口记录，产品本身没有缺陷（L-02）。M-05 第一次跑验收就因为协议正文超出字节上限变红，同一轮里删减了不在范围内的 session-protocol 文字（L-03）。

另外记一处事实，不作为教训：设计文档里五条「最新证据」指针的审计编号是 #2、#3、#4、#4、#5，而审计简报的编号是第 1–5 轮，两者对不上。它没有造成任何退回，所以不写成条目。

```yaml
- id: L-01
  domain: implementation
  symptom: '任务验收 7 要求实现轮在会话里跑 cargo fmt --all -- --check 与 cargo clippy --all-targets -- -D warnings，并记录退出码。实现 #1 写「本轮最新用户指令优先：未跑宿主快检（fmt check、clippy、Web lint/typecheck）」，之后宿主快检在 integration.rs:203 报 clippy::type_complexity，实现 #2 整轮只用来把 &dyn Fn 改成 &impl Fn。其后改动了 .rs 的实现 #3、#4、#5 也都没有记录 clippy 退出码：#3 只对 4 个改动文件跑了 rustfmt --check；#4 写「未运行全量 ci-local、宿主快检」；#5 写「按本轮用户指令，未执行全量 sh scripts/ci-local.sh，也未执行宿主负责的 fmt/clippy/Web 快检」'
  root_cause: '这是 T-61 L-07 的第二次出现（docs/门禁只在合并前跑一次-gate-run/lessons.md）。这次任务文件已经把它写成了验收 7，实现轮仍然服从会话 prompt 里「宿主快检不要跑」的指令，并认定「最新用户指令优先」。任务验收与会话 prompt 互相冲突，没有一方点明，所以几秒就能跑完的 clippy 每次都留给了宿主；宿主一失败，就要多花一个实现轮'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-01-r2-impl.md
  level: brief
  proposal: '改动了 .rs 文件的实现轮，证据里必须记录本轮提交上 cargo clippy --all-targets -- -D warnings 与 cargo fmt --all -- --check 的退出码，不得以「由宿主门禁负责」跳过这两条'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: verification
  symptom: '审计 #4 把 M-04 契约原写的负向对照（集成简报段 short() 的 take(7) 改成 take(12)）照写跑了一遍，brief_before_merge_audit_brief_never_shows_a_stale_result 与 4 个 integration_e2e 仍然全部通过，这条对照是惰性的。审计 #5 把 M-05 契约原写的「改成 #### 集成复验 时两个金样失败」照写跑了一遍，prompt_rendering 仍然 35 个全过，因为金样用的是空协议文本。两条都由审计就地改写契约，记为验证缺口。实现轮证据里，M-04 只跑了契约三条对照中的第一条（Round.integration 置 None），M-05 一条负向对照都没跑；M-01..M-03 的实现轮逐条跑了契约里的对照，审计没有发现契约描述有错'
  root_cause: '负向对照是设计轮在还没有代码时推理出来的。实现轮是第一个能实际执行它们的会话，但自审清单不要求它照契约原样逐条执行，于是各轮改跑自己挑的注入。没执行的那几条，恰好就是写错的那几条，直到审计才暴露'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-04-r5-audit.md
  level: protocol
  proposal: '实现轮证据的负向对照部分必须逐条列出本里程碑验收契约「负向对照」里的每一项注入、照原样执行的结果与输出路径；没有变红的项，同一轮就改写契约并注明实测结果，然后才能标待审'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: 'M-03 实现证据已经写到「协议正文当前接近 24 KiB 上限」。M-05 第一次跑验收就变红：正文 26590 字节，超过上限 24576；改动前基线是 24533，只剩 43 字节余量。实现 #5 只好精简设计范围之外的 Loop 引言、检查表引言和 session-protocol 三节，补记一条 J-3 clarify，并回头修改设计范围说明，中途还因为删掉四个固定短语被静态检查打红过一次'
  root_cause: '设计 §10 规划了在 loop-protocol 里新增一个二级「集成复验」节，加上检查表行与 CHANGELOG 条目，但没有核对协议正文的字节上限和当前余量。M-05 验收契约里也没有字节预算，超限要等到实现轮第一次跑验收才看得到，删哪些文字也只能由实现轮临场决定'
  evidence: docs/里程碑全部关闭后的实现轮没人审-rebase/evidence/M-05-r6-impl.md
  level: rule
  proposal: '设计里要往 protocol/ 正文新增文字的里程碑，验收契约必须写明当前正文字节数、上限和本里程碑预计新增的字节数；预计会超限时，设计里必须列出要删减的小节'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```
