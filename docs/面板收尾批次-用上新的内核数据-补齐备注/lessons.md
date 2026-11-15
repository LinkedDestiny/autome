# 复盘教训

本任务没有记录到 Autome 指标；下列计数全部数自本任务的证据文件、设计文档与简报，不是指标值。
第 2 轮复盘在第 1 轮的 L-01…L-04 之后补入合并前门禁超时（实现 #24、审计 #16）的材料，新增 L-05。
第 3 轮复盘补入第二次合并前门禁失败（宿主 df5ba1d 上 B-3 读到 0 个会话，实现 #25、审计 #17），新增 L-06。
第 4 轮复盘补入第三次合并前门禁失败（宿主 a92adbeb 上 e2e 夹具 `prepare()` 的 `w7-flow.sh` 60 s ETIMEDOUT，实现 #26、审计 #18）：
这一失败审计 #14 早已见过并按「环境限制」放行，L-06 据此改写为覆盖两次放行；L-02 计数按 26 份实现证据更新，
并把审计 #14 的 ETIMEDOUT 从「环境时有时无」里移出；新增 L-07（门禁失败后逐个修、三次往返）。
第 5 轮复盘补入第四次合并前门禁失败（宿主 da78fdf7 上 `flows.spec.ts` D-1 的「运行中」列不可见，实现 #27、审计 #19）：
这一失败不在 Backlog 或既往审计证据里，实现 #27 的因果分析指向实现 #24 把功能夹具 tick 改成 100 ms，新增 L-08；
L-02 计数按 27 份实现证据更新（22 份记了 EPERM，四次门禁修复都没在本地跑过浏览器）；L-07 计数改为四次。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现轮里有 8 个（r2、r4、r6、r12、r15、r17、r19、r21）只用来修宿主快检抓到的失败，其中 labels.test.ts「中文状态字面量」守卫在 M-01、M-03、M-08 三次重复失败，styles.test.ts 内联样式守卫在 M-02 失败一次
  root_cause: 实现轮拿到的快检命令是不带参数的 `sh scripts/ci-quick.sh`，脚本支持 `--since <rev>` 却没收到起点，7 份证据（M-01-r2、M-02-r4、M-03-r6、M-07-r12、M-08-r15、M-09-r17、M-10-r19）都记了「未提供本轮起点，未跑改动相关的测试」，本地快检只跑静态五步，相关测试要等宿主提交后才第一次运行
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-08-r15-impl.md
  level: brief
  proposal: 实现轮简报给出的快检命令带 `--since <本轮起点提交>`，实现证据里不再出现「未提供本轮起点，未跑改动相关的测试」
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: tooling
  symptom: M-07 用了 3 个实现轮、被审计 #8 退回一次，r11–r13 跑截图脚本都在启动前报 `spawnSync ps EPERM`，最后交付的 README 图是 r13 从审计轮的 `.autome/output/shots-2/` 复制来的；四次合并前门禁失败的修复（实现 #24 的 C-2 超时、#25 的 B-3 会话数、#26 的夹具 prepare 超时、#27 的 D-1 运行中不可见）在实现轮里跑 Playwright 都在 setup 的 ps EPERM 处 0 条执行，审计 #16、#17、#19 同样起不来、审计 #18 未跑浏览器，四次修复都没有经过任何一次本地浏览器运行就交回宿主门禁
  root_cause: 会话沙箱不允许 `ps` 与本地端口监听，27 份实现证据里有 22 份记了 EPERM；审计轮环境时有时无（审计 #3–#8、#12 跑通了浏览器或截图脚本，审计 #16、#17 ps EPERM），依赖进程枚举或监听的验收只能碰运气等某一轮第一次跑
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r27-impl.md
  level: protocol
  proposal: 实现轮与审计轮会话都放行 `ps` 与 127.0.0.1 监听，`npx playwright test` 在这两种轮次里都能越过 globalSetup 的进程表预检、执行到第一条用例
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: verification
  symptom: 审计 #5 在 M-05 退回：C-2 浏览器夹具第 2、3 步文档相同，第 3 步 `git commit` 无改动返回 1，任务以 session_crashed 失败，会话数停在 3
  root_cause: 夹具的多步脚本生成器只能经浏览器用例间接执行，实现轮浏览器被 EPERM 拦下，生成器的缺陷直到审计才第一次暴露；修复轮把生成器抽到 `e2e/harness/window-task.ts`，并加了不起浏览器的 `test/window-task.test.ts` 逐步执行，修复前就在 step 3 稳定变红
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r8-audit.md
  level: test
  proposal: 浏览器用例使用的、会生成多步脚本或数据的 e2e 夹具放在 `apps/web/e2e/harness/` 下，并且有一条不起浏览器的 vitest 单测逐步执行生成产物、断言每一步成功
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}

