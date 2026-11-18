# 修复轮 r22（门禁失败：task-log-live）

- 失败：`apps/web/test/task-log-live.test.ts > 页面历史会话折叠，运行中会话默认展开`，期望 2 个 `<details` 实得 4。
- 根因：M-07 起时间线条目文本也用 `<details class="timeline-text">`，测试按全部 `<details` 计数，已过时；产品行为（历史会话折叠、运行会话展开）未变。
- 修复：测试改为只统计 `class="session-log"` 的 details，并在这些标签上断言 `open=""` 仅 1 个。
- 复跑：`npx vitest run test/task-log-live.test.ts` → 3 通过（用例总数与基线 1429 不变，仅改断言）。
- 自审四项：情形表/失败分支/迁移/域边界均不适用（仅测试断言修正，无产品改动）。
- 快检以宿主重跑为准。
