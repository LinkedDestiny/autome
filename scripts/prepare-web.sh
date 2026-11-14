#!/bin/sh
# Install lockfile-matched dependencies, then always rebuild the panel.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "${1:-$root/apps/web}"
lock=$(shasum -a 256 package-lock.json)
lock=${lock%% *}
fingerprint=node_modules/.autome-lock.sha256
if [ ! -d node_modules ] || [ "$(cat "$fingerprint" 2>/dev/null)" != "$lock" ]; then
  npm ci
  printf '%s\n' "$lock" > "$fingerprint"
fi
npm run build
