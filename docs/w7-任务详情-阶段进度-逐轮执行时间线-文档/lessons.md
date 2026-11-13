# T-87 W7 任务详情 · 复盘教训

本任务 Autome 没有记录到指标；下面的轮次与用例数取自状态块（design-round 1/15、implementation-round 8/30、6 个里程碑全部已完成、reopen 全为 0）与证据文件，不是指标表。

```yaml
- id: L-01
  domain: process
  symptom: 8 个实现轮里有 2 轮（#2、#8）只用来修宿主快检，两次都是同一条 ESLint 规则 preserve-caught-error（测试收尾的 AggregateError 缺 cause），r7 引入的两处与 r1 引入的一处同类
  root_cause: 实现轮 prompt 在 before_merge 模式下写「不要自己跑项目门禁命令」，实现轮把 [gate] quick（fmt/clippy/lint/typecheck）也当成禁跑；r7 没跑任何 lint，r1 只跑了显式列文件的 eslint，项目级 `npm run lint` 的结果要等提交后宿主快检才暴露，每暴露一次就多耗一个实现轮
  evidence: docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/evidence/M-06-r8-impl.md
  level: brief
  proposal: 实现轮简报写出项目配置的 [gate] quick 命令原文，实现轮结束提交前必须原样跑一次并在证据文件里记录它的退出码
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}

- id: L-02
  domain: tooling
  symptom: 实现 #1、#4、#5、#6、#7 的验收都因沙箱 `spawnSync ps EPERM` / `listen EPERM 127.0.0.1` 不全绿，5 个里程碑在实现轮一律留「开放」从未标待审，真实内核收尾、WebSocket 实时与 HTTP 403 用例全部要等审计轮（同一机器、ps 与监听可用）才第一次跑通；M-05 实现轮在最后一次完整验收之后又改了 HTTP 夹具，这处改动在实现轮没有任何通过证据
  root_cause: 实现轮与审计轮运行的 CLI 沙箱权限不同，而设计的验收契约默认实现轮能监听 localhost、能枚举进程；实现轮事先不知道自己的沙箱拒绝这两项，每轮都重新撞一次 EPERM 再写探针
  evidence: docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/evidence/M-03-r4-impl.md
  level: brief
  proposal: Autome 在实现轮简报里写明本轮 CLI 沙箱对 listen(127.0.0.1:0) 与 ps 的探针结果，探针为 EPERM 时实现轮的证据必须逐条列出因此未执行或失败的验收用例名，作为审计轮的必跑清单
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: verification
  symptom: 审计 #5 把 stop 规则放宽为「凡是 active 都可停」，纯函数 task-ops 7 条失败，真实内核矩阵 task-ops-real 仍 16 条全过，契约里「矩阵失败」一句不成立，缺口记为 B-08；评审第 1 轮的 S-01 早已指出矩阵证明范围不足，只进了 Backlog B-07
  root_cause: 真实内核矩阵在每个停点只调用表判定「不可用」的操作并断言被拒，表判定「可用」的只调了 cancel；表多放出一个操作时内核从来没有机会反驳，交叉核对只覆盖了一个方向
  evidence: docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/evidence/M-05-r6-audit.md
  level: rule
  proposal: 前端「状态 × 可用操作」表与真实内核的交叉核对测试，每个真实停点除断言不可用操作被拒外，还必须实际调用至少一个表判定可用的非 cancel 操作并断言内核接受
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 5}
```
