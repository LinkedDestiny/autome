# 教训 · 单个会话的时长上限

本任务没有记录到指标；以下条目只引用证据文件与提交记录，不含数字推测。

```yaml
- id: L-01
  domain: design
  symptom: 评审 #2 的唯一阻塞 D2-P01 是裁决 #1 修订时新加的前置条件「只在已有退出标记的那一次 tick 查 session.timeout 事件」，它让「有超时事件、无退出标记、classify 已判 Vanished」的输入走不到超时恢复分支，连带违反 C-5 与 R-5，多耗一轮评审加一轮裁决
  root_cause: 裁决 #1 修订终止确认时只核对了评审给的反例（有 130 标记、组内有残留），没有列出新前置条件为假时 classify 的其余结果（Vanished）各自落到哪条路径
  evidence: docs/单个会话的时长上限-loop-session/单个会话的时长上限-loop-session-review.md
  level: brief
  proposal: 裁决轮给已有分支新增或收紧触发条件时，修订落点里必须逐一列出该条件为假的每种输入（至少覆盖相关纯函数的全部返回值）以及它们各自进入的路径
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: M-02 验收契约用例 ⑨ 要求 next_round 不计超时行，但这项行为在里程碑表里排在 M-03；实现 #2 只能提前实现并在证据里写「契约交叉」
  root_cause: 设计轮写每个里程碑的验收用例时，没有核对用例依赖的行为是否全部落在本里程碑或更早的里程碑里
  evidence: docs/单个会话的时长上限-loop-session/evidence/M-02-r2-impl.md
  level: protocol
  proposal: 设计定稿前，每个里程碑验收契约里的每条用例都要标出它依赖的方案小节，并且这些小节必须分配给本里程碑或更早的里程碑
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: 审计 #2 的负向对照 E/F（恢复路径不终止进程组）没有让断言失败，而是让 ⑨ 卡在 waiter.join 上，最后由人手动终止；audit 简报提交在 13:00，「M-02 审计通过」提交在 18:00（见 git log）
  root_cause: 新增的进程组测试用不设时限的 waiter.join 等待被测进程退出，负向对照命令也没有外层时限，注入的缺陷就表现为挂起而不是失败；墙钟时长不在 Autome 记录的指标里，只能从提交时间看出来
  evidence: docs/单个会话的时长上限-loop-session/evidence/M-02-r2-audit.md
  level: rule
  proposal: 等待子进程或进程组退出的测试必须带有上限的轮询或超时，审计运行负向对照命令时必须加外层时限（例如 `timeout 600 cargo test ...`），超时记为该负向对照「被拦下」
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: 实现 #3 新增的两条 e2e 首跑都红在等待 tick 日志超时，排查后发现 wrapper 的 renderer 实参是 current_exe()，而库集成测试里它指向测试 harness，所以输出 SIGPIPE、wrapper 以 141 退出，根本没走到超时条件
  root_cause: 夹具沿用了生产代码里取自身路径的写法，但在 cargo 集成测试里 current_exe() 并不是 automed；最后改成 env!("CARGO_BIN_EXE_automed")（crates/automed/tests/end_to_end.rs 新增尾部）
  evidence: docs/单个会话的时长上限-loop-session/evidence/M-03-r3-impl.md
  level: test
  proposal: automed 集成测试里凡是需要 automed 可执行文件的夹具（wrapper 的 renderer 等）都必须用 env!("CARGO_BIN_EXE_automed")，不得经过 std::env::current_exe()
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-05
  domain: process
  symptom: 四个里程碑全部已完成后，实现 #5 解决 rebase 冲突（crates/automed/tests/end_to_end.rs 与 prompt_rendering.rs 末尾追加冲突），只跑了与冲突文件相关的 54 条用例，证据写明「未运行全量门禁、宿主快检」；审计 #5 因没有待审里程碑未做任何复验，rebase 后提交的全量结果只来自宿主快检在 17b1f4ee 的退出码 0（见 evidence/M-04-r5-audit.md）
  root_cause: 审计轮只复验「待审」里程碑，关闭后的 rebase 冲突修复既不重开也不转待审，协议里没有哪一轮负责对 rebase 后的提交跑全量门禁，结果全靠宿主快检兜底
  evidence: docs/单个会话的时长上限-loop-session/evidence/M-03-r5-impl.md
  level: protocol
  proposal: 里程碑全部已完成后仍改动被跟踪文件（包括解决 rebase 冲突）的实现轮，必须在证据里写出在改动后 HEAD 上运行项目全量测试门禁的命令和退出码
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: project, horizon: 5}
```
