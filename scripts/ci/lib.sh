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
: > "$ci_tmp/not-run"

run() {
  name=$1
  shift
  parser=''
  if [ "${1-}" = --cases ]; then
    parser=$2
    shift 2
  fi
  ci_browser_not_run=0
  if [ "${ci_browser_active-0}" = 1 ]; then
    rm -f "$root/.autome/output/playwright-gate/report.json"
  fi
  ran=$((ran + 1))
  printf '\n==> %s\n' "$name"
  step_started=$(date +%s)
  # A pipeline's status is tee's, not the command's, on POSIX sh.
  { "$@" 2>&1; echo $? > "$ci_tmp/rc"; } | tee "$ci_tmp/out"
  capture_rc=$?
  if [ "${ci_browser_active-0}" = 1 ]; then
    node "$root/scripts/ci/e2e-result.mjs" "$(cat "$ci_tmp/rc")" > "$ci_tmp/e2e-result"
    ci_result=$?
    cat "$ci_tmp/e2e-result"
    if [ "$ci_result" -eq 3 ]; then
      ci_browser_not_run=1
      ci_not_run "Web e2e（$(cat "$ci_tmp/e2e-result")）"
      return
    fi
    [ "$ci_result" -eq 0 ] || capture_rc=1
  fi
  if [ "$capture_rc" -eq 0 ] && [ "$(cat "$ci_tmp/rc")" -eq 0 ]; then
    passed="$passed$name
"
    printf '    ok (%ss)%s\n' "$(($(date +%s) - step_started))" "${ci_active_key:+ · 输入 $ci_active_key}"
  else
    failed="$failed$name
"
    failed_count=$((failed_count + 1))
    printf '    FAILED (%ss)%s\n' "$(($(date +%s) - step_started))" "${ci_active_key:+ · 输入 $ci_active_key}"
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
  cat "$ci_tmp/reused"
  cat "$ci_tmp/not-run"
  node "$root/scripts/ci/summary.mjs" render "$ci_tmp/cases"
  printf '执行 %s 步，沿用 %s 步，失败 %s 步。\n' "$ran" "$reused" "$failed_count"
  elapsed=$(($(date +%s) - started))
  if [ -z "$failed" ]; then
    printf '全部通过，用时 %ss。\n' "$elapsed"
    return 0
  fi
  printf '有步骤未通过，用时 %ss。\n' "$elapsed"
  return 1
}

# Normalize binary selection before toolchain probing or any child command.
for ci_var in AUTOMED_BIN AUTOME_BIN CARGO_BUILD_TARGET CARGO_TARGET_DIR; do
  eval 'ci_value=${'"$ci_var"'-}'
  if [ -n "$ci_value" ]; then
    printf '已忽略环境变量 %s（门禁固定使用 target/debug 下本次构建的内核）\n' "$ci_var"
  fi
  unset "$ci_var"
done
reused=0
: > "$ci_tmp/reused"
: > "$ci_tmp/steps"

step() {
  node "$root/scripts/ci/inputs.mjs" declare "$ci_tmp/steps" step "$@" || exit 2
}

prep() {
  ci_prep_name=$1
  shift
  node "$root/scripts/ci/inputs.mjs" declare "$ci_tmp/steps" prep "$ci_prep_name" '' "$@" || exit 2
}

ci_dispatch() {
  export GLOBAL_INPUTS NO_GATE
  node "$root/scripts/ci/inputs.mjs" "$1" "$ci_tmp/steps" "$ci_tmp/execute" || return $?
  case "$1" in prepare|quick-prepare) ;; *) return 0 ;; esac
  if [ -n "${AUTOME_GATE_REPORT-}" ]; then
    printf 'autome-gate-report 1\n' > "$AUTOME_GATE_REPORT" || return 2
  fi
  . "$ci_tmp/execute"
}

ci_quick_start() {
  ci_quick_started=$(date +%s)
  ci_quick_budget=${CI_QUICK_BUDGET-120}
}

ci_not_run() {
  printf '  未跑    %s\n' "$1" >> "$ci_tmp/not-run"
  ci_report not_run - "$1"
}

