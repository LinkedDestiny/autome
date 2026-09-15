#!/bin/sh
# Builds a distributable macOS app. Technical design §18, milestone M4.
#
# Three steps, in this order for a reason:
#
#   1. Build the core in release mode. A debug `automed` in a shipped bundle
#      would be slow and would carry paths from the build machine.
#   2. Stage it into the Electron app under `core/`, where `sidecar.js` looks
#      for it when packaged (see `packagedBinaryPath`).
#   3. Package and, if a signing identity is configured, sign and notarise.
#
# Signing is deliberately opt-in rather than best-effort: an unsigned build
# that silently claims to be signed is worse than one that says it is not.
#
#   CSC_NAME="Developer ID Application: …"   sign with that identity
#   APPLE_ID / APPLE_APP_SPECIFIC_PASSWORD / APPLE_TEAM_ID   also notarise
#
# With none of those set the build still produces a runnable .app for local
# use; macOS will quarantine it on another machine, which is correct.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
desktop="$root/apps/desktop"
staging="$desktop/core"

echo "==> cargo build --release"
cargo build --release --manifest-path "$root/Cargo.toml" -p automed

echo "==> staging the core binary"
rm -rf "$staging"
mkdir -p "$staging"
cp "$root/target/release/automed" "$staging/automed"
chmod 755 "$staging/automed"

cd "$desktop"
if [ ! -d node_modules/electron-builder ]; then
  echo "==> installing electron-builder"
  npm install --no-save electron-builder@26
fi

if [ -n "${CSC_NAME:-}" ]; then
  echo "==> packaging, signed as: $CSC_NAME"
else
  echo "==> packaging unsigned (set CSC_NAME to sign)"
  export CSC_IDENTITY_AUTO_DISCOVERY=false
fi

npx electron-builder --mac --publish never

echo "==> done: $desktop/dist"
