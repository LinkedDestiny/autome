# M-12 · 实现 #19 · 宿主快检修复

本轮按简报修复宿主回归，不推进 M-13；M-12 保持待审，implementation-round 为 19/65。产品代码未改。

## 根因与修复

- `load-context.test.ts` 的六个失败来自旧契约：A-Q1 已增加 `skills.list`、项目完成数，B 总览已增加 `overview` 离线回退，今天花费与门禁实时更新需要 `session.` / `gate.`。测试仍期望三次读取、旧回包及忽略 session。逐调用仍精确核对方法、参数、总次数，并保持禁止写入；离线完整对象仍用 toEqual；补测 session.ended、gate.result。事件测试标题随新契约更新，原用例一对一保留。
- `flows.spec.ts` D-2 的旧断言要求正序，与任务 D-3 和设计「组内与组间都是倒序」冲突。改为倒序，并增加四个节点均存在的断言。原标题、逐轮出现、尚未开始节点不存在、复现结果、合并到完成列与整程只加载一次的断言均保留。
- 同类检查范围：两个 SSR 注入分支与直接 loader 调用、无网关及三类错误、根事件匹配；浏览器 flows 两个用例与 timelineGroups 的阶段/组内排序。未改产品排序或放松错误透传。

## 红绿记录与用例数量

- 原样运行 `(cd apps/web && npx vitest run --project exclusive test/load-context.test.ts)`：10 例，4 过、6 失败，与宿主失败清单一致；日志 `.autome/output/r19-load-red.txt`。修复后同命令 10/10 通过，日志 `.autome/output/r19-load-green.txt`。红日志有 Vite listen EPERM 提示，但六处断言均实际执行并失败；绿运行无未处理错误。
- 原样运行 `(cd apps/web && npx playwright test e2e/flows.spec.ts -g 'D-2')`：修复前后均在夹具准备阶段 `spawnSync ps EPERM`，正文未执行；日志 `.autome/output/r19-flow-red.txt`、`r19-flow-green.txt`。不声称本地红绿成功；修复前正文的红证据来自宿主 `gate-68e1ac6f119e.log.playwright.json`：实际位置 [4,3,2,0]，旧期望 [0,2,3,4]。修复后须由宿主以同命令复验。
- `(cd apps/web && npx vitest run --project unit test/shell-data.test.ts test/task-summary.test.ts)`：31/31 通过，分别 11、20，包含阶段与组内倒序、空输入/单会话、技能读取失败恢复；日志 `.autome/output/r19-related.txt`。
- 本轮直接子集共 **42 例**（load-context 10 + shell-data 11 + task-summary 20 + D-2 1），实际执行通过 41，浏览器 1 例准备失败未执行。相对同文件基线 **增减 0**，重验单用例内事件矩阵增加 2 行，D-2 增加节点存在性断言，不拆新增用例。上一轮 M-12 验收为 75 例，本轮 42 相比少 33，原因是本轮仅修宿主失败子集，不是删除测试；不以本轮子集替代上一轮技能验收。
- 上一轮审计通过 M-11，没有待迁移的临时审计检查；本次宿主抓缺陷的检查本来就在项目测试体系，先原样复跑再修订符合设计的期望，不新增独立临时检查替代它。

## 四项自审（修复范围）

1. **情形表逐行**：单项目/空项目通过；直接 loader、开发 SSR、生产 SSR 网关注入通过；无网关、503、504、409 错误及 overview 回退逐项通过；时间线阶段与组内倒序模型检查通过。真实浏览器逐轮出现与合并行保留，受 ps EPERM 阻塞。
2. **失败分支**：无网关、超时、离线、冲突原样透传通过；shell-data 的拒绝读取、读盘失败、技能读取失败/异常与恢复通过。此轮没有新增授权或外部服务分支。浏览器准备失败如实记录，没有改守卫或跳过用例。
3. **全部迁移**：task/project/bug/merge/session.started/session.ended/gate.result 触发重验、config.changed 不触发均通过；shell-data 失败→恢复、技能计数缺失→零→非零通过。Bug 定位→复现检查→修复→复核→合并的浏览器流程仍完整保留，未执行部分交宿主复验；本轮没有新增产品状态机。
4. **输入域边界**：空项目列表、单项目、零完成数、空时间线和单运行会话通过；D-2 对 indexOf=-1 新增拒绝断言。前导零、闰日、跨年、数值最大值不适用：此轮不改输入解析、日期计算或数值范围。

## 快检与交接

未运行 `sh scripts/ci-local.sh`，不启动下一会话。指定快检结果见下。

指定命令 `sh scripts/ci-quick.sh --since 68e1ac6f119ecade4245bdfbe3b4be032b615c26` 实际退出 **1**：13 步，12 过、1 失败，用时 41 秒，相关阶段 15 秒（预算 120 秒）。Format、Clippy、Web lint/typecheck、构建与 load-context 10 例均通过；唯一失败 Web e2e 是准备/清理的 `spawnSync ps EPERM`，未获得浏览器正文执行结果。日志 `.autome/output/r19-quick.txt`。快检选择范围依据本轮测试改动，与上一轮 25 步不同；重复执行的 load-context 10 例不重复计入上述 42 例。最终结果以 Autome 提交后复跑为准。

`git diff --check` 通过。提交在当前分支完成，不推送；若 .git 写入被拒，则由 Autome 兜底，记录实际结果。
