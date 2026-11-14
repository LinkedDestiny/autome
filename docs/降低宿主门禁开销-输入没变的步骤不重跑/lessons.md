# 教训 · 降低宿主门禁开销-输入没变的步骤不重跑

本任务 Autome 没有记录指标；以下只引用证据文件里的事实，不含估算数字。
运行概况（取自 `retro.md` 与设计文档状态块）：设计 1 轮定稿（评审 #1 退回，裁决后定稿）；实现 11 轮、审计 10 次；
M-01…M-05 各一轮通过；M-06 用了实现 #6–#9，审计退回 3 次（分组配置、内联样式、夹具清理竞态），审计 #9 关闭。
第 1 次复盘之后，宿主的合并前门禁在 0e2e2054 上又失败了 2 处（setup 作业文件残留、web-groups 彩色输出），
实现 #10 修复；之后提交后快检又在 cargo fmt 失败，实现 #11 修复，审计 #10 通过。
里程碑表里 M-03 的 reopen 是 0，但下面 L-02 的两处回归都是 M-03 引入的。

```yaml
- id: L-01
  domain: tooling
  symptom: M-06 的实现 #6–#9 四轮都报「listen EPERM 127.0.0.1」和「spawnSync ps EPERM」，没跑过一次完整 Web 全量，也没标过待审；全量连跑和残留检查都只在审计轮做成，ENOTEMPTY 偶发（8 次全量里 2 次红）也是审计 #8 才发现；实现 #10 再次尝试 ps，仍被拒绝
  root_cause: 实现会话的沙箱不让监听本机端口、不让执行 ps，审计会话可以；实现轮开工前不知道自己缺这两项权限，每轮都要重新探测，探测完只能写「保持开放」，验收需要的全量证据只能等审计轮补
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r9-impl.md
  level: brief
  proposal: 实现轮简报写明本会话能否监听 127.0.0.1、能否执行 ps；写「不能」时，验收需要这两项的里程碑由实现轮在证据里列出交给审计轮执行的全量命令，并照常标待审，而不是标「保持开放」
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: M-03（提交 1349131e）引入了两处回归，都在 M-03 所选验收子集之外的已有测试里，审计 #3 都没看到——一是 task-timeline.tsx:10 的内联样式，styles.test.ts 失败，3 个里程碑后审计 #7 才据此把 M-06 退回；二是 gate::start 新增 .passes/.report 作业文件，setup 的手工收尾没删，已有的 end_to_end 用例 setup_run_in_flight_is_collected_under_its_own_fingerprint_when_the_watched_file_changes 失败，6 个里程碑全部关闭后才由宿主合并前门禁发现，另花了实现 #10、#11 和审计 #10
  root_cause: 每个里程碑的实现轮和审计轮都只跑按测试名过滤的子集（M-03 跑的是 end_to_end gate_steps_、--lib gate_report、gate::tests::、task-timeline.test.ts）；整个运行中没有一轮跑过不带过滤的 cargo test -p automed，宿主门禁又只在合并前跑一次，所以改了共享代码后，同一测试目标里其它已有用例坏了，要到合并时才暴露
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r10-impl.md
  level: rule
  proposal: 改了 crates/<crate>/src 或 apps/web/app 下文件的里程碑，审计轮除验收子集外还要跑一次不带名称过滤的 `cargo test -p <crate>`（或 `cd apps/web && npx vitest run --project unit`），在证据里记下通过数与失败数；遇到已有用例失败时，要么判为缺陷并注明引入它的提交，要么写进 Backlog，不得以「HEAD 已有」略过
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: 审计 #6 跑 `npm run test` 时 Vitest 5.1.0 报「kernel 与 unit 的 maxWorkers 不同而 groupOrder 相同」，返回 no tests，M-06 被退回；实现 #6 只跑了 `--project unit` 和 4 文件子集，分组守卫 test-groups 4 项全绿
  root_cause: 分组守卫只检查配置对象（成员、worker 数、groupOrder），从没真正用这份配置启动过运行器；调度器在启动时才做的约束检查，配置对象层面看不到
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r6-audit.md
  level: rule
  proposal: 改了测试运行器配置（vitest.config、project 分组、groupOrder、maxWorkers）的轮次，必须用不加过滤参数的入口命令（如 `npm run test`）真实启动一次，或跑一条覆盖全部 project 的真实启动回归（如 scripts/test/web-groups.test.mjs），并在证据里记下运行器报告的 project 名和文件数
  predicted_impact: {metric: impl_defects, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: 审计 #8 在 8 次新配置全量里看到 2 次 ENOTEMPTY 失败；修复后审计 #9 拿「连续 5 次全绿」关闭 M-06，同时在自己的证据里承认，对约 25% 的失败率来说 5 次只是一般强度的证据；写入者没定位（BL-03），20+20 次诊断也没做
  root_cause: 设计里的「连续 5 次全绿」是固定次数，没按已观测到的失败率换算；失败率 2/8 时，即使问题没修好，连过 5 次的概率也约有 0.75^5≈0.24，这个验收区分不了修好和没修好
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r9-audit.md
  level: rule
  proposal: 偶发失败的修复要按观测失败率 p 定重复次数 n，满足 (1-p)^n ≤ 0.05（p=2/8 时 n≥11）；如果 n 达不到，要有一条在旧实现上能复现原报错、在新实现上不复现的确定性回归，否则不得据此关闭里程碑
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 5}
```

```yaml
- id: L-05
  domain: verification
  symptom: L-03 之后加的真实启动回归 scripts/test/web-groups.test.mjs，在实现 #7–#9 和审计 #7–#9 各轮都通过，却在宿主合并前门禁上失败；实现 #10 查明会话环境带 NO_COLOR=1，Vitest 打出无色标签 `|unit|`，宿主没有 NO_COLOR，打出的是彩色背景标签，断言匹配不上；会话里只有 `env -u NO_COLOR FORCE_COLOR=1` 才能复现
  root_cause: 测试解析子进程（Vitest）的文本输出，但 spawn 时继承了调用者的颜色环境变量；所有 Agent 会话恰好都是同一种取值，所以没有哪一轮能看到另一种输出格式，差异只在环境不同的宿主上出现
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r10-impl.md
  level: rule
  proposal: 断言子进程（测试运行器、CLI）文本输出的测试，spawn 时必须显式设定 NO_COLOR 与 FORCE_COLOR，不得原样继承 process.env 里的这两个变量；审计轮查 diff 时，对新增的这类测试逐个核对 spawn 的 env 参数
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 5}
```

```yaml
- id: L-06
  domain: implementation
  symptom: 实现 #10 改了 crates/automed/tests/end_to_end.rs，没有执行 cargo fmt，证据写「未运行完整门禁或宿主快检组合」；提交后快检第一步 `cargo fmt --all -- --check` 失败，只因一处 setup_fixture 调用需要换行，为此又花了实现 #11 和审计 #10 两个会话
  root_cause: 「快检交给宿主」被理解成连格式化也不做；M-03、M-05 的实现轮改 .rs 后都跑过 cargo fmt / rustfmt，修复轮的实现 #10 没跑，格式问题只能等宿主快检来拦
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r11-impl.md
  level: brief
  proposal: 改了任何 .rs 文件的实现轮（含修复轮），提交前执行 `cargo fmt --all`，并在证据里写一行它的退出码；这只是格式化，不等于替宿主跑快检
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

提交：本轮 `git add` + `git commit`；如果被拒绝，由 Autome 兜底提交。
