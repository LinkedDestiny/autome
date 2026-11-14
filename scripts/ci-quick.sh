#!/bin/sh
# Static checks share complete-gate keys; related tests never publish full passes.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
ci_mode=quick-prepare
while [ "$#" -gt 0 ]; do
  case "$1" in
    --plan) ci_mode=quick-plan ;;
    --all) export CI_LOCAL_ALL=1 ;;
    --since)
      [ "$#" -ge 2 ] && [ -n "$2" ] || exit 2
      export CI_QUICK_SINCE=$2
      shift ;;
    -h|--help) echo 'Usage: sh scripts/ci-quick.sh [--plan] [--since <rev>] [--all]'; exit 0 ;;
    *) echo "ci-quick: 不认识的参数 $1" >&2; exit 2 ;;
  esac
  shift
done
case "${CI_QUICK_BUDGET-120}" in ''|*[!0-9]*) echo 'CI_QUICK_BUDGET 必须是非负整数秒' >&2; exit 2 ;; esac
CI_QUICK_BUDGET=$(node -e 'const n = Number(process.env.CI_QUICK_BUDGET ?? 120); if (!Number.isSafeInteger(n) || n < 0) process.exit(2); console.log(n)') || exit 2
export CI_QUICK_BUDGET
. "$root/scripts/ci/lib.sh"
. "$root/scripts/ci/steps.sh"
ci_dispatch "$ci_mode" || exit $?
[ "$ci_mode" = quick-prepare ] || exit 0
summary
