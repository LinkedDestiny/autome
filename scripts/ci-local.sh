#!/bin/sh
# Runs, locally, what .github/workflows/ci.yml runs on a push.
#
# The workflow has never actually executed: this repository has no git remote,
# so nothing ever pushed. Its Format and Clippy gates sat red — 147 and 38
# findings — without anyone being told. Until there is a remote to push to,
# this script is the only thing that runs them.
#
# Usage:
#   sh scripts/ci-local.sh              the fast jobs (~35s with a warm target/)
#   sh scripts/ci-local.sh --package    also build the .app (minutes, and the
#                                       first run downloads electron-builder)
#
# Every step runs even when an earlier one fails. The workflow's jobs are
# parallel, so stopping at the first red would hide everything behind it — and
# "clippy is angry" plus "a test broke" is a different morning from either one
# alone. Exit status is non-zero if any step failed.
#
# `.githooks/post-commit` runs this in the background after every commit. To
# run it by hand, just call it; it takes no state from the hook.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2

with_package=0
for arg in "$@"; do
  case "$arg" in
    --package) with_package=1 ;;
    -h | --help)
      sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "ci-local: 不认识的参数 $arg" >&2
      exit 2
      ;;
  esac
done

# Colour only when someone is watching. The hook redirects to a log file, and
# escape codes in a log are noise.
if [ -t 1 ]; then
  b=$(printf '\033[1m')
  r=$(printf '\033[0m')
  red=$(printf '\033[31m')
  green=$(printf '\033[32m')
else
  b='' r='' red='' green=''
fi

# Newline-separated, not space-separated: step names have spaces in them, and
# `for name in $passed` would print "End to end" as three steps.
failed=''
passed=''
started=$(date +%s)

run() {
  name=$1
  shift
  printf '\n%s==> %s%s\n' "$b" "$name" "$r"
  step_started=$(date +%s)
  if "$@"; then
    passed="$passed$name
"
    printf '%s    ok%s (%ss)\n' "$green" "$r" "$(($(date +%s) - step_started))"
  else
    failed="$failed$name
"
    printf '%s    FAILED%s (%ss)\n' "$red" "$r" "$(($(date +%s) - step_started))"
  fi
}

# --- job: rust -------------------------------------------------------------
# ci.yml runs these as one job, in this order, with `-D warnings` because a
# warning that is allowed to accumulate stops being read.
run 'Format' cargo fmt --all -- --check
run 'Clippy' cargo clippy --all-targets -- -D warnings
run 'Test' cargo test --workspace --all-targets
# Separately from the unit suites so a failure here reads as "the loop broke"
# rather than being buried in the unit output.
run 'End to end' cargo test --test end_to_end -- --nocapture
# The free half of the real-CLI gate: asks each installed CLI for its own
# --help and checks every flag the adapter table passes appears in it. Skips
# whichever CLI this machine does not have.
run 'Adapter flags' cargo test --test real_cli every_adapter_flag -- --nocapture

# --- job: desktop ----------------------------------------------------------
# ci.yml does `npm ci || npm install` on a bare runner. Here node_modules is
# normally already there, and reinstalling it on every commit would dominate
# the runtime.
if [ ! -d apps/desktop/node_modules ]; then
  run 'Desktop install' sh -c 'cd apps/desktop && { npm ci || npm install; }'
fi
run 'Desktop' npm test --prefix apps/desktop

# --- job: contract ---------------------------------------------------------
# A guard rather than a gate: the desktop suite cross-checks the read
# allowlist by parsing dispatch.rs, so this fails loudly if the file it parses
# ever moves or the constant is renamed.
run 'Contract' sh -c '
  test -f crates/automed/src/dispatch.rs &&
  grep -q "pub const READ_METHODS" crates/automed/src/dispatch.rs
'

# --- job: package ----------------------------------------------------------
# Opt-in. A release build plus electron-builder is minutes, which is the wrong
# price for something that runs on every commit — but it breaks quietly and is
# otherwise discovered at release, which is the one time it must not.
if [ "$with_package" -eq 1 ]; then
  run 'Package' sh scripts/package.sh
  run 'Bundle carries a runnable core' sh -c '
    app="apps/desktop/dist/mac-arm64/Autome.app"
    test -x "$app/Contents/Resources/core/automed" &&
    "$app/Contents/Resources/core/automed" --help < /dev/null > /dev/null
  '
fi

# --- summary ---------------------------------------------------------------
elapsed=$(($(date +%s) - started))
printf '\n%s%s%s\n' "$b" '────────────────────────────────────────' "$r"
printf '%s' "$passed" | while IFS= read -r name; do
  printf '%s  ok%s      %s\n' "$green" "$r" "$name"
done
printf '%s' "$failed" | while IFS= read -r name; do
  printf '%s  FAILED%s  %s\n' "$red" "$r" "$name"
done

if [ -z "$failed" ]; then
  printf '\n%s全部通过%s，用时 %ss。\n' "$green" "$r" "$elapsed"
  if [ "$with_package" -eq 0 ]; then
    printf '（未跑 package —— 加 --package 打包，需要几分钟。）\n'
  fi
  exit 0
fi

printf '\n%s有步骤未通过%s，用时 %ss。\n' "$red" "$r" "$elapsed"
exit 1
