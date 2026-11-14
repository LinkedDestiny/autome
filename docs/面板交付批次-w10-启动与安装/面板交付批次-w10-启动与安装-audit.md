# 审计 · 第 16 轮 · M-10

结论：**实现缺陷**，M-10 退回 `开放`，reopen 1，领域 `e2e-flow`，收敛模式 `normal`。

- 失败命令：`cd apps/web && /usr/bin/time -p npm run e2e`（ps 可用，浏览器能起）。输出尾部：`1 failed: flows.spec.ts:37 D-2 … / 7 did not run / 12 passed (2.8m) / Timed out waiting 170s for the test suite to run / real 170.64`。
- D-2 失败：`[data-testid=timeline-entry][data-node=locate]` 30 秒内不可见。错误上下文显示时间线里「定位 · 第 1 轮 / 运行中」条目存在，但没有 `data-node`（`task-timeline-model.ts` 的 `sessionEntry` 在没有匹配到 session.started 事件时 `node` 为 undefined）。违反验收契约「Playwright 20 条全过」与设计 testid 表（`timeline-entry` 带 `data-node`，D-2 使用）。修法由实现轮定：让运行中会话条目带 node，或按实际可见的属性断言。
- 时限：D-1 52 秒、D-2 30 秒后撞上 `globalTimeout` 170 秒，live.spec（D-3/D-4）与 manual-items 共 7 条没跑；契约要求 20 条全过且 ≤ 180 秒，当前未满足，夹具需压缩（设计 R-6）。
- 负向对照：套件本身未通过，无法有意义地执行；未做。D-1 通过说明「不刷新」断言路径可用。
- 判别检查与日志：`.autome/output/a16-e2e.log`，Playwright 产物 `.autome/output/playwright/`。下一轮把 D-2 失败用例作为回归，先跑红再修绿。
- 提交由 Autome 兜底（如被拒）。

## Backlog 提示
无新增。
