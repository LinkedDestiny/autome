# 教训 · 面板交付批次-w10-启动与安装

Autome 没有为本任务记录指标。下面的计数全部取自 `retro.md` 的轮次表、所引证据文件、简报和 git 历史，没有估算值。本文件由复盘第 3 轮覆盖写。第 2 轮之后又跑了实现 #24（rebase 集成）、宿主快检失败、实现 #25（快检修复）和审计 #23（集成复验），所以本版有这些变动：L-01 的计数改按 25 个实现轮重算；L-04 改写为覆盖两次集成复验的范围缺口；新增 L-05。L-02、L-03 没有新证据，原样保留。

```yaml
- id: L-01
  domain: tooling
  symptom: 本任务 8 次审计退回（M-01×1、M-03×1、M-07×2、M-09×2、M-10×2），全部落在实现轮因 spawnSync ps 或 listen 127.0.0.1 EPERM 跑不成验收的里程碑上。25 个实现轮证据文件里有 20 个出现 EPERM，没出现的只有 M-04、M-06、M-08、M-12、M-13 首轮，这几个里程碑都是一次审计通过。M-01 审计 #1 退回的缺陷（web-stop-gate 测试在门禁进程出现前就取 pid）要等审计在本机实跑才暴露。实现 #25 改了 web-launcher 的并发 start 断言，证据自己写着「真实收敛与负向注入尚未实跑，必须由宿主确认」。
  root_cause: 实现轮的沙箱禁止 ps 和本地监听，审计轮却允许，所以测试和夹具的缺陷只能等审计轮第一次实跑时暴露，每暴露一层就多一次退回。任务文件 §9 第 1 条已经记下 W7、W9 有同样的现象。这次照它的对策「如实记录、交审计轮复验」做了，退回次数并没有减少。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-01-r1-audit.md
  level: rule
  proposal: 实现轮会话与审计轮使用相同的沙箱放行（允许 ps 和 127.0.0.1 监听），检验标准是实现轮证据中不再出现 `spawnSync ps EPERM` 或 `listen EPERM`。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: M-07 和 M-09 各有一条断言连续两次被退回。审计 #9 只报了横幅读到空串，并写「需实现轮确认」。实现 #10 没法实跑，只加了等待，仍沿用设计里的子串「内核比账本旧」，审计 #10 再次退回。审计 #13 报 data-scroll-region 的实际值是 "true"。实现 #15 只把期望值改成 "true"，审计 #14 再次退回，原因是 `.task-docs-body` 在默认的时间线标签下根本不存在。
  root_cause: 实现轮跑不了浏览器和 Electron，只能照着审计给的失败现象改断言；审计退回时只附了失败输出，没有附自己跑绿过的正确判据。审计 #10 给出真实横幅文本和 a10-probe.sh，审计 #14 给出已跑绿的 a16-docs-tab.spec.ts，之后的下一轮审计都是一次通过。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-09-r15-audit.md
  level: protocol
  proposal: 如果被审实现轮的证据记了验收被环境阻塞，审计退回证据必须包含一条注明「审计本机已跑绿」的判别检查路径，或被测元素的真实取值和文本；只附失败输出的退回不合规。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: M-10 第 2 次退回（审计 #17）的唯一理由是 e2e 被 170 秒的 globalTimeout 截断（real 170.64）。实现 #19 没有改任何产品、夹具或断言，审计 #18 跑同一条命令用了 real 149.80 秒，20 条全过。同一用例 D-1 在审计 #16、#17、#18 里分别耗时 52.3、44.1、31.2 秒。
  root_cause: 时限验收只看一次运行，而本机 e2e 耗时的轮间波动（单条 D-1 就差 21 秒）和实测值到 170 秒上限的余量是同一个量级。一次超时就按实现缺陷退回，结果多出一个零改动的实现轮和一个审计轮。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-10-r18-audit.md
  level: protocol
  proposal: 审计以耗时超限为唯一理由退回之前，要在同一提交上把验收命令连跑两次，并在证据里记下两次的 real；只有两次都超限才判为实现缺陷。
  predicted_impact: {metric: reopen_total, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: 两次集成复验都漏掉了一部分要复验的东西。第一次：实现 #23 的 rebase 冲突落在 M-02 的提交 34ac401 上，实现轮把 M-02 的 3 条 ps EPERM 用例标为「宿主需复验」。审计 #22 的简报却指定 M-13，证据只写「vitest 10 文件 102 用例」，没有写这 3 条的结果，就下了「无验证缺口」的结论。第二次：实现 #24 的 rebase 提交 d9793fc9 改了 scripts/ci/steps.sh、install-node.sh、test-groups.ts，以及 load-context、ci-config 的断言，还留下 2 条 install-alignment 用例待补验。之后宿主快检失败，Autome 直接排了实现 #25。审计 #23 的简报（brief/audit-23.md 第 77 行）只给了 `fc79509..235b6e2`，范围里只有实现 #25 的一个提交。审计 #23 的证据也只写这 5 个测试文件，d9793fc9 和那 2 条安装用例没有任何审计轮看过。
  root_cause: 集成复验简报的里程碑取的是最后关闭的 M-13，不是冲突提交所属的里程碑。提交范围从上一个实现轮简报起算，不从上一次审计下结论的提交起算。所以「实现轮 → 门禁失败 → 实现轮」这条路径上，前一个实现轮的改动和它留下的待复验用例都落在审计范围之外，审计跟着简报走也就漏了。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-13-r25-audit.md
  level: brief
  proposal: 集成复验简报的提交范围从上一次审计下结论的提交起算，并逐条列出范围内各实现轮证据标为「宿主需复验」的用例；审计证据对每一条写出本轮实跑结果。
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: 实现 #24 之后，宿主快检的 `npx vitest related --run` 失败了 5 条，其中 4 条是确定性失败，与环境无关。env-page 第 7 条、live-status、settings-onboarding 第 7 条，都被 M-04（实现 #6）引入的 1000 ms 合并重验窗口打破；shell-data 第 6 条被 M-05（实现 #7）在 root loader 新加的 clock 字段打破。git 历史显示这 4 个测试文件都早于 M-04、M-05 的产品提交。M-04 实现证据其实已经记了同一个原因：board-live 通知断言等 1000 ms，恰好等于新窗口，于是超时失败。但那一轮只改了这一个文件。M-04、M-05 的实现轮和审计轮都只跑了验收契约列出的文件和自选的相关子集。这 4 条失败一路带到实现 #24 之后才被发现，为此多跑了实现 #25 和审计 #23。
  root_cause: 每轮的验证范围就是验收契约列出的测试文件，没有一轮去跑「引用了本轮改动模块的全部测试」。M-04 已经碰到一个同类失败，却没有按共同原则第 3 条扫描同类位置。宿主快检改跑 related 测试以后，这类回归才第一次被跑到。
  evidence: docs/面板交付批次-w10-启动与安装/evidence/M-13-r25-impl.md
  level: rule
  proposal: 改动 apps/web/app 下源文件的实现轮，证据里必须有一条对本轮改动源文件执行的 `npx vitest related --run <文件…>`，写明总数和失败数，并把每条失败归为实现缺陷或环境阻塞（如 ps EPERM）。
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
