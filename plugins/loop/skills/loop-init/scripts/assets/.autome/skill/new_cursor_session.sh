#!/usr/bin/env bash
# 用 Cursor Agent CLI 拉起一个新的自治会话（后台、无头、启动即返回）。
# 用法: bash .autome/skill/new_cursor_session.sh [--dry-run] "Please execute docs/<slug>/<slug>-task.md Task 1 additional task 2."
# 可执行文件优先 cursor-agent（旧名，无歧义），其次 agent（新名）；LOOP_CURSOR_BIN 可显式指定。
# 可用 LOOP_CURSOR_FLAGS 覆盖默认权限模式；LOOP_MODEL 非空时透传为 --model（new_session.sh 按角色注入）。
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_launch_common.sh"

RUNTIME="cursor"

cursor_bin() {
  if [ -n "${LOOP_CURSOR_BIN:-}" ]; then printf '%s' "$LOOP_CURSOR_BIN"; return; fi
  if command -v cursor-agent >/dev/null 2>&1; then printf 'cursor-agent'; return; fi
  printf 'agent'
}

build_cmd() {
  # --force 允许改文件与执行命令；--trust 免去无头模式下的工作区信任确认。
  # shellcheck disable=SC2206
  CMD=("$(cursor_bin)" -p ${LOOP_MODEL:+--model "$LOOP_MODEL"} ${LOOP_CURSOR_FLAGS:---force --trust} "$1")
}

launch_session "$@"