ci_quick_admit() {
  if [ "$(($(date +%s) - ci_quick_started))" -ge "$ci_quick_budget" ]; then
    ci_not_run "$1（超出快检预算 ${ci_quick_budget}s，留给合并前门禁）"
    return 1
  fi
  return 0
}

ci_partial() {
  ci_partial_name=$1
  ci_partial_failures=$failed_count
  run "$@"
  if [ "${ci_browser_not_run-0}" = 1 ]; then
    return
  elif [ "$failed_count" -gt "$ci_partial_failures" ]; then
    ci_report failed - "$ci_partial_name"
  else
    ci_report partial - "$ci_partial_name"
  fi
}

ci_web_related() {
  ci_web_failures=$failed_count
  run 'Discover Web related files' node "$root/apps/web/scripts/quick-related.mjs" "$ci_tmp/web-related" "$@"
  if [ "$failed_count" -gt "$ci_web_failures" ]; then
    ci_report failed - 'Discover Web related files'
    return
  fi
  . "$ci_tmp/web-related"
}

ci_report() {
  if [ -n "${AUTOME_GATE_REPORT-}" ]; then
    printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$AUTOME_GATE_REPORT" || exit 2
  fi
}

execute_prep() {
  ci_prep_name=$1
  shift
  if [ "${1-}" = --if-missing ]; then
    [ ! -d "$2" ] || return 0
    shift 2
  fi
  run "$ci_prep_name" "$@"
}

execute_step() {
  ci_index=$1 ci_key=$2 ci_source=$3 ci_name=$4
  shift 4
  if [ -n "$ci_source" ]; then
    printf '\n==> %s\n    沿用 ← %s · 输入 %s\n' "$ci_name" "$ci_source" "$ci_key"
    printf '  沿用    %s ← %s · 输入 %s\n' "$ci_name" "$ci_source" "$ci_key" >> "$ci_tmp/reused"
    reused=$((reused + 1))
    ci_report reused "$ci_key" "$ci_name"
    return 0
  fi
  ci_previous_failures=$failed_count
  ci_active_key=$ci_key
  run "$ci_name" "$@"
  ci_active_key=''
  if [ "${ci_browser_not_run-0}" = 1 ]; then
    return
  elif [ "$failed_count" -gt "$ci_previous_failures" ]; then
    ci_report failed "$ci_key" "$ci_name"
  elif ci_after=$(node "$root/scripts/ci/inputs.mjs" key "$ci_tmp/steps" "$ci_index") && [ "$ci_after" = "$ci_key" ]; then
    ci_report ran "$ci_key" "$ci_name"
  else
    printf '    输入已变化或无法复核，不记录通过：%s\n' "$ci_name"
    ci_report partial - "$ci_name"
  fi
}

# Independent browser budget includes preparation, never the quick allowance.
ci_e2e_start() {
  ci_e2e_started=$(date +%s)
  ci_e2e_failures=$failed_count
}

ci_e2e_admit() {
  if [ "$failed_count" -gt "$ci_e2e_failures" ]; then
    ci_not_run 'Web e2e（Web build / Build core / Web install 未通过，未跑浏览器用例）'
    return 1
  fi
  ci_deadline=$(node -e 'const end = Math.min((Number(process.argv[1]) + Number(process.env.CI_E2E_BUDGET ?? 720)) * 1000, (Number(process.env.AUTOME_GATE_DEADLINE || Number(process.argv[2]) + 600) - 60) * 1000); console.log(Math.floor(end / 1000))' "$ci_e2e_started" "$started")
  ci_remaining=$((ci_deadline - $(date +%s)))
  if [ "$ci_remaining" -lt 240 ]; then
    ci_not_run "Web e2e（剩余 ${ci_remaining} 秒不够浏览器用例的准备阶段，留给合并前门禁）"
    return 1
  fi
  export CI_E2E_DEADLINE=$((ci_deadline * 1000)) CI_E2E_STAGE=quick
  export CI_E2E_REPORT_DIR="$root/.autome/output/playwright-gate"
  return 0
}
