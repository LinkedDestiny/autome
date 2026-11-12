# M-01 r1 审计复验

结论：通过。

负向对照（逐项还原，`git status` 干净）：
- `servicesContext` 去掉 `Symbol.for` 登记 → 第 2、6、7 条变红（3 failed）。
- 删掉 `server/http.ts` 生产分支的 `getLoadContext` → 第 8 条变红。
- 根 loader 改成 `gateway.core.call` → `npm run typecheck` 报 TS2339「`core` 不在 `KernelGateway`」；load-context 第 3、4、5、6、7 类用例变红（4 failed）。

验收命令（自己跑）：
- 点名 3 个文件：24 个用例全过。
- lint 退出码 0；typecheck 0；build 0。
- 全量 `npm test --prefix apps/web`：34 文件、310 用例全过，无 skipped，与预期一致（实现轮受阻的全量验收在本轮补上）。
- 测试文件不含 listen/location，未监听端口。

未验：apps/desktop 缺 Electron 与本里程碑无关。原始输出：`.autome/output/a1-*.txt`。
