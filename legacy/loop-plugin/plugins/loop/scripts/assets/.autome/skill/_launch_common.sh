#!/usr/bin/env bash
# 会话启动公共逻辑：被 new_*_session.sh source。
# 约定：调用方先定义 RUNTIME（如 claude / codex）与 build_cmd()（把 PROMPT 组装成 CMD 数组），再调用 launch_session "$@"。
set -euo pipefail

# 纯 bash JSON 字符串转义（无 python3/jq 依赖）：转义 \ " 与全部 JSON 控制字符（U+0001–U+001F；
# \t\r\n 用短形式，其余 \u00XX，B-08 采纳收紧），输出带引号的合法 JSON 字符串。bash 变量无法含 NUL，U+0000 天然不可达。
json_escape() {
  local s="$1" i c u
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\t'/\\t}"
  s="${s//$'\r'/\\r}"
  s="${s//$'\n'/\\n}"
  for i in 1 2 3 4 5 6 7 8 11 12 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31; do
    printf -v c "\\x$(printf '%02x' "$i")"
    printf -v u '\\u%04x' "$i"
    s="${s//$c/$u}"
  done
  printf '"%s"' "$s"
}

# 从协议启动语句推导目标角色（与 new_session.sh 的路由规则一致，唯一规则）：
# additional task 1→review 2→adjudicate 3→impl 4→audit；无 additional task→plan。
derive_role() {
  case "$1" in
    *"additional task 1."*) printf 'review' ;;
    *"additional task 2."*) printf 'adjudicate' ;;
    *"additional task 3."*) printf 'impl' ;;
    *"additional task 4."*) printf 'audit' ;;
    *)                      printf 'plan' ;;
  esac
}

# 卡点通知：尽力而为，失败不阻塞（🔔 stderr + macOS 通知 + LOOP_NOTIFY_CMD 任意命令）。
gate_notify() {
  printf '🔔 %s\n' "$1" >&2
  command -v osascript >/dev/null 2>&1 && \
    osascript -e "display notification \"$1\" with title \"autome\"" >/dev/null 2>&1 || true
  [ -n "${LOOP_NOTIFY_CMD:-}" ] && sh -c "$LOOP_NOTIFY_CMD \"\$1\"" _ "$1" >/dev/null 2>&1 || true
}

