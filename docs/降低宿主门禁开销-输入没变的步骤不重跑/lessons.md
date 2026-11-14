# 教训 · 降低宿主门禁开销-输入没变的步骤不重跑

本任务 Autome 没有记录指标；以下只引用证据文件里的事实，不含估算数字。
运行概况（取自 `retro.md` 与设计文档状态块）：设计 1 轮定稿；实现 9 轮；M-01…M-05 各一轮通过；
M-06 用了 r6–r9 四轮，审计退回 3 次（分组配置、内联样式、夹具清理竞态）。

```yaml
- id: L-01
  domain: tooling
  symptom: M-06 的实现 #6–#9 四轮都报「listen EPERM 127.0.0.1」和「spawnSync ps EPERM」，没跑过一次完整 Web 全量，也没标过待审；全量连跑和残留检查都只在审计轮做成，ENOTEMPTY 偶发（8 次全量里 2 次红）也是审计 #8 才发现
  root_cause: 实现会话的沙箱不让监听本机端口、不让执行 ps，审计会话可以；实现轮开工前不知道自己缺这两项权限，每轮都要重新探测，探测完只能写「保持开放」，验收需要的全量证据只能等审计轮补
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r9-impl.md
  level: brief
  proposal: 实现轮简报写明本会话能否监听 127.0.0.1、能否执行 ps；写「不能」时，验收需要这两项的里程碑由实现轮在证据里列出交给审计轮执行的全量命令，并照常标待审，而不是标「保持开放」
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 审计 #7 因 styles.test.ts「TSX 没有内联样式」失败把 M-06 退回；违规的 task-timeline.tsx:10 是 M-03 提交 1349131e 带进来的，M-03 审计只跑了 task-timeline.test.ts，就让它通过了；实现 #6 已经看到这条失败，但以「HEAD 已有」为由没处理，审计 #6 也没提
  root_cause: 每个里程碑的验收命令只选自己相关的测试文件，覆盖全仓的守卫测试（styles.test.ts）不在任何里程碑的验收里；宿主快检只跑 lint/typecheck，也不跑这一条，所以问题拖到 3 个里程碑之后才被算成缺陷
  evidence: docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r7-audit.md
  level: rule
  proposal: 改了 apps/web/app 下文件的里程碑，实现轮与审计轮都要跑 `cd apps/web && npx vitest run --project unit` 并在证据里记下文件数与用例数；看到已有的测试失败时，同一轮就要写进 Backlog 或判为缺陷并注明引入它的提交
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
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

提交：本轮 `git add` + `git commit`；如果被拒绝，由 Autome 兜底提交。
