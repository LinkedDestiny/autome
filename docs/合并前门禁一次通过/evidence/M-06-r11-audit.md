# M-06 独立复验（审计 #7，对应实现 #11）

结论：**实现缺陷，退回**（领域：依赖事件验收判别力）。

## 负向对照（本会话能起浏览器，全部实跑）
- `node apps/web/scripts/test-related-mutation.mjs`：相关分支恒假、重建、BL-01 独立 5 次，**5/5 因依赖文案未更新失败**（日志 `.autome/output/m06-a7-mut.log`），源码与构建已还原，`git status` 干净。上轮的判别力缺口已闭合。
- `tasks.$id.tsx` effect 改为不返回清理函数：task-page-effect 4 个中 2 个红（「after 变化时先清旧再登记新」「卸载时清除」），已还原。
- 相关分支恒假下 task-page-effect「挂载时登记」失败：上轮审计已见 3 红，本轮未重做。

## 验收命令
- vitest 三文件：22 用例通过（task-page-effect 4、live-routes、test-groups）。
- ★ 5 次独立调用 `playwright test e2e/controls.spec.ts -g 'B-3|BL-01'`：先单次 1 次通过，再连跑 5 次，**第 5 次 B-3 失败**（4 次全过，BL-01 5 次全过）。契约要求 10 条全通过，未达成。日志 `.autome/output/m06-a7-pos1.log`…`pos5.log`。

## 失败原因（B-3 第 5 次）
`controls.spec.ts:97`：`Expected "queued"  Received "active"`，当时负载 5.7–7。
该行在合并后的轮询里**先读前置状态，再读依赖任务状态**，两次是各自独立的 CLI 调用（每次数百毫秒到数秒）。前置在两次读取之间转 `done`，依赖任务随即 `active`，于是「前置未完成时依赖任务已 active」被误判。产品行为没有错：依赖任务只在前置 done 之后才启动；错的是断言的读取顺序，使 `前置未 done ∧ 依赖已 active` 在时序上可由单调推进的正确实现产生。
同类位置：第 92 行（approve 后到 await_merge 阶段）同序读取，但此阶段没人发 merge，前置不会 done，窗口不存在；仅 97 行有此窗口。

为什么不是偶发可放过：这正是契约要求 B-3 五次稳定的对象（T-98 观测 2/4 同类），宿主快检跑 B-3 时同样会随负载误红，把合并前门禁之前的一轮白白退回。

## 交实现轮
改 97 行为**先读依赖任务、再读前置**：若依赖任务已 active，则随后读到的前置必须是 done（状态单调），否则才是真缺陷；`queued` 时不对前置做断言。判别检查：在 `expect.poll` 里人为在两次读取之间插入前置合并（或用 `w7-sleep` 拉长两次读取间隔）应不再误红，而把依赖任务提前启动的注入仍应红。回归证据：`.autome/output/m06-a7-pos5.log`（原样复现为红的那次）。
