#!/bin/sh
# Structure checks use the repository's one-key-per-line JSON convention.
set -u
plugin=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
market=$(dirname "$plugin")
repo=$(CDPATH= cd -- "$market/.." && pwd)
passed=0
total=0
check() {
  total=$((total + 1))
  if "$@"; then passed=$((passed + 1)); printf 'ok %s\n' "$1"
  else printf 'not ok %s：断言不满足\n' "$1"; fi
}
field() { sed -n 's/^  "'"$2"'": "\([^"]*\)"[,]*$/\1/p' "$1"; }
manifest() { printf '%s/.%s-plugin/plugin.json' "$plugin" "$1"; }
entry() {
  # Each marketplace entry begins at exactly four spaces; nested objects do not.
  sed -n '/^    {$/,/^    }[,]*$/p' "$1" | awk -v name="$2" '
    /^    \{$/ { block=""; found=0 }
    { block=block $0 "\n" }
    $0 ~ "\"name\": \"" name "\"" { found=1 }
    /^    }[,]?$/ && found { printf "%s", block }'
}
S1() {
  for host in claude codex cursor; do
    [ -f "$(manifest "$host")" ] && [ "$(field "$(manifest "$host")" name)" = autome ] || return 1
  done
}
S2() {
  version=$(field "$(manifest claude)" version)
  [ -n "$version" ] && [ "$version" = "$(field "$(manifest codex)" version)" ] && [ "$version" = "$(field "$(manifest cursor)" version)" ]
}
S3() {
  description=$(field "$(manifest claude)" description)
  [ -n "$description" ] && [ "$description" = "$(field "$(manifest codex)" description)" ] && [ "$description" = "$(field "$(manifest cursor)" description)" ]
}
S4() {
  [ "$(field "$(manifest claude)" commands)" = './claude-commands/' ] &&
  [ "$(field "$(manifest codex)" skills)" = './skills/' ] &&
  [ "$(field "$(manifest cursor)" skills)" = './skills/' ] &&
  [ -d "$plugin/claude-commands" ] && [ -d "$plugin/skills" ]
}
S5() {
  for key in displayName shortDescription developerName category; do
    value=$(sed -n '/"interface": {/,/  }/p' "$(manifest codex)" | sed -n 's/^[[:space:]]*"'"$key"'": "\([^"]*\)"[,]*$/\1/p')
    [ -n "$value" ] || return 1
  done
}
S6() {
  for host in claude cursor; do
    block=$(entry "$market/.$host-plugin/marketplace.json" autome)
    [ "$(printf '%s\n' "$block" | grep -c '"name": "autome"')" = 1 ] || return 1
    printf '%s\n' "$block" | grep -q '"source": "./autome"' || return 1
  done
  block=$(entry "$market/.agents/plugins/marketplace.json" autome)
  for text in '"name": "autome"' '"source": "local"' '"path": "./autome"' '"policy": {' '"installation": "AVAILABLE"' '"authentication": "ON_INSTALL"' '"category": "Productivity"'; do
    printf '%s\n' "$block" | grep -Fq "$text" || return 1
  done
}
S7() {
  for file in .claude-plugin .cursor-plugin .agents/plugins; do
    block=$(entry "$market/$file/marketplace.json" loop)
    printf '%s\n' "$block" | grep -q '"name": "loop"' || return 1
    printf '%s\n' "$block" | grep -Eq '"(source|path)": "./loop"' || return 1
  done
}
S8() {
  file="$plugin/skills/autome-task/SKILL.md"
  [ "$(head -1 "$file")" = '---' ] &&
  sed -n '2,/^---$/p' "$file" | grep -qx 'name: autome-task' || return 1
  sed -n '2,/^---$/p' "$file" | grep -Eq '^description: .+' && ! grep -q 'disable-model-invocation' "$file"
}
S9() {
  grep -Fq '${CLAUDE_PLUGIN_ROOT}/skills/autome-task/SKILL.md' "$plugin/claude-commands/task.md" &&
  ! grep -Fq 'autome task new' "$plugin/claude-commands/task.md"
}
S10() {
  base=$(git -C "$repo" merge-base HEAD main) || return 1
  git -C "$repo" diff --quiet "$base" -- plugins/loop/ &&
  [ -z "$(git -C "$repo" ls-files --others --exclude-standard -- plugins/loop/)" ]
}
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
. "$plugin/tests/card-panel.sh"
. "$plugin/tests/skill.sh"
. "$plugin/tests/readme.sh"
[ "$#" -gt 0 ] || set -- all
[ "$*" != all ] || set -- structure card panel skill readme
for group do
  case "$group" in
    structure) for test in S1 S2 S3 S4 S5 S6 S7 S8 S9 S10; do check "$test"; done ;;
    readme) for test in R1 R2 R3 R4 R5; do check "$test"; done ;;
    skill) for test in T1 T2 T3 T4 T5 T6 T7 T8 T9 T10; do check "$test"; done ;;
    card) for test in K1 K2 K3 K4 K5 K6 K7 K8 K9 K10 K11 K12 K13 K14 K15 K16 K17 K18 K19; do check "$test"; done ;;
    panel) for test in P1 P2 P3 P4 P5 P6 P7 P8 P9 P10; do check "$test"; done ;;
    *) printf '未知检查组：%s\n' "$group" >&2; exit 2 ;;
  esac
done
printf '%s/%s 通过\n' "$passed" "$total"
[ "$passed" -eq "$total" ]
