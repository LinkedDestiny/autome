# 教训 · 面板交付批次-w10-启动与安装

Autome 没有为本任务记录指标。下面的计数全部取自 `retro.md` 的轮次表和所引的证据文件，没有估算值。本文件是复盘第 2 轮覆盖写的：第 1 轮之后又跑了实现 #23（rebase）和审计 #22（集成复验），L-01 的计数已按 23 个实现轮更新，L-04 是新增的一条。

```yaml
- id: L-01
  domain: tooling
  symptom: 本任务 8 次审计退回（M-01×1、M-03×1、M-07×2、M-09×2、M-10×2），全部落在实现轮因 spawnSync ps 或 listen 127.0.0.1 EPERM 跑不成验收的里程碑上；23 个实现轮证据文件里有 18 个出现 EPERM。实现轮能跑完验收的 M-04、M-06、M-12、M-13 都是一次审计通过。M-01 审计 #1 退回的缺陷（web-stop-gate 测试在门禁进程出现前就取 pid）要到审计本机实跑才暴露。
  root_cause: 实现轮的沙箱禁止 ps 和本地监听，审计轮却允许，所以测试和夹具的缺陷只能等审计轮第一次实跑时暴露，每暴露一层就多一次退回。任务文件 §9 第 1 条已经记下 W7、W9 有同样的现象；这次照其对策「如实记录、交审计轮复验」做了，退回次数并没有减少。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-01-r1-audit.md
  level: rule
  proposal: 实现轮会话与审计轮使用相同的沙箱放行（允许 ps 和 127.0.0.1 监听），检验标准是实现轮证据中不再出现 `spawnSync ps EPERM` 或 `listen EPERM`。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: M-07 和 M-09 各因同一条断言连续退回两次。审计 #9 只报了横幅读到空串，并写「需实现轮确认」；实现 #10 没法实跑，只加了等待，沿用设计里的子串「内核比账本旧」，审计 #10 再次退回。审计 #13 报 data-scroll-region 的实际值是 "true"；实现 #15 只把期望值改成 "true"，审计 #14 再次退回，原因是 `.task-docs-body` 在默认时间线标签下根本不存在。
  root_cause: 实现轮跑不了浏览器和 Electron，只能照着审计给的失败现象改断言；而审计退回时只附了失败输出，没有附自己跑绿过的正确判据。审计 #10 给出真实横幅文本和 a10-probe.sh、审计 #14 给出已跑绿的 a16-docs-tab.spec.ts 之后，下一轮审计都是一次通过。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-09-r15-audit.md
  level: protocol
  proposal: 如果被审实现轮的证据记了验收被环境阻塞，审计退回证据必须包含一条注明「审计本机已跑绿」的判别检查路径，或被测元素的真实取值和文本；只附失败输出的退回不合规。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: M-10 第 2 次退回（审计 #17）的唯一理由是 e2e 被 170 秒 globalTimeout 截断（real 170.64）。实现 #19 没有改任何产品、夹具或断言，审计 #18 跑同一条命令 real 149.80 秒，20 条全过。同一用例 D-1 在审计 #16、#17、#18 里分别耗时 52.3、44.1、31.2 秒。
  root_cause: 时限验收只看一次运行，而本机 e2e 耗时的轮间波动（单条 D-1 就差 21 秒）和实测值离 170 秒上限的余量是同一个量级。一次超时就按实现缺陷退回，结果换来一个零改动的实现轮和一个额外的审计轮。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-10-r18-audit.md
  level: protocol
  proposal: 审计以耗时超限为唯一理由退回之前，要在同一提交上把验收命令连跑两次，并在证据里记下两次的 real；只有两次都超限才判为实现缺陷。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: rebase 冲突落在 `crates/autome-cli/src/main.rs` 和 `tests/cli.rs`，冲突提交 34ac401（「add CLI forwarding and login-shell launch service」）是 M-02 的实现，实现 #23 也把本轮记为「关联 M-02」，并把 M-02 验收集合中 3 条 ps EPERM 用例（login-path-real ×2、web-service 活实例复用）标为「宿主需复验」。审计 #22 的简报却把证据路径指定为 M-13-r23-audit.md；审计证据写的是「验收命令（M-13）与相关子集：vitest 10 文件 102 用例」，没有列出跑了哪 10 个文件，也没有写这 3 条用例的结果，就下了「无验证缺口」的结论。
  root_cause: 集成复验简报给的里程碑是 M-13（最后关闭的那个），不是冲突提交所属的 M-02。审计跟着简报把 M-13 当成「受影响里程碑」去跑验收（协议「集成复验」第 4 步），M-02 验收命令和实现轮留下的待复验用例就没有被逐条交代。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-13-r23-audit.md
  level: brief
  proposal: 集成复验简报指定的证据里程碑及其验收命令，取自冲突文件或消失提交所属的里程碑；审计证据对实现轮标为「宿主需复验」的每条用例写出本轮实跑结果。
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```
