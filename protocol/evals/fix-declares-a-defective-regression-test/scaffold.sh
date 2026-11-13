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
cat > regression.sh <<'TEST'
#!/bin/sh
# 夹具应当传空串，但意外传了一个空格。
sh check.sh ' '
code=$?
[ "$code" -eq 1 ] || exit 1
sh check.sh good
TEST
git add regression.sh
git commit -qm 'locate: reproduce empty input'
