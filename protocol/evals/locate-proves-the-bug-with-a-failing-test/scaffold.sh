#!/bin/sh
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p docs .autome/output docs/eval-case/protocol
cp -R "$here/fixture/." docs/
cp "$here/../../bug-protocol.md" docs/eval-case/protocol/
git init -q
git config user.name 'Eval'
git config user.email 'eval@example.invalid'
printf 'exit 0\n' > check.sh
git add .
git commit -qm baseline
