# 复盘教训

本任务没有记录到 Autome 指标；下列计数全部数自本任务的证据文件、设计文档与简报，不是指标值。
第 2 轮复盘在第 1 轮的 L-01…L-04 之后补入合并前门禁失败（实现 #24、审计 #16）的材料：L-02 计数与根因按新证据更新，新增 L-05。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现轮里有 8 个（r2、r4、r6、r12、r15、r17、r19、r21）只用来修宿主快检抓到的失败，其中 labels.test.ts「中文状态字面量」守卫在 M-01、M-03、M-08 三次重复失败，styles.test.ts 内联样式守卫在 M-02 失败一次
  root_cause: 实现轮拿到的快检命令是不带参数的 `sh scripts/ci-quick.sh`，脚本支持 `--since <rev>` 却没收到起点，7 份证据都记了「未提供本轮起点，未跑改动相关的测试」，本地快检只跑静态五步，相关测试要等宿主提交后才第一次运行
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-08-r15-impl.md
  level: brief
  proposal: 实现轮简报给出的快检命令带 `--since <本轮起点提交>`，实现证据里不再出现「未提供本轮起点，未跑改动相关的测试」
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: tooling
  symptom: M-07 用了 3 个实现轮、被审计 #8 退回一次，r11–r13 跑截图脚本都在启动前报 `spawnSync ps EPERM`，最后交付的 README 图是 r13 从审计轮的 `.autome/output/shots-2/` 复制来的；实现 #24 修合并前门禁的 Web e2e 超时，实现轮与审计 #16 跑 Playwright 都在 setup 的 ps EPERM 处 0 条执行，修复没有经过任何一次本地浏览器运行就交回宿主门禁
  root_cause: 会话沙箱不允许 `ps` 与本地端口监听，24 份实现证据里有 19 份记了 EPERM；审计轮环境时有时无（审计 #3–#8、#12 跑通了浏览器或截图脚本，审计 #14 在 setup 里 ETIMEDOUT，审计 #16 ps EPERM），依赖进程枚举或监听的验收只能碰运气等某一轮第一次跑
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r24-audit.md
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
```
