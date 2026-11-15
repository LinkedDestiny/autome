# 复盘教训

本任务没有记录到 Autome 指标；下列计数全部数自本任务的证据文件与设计文档里程碑表，不是指标值。

```yaml
- id: L-01
  domain: tooling
  symptom: 23 个实现轮里有 8 个（r2、r4、r6、r12、r15、r17、r19、r21）只用来修宿主快检抓到的失败，其中 labels.test.ts「中文状态字面量」守卫在 M-01、M-03、M-08 三次重复失败，styles.test.ts 内联样式守卫在 M-02 失败一次
  root_cause: 实现轮拿到的快检命令是不带参数的 `sh scripts/ci-quick.sh`，脚本支持 `--since <rev>` 却没收到起点，7 份证据都记了「未提供本轮起点，未跑改动相关的测试」，本地快检只跑静态五步，相关测试要等宿主提交后才第一次运行
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-08-r15-impl.md
  level: brief
  proposal: 实现轮简报给出的快检命令带 `--since <本轮起点提交>`，实现证据里不再出现「未提供本轮起点，未跑改动相关的测试」
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: tooling
  symptom: M-07 用了 3 个实现轮、被审计 #8 退回一次；实现轮 r11、r12、r13 跑截图脚本都在启动前报 `spawnSync ps EPERM`，最后交付的三张 README 图是 r13 从审计轮生成的 `.autome/output/shots-2/` 复制过来的
  root_cause: 实现轮会话的沙箱不允许 `ps` 和本地端口监听，审计轮环境却允许（审计 #8 原话「脚本在本环境可运行」）；23 份实现证据里有 18 份记了 EPERM，依赖进程枚举或监听的验收只能留给审计轮第一次跑
  evidence: docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-07-r13-impl.md
  level: protocol
  proposal: 实现轮会话环境与审计轮一致，放行 `ps` 与 127.0.0.1 监听，`node apps/web/scripts/screenshots.ts` 与 `npx playwright test` 在实现轮能越过启动预检、执行到第一条用例
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}

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
```
