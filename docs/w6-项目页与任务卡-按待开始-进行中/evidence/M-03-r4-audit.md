# M-03 · r4 · 审计复验

- 验收命令 `cd apps/web && npx vitest run test/uploads.test.ts test/uploads-http.test.ts test/gateway.test.ts test/load-context.test.ts`：4 文件、37 用例通过、0 失败（uploads 8 + uploads-http 6 + gateway 13 + load-context 10），与预期总数一致；本环境可监听，HTTP 6 项全部实跑通过。
- 负向对照（逐个注入、跑 uploads 与 uploads-http、`git checkout -- server` 还原，最终 `git status` 干净）：
  - 去掉流式计数 → uploads (3)、uploads-http (4) 失败 ✔
  - 不清洗文件名 → uploads (4) 失败 ✔
  - 上传路由移到 `securityMiddleware` 之前 → uploads-http (2)、(5) 失败 ✔
  - `main.ts` 不把 `uploads` 传给 `createGateway` → uploads-http (6) 失败 ✔
- 新造检查：无额外；契约四项对照已覆盖设计点名的失败分支。
- 结论：通过。
