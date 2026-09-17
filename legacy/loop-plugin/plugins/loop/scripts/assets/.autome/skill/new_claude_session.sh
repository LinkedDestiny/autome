#!/usr/bin/env bash
# 用 Claude Code CLI 拉起一个新的自治会话（后台、无头、启动即返回）。
# 用法: bash .autome/skill/new_claude_session.sh [--dry-run] "Please execute docs/<slug>/<slug>-task.md Task 1 additional task 2."
# 可用 LOOP_CLAUDE_FLAGS 覆盖默认权限模式；LOOP_MODEL 非空时透传为 --model（new_session.sh 按角色注入）。
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_launch_common.sh"

RUNTIME="claude"

build_cmd() {
  # shellcheck disable=SC2206
  CMD=(claude -p "$1" ${LOOP_MODEL:+--model "$LOOP_MODEL"} ${LOOP_CLAUDE_FLAGS:---dangerously-skip-permissions})
}

launch_session "$@"
