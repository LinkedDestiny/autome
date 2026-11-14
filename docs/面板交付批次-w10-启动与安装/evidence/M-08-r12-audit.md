# M-08 r12 审计复验

结论：通过（打包与 .app 冒烟移入 H-08）。

- `npm --prefix apps/desktop test`：22 条全过，skipped 0（与契约 22 一致）。
- `vitest run test/styles.test.ts test/ci-config.test.ts`：2 文件 21 条全过；`test ! -e apps/desktop/renderer` 成立。
- 负向对照（注入后还原，`git status` 干净）：
  - `package.sh` 增加复制 `automed` → packaging「package builds and stages autome plus web」失败（21 过 1 败）；
  - `package.json` 的 `extraResources` 去掉 `web` → packaging「core and web are staged as resources」失败。
  - 契约第三条（打包冒烟第 1 条找不到 autome-web.js）依赖 .app，无法跑；静态 packaging 测试已在同一缺陷上变红。
- `sh scripts/package.sh`：cargo 与 stage-web 通过，electron-builder 在下载阶段因网络不可达失败（沙箱内外各试一次，`AggregateError internalConnectMultiple`），无 .app 产物。属环境问题，不是产品缺陷；契约已写明此情形由审计轮或 GitHub package job 复验。
- 引用扫描：ci-local.sh 有「Bundle launches the panel」，ci.yml 的 package job 跑 smoke；无 `desktop/renderer`、`Resources/core/automed` 残留。
- 判别检查：仅上述临时注入，已丢弃。
