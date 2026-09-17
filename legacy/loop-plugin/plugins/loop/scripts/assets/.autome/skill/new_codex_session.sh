#!/usr/bin/env bash
# 用 Codex CLI 拉起一个新的自治会话（后台、无头、启动即返回）。
# 用法: bash .autome/skill/new_codex_session.sh [--dry-run] "Please execute docs/<slug>/<slug>-task.md Task 1 additional task 1."
# 可用 LOOP_CODEX_FLAGS 覆盖默认沙箱/审批模式；LOOP_MODEL 非空时透传为 -m（new_session.sh 按角色注入）。
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_launch_common.sh"

RUNTIME="codex"

build_cmd() {
  # shellcheck disable=SC2206
  CMD=(codex exec ${LOOP_MODEL:+-m "$LOOP_MODEL"} ${LOOP_CODEX_FLAGS:---full-auto --skip-git-repo-check} "$1")
}

launch_session "$@"
