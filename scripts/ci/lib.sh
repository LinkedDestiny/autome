# Shared POSIX-shell step runner. The caller sets root to the repository root.
ci_tmp=$(mktemp -d) || exit 2
trap 'rm -rf "$ci_tmp"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
passed=''
failed=''
ran=0
failed_count=0
started=$(date +%s)
: > "$ci_tmp/cases"

run() {
  name=$1
  shift
  parser=''
  if [ "${1-}" = --cases ]; then
    parser=$2
    shift 2
  fi
  ran=$((ran + 1))
  printf '\n==> %s\n' "$name"
  step_started=$(date +%s)
  # A pipeline's status is tee's, not the command's, on POSIX sh.
  { "$@" 2>&1; echo $? > "$ci_tmp/rc"; } | tee "$ci_tmp/out"
  capture_rc=$?
  if [ "$capture_rc" -eq 0 ] && [ "$(cat "$ci_tmp/rc")" -eq 0 ]; then
    passed="$passed$name
"
    printf '    ok (%ss)\n' "$(($(date +%s) - step_started))"
  else
    failed="$failed$name
"
    failed_count=$((failed_count + 1))
    printf '    FAILED (%ss)\n' "$(($(date +%s) - step_started))"
    if [ -n "$parser" ]; then
      node "$root/scripts/ci/summary.mjs" extract "$parser" "$name" "$ci_tmp/out" >> "$ci_tmp/cases"
    fi
  fi
}

summary() {
  printf '\n==> 汇总\n────────────────────────────────────────\n'
  printf '%s' "$passed" | while IFS= read -r name; do
    printf '  ok      %s\n' "$name"
  done
  printf '%s' "$failed" | while IFS= read -r name; do
    printf '  FAILED  %s\n' "$name"
  done
  node "$root/scripts/ci/summary.mjs" render "$ci_tmp/cases"
  printf '执行 %s 步，沿用 0 步，失败 %s 步。\n' "$ran" "$failed_count"
  elapsed=$(($(date +%s) - started))
  if [ -z "$failed" ]; then
    printf '全部通过，用时 %ss。\n' "$elapsed"
    return 0
  fi
  printf '有步骤未通过，用时 %ss。\n' "$elapsed"
  return 1
}
