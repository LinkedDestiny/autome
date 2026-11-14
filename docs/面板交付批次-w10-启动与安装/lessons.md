# 教训 · 面板交付批次-w10-启动与安装

Autome 没有为本任务记录指标；下面的计数全部取自 `retro.md` 的轮次表与所引证据文件，没有估算值。

```yaml
- id: L-01
  domain: tooling
  symptom: 本任务 8 次审计退回（M-01×1、M-03×1、M-07×2、M-09×2、M-10×2）全部落在实现轮因 spawnSync ps / listen 127.0.0.1 EPERM 跑不成验收的里程碑；实现轮能跑完验收的 M-04、M-06、M-12、M-13 都一次审计通过。retro.md 阻塞列里 22 个实现轮有 16 个记了 EPERM。
  root_cause: 实现轮的沙箱禁止 ps 和本地监听，审计轮却允许；测试和夹具的缺陷只能等审计轮第一次实跑时暴露，每暴露一层就多一次退回。任务文件 §9 第 1 条已记下 W7、W9 有同样的现象，这次按其对策「如实记录、交审计轮复验」做了，退回次数并没有减少。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-10-r17-impl.md
  level: rule
  proposal: 实现轮会话使用与审计轮相同的沙箱放行（允许 ps 与 127.0.0.1 监听），检验标准是实现轮证据中不再出现 `spawnSync ps EPERM` 或 `listen EPERM`。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: M-07 和 M-09 各因同一条断言连续退回两次。审计 #9 只报了横幅读到空串，写的是「需实现轮确认」；实现 #10 没法实跑，只加了等待、沿用设计里的子串「内核比账本旧」，审计 #10 再次退回。审计 #13 报 data-scroll-region 的实际值是 "true"；实现 #15 只把期望值改成 "true"，审计 #14 再次退回，原因是该元素在默认标签下根本不存在。
  root_cause: 实现轮跑不了浏览器和 Electron，只能照着审计给的失败现象改断言；审计退回时只附了失败输出，没有附自己跑绿过的正确判据。审计 #10 给出真实横幅文本和 a10-probe.sh、审计 #14 给出已跑绿的 a16-docs-tab.spec.ts 之后，下一轮审计都一次通过。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-09-r15-audit.md
  level: protocol
  proposal: 当被审的实现轮证据记了验收被环境阻塞时，审计退回证据必须包含一条注明「审计本机已跑绿」的判别检查路径，或被测元素的真实取值和文本；只附失败输出的退回不合规。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: M-10 第 2 次退回（审计 #17）的唯一理由是 e2e 被 170 秒 globalTimeout 截断（real 170.64）；实现 #19 没有改任何产品、夹具或断言，审计 #18 跑同一条命令 real 149.80 秒，20 条全过。同一用例 D-1 在三次审计里分别耗时 52.3、44.1、31.2 秒。
  root_cause: 时限验收只看一次运行，而本机 e2e 耗时的轮间波动（单条 D-1 就差 21 秒）和实测值到 170 秒上限之间的余量是同一个量级；一次超时就按实现缺陷退回，结果换来一个零改动的实现轮和一个额外的审计轮。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-10-r19-impl.md
  level: protocol
  proposal: 审计以耗时超限作为唯一理由退回之前，要在同一提交上把验收命令连跑两次，并在证据里记下两次的 real；只有两次都超限，才判为实现缺陷。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```
