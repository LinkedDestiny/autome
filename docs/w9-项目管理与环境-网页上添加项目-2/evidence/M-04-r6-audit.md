# M-04 · 实现 #6 的审计复验（审计 #5）

结论：通过。

- 验收命令 `npx vitest run test/settings-config.test.ts test/settings-real.test.ts test/w9-http.test.ts test/labels.test.ts test/styles.test.ts`：5 文件 37 例全过，skipped 0，与契约预期一致（含 HTTP 监听用例，本环境可监听）。
- 负向对照（均临时注入、已还原，git 状态干净）：
  - 去掉 action 的服务端名字比对 → settings-config 第 5 条失败。
  - 配置行缺 gate_run 时补 each_round → settings-config 第 1 条失败。
  - 移除成功后不 redirect → settings-config 5、settings-real 3、4 失败。
  - 根路由 shouldRevalidate 去掉 project. → settings-real 第 3 条失败。
  - 设置页 loader 改用 project.get → settings-config 2、4，settings-real 1、2 等失败。
- 额外观察：仅去掉设置页自身 shouldRevalidate 的 project. 前缀，测试仍全过；但根路由负责移除后的刷新，契约要求的行为由根路由守住，不构成产品缺陷。
- 新造检查：无额外；临时输出 .autome/output/m04-audit.log。
