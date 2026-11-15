# 审计 · M-05（审计 #5）

结论：**实现缺陷，退回开放**（reopen 1，领域「详情窗口浏览器验收夹具」，收敛模式 normal）。

违反：M-05 验收契约的验收命令 `npx playwright test e2e/manual-items.spec.ts e2e/layout.spec.ts`（预期 manual-items 7 与 layout 全部通过）；项目测试体系不通过按协议属实现缺陷。

失败命令与输出尾部：`cd apps/web && npx playwright test e2e/manual-items.spec.ts e2e/layout.spec.ts` → `1 failed / 18 passed`；失败项 `manual-items.spec.ts:94 C-2 展开日志后新会话到来再加载更早的会话…`，`expect.poll(() => detail().sessions.length).toBe(22)` 实得 3（75 秒超时），随后 `finally` 里的 `autome stop T-63` 抛「任务不在运行中」把真实错误盖住。

根因：`startWindowTask`（`apps/web/e2e/manual-items.spec.ts`）里 `round = step===1 ? 0 : floor((step+2)/3)`，第 2、3 步生成的设计文档逐字相同，第 3 步脚本的 `git commit` 没有改动、在 `set -e` 下退出码 1；第 3 个会话 crashed，任务进入 failed，窗口永远凑不到 22 个会话。这是测试夹具缺陷，不是产品行为错误。

产品侧判断：临时把第 `${step}` 步写进 `next-action`（让每步文档不同，临时副本 `e2e/zz-audit-tmp.spec.ts`，已删除）后，C-2 与 C-2b 都通过（C-2 53 秒）；vitest 4 个文件 41 个用例也全过。所以只需修夹具，产品不必动。

给实现轮：
1. 让 `startWindowTask` 每一步的文档内容互不相同（如把步号写进 `next-action`），再原样跑 C-2、C-2b 与 layout；
2. 把 `finally { cli(stop) }` 改成不吞掉主错误（stop 失败时忽略），否则下次同类失败又只看到「任务不在运行中」；
3. 之后补做契约里的浏览器负向对照（窗口起点不钉住、条目 `key` 按序号、`withParams` 丢键），本轮因夹具先挂，没能在浏览器侧执行。

判别检查留存：`.autome/output/pw.log`（完整运行）；临时副本的改法见上第 1 条（一行改动）。

Backlog：无新增。
