# 审计 · 第 17 轮 · M-10

## Convergence Note

1. **未闭合领域 e2e-flow（第二次 reopen）**：`cd apps/web && /usr/bin/time -p npm run e2e` 在 ps 可用的环境下，D-1 通过（44.1s）、D-2 通过（14.5s，上轮的节点缺陷已修好）、layout 11 条通过，之后 D-3 在 170 秒总时限被切断，`Timed out waiting 170s for the test suite to run`，13 passed / 7 did not run，real 170.64。D-3 失败上下文：卡片 T-33 在 60 秒内未进入 needs_you（页面仍显示进行中 1）。违反验收契约「20 条全过、real ≤ 180 秒」与 R-6。
2. **根因与漏检**：不是单个断言错，而是夹具总耗时预算：D-1 44s 加 D-3 的长等待 60s 加 D-4 与 manual-items，叠加后超过 170s；前两轮被缺陷与 ps EPERM 挡在前面，时限问题从未暴露。同类检查范围：flows/live 全部等待上限与夹具步骤延迟。
3. **下一轮工作**：按 R-6 压缩假 CLI 步骤延迟（D-1、D-3 的 needs_you 到达时间），保持 D-3 的 3000ms tick 与所有断言不放宽，查清 D-3 中 T-33 为何迟迟不到 needs_you；完成条件：20 条全过、real ≤ 180 秒、残留断言通过，并跑负向对照三条。
4. **命令**：当前应红、完成应绿：`cd apps/web && /usr/bin/time -p npm run e2e`；不得退化：`npx vitest run test/task-timeline.test.ts test/e2e-preflight.test.ts`。

## 结论

**实现缺陷**，M-10 退回 `开放`，reopen 2，领域 `e2e-flow`，收敛模式 `domain-review`。

- 失败命令与输出尾部见上；日志 `.autome/output/a17-e2e.log`，产物 `.autome/output/playwright/`（D-3 error-context.md）。下一轮实现轮先复跑原命令确认红，再修绿。
- 负向对照：套件未通过，无法有意义地执行，未做。
- 提交由 Autome 兜底（如被拒）。

## Backlog 提示
无新增。
