# M-06 · 审计 #8 独立复验（对应实现 #8）

## 通过的部分
- `node --test scripts/test/web-groups.test.mjs`：1 个通过。
- `npx vitest run test/styles.test.ts test/test-groups.test.ts test/core-sidecar.test.ts test/eventbus-live.test.ts test/logtail-live-cleanup.test.ts`：5 文件 / 42 用例全绿。上轮的内联样式缺陷已修复（`grep 'style={' apps/web/app` 无命中）。
- 负向对照沿用审计 #7 已做的（eventbus-live 放进 unit → test-groups 红）；本轮未改任何被跟踪文件，临时 `apps/web/vitest.old.config.ts`（取自 a762d3f8）已删，`git status` 干净。

## 交替基准与连续全量（本机 load 4.5–10，非空闲，仅负载口径）
命令：旧配置 `npx vitest run --config vitest.old.config.ts`（cwd=apps/web）与 `npm run test` 交替 3 组，再 `npm run test` 连跑 5 次。原始：`.autome/output/m06c/summary.txt`。

| 次 | 配置 | 墙钟 s | 结果 |
|---|---|---|---|
| 1 | 旧 | 559 | 111 文件 / 840 通过 |
| 1 | 新 | 246 | 全绿 |
| 2 | 旧 | 668 | 全绿 |
| 2 | 新 | 275 | **1 失败** w9-http（ENOTEMPTY） |
| 3 | 旧 | 702 | 全绿 |
| 3 | 新 | 220 | 全绿 |
| 连1 | 新 | 248 | 全绿 |
| 连2 | 新 | 285 | 全绿 |
| 连3 | 新 | 222 | **1 失败** alerts-real（ENOTEMPTY） |
| 连4 | 新 | 238 | 全绿 |
| 连5 | 新 | 222 | 全绿 |

- 用例数 111 / 840，与预期一致。负载口径中位数：新 ≈ 246，旧 = 668，比值 ≈ 0.37 ≤ 0.5，通过。空闲口径本机取不到，留 H-01。
- 新配置共 8 次全量，**2 次失败**；旧配置 3 次全绿。验收要求「5 次全绿」不成立。

## 缺陷：ENOTEMPTY 出现在 `test/fixtures/w9-replies.ts:23`
两次失败同一位置、同一报错：`close()` 里 `await core.stop()` 之后的 `rmSync(root, { recursive: true, force: true })` 抛 `ENOTEMPTY, Directory not empty: …/T/w9-xxxx`（失败日志 `.autome/output/m06c/new2.log`、`seq3.log`）。一次落在 `|exclusive| w9-http`，一次落在 `|kernel| alerts-real`。这里的 `rmSync` 没有任何重试，`core.stop()` 返回后内核仍可能在 root 下写文件。实现 #8 只在 `eventbus-live` 保留了 `maxRetries`，设计 D.3 要求「ENOTEMPTY 有结论」，而实现证据写「写入者仍未定位」，同类的 `w9-replies` 位置既未排查也未处理；验收「5 次全绿」被它直接打破。违反：M-06 验收契约（5 次全绿无残留、ENOTEMPTY 有结论）、D-R3。
全部同类位置待实现轮 grep：所有对内核根目录 `rmSync/rm` 且前面只有 `core.stop()` 的夹具（`w9-replies.ts` 及其调用方）。

## 其余
- 未用到的检查：project-confirm-real 偶发本轮 8 次全量未再出现。
- `ps` 残留：测试后无命令行含 `w9-` 或本 worktree 的 automed 进程（0 个）。
