#!/usr/bin/env bash
# Loop 工程项目初始化：把运行时资产装进目标项目的 .autome/ 目录（幂等，默认不覆盖已存在文件）。
# 0.6.x 旧布局（prompt/skill、prompt/output、根 SCHEDULE.json）自动迁移进 .autome/，
# 原路径留符号链接——旧任务文件里写死的 prompt/skill/new-session.md 等引用仍然可用。
# 用法: bash init.sh [--force] [目标目录]
#   --force     覆盖已存在的运行时文件（清单外文件永不触碰）
#   目标目录     默认当前目录
set -euo pipefail

usage() {
  echo "用法: bash init.sh [--force] [目标目录]" >&2
}

# 参数校验（C-04）：拒绝未知/非法参数与多余位置参数；拒绝时无任何文件写入。
FORCE=0
TARGET=""
POSITIONAL=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    -*)
      echo "错误：未知参数 '$arg'" >&2
      usage
      exit 2
      ;;
    *)
      POSITIONAL=$((POSITIONAL + 1))
      if [ "$POSITIONAL" -gt 1 ]; then
        echo "错误：多于一个位置参数（'$arg'）" >&2
        usage
        exit 2
      fi
      TARGET="$arg"
      ;;
  esac
done
[ -n "$TARGET" ] || TARGET="."

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ASSETS="$SCRIPT_DIR/assets"
TARGET="$(cd "$TARGET" && pwd)"

echo "Loop 工程初始化 → $TARGET"
echo

# 0. 旧布局迁移（0.6.x → .autome/）。mv 保数据，原路径留符号链接保兼容：
#    旧任务文件是生成时快照，里面写死 prompt/skill/... 与根 SCHEDULE.json；
#    dashboard 的项目判据（statSync().isFile()）也跟随符号链接，迁移后照常识别。
MIGRATED=0
if [ -d "$TARGET/prompt/skill" ] && [ ! -L "$TARGET/prompt/skill" ] && [ ! -d "$TARGET/.autome/skill" ]; then
  mkdir -p "$TARGET/.autome"
  mv "$TARGET/prompt/skill" "$TARGET/.autome/skill"
  ln -s ../.autome/skill "$TARGET/prompt/skill"
  echo "  MOVE  prompt/skill → .autome/skill（原路径留符号链接）"
  MIGRATED=1
fi
if [ -d "$TARGET/prompt/output" ] && [ ! -L "$TARGET/prompt/output" ] && [ ! -d "$TARGET/.autome/output" ]; then
  mkdir -p "$TARGET/.autome"
  mv "$TARGET/prompt/output" "$TARGET/.autome/output"
  ln -s ../.autome/output "$TARGET/prompt/output"
  echo "  MOVE  prompt/output → .autome/output（原路径留符号链接）"
  MIGRATED=1
fi
if [ -f "$TARGET/SCHEDULE.json" ] && [ ! -L "$TARGET/SCHEDULE.json" ] && [ ! -f "$TARGET/.autome/SCHEDULE.json" ]; then
  mkdir -p "$TARGET/.autome"
  mv "$TARGET/SCHEDULE.json" "$TARGET/.autome/SCHEDULE.json"
  ln -s .autome/SCHEDULE.json "$TARGET/SCHEDULE.json"
  echo "  MOVE  SCHEDULE.json → .autome/SCHEDULE.json（原路径留符号链接）"
  MIGRATED=1
fi
if [ -f "$TARGET/skill/testing/test-design-methodology.md" ] && [ ! -L "$TARGET/skill/testing/test-design-methodology.md" ] \
   && [ ! -f "$TARGET/.autome/skill/testing/test-design-methodology.md" ]; then
  mkdir -p "$TARGET/.autome/skill/testing"
  mv "$TARGET/skill/testing/test-design-methodology.md" "$TARGET/.autome/skill/testing/test-design-methodology.md"
  ln -s ../../.autome/skill/testing/test-design-methodology.md "$TARGET/skill/testing/test-design-methodology.md"
  echo "  MOVE  skill/testing/test-design-methodology.md → .autome/skill/testing/（原路径留符号链接）"
  MIGRATED=1
