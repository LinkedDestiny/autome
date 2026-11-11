# 设计裁决记录

## 第 1 轮（design-round 0 → 1）

评审结论：评审 #1 提出 2 项符合第 2 类「内部逻辑矛盾」的阻塞问题（R1-P01、R1-P02）与 1 项非阻塞建议（R1-N01）。两项阻塞均已对照源码和实验输出核对属实，全部采纳并改在设计文档里；均为首次提出，复提计数 0，无争议项。
裁决后设计中已没有剩余的六类问题：状态块 `status` 改为 `实现中`，`design-round` 为 1/15，`current-milestone` 为 M-01，里程碑表五行完整（M-01..M-05，均为 `开放`，reopen 0）。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01（评审 R1-P01） | 内核离线时 SSR 的根 loader 返回空数据；之后 `core: connected` 不触发任何重验，账本无新事件时项目栏、首页、⌘K 一直停在离线结果 | 「根 loader 的数据」、R-05 一节、M-02 | 采纳 | 核对 `app/lib/live.tsx:20-22`：`onCore` 只是 `setCore`，重验只挂在 `onEvents` 与 `onReload`；`live-client.ts` 的 `hello`/`core` 分支不调 `onReload`。评审的最小实验 `.autome/output/review-1-recovery-probe.txt` 给出 `reload=0, events=0, core=[offline,connected]`，与源码一致。反例成立，且属于设计内部矛盾（横幅与写判定会恢复，数据不会） | R-05 一节新增「恢复后的根数据重验」：纯函数 `needsKernelReload`、hook `useKernelRecovery`（用 `revalidateAll`，覆盖首个 `hello` 即 connected 的时序；输入不变不重试）；M-02 的结果文字、`live-status` 新增第 6 条（5→6）、负向对照；H-05 补一步人工核对；Web 总数顺延（M-02 后 316、M-03 后 329、M-04 后 343、M-05 后 350）。`live-client.ts`、`live-routes.ts`、服务端协议不动 | 0 |
| D1-P02（评审 R1-P02） | ⌘K 要求按 slug 匹配，但候选唯一来源（根 loader）的任务字段白名单不含 slug | 「R-03 ⌘K」`filterCandidates`、M-04 `shell-data` 第 5 条 | 采纳（取删除方案） | 白名单（`id, kind, title, project_id, state, fix_round?`）确实不含 slug；`dispatch.rs:554-557` 的 slug 是独立字段，不能从标题或 ID 推出。任务 R-03 并不要求 slug 搜索，保留它只会扩大根数据。两种修法里删除最小，无需改 M-04 第 5 条断言 | `filterCandidates` 的匹配字段改为 ID、标题、项目名和「新建任务卡」字样，并注明不按 slug 搜索；M-05 `palette` 第 4 条原本就只列这三项，无需改 | 0 |
| N1-01（评审 R1-N01） | 设计称「上限值不在 `task_json` 里」，与源码不符 | 「R-04」末段、Backlog B-03 | 采纳（文字更正，不触发轮次） | `dispatch.rs:560,572-573` 输出 `budget_n` 与 `fix_round`；`bug_loop.rs:53-57` 用 `budget_n` 或 `bug.fix_rounds` 决定上限；`project.get` 返回解析后的 `config`。需求里程碑进度的数据来源评审也没有证明，设计不下结论 | R-04 末段与 B-03 改为：推迟的理由是 R-02 允许简版卡片，完整卡片归 W6/W7；Bug 修复轮数据已有，需求里程碑进度待 W6/W7 设计时核实。B-03 保留在 Backlog，等用户处置 | — |

## 其他改动

- 把设计文档里的「## 里程碑说明」改名为「## 验收说明」，避免与协议要求「`## 里程碑` 一节里只放一张表」的按位置读表规则撞名（W4 同样处理过）。章节内容不变。
- 本轮未核验：D1-P01 新增的 `useKernelRecovery` 尚无实现，其真实浏览器行为由 `live-status` 第 6 条（`createMemoryRouter`）和 H-05 新增步骤覆盖，这两处在设计里已写明。

提交由 Autome 兜底（如本轮提交被拒）。
