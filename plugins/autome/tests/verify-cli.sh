#!/bin/sh
set -u
group=${1:-skill}
case "$group" in skill|readme) ;; *) exit 2 ;; esac
plugin=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
AUTOME_HOME="$tmp/home"
export AUTOME_HOME
unset AUTOMED_DB_PATH
[ "$AUTOME_HOME" != "$HOME/.autome" ] || exit 1
mkdir -p "$tmp/repo"
git -C "$tmp/repo" -c init.defaultBranch=master init -q || exit 1
autome version
# No runner or session is launched: task new only records scheduling state.
autome project add "$tmp/repo" --skip-onboarding > "$tmp/add" 2>&1 || { cat "$tmp/add"; exit 1; }
. "$plugin/tests/card-panel.sh"
. "$plugin/tests/readme.sh"
project_id=$(autome project ls --json | sed -n 's/.*"id": "\([^"]*\)".*/\1/p' | head -1)
[ -n "$project_id" ] || exit 1
template=$(sed -n '/^<!-- create-command -->$/,/^<!-- \/create-command -->$/p' "$plugin/skills/autome-task/SKILL.md" | sed '1d;$d')
passed=0; total=0
check() { total=$((total+1)); if "$@"; then passed=$((passed+1)); printf 'ok %s\n' "$1"; else printf 'not ok %s\n' "$1"; fi; }
create() {
  # Substitute only controlled fixture paths and enum/id values, never card data.
  command_text=$(printf '%s\n' "$template" | sed "s/{{KIND}}/$1/g;s/{{PROJECT_ID}}/$2/g;s|{{CARD_FILE}}|$tmp/card|g")
  [ "$3" = draft ] || command_text=$(printf '%s\n' "$command_text" | sed 's/ --draft//')
  (cd /tmp && sh -c "$command_text") > "$tmp/result" 2> "$tmp/error"
}
V1() {
  autome project ls --json > "$tmp/projects" &&
  grep -Fq "$(CDPATH= cd "$tmp/repo" && pwd -P)" "$tmp/projects" && grep -q '"removed_at": null' "$tmp/projects"
}
V2() {
  feature > "$tmp/card"
  printf 'AUTOME_CARD_EOF\ntouch %s/probe\n' "$tmp" >> "$tmp/card"
  create feature "$project_id" draft || return 1
  for text in '"id": "T-1"' '"kind": "feature"' '"state": "draft"' '"started": false' '测试'; do grep -Fq "$text" "$tmp/result" || return 1; done
  autome status T-1 --json > "$tmp/status" || return 1
  [ ! -e "$tmp/probe" ] && grep -Fq '"request": "测试"' "$tmp/result" && grep -Fq 'AUTOME_CARD_EOF' "$tmp/status" && grep -Fq "touch $tmp/probe" "$tmp/status"
}
V3() { bug > "$tmp/card"; create bug "$project_id" active && grep -q '"started": true' "$tmp/result" && grep -q '"kind": "bug"' "$tmp/result"; }
V4() { feature > "$tmp/card"; create feature prj-不存在 draft; code=$?; [ "$code" = 1 ] && grep -q '^autome：没有叫' "$tmp/error"; }
V5() {
  sized_card 65537
  sh "$scripts/check-card.sh" feature < "$tmp/card" > "$tmp/check"; code=$?
  [ "$code" = 1 ] || return 1
  create feature "$project_id" draft; code=$?
  [ "$code" = 1 ] && grep -q '超过 64KB' "$tmp/error"
}
V6() { sized_card 65536; sh "$scripts/check-card.sh" feature < "$tmp/card" > "$tmp/check" && create feature "$project_id" draft; }
V7() {
  autome project ls --json > "$tmp/projects" || return 1
  feature > "$tmp/card"
  autome task new --brief - --kind feature --draft -p "$project_id" --json < "$tmp/card" > "$tmp/result" || return 1
  compare_example task-new-json "$tmp/result"
}
V8() { compare_example project-ls-json "$tmp/projects"; }
V9() {
  autome task new --brief - --kind feature --draft -p "$project_id" < "$tmp/card" > "$tmp/plain" || return 1
  example task-new-text > "$tmp/expected"
  diff -u "$tmp/expected" "$tmp/plain"
}
V10() (
  # Use V7's real draft response; never mutate the checked-in README.
  compare_example task-new-json "$tmp/result" || exit 1
  sed 's/"state": "draft"/"state": "pending"/' "$readme" > "$tmp/pending-readme.md"
  readme="$tmp/pending-readme.md"
  example task-new-json | grep -q '"state": "pending"' || exit 1
  compare_example task-new-json "$tmp/result" > "$tmp/pending-diff" 2>&1
  code=$?
  [ "$code" = 1 ] && grep -q '"state": "pending"' "$tmp/pending-diff" &&
    grep -q '"state": "draft"' "$tmp/pending-diff"
)
case "$group" in
  skill) tests='V1 V2 V3 V4 V5 V6' ;;
  readme) tests='V7 V8 V9 V10' ;;
esac
for test in $tests; do check "$test"; done
printf '%s/%s 通过\n' "$passed" "$total"
[ "$passed" = "$total" ]