fi

# 1. 目录骨架
mkdir -p "$TARGET/docs" "$TARGET/.autome/skill/testing" "$TARGET/.autome/output/sessions" "$TARGET/.autome/output/gates"

# 1b. 兼容符号链接（全新项目也建）：.autome/ 是唯一权威，根部路径只是出口。
#     作用：旧任务文件快照里写死的 prompt/skill/...、根 SCHEDULE.json 继续可用；
#     autome-dashboard 的项目判据（SCHEDULE.json + prompt/skill/new-claude-session.md，
#     statSync 跟随符号链接）无需改动即可识别新布局项目。
# [ -L ] 兜底：SCHEDULE.json 的目标要到第 2 步才落盘，此刻链接是 dangling 的，[ -e ] 会判 false。
mkdir -p "$TARGET/prompt"
{ [ -e "$TARGET/prompt/skill" ]  || [ -L "$TARGET/prompt/skill" ];  } || ln -s ../.autome/skill  "$TARGET/prompt/skill"
{ [ -e "$TARGET/prompt/output" ] || [ -L "$TARGET/prompt/output" ]; } || ln -s ../.autome/output "$TARGET/prompt/output"
{ [ -e "$TARGET/SCHEDULE.json" ] || [ -L "$TARGET/SCHEDULE.json" ]; } || ln -s .autome/SCHEDULE.json "$TARGET/SCHEDULE.json"

# 2. 运行时文件（相对 assets 的路径 → 相对项目根的同名路径）
FILES=(
  ".autome/skill/_launch_common.sh"
  ".autome/skill/new_session.sh"
  ".autome/skill/new_claude_session.sh"
  ".autome/skill/new_codex_session.sh"
  ".autome/skill/loop-roles.conf"
  ".autome/skill/new-session.md"
  ".autome/skill/new-claude-session.md"
  ".autome/skill/new-codex-session.md"
  ".autome/skill/testing/test-design-methodology.md"
  ".autome/SCHEDULE.json"
)
for f in "${FILES[@]}"; do
  # 0.6.x 迁移后必须刷新插件管理的脚本与说明（旧内容引用旧路径、缺双模型校验）；
  # 人工/状态文件（loop-roles.conf、SCHEDULE.json）不在刷新之列。
  refresh=0
  if [ "$MIGRATED" = 1 ]; then
    case "$f" in
      .autome/skill/loop-roles.conf | .autome/SCHEDULE.json) refresh=0 ;;
      *) refresh=1 ;;
    esac
  fi
  if [ -e "$TARGET/$f" ] && [ "$FORCE" != 1 ] && [ "$refresh" != 1 ]; then
    echo "  SKIP  ${f}（已存在，--force 可覆盖）"
  elif [ -e "$TARGET/$f" ]; then
    cp "$ASSETS/$f" "$TARGET/$f"
    # bash 3.2 多字节坑：$f 后紧跟全角字符必须用 ${f}，否则「（」的首字节被并进变量名（unbound variable）
    if [ "$FORCE" = 1 ]; then echo "  NEW   $f"; else echo "  UPDATE ${f}（迁移刷新为新布局脚本）"; fi
  else
    cp "$ASSETS/$f" "$TARGET/$f"
    echo "  NEW   $f"
  fi
done
chmod +x "$TARGET/.autome/skill/"*.sh