- id: L-04
  domain: verification
  symptom: M-10 设计契约说浏览器用例 BL-01 在匹配器变异下会在 5 秒处超时，审计 #12 实测两种变异下它都仍然通过，只好把契约改成以单测为判别者；M-12 的浏览器负向对照在实现轮和审计轮都跑不起来，转为人工项 H-07
  root_cause: 设计阶段把浏览器用例写成负向对照的唯一判别者，却没有人实际跑过注入；实现轮 r18 记了「浏览器负向对照因相同环境阻塞未跑」，判别力缺口要到审计才发现
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-10-r19-audit.md
  level: rule
  proposal: 设计验收契约里每条负向对照都点名一条不依赖浏览器启动的用例作为判别者，浏览器用例只标注为冒烟
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-05
  domain: verification
  symptom: 13 个里程碑全部审计通过后，合并前门禁的 Web e2e 在 29 条中跑完 27 条时触发 360 s globalTimeout（FAILED 361s），C-2、C-2b 未执行，任务回到实现 #24 与审计 #16，两轮都没能在本地测出改后的总耗时
  root_cause: 设计「风险与回退」R-1 只按新增用例估了约 +175 s，拿 360 s 去比 ci.yml 的 15 分钟与宿主 30 分钟，没有算 globalTimeout 本身的余量；它的触发条件「若审计实测整步超过 300 s」没有落到任何里程碑的验收命令上——各审计只跑本里程碑点名的 spec 子集（如审计 #6 的 manual-items+layout 19 条），整套 29 条第一次完整运行就是合并前门禁
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r24-impl.md
  level: rule
  proposal: 设计里以「审计实测 X」为触发条件的风险项，必须点名一个验收命令会执行 X 的里程碑（本例即某个里程碑的验收含不带过滤的 `npx playwright test` 并在审计证据里写出总耗时与 globalTimeout 的余量）
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-06
  domain: verification
  symptom: 合并前门禁后两次失败的位置，都是审计早已亲眼看到又放行的——审计 #4 复验 M-04 时 B-3 约 4 次中 2 次在 `controls.spec.ts:100` 读到 0 个会话，判「测试竞态、产品正确」记 BL-13 后通过，宿主 df5ba1d 门禁正是在这一行失败（实现 #25 改成 expect.poll）；审计 #14 复验 M-12 时 `npx playwright test e2e/live.spec.ts` 在 globalSetup `prepare()` 的 `bash w7-flow.sh` 60 秒 ETIMEDOUT，判「环境限制」后通过，宿主 a92adbeb 门禁正是以同一个 `spawnSync bash ETIMEDOUT` 失败（实现 #26 把这一处 60 s 对齐为其它 6 个调用方已在用的 150 s）
  root_cause: 审计把门禁会运行的步骤上的失败按「产品行为正确」或「环境限制」放行；但间歇断言失败与超时都不是会话沙箱特有的权限拒绝（EPERM），宿主门禁同样会碰上，而 Backlog 条目与审计证据里的「环境限制」备注在里程碑全部完成前不会被任何一轮处理，于是 M-04、M-12 关闭后又被门禁推翻
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-12-r22-audit.md
  level: protocol
  proposal: 审计复验中，合并前门禁会运行的用例或 setup 步骤出现断言失败或超时（含间歇出现）时，审计结论为退回并点名该步骤；只有权限拒绝（EPERM）这一类沙箱特有的失败可以记为环境限制后通过
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}

- id: L-07
  domain: process
  symptom: 13 个里程碑全部完成后，合并前门禁连续失败四次（f579963 的 360 s globalTimeout、df5ba1d 的 B-3 读到 0 个会话、a92adbeb 的夹具 prepare 60 s ETIMEDOUT、da78fdf7 的 D-1 运行中不可见），每次各花一个修复实现轮、一个集成审计和一个复盘（实现 #24–#27、审计 #16–#19、复盘第 2–5 轮）；第二、三次失败的位置在第一次失败之前已分别记在 Backlog BL-13 与审计 #14 证据里（第四次见 L-08）
  root_cause: 实现 #24 定位门禁失败时只看宿主当次报告的失败（C-2/C-2b 未执行），列的候选原因只有 tick 空等、造卡成本、C-2 会话链，没有翻本任务 Backlog 与既往审计证据里门禁路径上已知的失败；共同原则 3「一次处理同类问题」被落成「同一个用例」而不是「同一个门禁」，于是同一门禁上的已知问题要等宿主一个一个报出来
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r24-impl.md
  level: brief
  proposal: 合并前门禁失败后的第一个修复实现轮，在证据里列出本任务 Backlog 与既往审计证据中所有涉及门禁会运行的用例或步骤的失败记录（本例应列出 BL-13 与审计 #14 的 60 s ETIMEDOUT），并逐条修复或写明不修的理由
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-08
  domain: verification
  symptom: 合并前门禁第四次失败在 `flows.spec.ts` D-1 的 `expect(inColumn('running')).toBeVisible()`（宿主 da78fdf7：19 通过、1 失败、9 未执行）；这条用例在本任务开工前就有，本任务里没有任何审计跑过它，失败前也不在 Backlog 里；实现 #27 的因果分析是功能实例 tick 100 ms、页面重验合并窗口 1000 ms，需求前几步写完文档立即结束，可能在浏览器再次取数前就从运行中走到等批准，而 100 ms 是实现 #24 为修第一次门禁超时把 `setup.ts` 普通功能实例从 1000 ms 改出来的
  root_cause: 实现 #24 把共享夹具的调度间隔一刀改快，只核对了耗时与防重入，没有列出哪些浏览器用例要看到某个中间状态；审计 #16 以「规格内无依赖 1000 ms tick 的断言（grep 无命中）」判定没有影响，但这种依赖不以字面数值出现，而是落在「中间状态的停留时间要长过页面重验间隔」这个时序关系上（实现 #27 自己也注明浏览器受阻，不能排除宿主上还有其它原因）
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r27-impl.md
  level: rule
  proposal: 修改 e2e 共享夹具的 tick、超时等时序参数的实现轮，在证据里列出断言中间状态可见（如运行中、排队中列）的全部浏览器用例，并逐条写明新时序下该状态的停留时间与页面重验间隔的比较；审计不得只凭 grep 旧数值无命中判定没有影响
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
