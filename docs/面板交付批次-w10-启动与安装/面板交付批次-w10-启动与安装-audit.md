# 审计 · 第 14 轮 · M-09

结论：**实现缺陷，退回开放**（reopen 1→2，领域 e2e-infra；同领域第二次 reopen → `convergence-mode: domain-review`）。

## Convergence Note
1. **未闭合领域**：e2e-infra。`manual-items.spec.ts:33`（W7 H-04）失败：`.task-docs-body` 找不到。
2. **根因与漏检**：任务详情页文档与时间线分属不同标签，默认 `?tab=timeline` 下 `TaskDocs` 不渲染；用例在同一页同时找时间线、文档、日志三个滚动区。前两轮（#13 属性值、#13 补全值）都是用例与真实渲染不一致，实现 #14/#15 因沙箱禁 ps 从未真实跑过浏览器，期望都是推测的。同类范围：manual-items.spec.ts 与 layout.spec.ts 里所有「同页断言多个区域」处，本轮实跑其余 15 条均过。
3. **下一轮完成条件**：把 `.task-docs-body` 的断言放到 `visit(page, '/tasks/T-1?tab=docs')` 的页面上（时间线、日志仍在默认页），并保持一屏判据；然后跑验收命令全绿，再做负向对照 2（去掉看板 `data-scroll-region`）与 3（日志 `defaultOpen` 重建）。
4. **命令**：当前应红 `cd apps/web && npx playwright test e2e/manual-items.spec.ts`（1 失败、其余过）；完成应绿 16 条 + vitest 33 条；不得退化：e2e-preflight/core-sidecar/ci-config 三文件 33 条。

## 失败命令
`cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts` → 15 通过、1 失败（契约要求 16 全过），尾部：
`Error: element(s) not found … waiting for locator('.task-docs-body').first() … manual-items.spec.ts:38`。vitest 三文件 33 条全过。输出 `.autome/output/a16-pw.txt`。

## 判别检查
`.autome/output/a16-docs-tab.spec.ts`：访问 `/tasks/T-1?tab=docs` 后一屏且 `.task-docs-body` 为 `data-scroll-region="true"`，临时放入 e2e 跑过，1 通过，已删除，工作区干净。证明产品正确、用例错误。下一轮把该断言搬进 manual-items 第 H-04 条。

## 说明
- 本机审计会话可运行 ps 与浏览器（实现轮被沙箱阻断，这里没有）。
- 产品行为未被证明违背设计；缺陷在验收用例，项目测试体系不通过，按协议退回。
- 负向对照 2、3 未做（验收命令未绿）。提交由 Autome 兜底（如被拒）。
