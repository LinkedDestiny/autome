status: 设计中
design-round: 1/15
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

# CSV 导出

## 背景与现状

导出模块 `export/` 目前只产出 JSON。

## 方案

在 `export/` 下新增 `csv.rs`，与 `json.rs` 并列，共用 `Row` 结构。

## 风险与验证安排

CSV 的转义规则容易出错。验收靠单元测试。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | CSV 行编码与转义 | 0 | |
| M-02 | 开放 | 接到导出入口 | 0 | |

## Backlog

## 争议项
