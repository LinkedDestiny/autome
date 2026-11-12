#!/bin/sh
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p docs .autome/output docs/eval-case/protocol
cp -R "$here/fixture/." docs/
cp "$here/../../loop-protocol.md" "$here/../../session-protocol.md" "$here/../../bug-protocol.md" docs/eval-case/protocol/
printf 'exit 0\n' > check-m01.sh
printf 'mkdir -p .autome/output\nprintf "ran\\n" >> .autome/output/full-gate-ran\n' > full-gate.sh
git init -q
git config user.name Eval
git config user.email eval@example.invalid
git add .
git commit -qm baseline
