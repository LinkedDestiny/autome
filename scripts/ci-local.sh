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
unset CI_E2E_DEADLINE CI_E2E_STAGE

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2

with_package=0
ci_mode=prepare
for arg in "$@"; do
  case "$arg" in
    --package) with_package=1 ;;
    --all) export CI_LOCAL_ALL=1 ;;
    --plan) ci_mode=plan ;;
    --check-claims) ci_mode=claims ;;
    -h | --help)
      echo 'Usage: sh scripts/ci-local.sh [--package] [--all] [--plan] [--check-claims]'
      echo '--all / CI_LOCAL_ALL=1: force every step; --plan: decisions only; --check-claims: reject unclaimed paths'
      exit 0
      ;;
    *)
      echo "ci-local: 不认识的参数 $arg" >&2
      exit 2
      ;;
  esac
done

. "$root/scripts/ci/lib.sh"

. "$root/scripts/ci/steps.sh"
ci_dispatch "$ci_mode" || exit $?
[ "$ci_mode" = prepare ] || exit 0

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
    test -x "$app/Contents/Resources/core/autome" &&
    "$app/Contents/Resources/core/autome" --version < /dev/null > /dev/null
  '
  run 'Bundle launches the panel' env AUTOME_SMOKE_APP="$PWD/apps/desktop/dist/mac-arm64/Autome.app" npm --prefix apps/desktop run smoke
fi

# --- summary ---------------------------------------------------------------
summary
exit $?
