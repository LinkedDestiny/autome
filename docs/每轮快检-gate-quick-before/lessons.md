# 教训 · 每轮快检（`[gate] quick`，T-67）

复盘轮 #1 覆盖写。依据：任务文件、任务卡、`retro.md`、`evidence/` 下 10 份证据、评审与裁决记录、审计记录、任务分支
`git log`，以及本 worktree 与主检出 `.autome/config.toml` 的只读查看。Autome 本任务没有记录到指标，下文不引用任何指标
数值；文中的数字都取自证据文件或 `git` 的实际输出。

可核对的运行事实：设计 1 轮（评审 #1 提出 2 项阻塞意见 R1-01、R1-02，裁决 #1 全部采纳并定稿），实现 5 轮，M-01..M-05
各一次审计通过，没有 reopen，审计记录没有实现缺陷或验证缺口。宿主门禁在本任务里还没跑过（项目是 `before_merge`，
retro 时尚未到合并前门禁）；5 个实现轮都在证据里记了 `cargo fmt --check` 与 `cargo clippy -D warnings` 的退出码 0。
人工验收 H-01、H-02 未确认。

本次运行没有退回，所以下面四条都不是「哪个里程碑被退回」的教训，而是：一条设计评审拦下的同类错误（与 T-60 L-03 同源）、
一条负向对照空转（与 T-61 L-02 同源，措辞照抄以便合并）、一条 H-01 落地时会踩的环境坑、一条协议里用例数对照口径的问题。

```yaml
- id: L-01
  domain: design
  symptom: '评审 #1 R1-02（裁决 D1-P02 采纳）：设计初稿让作业按启动时的 kind 收结果，却让 Bug 复核的宿主门禁说明 {fix_gate_note} 只出现在按当前配置选出的 before_merge 分段里，且非 quick 结果一律回落成固定句「修复门禁只跑了复现命令」。评审给出三条可由设计推出的错误路径：快检在途切 each_round，复核丢掉说明；each_round 的完整 FixGate 在途切到 before_merge + quick，复核被告知「只跑了复现命令」；需求快检在途切 each_round，审计简报仍写「全量门禁只在合并前运行」'
  root_cause: '设计把「这次实际跑了什么」（应取自启动时落盘的 kind / command / exit_code）与「当前配置规定以后跑什么」（每个 tick 重新解析）放进了同一个由当前配置选择的模板分段和固定句子里。这与 T-60 L-03 同源：那次是 gate.result.command 在收结果的 tick 取当时配置，这次是陈述文字的有无与措辞由当前配置决定'
  evidence: docs/每轮快检-gate-quick-before/每轮快检-gate-quick-before-review.md
  level: rule
  proposal: '设计里凡向人或会话陈述「实际执行了什么」的字段或文字（账本字段、简报段落、prompt 分段与补充句），必须写明它取自作业启动时的记录还是当前配置；取自当前配置的只能陈述以后的策略，不得陈述已发生的运行'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '审计 #3 第一次负向对照用 BSD sed 注入「禁用 FixGate 快检分支」，改动实际没落到文件上，4 个用例全过；审计发现后改用 python 重做，lib 3 个、e2e 2 个才变红。这是同类空转第三次出现（T-61 审计 #5、文档补齐-bug-loop 审计 #1 都是 BSD sed 静默不改）'
  root_cause: 'macOS 的 BSD sed 与 GNU sed 的 -i 及地址语法不同，不匹配时静默不改、退出 0；负向对照只看「测试红没红」，不先确认注入已落盘。本次靠审计对「全过」起疑才没记成「验收命令拦不住」'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-03-r3-audit.md
  level: rule
  proposal: '每条负向对照在跑验收命令之前，证据里必须记录注入后 git diff --stat 的输出，且被注入的文件出现在输出中；没有这条记录的对照不得写成「拦得住」或「拦不住」'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '实现 #5 实测任务卡给本仓库拟的快检命令时，第一次在 npm run lint --prefix apps/web 处退出 127（eslint not found），手动 npm ci --prefix apps/web 之后才跑通（整条 6.00 秒）。人工验收 H-01 只要求在主检出 [gate] 加上这条 quick'
  root_cause: '新建的任务 worktree 没有 apps/web/node_modules。完整门禁 scripts/ci-local.sh 在 node_modules 不存在时自己跑 npm ci，所以今天看不出问题；快检命令没有这一步。本 worktree 与主检出的 .autome/config.toml 都没有 [setup]（只读查看），宿主在快检前不会装依赖，于是照 H-01 原样启用后，每个新任务第一个实现轮的快检都会以 127 失败并退回实现轮'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-05-r5-impl.md
  level: rule
  proposal: '主检出 [gate] 启用含 npm 步骤的 quick 时，同一次改动必须让新建 worktree 在快检前具备 apps/web/node_modules（加 [setup] command = "npm ci --prefix apps/web"，或快检命令自带 node_modules 缺失时的安装）；H-01 的确认以一个新建任务第一个实现轮的快检退出码为 0 为准'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: '实现 #2..#5 按协议报「用例总数与上一轮基线的差」，四轮都是跨里程碑比较，四轮都得花一段解释差值不是丢用例：#2 写 -2、#3 写 -15、#4 写 +44、#5 写 -42，每次都说「验收范围换了」。真正有用的是各轮另外自发实跑的同一验收命令改前基线（#5：CLI 9 → 11，桌面 0 → 3；#4：43 → 56）'
  root_cause: 'loop-protocol「验收结果报什么」要求与「上一轮基线」比，而上一轮通常是另一个里程碑、另一组验收命令；两组命令的用例数之差既不能发现整层被跳过，也不能证明新增了多少，只有同一命令改前改后的差才能'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-05-r5-impl.md
  level: protocol
  proposal: '实现轮报验收用例总数时，基线是同一条验收命令在本轮改动前实跑的结果（证据里写出该命令与改前、改后两个数）；与上一里程碑的用例总数比较不作要求'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```
