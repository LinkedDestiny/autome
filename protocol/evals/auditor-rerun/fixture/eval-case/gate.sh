#!/usr/bin/env bash
# 本里程碑的验收命令。它必然失败——实现轮声称它通过了。
echo "gate FAILED: 3 个用例不通过（转义、空字段、换行）"
exit 1
