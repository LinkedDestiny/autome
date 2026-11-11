#!/bin/sh
set -u
plugin=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
market=$(dirname "$plugin")
repo=$(dirname "$market")
output="$repo/.autome/output"
mkdir -p "$output"
work=$(mktemp -d "$output/autome-hosts.XXXXXX") || exit 1
CODEX_HOME="$work/codex"
export CODEX_HOME
mkdir -p "$CODEX_HOME"
passed=0
total=0
check() {
  total=$((total + 1))
  if "$@" > "$work/$1.log" 2>&1; then
    passed=$((passed + 1)); printf 'ok %s\n' "$1"
  else printf 'not ok %s：%s\n' "$1" "$work/$1.log"; fi
}
H1() { claude plugin validate "$market"; }
H2() { claude plugin validate "$plugin" --strict; }
H3() {
  claude --plugin-dir "$plugin" plugin details autome > "$work/details.txt" 2>&1 &&
  grep -Fq 'Skills (2)' "$work/details.txt" &&
  grep -wq autome-task "$work/details.txt" && grep -wq task "$work/details.txt"
}
H4() {
  codex plugin marketplace add "$market" &&
  codex plugin add autome@autome &&
  codex plugin list > "$work/list.txt" &&
  grep -Eq 'autome@autome[[:space:]]+installed, enabled[[:space:]]+0\.1\.0([[:space:]]|$)' "$work/list.txt"
}
H5() { test -f "$CODEX_HOME/plugins/cache/autome/autome/0.1.0/skills/autome-task/SKILL.md"; }
for test in H1 H2 H3 H4 H5; do check "$test"; done
# Only logs are retained; do not accumulate installed test plugins.
rm -rf "$CODEX_HOME"
printf '%s/%s 通过\n' "$passed" "$total"
[ "$passed" -eq "$total" ]
