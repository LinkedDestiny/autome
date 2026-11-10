# 教训 · 用户按合并时如果默认分支已经前进-任务自动（K0b）

复盘轮 #1 覆盖写。依据：`retro.md`、`evidence/` 下 12 份证据、审计与裁决记录、评审记录中的 N1。Autome 本任务没有记录到指标，下文不引用任何指标数值。

设计 1 轮定稿，六个里程碑都是一次审计关闭，所有审计都写「无实现缺陷、无验证缺口」。可带走的东西不在缺陷里，而在**设计阶段写下的预测与真实行为不符**：契约里负向对照「由哪条测试拦住」猜错两次（L-01），评审已指出的 Git 行为疑问没有当场实验而多出一个里程碑（L-02），契约断言的中间节点与错误分类和源码调用顺序不符、实现轮当场返工四次（L-03）。另有一条与 `在-apps-web-建-react` L-06 同源的门禁抖动（L-04）。

```yaml
- id: L-01
  domain: verification
  symptom: '验收契约两次把负向对照的拦截者写错：M-03 契约说授权位不清除时 E3 会直接合并到 done，审计 #3 实测 E3 仍通过、只有 S6 拦住；M-04 契约说删掉 merge_all 的已并入跳过会让 E5 出现第 2 个合并提交，审计 #4 实测 E5 仍通过（Git 返回 already-up-to-date）'
  root_cause: '契约里的「变异 → 哪条测试变红」是设计轮推理出来的；实现 #3 只对自己挑的 S6 跑了该变异，实现 #4 把 E5 的变异留给 M-06，两处都没有对契约点名的那条测试实跑，偏差直到审计才暴露'
  evidence: 'docs/用户按合并时如果默认分支已经前进-任务自动/evidence/M-03-r3-audit.md'
  level: rule
  proposal: '实现轮对验收契约中的每条负向对照，必须对契约点名的那条测试实跑该变异并在证据里写出它的退出码；点名的测试没有变红时，在同一轮改写契约里的这条表述'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: '评审 #1 的 N1 已经指出「已并入仓 rebase 后与 base 相同，Git 可能 already-up-to-date 而不新增提交」；裁决 D1-P04 以「验证可以更严格，非六类」把它放进 B-03，用户纳入后为它单开 M-06，实现 #6 与审计 #6 一整对轮次只为证实 N1 成立并换一个有效变异'
  root_cause: '这是一个几分钟内就能用真实 Git 验证的事实问题，裁决轮按「非阻塞建议」处理，只凭推理归类，没有做最小实验，于是事实确认被推迟到一个独立里程碑'
  evidence: 'docs/用户按合并时如果默认分支已经前进-任务自动/用户按合并时如果默认分支已经前进-任务自动-adjudication.md'
  level: brief
  proposal: '评审意见质疑 Git 等外部工具的某个具体行为（如某操作是否产生新提交）时，裁决轮必须跑一次最小实验，把命令与输出写进该条裁决，然后再决定采纳、驳回或进 Backlog'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: design
  symptom: '契约断言的中间状态或错误分类与真实代码路径不符，实现轮首跑即红、当场返工四次：E2 假定 RPC task.merge 返回时停在 merging，实际已到 cleanup（M-02）；S1 用空 base 触发读盘错误，实际先被 NotOnDefault 前置条件挡回（M-02）；G1 预期 I/O 错误，实际按前置条件顺序先返回 NotOnDefault（M-01）；E6 要求清理后仍有 rebase_first，实际分支已清理、available=false（M-05）'
  root_cause: '设计轮按对代码的印象写断言，没有对照决定结果的源码：dispatch.rs 的 task_trigger 在 apply_trigger 之后立即 tick（dispatch.rs:1745-1747），git.rs 的合并前检查先判 NotOnDefault（git.rs:769）、后做 is_ancestor（git.rs:774）；实现轮只能边跑边改契约'
  evidence: 'docs/用户按合并时如果默认分支已经前进-任务自动/evidence/M-02-r2-impl.md'
  level: rule
  proposal: '设计契约断言某次调用返回后的节点、或某个异常输入得到的错误分类时，必须在该断言旁写出决定这一结果的源码位置（文件:行）'
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: '审计 #5 首次全量运行时 `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 失败一次，审计记为「5 秒等待的时序抖动」后用 --no-fail-fast 重跑转绿；设计文档 Backlog 没有为它记一条。同一用例在 `在-apps-web-建-react` 里已被定位为「wrapper 先发布退出标记再删 PID」并专门花一轮修复（该任务 L-06），本分支的 gate.rs 仍是原样'
  root_cause: '与本任务改动无关的测试失败只写在当轮证据里，并且只给了现象性的归因，没有进入任何跨轮或跨任务会读的位置，下一次失败又要从零判断'
  evidence: 'docs/用户按合并时如果默认分支已经前进-任务自动/evidence/M-05-r5-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
