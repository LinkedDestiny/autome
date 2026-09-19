status: 实现中
design-round: 1/15
implementation-round: 7/10
current-milestone: M-05
current-milestone-reopens: 3
convergence-mode: normal
next-action: 在有真实麦克风与授权的机器上完成 M-05 的真机验收并留证，然后关闭 M-05。

# 语音便签

### M-05 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`（解析与存储部分）
- 预期：12 个用例通过。
- 负向对照：把时刻解析的分钟上界改成 61，第 7 号用例失败。

未取证项（实现 #5、#6、#7 连续三轮都没拿到）：

1. 真实长按面板按钮 3 秒、真人说一句话，面板出现「已创建：…」。
2. 日程开始前 30 分钟，真人在锁屏上看见提醒横幅。
3. 真人点「撤销」后，刚删掉的那条日程回到列表。

这三项要真实鼠标、真实麦克风和肉眼确认。本机 `AXIsProcessTrusted()` 为 false，
合成鼠标事件被静默丢弃；语音识别授权为 `denied`。

最新证据：docs/eval-case/evidence/M-05-r7-impl.md · 自动部分通过、三项真机缺证 · 实现 #7

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 工程骨架 | 0 | |
| M-02 | 已完成 | 录音 | 0 | |
| M-03 | 已完成 | 转写 | 1 | transcription |
| M-04 | 已完成 | 解析日程 | 1 | parsing |
| M-05 | 待审 | 端到端验收 | 3 | parsing |

## Backlog

## 争议项
