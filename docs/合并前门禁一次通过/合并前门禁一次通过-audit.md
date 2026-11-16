# 实现审计（审计 #7）

## Convergence Note
1. **未闭合的行为领域**：依赖事件验收判别力。当前失败：B-3 五次独立调用第 5 次红（`controls.spec.ts:97` 期望 `queued` 实得 `active`），见 evidence/M-06-r11-audit.md、`.autome/output/m06-a7-pos5.log`。
2. **根因与同类范围**：97 行先读前置、后读依赖任务，两次独立 CLI 调用之间前置可合并且依赖任务启动，断言对正确实现误判。同类位置 92 行经核查无窗口。前轮只在空闲或少次数下跑，且上轮把重心放在 BL-01 判别力，未多次连跑 B-3。
3. **下一轮任务**：97 行改成先读依赖任务再读前置（active ⇒ 前置必须 done）；完成条件：B-3|BL-01 五次独立调用 10 条全过，且恒假变异脚本仍 5/5 红。
4. **命令**：当前应红：`for i in 1 2 3 4 5; do (cd apps/web && npx playwright test e2e/controls.spec.ts -g 'B-3|BL-01') || exit 1; done`（高负载下第 5 次类）；完成应绿：同命令；不得退化：`node apps/web/scripts/test-related-mutation.mjs`（5/5 红）与 vitest 三文件 22 通过。

## M-06：退回（实现缺陷，领域：依赖事件验收判别力）
- 已闭合：BL-01 在相关分支恒假下 5/5 变红；effect 无清理函数时 2 个组件测试红；vitest 22 通过。变异均已还原。
- 失败项：验收命令 ★ 五次独立调用中第 5 次 B-3 失败，原因是断言读取顺序竞态（非产品错误）。判别检查与失败日志在 `.autome/output/m06-a7-pos5.log`。
- 详见 evidence/M-06-r11-audit.md。
