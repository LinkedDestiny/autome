#!/bin/sh
# 判别检查：完整 Web 测试必须能启动并跑出用例（当前应红：groupOrder 冲突，no tests）
cd apps/web && npm run test 2>&1 | tee /dev/stderr | grep -q "Test Files .*passed" 
