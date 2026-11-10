# 教训 · apps-web-的服务端托管-automed-2（T-12，W2 内核网关）

本次运行：设计 1 轮、实现 5 轮，M-01..M-05 全部一次审计通过，reopen 0，没有实现缺陷。Autome 没有记录到本任务的指标，下面不引用任何指标数值。教训来自证据文件与提交历史，不来自退回。

```yaml
- id: L-01
  domain: verification
  symptom: 实现 #3 的提交 cd78b3b 把 M-03 验收契约的预期从「137（113 + 24）」改成「138（113 + 25）」，并顺带改了 M-04、M-05 的预期；实现 #5 的提交 a6d2142 又把 M-05 从「173（152 + 21）」改成「175（152 + 23）」。审计 #3、#5 按改过的数字判「与预期一致」
  root_cause: 协议只禁止实现轮把里程碑标成已完成，没有禁止实现轮改本里程碑验收契约的预期行；被评测方改了评测基准，审计拿到的已经是改后的数字。本次只是多了用例，但同样的做法也能把少掉的用例改成「符合预期」
  evidence: docs/apps-web-的服务端托管-automed-2/evidence/M-03-r3-impl.md
  level: protocol
  proposal: 实现轮的提交不得改动任何「### M-xx 验收契约」小节的文字；实际用例数与预期不符时，只在实现证据里写差值和每个用例的来源，由审计轮按未改动的契约逐条判断
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 5}

- id: L-02
  domain: tooling
  symptom: 5 个实现轮和评审 #1 全部遇到 `listen EPERM 127.0.0.1`（同时伴有 watcher EMFILE），审计轮没有遇到；M-05 新写的 10 个 gateway-http 用例在实现轮一次都没跑过，第一次执行是在审计 #5。实现 #2 的验收命令用 `&&` 串起来，Web 一失败 desktop 就没跑，之后的实现轮才改成逐段运行
  root_cause: 实现轮会话所在的沙箱不允许回环监听，审计轮的沙箱允许；每个实现轮都要自己重新发现这一点、自己区分 EMFILE 和 EPERM（实现 #3 为此用 CHOKIDAR_USEPOLLING 重跑）。W1、T-11 已经出现过同样的问题，写进了任务文件的「前车之鉴」，但只是提醒，没有在开场时探测
  evidence: docs/apps-web-的服务端托管-automed-2/evidence/M-05-r5-impl.md
  level: brief
  proposal: 实现轮简报写明 Autome 在开场前对 127.0.0.1 做监听探测的结果；探测失败时，实现证据对验收契约的每一段分别给出单独运行的退出码，不用 `&&` 串起来的单条命令结果
  predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: verification
  symptom: 审计 #1 在 apps/web 目录下用 `git checkout -- ..` 还原负向对照，没有还原仓库根下的 `.github/workflows/ci.yml`，注入一度残留，后来审计自己发现，重新在干净文件上做了两项注入
  root_cause: 用相对路径还原负向对照注入，覆盖范围取决于当前工作目录；注入点在被测目录之外（.github）时就漏掉了。残留的注入会让后面的验收结果作废，也可能被 Autome 兜底提交进分支
  evidence: docs/apps-web-的服务端托管-automed-2/evidence/M-01-r1-audit.md
  level: brief
  proposal: 审计与实现证据的负向对照部分，必须给出全部注入还原之后在仓库根执行 `git status --porcelain` 的输出，而且输出为空
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```
