# M-04 · 实现轮 #6（快检修复）

## 修复与范围

宿主在 30dfc4e3b3 上发现 `clippy::filter_next`：`queue_move_positions_and_event` 使用 `filter(...).next_back()` 查找最后一条移动事件。本轮改为等价的 `rfind(...)`，保留原有事件 payload 断言，不改产品代码。搜索 crates 下同类链式调用，仅此一处。M-04 保持待审，不推进 M-05。

上一轮审计为 M-03 通过，没有审计退回检查需要迁入；本次失败检查已经是项目现有 Clippy 检查，直接保留该检查，未新增重复测试。

## 命令与结果

- 红：`cargo clippy --all-targets -- -D warnings`，退出 101，复现 dispatch.rs 的 `clippy::filter_next`。原始输出：`.autome/output/m04-r6-clippy-red.txt`。
- 首次修改后的快检退出 1：rustfmt 要求该表达式收为一行；已按提示修正。原始输出：`.autome/output/m04-r6-quickcheck.txt`。
- 绿：原样执行 `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`，退出 0。原始输出：`.autome/output/m04-r6-quickcheck-green.txt`。提交后的最终快检结果以 Autome 重跑为准。
- M-04 验收原样执行：`cargo test -p automed queue_move_ && cargo test -p automed task_start_queues_a_draft && cargo test -p automed --test end_to_end task_card_a_started_draft_waits_behind_the_parallel_limit`，退出 0。原始输出：`.autome/output/m04-r6-tests.txt`。
- 用例总数 **9**：queue_move_ 7（lib 6、e2e 1），start 单测 1、start e2e 1；全部通过，失败 0、忽略 0。与上一实现轮 M-04 的同口径基线 9 相差 **0**，没有新增、删除或跳过用例。快检为静态检查，不计入测试用例总数。
- `git diff --check` 通过。未运行 `sh scripts/ci-local.sh`，未使用真实账本。

## 标待审之前的自审清单

本轮保持既有待审状态，仍逐项执行 M-04 验收复核。以下用例均在上述验收命令中运行通过。

1. **情形表逐行**：top / before / after 与最后一条事件 payload → `queue_move_positions_and_event`；两个未定位 Draft 的 before / after → `queue_move_unpositioned_draft_reference`；其它项目序号不变 → `queue_move_other_project_unchanged`；移动 Draft 启动保位 → `queue_move_draft_keeps_position_on_start`；未移动 Draft 启动入尾 → `queue_move_unmoved_draft_starts_at_tail_and_singleton`；第三个移首先启动 → `queue_move_the_third_task_moved_to_top_starts_first`。均通过。
2. **失败分支**：`queue_move_rejects_without_changes` 覆盖零个/两个位置、top false、错误类型、自己、跨项目、不存在、空串，以及运行/暂停/停止/失败/Done/Cancelled 目标或参照，检查拒绝且无移动事件、原序号不变，通过。授权、读盘、超时、外部服务故障不适用：M-04 未新增这些分支，本轮仅替换测试迭代器表达式。
3. **全部迁移**：移动后排队/待开始状态保持；已定位与未定位 Draft start → Queued 分别保位/入尾；槽位释放后移首任务 Queued → Active，其余保持 Queued；禁止移动状态由拒绝用例覆盖。对应上述 queue_move_ 子集及两个 start 回归均通过。未新增状态迁移。
4. **域边界**：单元素、队首/中间/队尾、两个未定位 Draft、空 id、前导零不存在 id、null/数字/string、top false，由上述用例覆盖并通过。日期/闰日/跨年不适用：没有日期输入；序号最大值不属于本轮测试表达式或 RPC 输入，本轮不重复 M-01 store 检查。