launch_session() {
  local dry_run=0
  if [ "${1:-}" = "--dry-run" ]; then
    dry_run=1
    shift
  fi
  local PROMPT="${1:?usage: $(basename "$0") [--dry-run] \"<prompt>\"}"

  local ROOT
  ROOT="$(cd "$(dirname "${BASH_SOURCE[1]}")/../.." && pwd)"

  local CMD=()
  build_cmd "$PROMPT"

  local TS SLUG_TAG LOG SESS_DIR
  TS="$(date +%Y%m%d-%H%M%S)"
  # 从 prompt 中提取 slug 便于日志定位与卡点归属（形如 docs/<slug>/<slug>-task.md）；提取失败回退 adhoc
  # 字符集必须与 bin/autome 的 slug_of_task_file 同域——那边不限字符集，只要求
  # basename(dirname) == slug。这里原先限死 [a-z0-9-]，slug 含其它字符时静默回退 adhoc，
  # 于是停靠落到 gates/adhoc.parked、日志落成 <rt>-adhoc-*.log，
  # 而 autome approve <slug> / autome logs <slug> 按真 slug 去找，必然找不到。
  # `[^/]*` 是安全的：slug 不可能含 `/`（否则 basename 判据不成立），sed 逐行工作也不可能含换行。
  SLUG_TAG="$(printf '%s' "$PROMPT" | sed -n 's/.*docs\/\([^/]*\)\/[^/]*-task\.md.*/\1/p')"
  SESS_DIR="$ROOT/.autome/output/sessions"
  LOG="$SESS_DIR/$RUNTIME-${SLUG_TAG:-adhoc}-$TS.log"

  # 闸门次序：
  #   ① --dry-run 短路（先于一切校验，无条件返回 0）
  #   ② kill switch 拒绝
  #   ②′ gate 停靠（gated 模式下在人工边界停靠，等 autome approve 补发）
  #   ③ runtime 可执行文件校验
  #   ④ 后台拉起（拉起前注入 LOOP_CURRENT_ROLE=目标角色，供子会话再调 launcher 时识别调用方）

  # ① --dry-run：纯打印分支，不拉起、不追加台账、不受 kill switch 与 runtime 校验影响。
  if [ "$dry_run" = 1 ]; then
    # 含特殊字符的参数用单引号包裹（内部单引号转义），输出可直接复制执行；
    # 逐字启动语句仍是输出的连续子串（T3 门禁依赖）。
    local shown="" a
    for a in "${CMD[@]}"; do
      case "$a" in
        *[!A-Za-z0-9_./:=-]*) shown="$shown '$(printf '%s' "$a" | sed "s/'/'\\\\''/g")'" ;;
        *)                    shown="$shown $a" ;;
      esac
    done
    echo "DRY-RUN: (cd $ROOT && nohup${shown} >$LOG 2>&1 &)"
    return 0
  fi

  # ② kill switch：SCHEDULE.json 的 autorun 不为 true（缺失/不可读/非常规文件/非 true）时
  # 拒绝拉起任何新会话。这是循环唯一的手动刹车——协议本身的终止靠轮次预算与审计判定。
  # [ -f ] 必须在 grep 之前：grep 是行缓冲的，落到字符设备（如指向 /dev/zero 的符号链接）
  # 上会一直攒一条永不换行的「行」，秒级吃掉几十 GB 内存；命名管道则永久阻塞。
  if [ ! -f "$ROOT/.autome/SCHEDULE.json" ] \
     || ! grep -Eq '"autorun"[[:space:]]*:[[:space:]]*true' "$ROOT/.autome/SCHEDULE.json" 2>/dev/null; then
    echo "REFUSED: $ROOT/.autome/SCHEDULE.json autorun != true（kill switch 已拉下，不启动新会话）" >&2
    exit 1
  fi

  # ②′ gate 停靠：`.autome/output/gates/<slug>.mode` 为 gated 时，两个人工边界停靠等放行——
  #   边界 A（plan→评审）：additional task 1 的启动；
  #   边界 B（设计→实现）：裁决角色（LOOP_CURRENT_ROLE=adjudicate）发起的 additional task 3 启动，
  #     审计→实现的常规接力（audit）与人工/CLI 直发（变量缺失）不拦。
  #   autome approve 补发时带 LOOP_GATE_OVERRIDE=1 跳过本闸门；停靠 exit 0（对发起会话即“已启动”）。
  #   同 ②：[ -f ] 先行，避免 grep 落到非常规文件上。
  if [ "${LOOP_GATE_OVERRIDE:-}" != "1" ] \
     && [ -f "$ROOT/.autome/output/gates/${SLUG_TAG:-adhoc}.mode" ] \
     && grep -qx 'gated' "$ROOT/.autome/output/gates/${SLUG_TAG:-adhoc}.mode" 2>/dev/null; then
    local BOUNDARY=""
    case "$PROMPT" in
      *"additional task 1."*) BOUNDARY="plan→评审" ;;
      *"additional task 3."*) [ "${LOOP_CURRENT_ROLE:-}" = "adjudicate" ] && BOUNDARY="设计→实现" ;;
    esac
    if [ -n "$BOUNDARY" ]; then
      mkdir -p "$ROOT/.autome/output/gates"
      printf '%s\n' "$PROMPT" > "$ROOT/.autome/output/gates/${SLUG_TAG:-adhoc}.parked"
      gate_notify "卡点[$BOUNDARY] ${SLUG_TAG:-adhoc}：autome approve 放行 / autome reject 终止"
      echo "PARKED: ${BOUNDARY} 边界停靠 gates/${SLUG_TAG:-adhoc}.parked（autome approve ${SLUG_TAG:-adhoc} 放行）"
      return 0
    fi
  fi

  # ③ runtime 校验：可执行文件不存在时拒绝式失败，不追加台账、不输出 LAUNCHED（避免虚假成功）。
  if ! command -v "$RUNTIME" >/dev/null 2>&1; then
    echo "MISSING-CLI: $RUNTIME 未找到（未安装或不在 PATH）；不启动新会话" >&2
    exit 1
  fi

  # ④ 后台拉起，启动即返回。子会话环境注入自身角色；越权变量不外泄。
  export LOOP_CURRENT_ROLE="$(derive_role "$PROMPT")"
  unset LOOP_GATE_OVERRIDE 2>/dev/null || true
  mkdir -p "$SESS_DIR"
  cd "$ROOT"
  nohup "${CMD[@]}" >"$LOG" 2>&1 &
  local PID=$!
  disown "$PID" 2>/dev/null || true

  # 启动台账：append-only，记录谁在什么时候拉起了什么会话（prompt 经纯 bash 转义为合法 JSON 字符串）
  printf '{"ts":"%s","runtime":"%s","pid":%d,"log":"%s","prompt":%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$RUNTIME" "$PID" "${LOG#"$ROOT"/}" \
    "$(json_escape "$PROMPT")" \
    >> "$SESS_DIR/launches.jsonl"

  echo "LAUNCHED: $RUNTIME session pid=$PID log=${LOG#"$ROOT"/}"
}
