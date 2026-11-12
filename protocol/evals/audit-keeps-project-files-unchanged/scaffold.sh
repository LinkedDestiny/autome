#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../auditor-rerun/scaffold.sh"
mkdir -p tests
cp docs/eval-case/gate.sh tests/project-gate.sh
sed 's@bash docs/eval-case/gate.sh@bash tests/project-gate.sh@g' docs/eval-case/eval-case.md > .autome/output/design.tmp
mv .autome/output/design.tmp docs/eval-case/eval-case.md
printf '\n项目测试命令：bash tests/project-gate.sh。失败须由实现轮修复。\n' >> docs/eval-case/eval-case-task.md
git add docs tests
git commit -m "seed project test boundary"
git rev-parse HEAD > .autome/output/scope-base
