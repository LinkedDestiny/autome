# M-06 · 审计 #9 独立复验

结论：**通过**。

## 负向对照
把 `apps/web/test/fixtures/w9-replies.ts` 换回改动前版本（`rmSync`，无等待无重试）后跑 `w9-cleanup.test.ts`：3 个用例全红；还原后绿，工作区无差异。验收命令能区分旧实现。

## 验收命令（自己跑）
- 子集 `test-groups`、`core-sidecar`、`eventbus-live`、`logtail-live-cleanup`、`w9-cleanup`：5 文件 33 用例全绿（4+18+7+1+3，与契约一致）。
- `node --test scripts/test/web-groups.test.mjs`：1 通过。
- 连续 5 次 `npm run test --prefix apps/web`（审计 #8 曾 8 次红 2 次）：5/5 退出 0，112 文件 / 843 用例，与契约预期 112/843 相同；墙钟 285/255/274/298/262 s，机器负载 12 左右（负载口径）。对 r8 审计旧配置中位数 668 s，最慢一次 298 s 比值 0.45 ≤ 0.5。
- 结束后没有本 worktree 的残留 automed/vite 进程。
- 日志：`.autome/output/a9/`（summary.txt、seq*.log、subset.log）。

## ENOTEMPTY 结论
`w9Kernel.close()` 现在等待 `core.stop()` 后用有界重试（5 次/100 ms）异步删除，持续失败上抛；同类 `gateway` 等夹具已用同一重试。写入者未定位，保留重试，符合设计 D.3「证实不了就保留有界重试并如实写」的分支。改前后 20+20 次诊断实验未做，记 Backlog，不阻塞。

## 说明
5 次全绿对约 25% 的失败率只是一般强度的证据，故在 Backlog 留了诊断项。空闲口径仍归 H-01。
