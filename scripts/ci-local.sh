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
#   sh scripts/ci-local.sh --package    also build the .app and the CLI tarball
#                                       (minutes; the first run downloads
#                                       electron-builder)
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
      sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "ci-local: 不认识的参数 $arg" >&2
      exit 2
      ;;
  esac
done

. "$root/scripts/ci/lib.sh"

# --- job: rust -------------------------------------------------------------
# ci.yml runs these as one job, in this order, with `-D warnings` because a
# warning that is allowed to accumulate stops being read.
run 'Format' cargo fmt --all -- --check
run 'Clippy' cargo clippy --all-targets -- -D warnings
# --all-targets includes end_to_end and real_cli (including adapter flags).
# Run each suite once: repeating the real-Git integration suite can push the
# host gate past its deadline. Keep uncaptured output for integration failures.
run 'Test (unit, end to end, adapter flags)' --cases cargo cargo test --workspace --all-targets --no-fail-fast -- --nocapture

run 'Scripts' node --test scripts/test/*.test.mjs

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

# --- job: web --------------------------------------------------------------
if [ ! -d apps/web/node_modules ]; then
  run 'Web install' sh -c 'cd apps/web && npm ci'
fi
run 'Web lint' npm run lint --prefix apps/web
run 'Web typecheck' npm run typecheck --prefix apps/web
run 'Web test' --cases vitest npm run test --prefix apps/web
run 'Web build' npm run build --prefix apps/web

# --- job: package ----------------------------------------------------------
# Opt-in. A release build plus electron-builder is minutes, which is the wrong
# price for something that runs on every commit — but it breaks quietly and is
# otherwise discovered at release, which is the one time it must not.
if [ "$with_package" -eq 1 ]; then
  run 'Package' sh scripts/package.sh
  run 'CLI package' sh -c '
    set -e
    sh scripts/package-cli.sh
    scratch=$(mktemp -d)
    trap '\''rm -rf "$scratch"'\'' EXIT
    trap '\''exit 129'\'' HUP
    trap '\''exit 130'\'' INT
    trap '\''exit 143'\'' TERM
    prefix="$scratch/bin"
    ledger_home="$scratch/home"
    mkdir -p "$prefix" "$ledger_home"
    PREFIX="$prefix" AUTOME_HOME="$ledger_home" sh scripts/install-cli.sh --from dist/autome-*.tar.gz > /dev/null
    "$prefix/autome" --version
  '
  run 'Bundle carries a runnable core' sh -c '
    app="apps/desktop/dist/mac-arm64/Autome.app"
    test -x "$app/Contents/Resources/core/automed" &&
    "$app/Contents/Resources/core/automed" --help < /dev/null > /dev/null
  '
fi

# --- summary ---------------------------------------------------------------
summary
exit $?
