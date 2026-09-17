#!/usr/bin/env bash
# 造夹具：一个已初始化的微型 Loop 项目 + 一个预算未满、有 开放 里程碑的设计文档。
# 合格的实现轮（AT3）本轮结束时应用启动脚本拉起 AT4 后立即结束（LAUNCHED），不等待。
set -euo pipefail
mkdir -p docs/mini-eval-task .autome/output/sessions .autome/skill
printf '{ "autorun": true }\n' > .autome/SCHEDULE.json

# 装入启动脚本与 SESSION_SKILL（拷自插件 assets，保证接力可用）。
PLUGIN_ASSETS="${CLAUDE_PLUGIN_ROOT:-.}/scripts/assets"
if [ -d "$PLUGIN_ASSETS/.autome/skill" ]; then
  cp "$PLUGIN_ASSETS"/.autome/skill/* .autome/skill/ 2>/dev/null || true
  chmod +x .autome/skill/*.sh 2>/dev/null || true
fi

cat > docs/mini-eval-task/mini-eval-task-task.md <<'EOF'
## Task 1 微型评估任务
请对 docs/mini-eval-task/mini-eval-task.md 执行 additional task 3（实现轮）。

### additional task 3
你是**实现轮**。推进编号最小的 `开放` 里程碑（通过验收后标记 `待审`，不得标记 `已完成`），
本轮结束时**一律**使用
`Please execute docs/mini-eval-task/mini-eval-task-task.md Task 1 additional task 4.` 启动新会话
（用 .autome/skill/new-claude-session.md 的方法），只要启动就结束，不要等待/轮询新会话。
EOF

cat > docs/mini-eval-task/mini-eval-task.md <<'EOF'
status: 实现中
design-round: 1/15
implementation-round: 1/5
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 在 docs/note.txt 写一行字

## Backlog

## 争议项

## 里程碑
- M-01 开放 reopen-count: 0 reopen-domains: [] 验收命令: test -s docs/note.txt
EOF
