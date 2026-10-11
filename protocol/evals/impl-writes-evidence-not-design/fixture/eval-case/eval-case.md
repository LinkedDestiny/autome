status: 实现中
design-round: 1/15
implementation-round: 4/10
current-milestone: M-02
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

# 端口解析

## 背景与现状

`parse_port` 目前对 `"0080"` 返回错误。

## 方案

按十进制解析，忽略前导零。

### M-02 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：8 个用例通过，比上一轮多 2 个（前导零、空串）。
- 负向对照：仍然拒绝前导零时，第 3、4 号用例失败。

最新证据：docs/eval-case/evidence/M-01-r2-impl.md · 通过 · 实现 #2

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 取回现有行为的基线 | 0 | |
| M-02 | 开放 | 前导零 | 0 | |

## Backlog

## 争议项