# 2b. 双模型纪律检查：迁移保留的旧 conf 若仍是 0.6.x 未改动默认（五角色全 claude），
#     升级为新默认（生成 opus / 评测 sonnet）；用户改过的 conf 不动，只在违反配对时告警。
CONF="$TARGET/.autome/skill/loop-roles.conf"
conf_spec() { sed -n "s/^${1}[[:space:]]*=[[:space:]]*//p" "$CONF" 2>/dev/null | tail -1; }
if [ -f "$CONF" ]; then
  pristine=1
  for r in plan review adjudicate impl audit; do
    [ "$(conf_spec "$r")" = "claude" ] || { pristine=0; break; }
  done
  if [ "$pristine" = 1 ]; then
    cp "$ASSETS/.autome/skill/loop-roles.conf" "$CONF"
    echo "  UPDATE .autome/skill/loop-roles.conf（0.6.x 未改动默认 → 生成/评测双模型默认）"
  else
    plan_s="$(conf_spec plan)"; review_s="$(conf_spec review)"
    impl_s="$(conf_spec impl)"; audit_s="$(conf_spec audit)"
    [ "${review_s:-claude}" = "${plan_s:-claude}" ] && \
      echo "  ⚠ WARN loop-roles.conf: review 与 plan 配置相同（${plan_s:-claude}）——评审启动将被 SAME-MODEL 拒绝，请分开模型"
    [ "${audit_s:-claude}" = "${impl_s:-claude}" ] && \
      echo "  ⚠ WARN loop-roles.conf: audit 与 impl 配置相同（${impl_s:-claude}）——审计启动将被 SAME-MODEL 拒绝，请分开模型"
  fi
fi

# 3. AGENTS.md：无则创建；有 marker 且 --force 或迁移时以 marker 边界原地替换 loop 片段（C-08e）；否则缺 marker 时追加、有 marker 非 force 时 SKIP。
MARKER="<!-- loop-plugin:begin -->"
if [ ! -f "$TARGET/AGENTS.md" ]; then
  {
    echo "# $(basename "$TARGET")"
    echo
    cat "$ASSETS/AGENTS.loop.md"
  } > "$TARGET/AGENTS.md"
  echo "  NEW   AGENTS.md（含 Loop 章节）"
elif ! grep -qF "$MARKER" "$TARGET/AGENTS.md"; then
  { echo; cat "$ASSETS/AGENTS.loop.md"; } >> "$TARGET/AGENTS.md"
  echo "  APPEND AGENTS.md ← Loop 章节"
elif [ "$FORCE" = 1 ] || [ "$MIGRATED" = 1 ]; then
  # C-08e：以 <!-- loop-plugin:begin/end --> 边界原地替换 loop 片段为 assets/AGENTS.loop.md，
  # marker 外的人工维护内容原样保留（代码职责 5）。asset 文件自身含 begin/end marker。
  awk -v assetfile="$ASSETS/AGENTS.loop.md" '
    BEGIN { asset = ""; while ((getline line < assetfile) > 0) asset = asset line "\n" }
    /<!-- loop-plugin:begin -->/ { printf "%s", asset; inblk = 1; next }
    /<!-- loop-plugin:end -->/   { inblk = 0; next }
    !inblk { print }
  ' "$TARGET/AGENTS.md" > "$TARGET/AGENTS.md.loop.tmp"
  mv "$TARGET/AGENTS.md.loop.tmp" "$TARGET/AGENTS.md"
  echo "  FORCE AGENTS.md ← Loop 章节（marker 边界原地替换）"
else
  echo "  SKIP  AGENTS.md（Loop 章节已存在）"
fi

# 4. .gitignore：git 仓库中追加缺失条目；非 git 目录显式 SKIP（C-05）。
if [ -d "$TARGET/.git" ]; then
  touch "$TARGET/.gitignore"
  if ! grep -qxF ".autome/output/" "$TARGET/.gitignore"; then
    echo ".autome/output/" >> "$TARGET/.gitignore"
    echo "  APPEND .gitignore ← .autome/output/"
  else
    echo "  SKIP  .gitignore（已含 .autome/output/）"
  fi
else
  echo "  SKIP  .gitignore（非 git 目录）"
fi

echo
echo "完成。下一步（CLI 与 Claude Code 命令任选其一）："
echo "  CLI:    autome task \"<一句话需求>\"    生成任务文件 docs/<slug>/<slug>-task.md"
echo "          autome go <slug>              启动自驱动循环（--gated 启用人工卡点停靠）"
echo "  插件:   /loop:task <一句话需求> → /loop:run <slug> task1"
echo "  （首次生成任务时会调研仓库起草项目画像 docs/agent-project-profile.md，默认停下待确认）"
echo "角色模型：.autome/skill/loop-roles.conf（生成/评测强制分模型：plan≠review、impl≠audit；autome roles 查看）"
echo "紧急停止：autome stop 或 /loop:stop（.autome/SCHEDULE.json autorun=false）"
