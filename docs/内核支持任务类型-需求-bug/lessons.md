# 教训 · 任务类型（需求/Bug）、任务卡正文与待开始状态（K1）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 14 份证据、审计与裁决记录、评审记录。Autome 本任务没有记录到指标，下文不引用任何指标数值；文中的数字都取自证据文件或 `git` 的实际输出。

设计 1 轮定稿，M-01..M-07 都是一次审计关闭，没有 reopen。可带走的四条：契约里的负向对照没人实跑、审计时才发现拦不住（L-01，与 `用户按合并时如果默认分支已经前进-任务自动` 的 L-01 同一句，按原文复用以便跨任务计数）；任务分支被反复改成只剩脚手架，七轮都只当「环境偏差」绕过（L-02）；评审带复现的非阻塞意见只进 Backlog，M-03 带着这个缺陷关闭（L-03）；用户文本经派生后又进了 prompt 占位符渲染（L-04）。

```yaml
- id: L-01
  domain: verification
  symptom: '审计 #3 发现 M-03 契约的负向对照「queued_tasks 包含草稿时 fill_slots_never_starts_a_draft 失败」不成立：注入后该用例与 e2e 仍全绿，审计当场补一行断言才让它变红；实现 #3 的证据里没有负向对照一节'
  root_cause: '「哪条测试会变红」是设计轮推理出来的（状态机对 (Draft, SlotAvailable) 的拒绝兜住了 fill_slots，于是被点名的测试拦不住）；实现 #3 没有对点名的测试实跑该变异，偏差留到审计才暴露'
  evidence: 'docs/内核支持任务类型-需求-bug/evidence/M-03-r3-audit.md'
  level: rule
  proposal: '实现轮对验收契约中的每条负向对照，必须对契约点名的那条测试实跑该变异并在证据里写出它的退出码；点名的测试没有变红时，在同一轮改写契约里的这条表述'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '实现 #3 至 #7 与审计 #5 都记录开场 HEAD 是一个 initial 提交、项目文件全部变成未跟踪；M-05 的字面验收命令 git diff --quiet f20fe14 -- protocol/ apps/ … 因此必然退出 1，两轮都改用临时 GIT_INDEX_FILE 做等价比对，审计 #5 记为验证缺口。复盘时任务分支上有 24 个 initial 提交，提交者均为 Autome <autome@localhost>，每个删除约 535 个已跟踪文件；分支末端 eec8fbb 只跟踪 .gitignore、AGENTS.md、.autome/rules/README.md、.autome/skill/run_session.sh 4 个文件，而基线 f20fe14 跟踪 502 个'
  root_cause: '复盘轮当场复现：本轮提交 172ca65 触发 .githooks/post-commit 在后台跑 ci-local，约 20 秒内分支上多出 1cddf3b initial，作者与时间戳和 172ca65 完全相同，删掉刚提交的 lessons.md，已跟踪文件回到 4 个——ci-local 里有测试的 git 提交继承了钩子的 git 环境、落到了任务 worktree 上（具体是哪条测试未定位；git.rs 的 run_with_binary 已 env_clear，不是它）。Autome 没有任何检查比对任务分支与基线的已跟踪文件，于是七轮会话都只把它写成「环境偏差」后绕过，这个状态会被带到合并'
  evidence: 'docs/内核支持任务类型-需求-bug/evidence/M-05-r5-impl.md'
  level: test
  proposal: 'Autome 在每次会话开始前和兜底提交后比对任务分支 HEAD 与基线提交的已跟踪文件，HEAD 缺少基线里任一文件即暂停任务并报警，并由一条 automed 测试断言只剩脚手架文件的 HEAD 会被这一检查拦下'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: '评审 N1-01 附隔离仓库复现日志，指出写卡成功、git add 失败后按「文件存在就跳过」重试，卡片永远不进提交（违反验收 A2「第一个会话之前提交」）；裁决 D1-N01 判非阻塞只进 Backlog B-05；实现 #3 照「已有文件不覆盖」落地、审计 #3 通过关闭 M-03；直到用户把 B-05 纳入 M-07，实现 #7 改代码前实跑才确认重试后 HEAD:plans/retry/retry-card.md 不存在'
  root_cause: '裁决轮按「设计没写跳过提交」把一条已复现、且直接违反任务验收条款的失败路径归为实现注意事项；Backlog 不触发轮次，也不进任何里程碑的情形表，实现轮和审计轮都不会去验它'
  evidence: 'docs/内核支持任务类型-需求-bug/evidence/M-07-r7-impl.md'
  level: protocol
  proposal: '评审非阻塞意见附有复现日志、且复现的失败违反任务文件的某条验收标准时，裁决轮必须把这条复现写成某个里程碑的情形表行或负向对照，不得只放进 Backlog'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: design
  symptom: '评审 R1-01：设计只承诺「正文不进 prompt」，但 Q1 从正文一级标题派生 request，request 又经 {request} 替换、其余占位符替换与 check_no_placeholders_left；标题为「修复 {request} 展示」的 brief 会让首个任务整理会话拒绝启动，违反 C7。裁决 D1-P01 采纳，改为按 {request} 切段、request 原文最后拼入'
  root_cause: '设计只追踪了用户原文本身的去向，没有追踪从原文派生出的字符串（标题）的去向；launcher.rs 先替换 {request} 再替换其余占位符并检查剩余占位符，派生字符串因此成了模板的一部分'
  evidence: 'docs/内核支持任务类型-需求-bug/内核支持任务类型-需求-bug-adjudication.md'
  level: rule
  proposal: '设计中凡把用户提供或从用户文本派生的字符串（request、正文标题、reply 反馈等）放进 prompt 的，必须写明它在占位符替换和 check_no_placeholders_left 之后才拼入，并在某个里程碑里列一条该字符串含 `{request}` 字样的用例'
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
```
