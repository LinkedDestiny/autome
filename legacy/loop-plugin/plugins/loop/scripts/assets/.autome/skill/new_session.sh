#!/usr/bin/env bash
# 按角色路由的会话启动调度器：从协议启动语句推导角色，读 loop-roles.conf 选运行时与模型，
# 委托给 new_claude_session.sh / new_codex_session.sh（复用其 kill switch / dry-run / 台账闸门链）。
# 用法: bash .autome/skill/new_session.sh [--dry-run] "Please execute docs/<slug>/<slug>-task.md Task 1 additional task 2."
# 角色推导（唯一规则）：additional task 1→review 2→adjudicate 3→impl 4→audit；无 additional task→plan。
#
# 双模型纪律：生成侧与评测侧必须用不同的 runtime:model（plan ≠ review、impl ≠ audit）。
# conf 存在且两侧字面配置相同时输出 SAME-MODEL 并拒绝启动（LOOP_ALLOW_SAME_MODEL=1 可跳过，仅限调试）。
set -euo pipefail

SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF="$SKILL_DIR/loop-roles.conf"

DRY=""
if [ "${1:-}" = "--dry-run" ]; then
  DRY="--dry-run"
  shift
fi
PROMPT="${1:?usage: new_session.sh [--dry-run] \"<prompt>\"}"

case "$PROMPT" in
  *"additional task 1."*) ROLE="review" ;;
  *"additional task 2."*) ROLE="adjudicate" ;;
  *"additional task 3."*) ROLE="impl" ;;
  *"additional task 4."*) ROLE="audit" ;;
  *) ROLE="plan" ;;
esac

# conf 行格式: role=runtime[:model]（# 注释与空行忽略，同名行取最后一条）；缺文件/缺行回退 claude 默认模型。
spec_of() {
  local s=""
  if [ -f "$CONF" ]; then
    s="$(sed -n "s/^${1}[[:space:]]*=[[:space:]]*//p" "$CONF" | tail -1)"
  fi
  printf '%s' "${s:-claude}"
}

SPEC="$(spec_of "$ROLE")"
RUNTIME_NAME="${SPEC%%:*}"
MODEL=""
case "$SPEC" in *:*) MODEL="${SPEC#*:}" ;; esac

case "$RUNTIME_NAME" in
  claude) SCRIPT="$SKILL_DIR/new_claude_session.sh" ;;
  codex)  SCRIPT="$SKILL_DIR/new_codex_session.sh" ;;
  *)
    echo "BAD-ROLES-CONF: 角色 ${ROLE} 的 runtime '${RUNTIME_NAME}' 非法（仅支持 claude|codex）；不启动新会话" >&2
    exit 2
    ;;
esac

# 双模型纪律：评测角色（review/audit）的配置不得与对应生成角色（plan/impl）完全相同。
# 只在 conf 存在时校验——conf 缺失的旧项目回退单 claude，不因升级直接断链，但会在 stderr 提醒。
PAIR=""
case "$ROLE" in
  review) PAIR="plan" ;;
  audit)  PAIR="impl" ;;
esac
if [ -n "$PAIR" ] && [ "${LOOP_ALLOW_SAME_MODEL:-}" != "1" ]; then
  if [ -f "$CONF" ]; then
    PAIR_SPEC="$(spec_of "$PAIR")"
    if [ "$SPEC" = "$PAIR_SPEC" ]; then
      echo "SAME-MODEL: 角色 ${ROLE} 与 ${PAIR} 配置相同（${SPEC}）——生成与评测必须用不同模型；" >&2
      echo "  修改 .autome/skill/loop-roles.conf（或 autome roles set ${ROLE} <claude|codex>[:model]）后重试；不启动新会话" >&2
      exit 2
    fi
  else
    echo "WARN: loop-roles.conf 缺失，无法校验生成/评测双模型分工（重跑 autome init 可补齐默认路由）" >&2
  fi
fi

LOOP_MODEL="$MODEL" exec bash "$SCRIPT" ${DRY:+--dry-run} "$PROMPT"
