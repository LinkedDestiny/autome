#!/bin/sh
# Build the CLI core, stage the Web panel, then package the Electron shell.
# Signing remains opt-in via CSC_NAME.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
desktop="$root/apps/desktop"
staging="$desktop/core"

echo "==> cargo build --release"
cargo build --release --manifest-path "$root/Cargo.toml" -p autome-cli

echo "==> staging the core binary"
rm -rf "$staging"
mkdir -p "$staging"
cp "$root/target/release/autome" "$staging/autome"
chmod 755 "$staging/autome"

npm --prefix "$desktop" run stage-web

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
