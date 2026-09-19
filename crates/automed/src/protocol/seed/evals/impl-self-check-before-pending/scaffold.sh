#!/usr/bin/env bash
# 从 fixture/ 搭出最小 worktree。只有任务文档，没有项目代码——
# 用例断言的是会话的头几步动作，不是任务能不能做完。
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p docs .autome/output/sessions
cp -R "$here/fixture/." docs/
