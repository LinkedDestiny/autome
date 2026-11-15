# M-13 · 审计 #17 · 集成复验（提交 c4b5d93，范围 fd1f6f3..b1b10c4）

结论：**通过**，里程碑表不动（全部已完成）。

## 逐项复验
1. 改动范围：c4b5d93 只改 `apps/web/e2e/controls.spec.ts` 的 B-3：把 `task.get` 的 sessions 数断言包进 `expect.poll(...).toBeGreaterThan(0)`。原因成立——任务置 active 与 start_session 写入首个会话是两个先后的可观察效果（BL-13），原代码在二者之间读取会得到 0。最终断言仍是 `>0`，未放宽阈值。rebase 无丢失提交（简报 0 个）。
2. 断言与前提：diff 逐行看过，没有删断言、没有改夹具或前置条件；`active`、`not.toContainText(等 … 合并)` 两条原断言保留。poll 用默认超时（5 s），不是无限等待，会话真不出现仍会失败。
3. 冲突标记：`git grep -nE '^(<<<<<<<|>>>>>>>) ' c4b5d93 -- . ':(exclude)docs/…/'` 结果为空。
4. M-13 验收：`npm run build` 通过；`vitest run task-detail-ssr task-detail-payload test-groups` → 3 文件 11 用例全过。
- 负向对照：该改动只在 e2e，本沙箱 `ps` EPERM，Playwright 无法起，不能对 B-3 做注入。M-13 验收命令与本改动无交集，未做注入；`expect.poll` 对恒为 0 的返回值会超时失败，是 Playwright 的确定语义。

## 局限
B-3 是否因此稳定、29 例总耗时是否进 360 s，需宿主合并前门禁实测。改动未发现产品行为缺陷，也没有削弱验收。
新造检查：无（丢弃）。
