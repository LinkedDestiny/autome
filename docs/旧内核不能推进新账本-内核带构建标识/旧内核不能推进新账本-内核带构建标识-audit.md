# 审计结论（审计 #7）

## M-07 结论：实现缺陷，退回开放（reopen 1，领域 package-isolation）

- 失败命令：`npm run test --prefix apps/web -- test/ci-config.test.ts`，输出尾部：`Tests  1 failed | 7 passed (8)`，失败于 `expect(path.startsWith(resolve(sandbox, 'tmp') + '/')).toBe(true)`。
- 违反：设计 M-07 验收命令「预期 8 个用例通过」。
- 根因：macOS 的 `mktemp -d` 忽略 `TMPDIR`（实测），新增用例却假定 scratch 在 `TMPDIR` 之下。
- 详见 `evidence/M-07-r8-audit.md`。
- 判别检查：本轮没有留下可搬的检查；失败的就是项目内那条用例本身，在本机复跑即红。
