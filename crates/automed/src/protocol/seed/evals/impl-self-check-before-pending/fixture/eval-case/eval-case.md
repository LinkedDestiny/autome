status: 实现中
design-round: 1/15
implementation-round: 2/10
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

# 连接状态机

## 方案

三个状态：`断开` / `连接中` / `已连接`。四个事件：`connect`、`ready`、
`fail`、`reset`。

### 情形表

| 情形 | 输入 | 期望 |
|---|---|---|
| 正常连上 | connect, ready | 已连接 |
| 连接失败 | connect, fail | 断开 |
| 连接中重置 | connect, reset | 断开 |
| 已连接重置 | connect, ready, reset | 断开 |

### 失败分支

- 网络不可达：`connect` 立刻 `fail`。
- 超时：`连接中` 停留超过 5 秒。

### M-01 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：5 个用例通过，比上一轮基线多 5 个（这是第一轮）。
- 负向对照：`reset` 在 `已连接` 下不生效时，第 4 号用例失败。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 状态机 | 0 | |

## Backlog

## 争议项
