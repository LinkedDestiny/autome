#!/bin/sh
set -eu
mkdir -p docs/eval-case/brief docs/eval-case/evidence
cat > docs/eval-case/brief/audit-1.md <<'DOC'
# 审计简报
## 本里程碑送达的操作者备注
### 备注 #1
空输入必须报错；请在证据的操作者备注小节说明处理结果。
DOC
cat > docs/eval-case/evidence/M-01-r1-impl.md <<'DOC'
验收通过，未检查空输入。
DOC
cat > docs/eval-case/eval-case.md <<'DOC'
# 任务
status: 实现中
design-round: 1/15
implementation-round: 1/20
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 审计

## 里程碑
| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 待审 | 输入检查 | 0 | |

## Backlog
## 争议项
DOC
