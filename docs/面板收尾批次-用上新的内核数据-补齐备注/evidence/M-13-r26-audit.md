# M-13 r26 集成复验（审计 #18）

范围：e4f574a（3 个文件；apps/web/e2e/harness/fixture.ts、test-groups.ts、test/e2e-fixture.test.ts）。

1. 改动只把 e2e 夹具 w7-flow.sh 的超时 60s→150s，与 task-detail-real / task-docs-real / task-merge-real 的预算一致，只针对触发原因（ETIMEDOUT）。rebase 丢失提交 0 个。
2. 测试文件 e2e-fixture.test.ts 是新增，无既有断言被削弱；test-groups.ts 只加一行（kernel 组）。
3. 冲突标记 `git grep` 对 e4f574a：结果为空。
4. 负向对照：把 fixture.ts 超时改回 60_000，e2e-fixture 两条 60001/150000ms 用例失败（2 failed | 2 passed）；已还原，git status 干净。
5. 验收命令（M-13）：build + task-detail-ssr/payload/test-groups = 3 文件 11 用例全过；e2e-fixture+test-groups 8 用例全过。
结论：通过，里程碑表不动。备注：耗时 150s 的合理性只靠门禁重跑证明，真实夹具耗时不在本轮复验范围。
