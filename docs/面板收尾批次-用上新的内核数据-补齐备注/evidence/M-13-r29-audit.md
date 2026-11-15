# M-13 r29 审计证据 · 集成复验（审计 #21）

范围：`3edaa8b..ccf32fb`，仅 `ed7d315` test(web): reserve preparation time for C-2 sessions，改动文件 `apps/web/e2e/manual-items.spec.ts`。

## 结论：通过（里程碑表不动）

1. **改动只解决触发原因。** 触发是 C-2 准备 22 个会话时 75 s 轮询只到 17。diff 只有两处：该用例加 `test.setTimeout(240_000)`，准备轮询超时 75 s → 150 s。目标值 22、其后全部浏览器断言、其它用例超时均未动，没有削弱断言或改前提。rebase 丢失提交 0 个。
2. **冲突标记。** `git grep -nE '^(<<<<<<<|>>>>>>>) ' ed7d315 -- . ':(exclude)docs/…/'` 结果为空。
3. **用例判别力（负向对照）。** 把准备轮询超时临时改成 20 s：用例失败，`Expected: 22 / Received: 5`，说明轮询断言真实有效。已从备份还原，`git diff` 为空。
4. **正向复验。** `npx playwright test e2e/manual-items.spec.ts -g "C-2 展开"` 通过，用例 1.5 m（超过原 75 s 准备上限的同量级耗时，新预算下通过），总 3.9 m。日志 `.autome/output/a21-c2.log`。
5. **M-13 验收命令**（vitest 三文件）：本提交只改 e2e 文件，不触及 `app/`、`server/`、其测试；未重跑，影响面为零。
6. **全局上限。** `globalTimeout` 720 s 未改，设计 R-1 已记录 C-2 为 240 s，与实现一致；整套总耗时由宿主合并前门禁复验。

新造检查：准备超时缩短的负向注入（已还原）。无缺陷、无需退回。
