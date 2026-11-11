# 教训 · Bug 修复 Loop：定位、复现检查、修复、门禁、复核（K2）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 13 份证据、审计与裁决记录、`brief/audit-3.md`，以及任务分支的 `git log 81a4394..HEAD`。

设计 1 轮定稿；M-01..M-06 都是一次审计关闭，reopen 0。异常集中在 M-03：实现 #3 标待审后宿主门禁报 Test 失败，多花一个实现轮（#4）只改一行测试输入（L-01、L-02）；随后审计 #3 把证据写错文件名，Autome 事后改名，01:03 到 09:50 之间没有会话（L-03）。另有一条 Autome 指标与状态块不一致，与前两次任务的同类教训按原文复用 proposal，以便跨任务计数（L-04）。

```yaml
- id: L-01
  domain: verification
  symptom: '实现 #3 证据写「本轮未执行 workspace 全量，不作全量通过声明」后把 M-03 标待审；宿主门禁在 8683af4 上报 Test 失败（autome-cli 的 task_card_reply_rejects_the_design_with_the_text_verbatim，Null != await_design_approval），于是派出实现 #4，它只把该用例的输入集合从 ["feature","bug"] 改成 ["feature"]，新增与删除的测试函数都是 0'
  root_cause: 'M-03 验收契约只跑 end_to_end、scheduler::tests、task_card 与一条点名的 CLI 用例，不含 cargo test --workspace；全量只排在 M-05「全量门禁」里程碑，所以 M-03 的调度器改动打破 autome-cli 测试时，实现轮自己的验收看不到'
  evidence: 'docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-03-r4-impl.md'
  level: rule
  proposal: '凡改动产品代码的里程碑，其验收契约的验收命令必须包含 `cargo test --workspace`，不得把全量运行只放在单独的「全量门禁」里程碑'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: '定稿设计（258fae2）§11 第 5 条把 task_card_reply_rejects_the_design_with_the_text_verbatim 去掉 bug 分支标为（M-04），但让这条断言失效的是 M-03 的「新 Bug 进 Locate」；评审 #1 与裁决 #1 都没指出，实现 #4 才就地改成「须与 M-03 启用新 Bug 路径配套」'
  root_cause: '§11 第 5 条的里程碑是按「替代用例在哪个里程碑新增」（M-04 的 bug_loop_reply_returns_a_bug_to_locate_with_the_text_verbatim）定的，而不是按「哪个里程碑的产品改动让旧断言变红」定的'
  evidence: 'docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-03-r4-impl.md'
  level: rule
  proposal: '设计的「既有测试改动清单」每条标注的里程碑，必须是让该旧断言失效的那项产品改动所在的里程碑，替代用例可以晚到，但对旧断言的改动不得晚于它'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: protocol-format
  symptom: '审计 #3 复验的是实现 #4，却把证据写成 evidence/M-03-r3-audit.md（标题「M-03 审计复验（第 3 轮）」），设计文档指针和 -audit.md 也指向 r3；Autome 在 09:50 的提交 b711d23 才改名为 M-03-r4-audit.md，01:03 到 09:50 之间没有任何会话提交'
  root_cause: '协议规定证据名用实现轮号 k、审计轮不增加 k，但审计简报只给出实现证据路径，没给出本轮该写的审计证据路径；M-03 之前审计序号与 k 恰好相等（#1→r1、#2→r2），审计 #3 是第一次两者不相等的地方，会话按「第 3 轮」命名'
  evidence: 'docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-03-r4-audit.md'
  level: brief
  proposal: '审计轮简报必须写出本轮审计证据文件的完整目标路径 `evidence/M-xx-r<k>-audit.md`，其中 k 等于被复验的实现证据文件名里的 k'
  predicted_impact: {metric: protocol_failures, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '复盘轮收到的 Autome 指标是「实现轮 4/30」，而设计文档状态块终值是 implementation-round: 7/30，retro.md 记有实现 #1..#7；4/30 正是 01:03 那次提交（838723b，审计 #3）之后状态块的值。同一组指标里「reopen 合计 0」，「reopen 按领域」却给六个领域各 ×1，与里程碑表的领域列一一对应'
  root_cause: '指标里的实现轮数没有在任务结束时按状态块重新读取，停在了中途某一刻的值；按领域的 reopen 统计把领域列本身当成了计数'
  evidence: 'docs/按计划-4-2-节实现-bug-修复-loop/retro.md'
  level: test
  proposal: 复盘轮 prompt 里的设计轮、实现轮与里程碑数必须等于启动复盘时设计文档状态块的 design-round、implementation-round 与里程碑表行数，由一条 automed 测试在「指标列已有旧值」的前提下断言这一点
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```
