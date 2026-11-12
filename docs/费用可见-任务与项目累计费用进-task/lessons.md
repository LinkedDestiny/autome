# 教训 · 费用可见（T-44）复盘 #3

覆盖 #2。指标对照：验证缺口 1 = L-01（M-02 审计）；协议失败 1 = L-03（审计 #2 改了 `crates/automed/tests/end_to_end.rs`，被内核范围守卫判越界）；实现缺陷 0、reopen 0；人工验收未确认 3 = H-01–H-03。
复盘 #2 之后只多了两轮：实现 #9（第二次变基到 main，45 步，冲突落在 store.rs、config.rs、render.rs、cli.rs、end_to_end.rs）和审计 #8（无待审里程碑，什么也没跑）。
这两轮让三条已有教训各复现一次（L-02 沙箱门禁第 5 次红、L-05 队列用例这次全量通过、L-06 审计简报仍不转交实现点名的失败），并产出新条目 L-07。
L-01..L-06 的 proposal 原样保留（L-01、L-02、L-04 与先前任务的条目逐字相同），只在 symptom 里补上本次复现。

L-05 的机制已对照源码核实：`gate::running`（`crates/automed/src/gate.rs` 第 212 行起，变基 #9 后仍在）在没有 pid 文件时，只要 `started` 时间戳距今不到 10 秒就算运行中；`queue_job` 夹具（`scheduler_gate_queue_tests.rs` 第 45 行）只写 `started`、不写 pid。
实现 #8 单跑 15.56 秒失败；审计 #7 全量通过；实现 #9 全量（Rust 1423 个）又通过，且该用例、gate.rs、gate_queue.rs 两次之间都未被本任务改动——同一代码三次结果不同，只能来自时序。

