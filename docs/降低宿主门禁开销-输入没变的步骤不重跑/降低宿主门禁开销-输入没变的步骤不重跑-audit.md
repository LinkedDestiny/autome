# 审计 #8 · M-06 · 实现缺陷（退回，领域：Web 测试夹具清理竞态）

## Convergence Note
1. **未闭合领域与失败证据**：Web 测试夹具清理竞态。新配置 8 次全量 2 次红，均为 `test/fixtures/w9-replies.ts:23` 的 `rmSync` 抛 ENOTEMPTY（`|exclusive| w9-http`、`|kernel| alerts-real`），日志 `.autome/output/m06c/new2.log`、`seq3.log`。旧配置 3 次全绿。另：上轮 Web 内联样式规则已修复（styles.test 绿）。
2. **根因与漏检**：`close()` 在 `core.stop()` 后立即无重试删根目录，内核可能仍在写；前两轮只在 `eventbus-live` 处理 ENOTEMPTY，没有 grep 同类夹具，且环境阻塞导致从未跑过连续全量，所以漏检。同类范围：所有用 `core.stop()` 后删临时根的夹具与测试。
3. **下一轮工作与完成条件**：让 `w9Kernel().close()` 等待内核真正退出（或有界重试）后再删；排查同类；做 D.3 的 ENOTEMPTY 前后各 20 次实验并给结论（可去掉 eventbus-live 重试则去掉，否则写明已排除项）；`npm run test --prefix apps/web` 连续 5 次全绿。
4. **命令**：当前红 = 连续跑全量（`cd apps/web && npm run test`，约 4 分钟/次，~25% 概率红）；完成应绿 = 连续 5 次 111 文件 / 840 用例全绿；不得退化 = `node --test scripts/test/web-groups.test.mjs`、`test/styles.test.ts`、`test/test-groups.test.ts`，负载比值 ≤ 0.5（本轮实测 ≈ 0.37）。

## 结论
通过的：分组配置、样式回归修复、负载口径墙钟比（中位数新 246 s / 旧 668 s ≈ 0.37）、用例数 111/840。失败的：验收「5 次全绿」——本轮新配置 8 次全量 2 次因 ENOTEMPTY 失败（见证据 `evidence/M-06-r8-audit.md`）。空闲口径留 H-01。
判别检查：`.autome/output/m06c/`（bench.sh、summary.txt、各次日志）；回归标准即连续全量。
