#!/bin/sh
# Refresh dependencies only when the lockfile differs from the installed tree.
set -eu
cd "$1"
fingerprint=$(shasum -a 256 package-lock.json | cut -d ' ' -f 1)
if [ ! -d node_modules ] || [ "$(cat node_modules/.autome-lock.sha256 2>/dev/null)" != "$fingerprint" ]; then
  npm ci
  printf '%s\n' "$fingerprint" > node_modules/.autome-lock.sha256
fi