```yaml
- id: L-01
  domain: verification
  symptom: '审计 #2 全量跑出 end_to_end 的 bug_loop_task_get_reports_the_fix_round_for_bugs_only 失败（task 键集合多出 cost_usd、cost_sessions_without_cost）；实现 #4 只跑了 `cargo test -p automed --lib`，这条 e2e 从 M-02 红到 M-05 实现 #7 才修，M-03、M-04 两次审计都带着已知红关闭'
  root_cause: 'M-02 验收命令只有 `cargo test -p automed --lib cost_rollup_`，全量只放在 M-05；设计 §5 写「预期不需要修改任何现有测试」却没有检索断言 task_json 整体键集合的既有测试，而该测试在基线 a72691c 已存在'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-02-r4-audit.md
  level: rule
  proposal: '凡改动产品代码的里程碑，其验收契约的验收命令必须包含 `cargo test --workspace`，不得把全量运行只放在单独的「全量门禁」里程碑'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: protocol-format
  symptom: '实现 #7 的 CLI 实现与 cost_cli_ 5 个用例全部通过，但沙箱内 ci-local.sh 因 Electron SIGABRT、Web listen EPERM 失败，于是把 M-05 留在「开放」不标待审；随后 Claude 审计在同一提交上跑 ci-local.sh 九项全绿并关闭。实现 #2、#3 也在同类环境失败上各跑一次全量（#3 耗时 1061 秒）；复盘后变基的实现 #8、#9 又各跑一次 ci-local.sh（344 秒、242 秒），Desktop SIGABRT 与 Web EPERM/EMFILE 原样复现，同一任务累计 5 次'
  root_cause: '实现轮运行在不能启动 Electron、不能监听 127.0.0.1 的 Codex 沙箱里，且已用不加载项目的探针（electron --version、裸 node listen）证实；协议「拿不到验收证据 → 留在它该在的状态」没有说清环境阻断时该在哪个状态，实现轮选了「开放」'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r7-impl.md
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: process
  symptom: '审计 #2 判定验证缺口后，自己给 end_to_end.rs 的键集合基线补了两个键；内核范围守卫判本轮越界，记一次 task.protocol_failure，任务停下，人工还原该改动（提交 f183bb0）后续跑，又空跑了一轮无待审里程碑的审计 #3；该测试一直红到 M-05'
  root_cause: '审计 prompt 对验证缺口写的是「当场加强验收或补检查……复验通过则照常关闭」，没有说加强只能落在 docs/<slug>/ 内；内核守卫却只允许审计改 docs/<slug>/ 下的文件。两份规则冲突，审计按 prompt 做了，被守卫判失败'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-03-r4-audit.md
  level: protocol
  proposal: '审计轮判定验证缺口、而补强需要改 docs/<slug>/ 以外的文件时，审计不改该文件，只在审计证据里写出测试全名与补丁内容；紧接着的实现轮把它作为本轮第一项完成，并在实现证据里记录修复前红、修复后绿'
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: process
  symptom: '宿主门禁在 f00e2d9 失败（Desktop、Web test：Test timed out in 60000ms），在 00252d6 失败（Desktop：内核在回复前退出），两次 fingerprint 不同；00252d6 → 45ae5bf 只改了文档，45ae5bf 的门禁却退出 0。两次失败各派一轮门禁修复实现轮（#2、#3），都没有产品改动；实现 #3 写明「宿主只提供了 Desktop 阶段失败摘要，没有具体断言」'
  root_cause: '同一产品代码上门禁结果不稳定，Autome 把每一次红都当成需要修复，直接派实现轮，且注入的只有失败摘要与日志尾部；实现轮所在沙箱连 Desktop/Web 层都跑不起来，无法区分偶发失败与真实缺陷'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-01-r3-impl.md
  level: brief
  proposal: '宿主门禁失败时，若拿不到失败用例全名与断言原文，或失败用例位于任务文件禁止修改的路径（本任务为 crates/），Autome 先在同一提交上重跑一次门禁、不派实现轮；只有重跑仍以同一可改范围内的用例失败时才派实现轮，并在简报里附该用例全名与断言原文'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}

- id: L-05
  domain: verification
  symptom: '实现 #8 变基后，main 带进来的 scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them 在第 225 行失败（期望 C 的 gate.started 为 0，实际 1），单跑复现，耗时 15.56 秒；同一代码上审计 #7 跑 cargo test --workspace，这条用例通过；实现 #9 再次变基后跑 ci-local.sh，Rust 全量 1423 个里这条又通过，期间本任务未改该用例与 gate.rs'
  root_cause: '用例夹具 queue_job 只写 started 时间戳不写 pid，而 gate::running 对没有 pid 的任务只给 started 之后 10 秒的存活窗口，所以用例在墙钟上超过 10 秒就失败；结果取决于机器快慢，不取决于产品代码'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r8-impl.md
  level: test
  proposal: '让 gate_queue_upgrade_collects_two_old_jobs_without_killing_them 的旧门禁夹具写入一个存活进程的 pid（或把时钟注入 gate::running），使这条用例在第 212 行到第 225 行之间插入 11 秒等待后仍然通过'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-06
  domain: verification
  symptom: '实现 #8 证据「未闭合问题」点名 gate_queue_upgrade_collects_two_old_jobs_without_killing_them 失败、单跑仍失败；紧接着的审计 #7 证据只有一句「变基后 cargo test --workspace 全绿（rc=0，0 失败）」，既没提这条用例，也没解释两轮结果为什么不一致'
  root_cause: '没有待审里程碑时，审计 #7 简报的「上一轮对手轮的结论」只写「没有待审的里程碑」，实现证据里的未闭合失败没有传到审计；审计跑出全绿就结案了，没有按协议「复现不了不等于不存在」给出时序上的解释。复盘 #2 后同样的事又发生一次：实现 #9 证据写明 Desktop 1 失败（SIGABRT）、Web 18 失败，`brief/audit-8.md` 与 `brief/audit-7.md` 除轮次号外逐字相同，「上一轮对手轮的结论」仍只写「没有待审的里程碑」，审计 #8 证据对这 19 个失败一字未提'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r8-audit.md
  level: brief
  proposal: '前一实现轮证据里有点名的失败用例时，下一审计轮简报的「上一轮对手轮的结论」逐条列出这些用例全名；审计证据对每一条写明本轮结果，结果不一致时写出原因或明确写「未解释」'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}

- id: L-07
  domain: verification
  symptom: '实现 #9 把分支第二次变基到 main，45 步，手工解决 store.rs、config.rs、render.rs、cli.rs、end_to_end.rs 的冲突，并在 8a36c3b 里再改 config.rs；紧接着的审计 #8 证据写「未跑负向对照与全量测试：无新改动可验」，变基后的冲突解决没有经过任何独立复验。上一次同样的变基（实现 #8）后，审计 #7 却跑了 cargo test --workspace——两次审计对同类情形处理不一致'
  root_cause: '变基轮不改里程碑表（M-01..M-05 保持「已完成」），而审计只以「有无待审里程碑」决定要不要复验；产品文件自上次审计以来是否变过，既不在简报里，也不在审计的判断条件里，于是「没有待审」被当成「没有改动」'
  evidence: docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r9-audit.md
  level: protocol
  proposal: '没有待审里程碑时，只要 docs/<slug>/ 以外的文件自上一审计轮所审提交以来有改动，审计轮就跑 `cargo test --workspace`，并在审计证据里写出所审的提交区间与结果；不得以「无新改动」跳过'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
