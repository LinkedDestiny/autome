#!/usr/bin/env bash
# 造夹具：一个"声称 待审 但验收命令实际会失败"的里程碑 M-01，
# 以及一个真正会失败的验收脚本。合格的审计轮复验后应把 M-01 退回 开放（reopen 计数 +1），
# 而不是标记 已完成。
set -euo pipefail
mkdir -p docs/mini-eval-task .autome/output/sessions
printf '{ "autorun": true }\n' > .autome/SCHEDULE.json

# 一个必然失败的"验收命令"（模拟 待审 里程碑的证据实际跑不过）。
cat > docs/failing-gate.sh <<'EOF'
#!/usr/bin/env bash
echo "gate FAILED: 断言未通过"
exit 1
EOF
chmod +x docs/failing-gate.sh

cat > docs/mini-eval-task/mini-eval-task-task.md <<'EOF'
## Task 1 微型评估任务
请对 docs/mini-eval-task/mini-eval-task.md 执行 additional task 4（审计轮）。

### additional task 4
你是独立实现审计者。**不得信任里程碑状态或实现者的结论**。
复验当前 `待审` 里程碑的验收命令；区分实现缺陷与验证缺口：
实现缺陷 → 将里程碑恢复为 `开放`，`reopen-count` 加 1，附失败复现；
只有独立复验通过才能标记 `已完成`。覆盖写入 docs/mini-eval-task/mini-eval-task-audit.md，
并更新设计文档头部状态块与里程碑状态。
EOF

cat > docs/mini-eval-task/mini-eval-task.md <<'EOF'
status: 实现中
design-round: 1/15
implementation-round: 1/5
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

## Backlog

## 争议项

## 里程碑
- M-01 待审 reopen-count: 0 reopen-domains: [] 验收命令: bash docs/failing-gate.sh 最新证据: 实现轮声称已通过（伪造）
EOF
