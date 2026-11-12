# 教训 · 费用可见（T-44）复盘 #1

指标对照：验证缺口 1 = L-01（M-02 审计）；协议失败 1 = L-03（账本 `task.protocol_failure`，审计 #2 改了 `crates/automed/tests/end_to_end.rs`）；
两次门禁修复实现轮（实现 #2、#3）在账本 sessions 表合计 8,334,491 tokens（含缓存读写，口径大于 Autome 的 total_tokens）、31 分钟，零产品改动 = L-02、L-04。
L-01、L-02、L-04 的 proposal 与先前任务的条目逐字相同（L-04 的括注「本任务为 crates/」原样保留，本任务命中的是前一个条件「拿不到失败用例全名与断言原文」），是第二次及以上出现；L-03 是首次出现。

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
  symptom: '实现 #7 的 CLI 实现与 cost_cli_ 5 个用例全部通过，但沙箱内 ci-local.sh 因 Electron SIGABRT、Web listen EPERM 失败，于是把 M-05 留在「开放」不标待审；随后 Claude 审计在同一提交上跑 ci-local.sh 九项全绿并关闭。实现 #2、#3 也在同类环境失败上各跑一次全量（#3 耗时 1061 秒）'
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
```
