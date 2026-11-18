# 交接修复轮 r23（承接 r22 门禁失败；不改里程碑表）

## 修复说明（沿用并补全原 fix-r22-impl.md）
- 失败：`apps/web/test/task-log-live.test.ts > 页面历史会话折叠，运行中会话默认展开`，期望 2 个 `<details` 实得 4。
- 根因：引入变化的是 M-07——时间线条目文本也用 `<details class="timeline-text">`，测试按全部 `<details` 计数已过时；产品行为（历史会话折叠、运行会话展开）未变。
- 修复（r22 已提交）：测试只统计 `class="session-log"` 的 details，并断言 `open=""` 仅 1 个。本轮不回退。
- 用例总数与基线 1429 不变，仅改断言。

## 本轮实跑
- `npx vitest run test/task-log-live.test.ts` → 3 通过 / 0 失败 / 0 跳过。
- `npx playwright test e2e/layout.spec.ts` → 16 通过（41.5s）。输出见 .autome/output/r23-e2e.txt。

## 操作者备注处理
1. 同意，不回退断言修改。
2. 文件名冲突：备注第 2 条要求写 `M-07-r<k>-impl.md`，备注末尾与本轮指令要求写 `evidence/M-13-r23-impl.md`。按末尾及本轮指令写 M-13-r23（简报给定轮次 23）；修复归属 M-07 在本文已说明。已删除 `fix-r22-impl.md`。
3. Scripts 步骤 `ci-e2e.test.mjs` 用例 02 退出码 143：宿主高负载下 `--plan` 子进程超 60 秒被终止（整套 404 秒，平时约 200 秒）；本任务未改 `scripts/`，未放宽或改写用例，不处理。
4. 已在 worktree 实跑上述两项，结果见上。

## 自审四项
情形表 / 失败分支 / 迁移 / 域边界：均不适用（本轮无产品改动，仅交接与证据整理）。
