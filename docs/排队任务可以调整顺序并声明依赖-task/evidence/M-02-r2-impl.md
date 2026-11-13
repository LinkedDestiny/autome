# M-02 · 实现轮 2/30

结论：本里程碑验收通过，提交独立审计；未推进 M-03。上一轮审计通过 M-01，没有退回缺陷，因此不存在需要逐字移入测试体系的审计失败检查。

## 实现

- `task.create` 严格解析可选字符串数组 `after`；分配编号后调用既有纯函数校验，拒绝不落任务、不写创建事件。读取错误向上传播，不伪装成不存在；跨项目任务按编号查找，包括已移除项目的任务。
- 调用 M-01 的 `create_task_after` 在同一事务保存任务及依赖，之后才 tick。非空依赖写入 `task.created`；空依赖保持原事件 payload。
- `task_json` 返回声明顺序的 `after` 和未完成依赖的 `blocked_by`（当前状态与原因），供 task.get / project.get 共用。
- 出队时仅约束可编辑依赖的 Queued / Requeued；未满足则 continue，保持状态和 queued_seq。Requeued 用真实 Impl 会话历史判断，避免实现后回到 Design 的任务被重新阻塞。
- 现有测试没有修改；新增 5 个单测、1 个真实 Git e2e。依赖修改、通知去重与 dashboard 排队组仍属 M-03。

## 验收命令与数量

最终代码运行设计指定的命令（串联退出码 0）：

1. `cargo test -p automed after_create_`：6 个通过（lib 5、end_to_end 1），0 失败、0 忽略。日志 `.autome/output/m02-final-create.txt`。
2. `cargo test -p automed fill_slots_`：2 个既有单测通过。日志 `.autome/output/m02-final-slots.txt`。
3. `cargo test -p automed --test end_to_end the_parallel_limit`：2 个既有 e2e 通过。日志 `.autome/output/m02-final-parallel.txt`。

**总数 10 个不同用例，较本轮修改前同一组过滤器的基线 4 个增加 6 个**，全部来自新增 after_create_ 用例。设计原先预计 9 个，少算了既有 `task_card_a_started_draft_waits_behind_the_parallel_limit`；它也匹配第三条命令，已更正预期为 2 个，没有删除或改名任何既有测试。

上一轮 M-01 报告共 69 个（deps 12 + store 57），本轮 10 个，表面差为 **-59**：切换验收范围后不再重跑那 69 个，改跑 4 个调度/e2e 基线并新增 6 个；不是同一测试集合减少，也不是跳过了测试层。

另执行直接涉及三个 Rust 文件的 `rustfmt --edition 2024` 和 `git diff --check`，通过。未运行项目全量门禁、cargo fmt --check、clippy 或 Web 快检，按本轮指令由宿主承担。所有测试使用临时仓库与隔离账本，没有打开真实 ~/.autome。

## 标待审之前的自审清单

### 1. 情形表逐行

- 创建自引用、编号不存在、跨项目：`after_create_rejects_invalid_dependencies_without_writing` 逐项检查中文单行拒绝、任务数不变、没有 task.created，通过。
- task.get / project.get：`after_create_reads_dependencies_and_input_boundaries` 对照完整任务对象，覆盖空数组、声明保序与去重、Done 不进入 blocked_by、Cancelled 带 cancelled、Draft 原因为 null，通过；并核对创建事件空列表兼容性及非空 after。
- 跳过但不移位：`after_create_skips_blocked_without_changing_position` 检查 B 状态、queued_seq、队列下标不变且 C 启动，通过。e2e 另核对创建及 C 启动后的 task.get queue_position 均为 1。
- 仅 Done 满足：`after_create_done_is_the_only_satisfied_dependency` 对 Draft / Active Design / Paused / Stopped / Failed / Cancelled 逐项阻塞；Done 且归档后 B 启动，通过。未归档 Done 由 e2e 的真实完成路径覆盖。
- Requeued：`after_create_requeued_checks_design_and_implementation_history` 对 Feature Design 无历史（阻塞）、Feature Implement（放行）、Feature Design 有 Impl 会话（放行）、Bug Locate（放行）逐项通过。
- parallel=2：e2e 创建 A、B after A、C；A/C 有运行会话，B 排队且无会话；只批准及合并 A，不对 B 做启动操作。真实合并完成后 B 自动启动，A Done 事件 seq 小于 B 首次 Active 事件 seq，通过。

### 2. 失败分支

- 本里程碑点名的拒绝分支（自引用、不存在、跨项目、非字符串数组、超限）均在上述 RPC 单测执行且无副作用。
- 创建时形成多节点环不适用：新任务还没有任何入边，外键不允许旧任务预先指向未创建任务；自环本轮已测，多节点环纯函数已由 M-01 审计关闭，修改依赖的 RPC 环拒绝属于 M-03。
- 拒绝授权、网络不可达、超时不适用：本里程碑没有引入相应服务或授权通道。读盘/事务失败的专项注入不属于 M-02 点名分支；沿用 M-01 的原子落库接口并传播 StoreError，本轮未重跑其外键回滚测试。

### 3. 全部迁移

- 创建 → Draft：显式 draft 不调 tick，读接口测试覆盖。
- 创建 → Queued → Active：空依赖由既有并发回归覆盖；依赖不满足保持 Queued、后继 C 可 Active，由单测及 e2e 覆盖。
- 依赖未完成 → Done → 被依赖任务自动 Active：单测与 e2e 覆盖，未触发自动批准或自动合并。
- Requeued Design 保持 Requeued 与三类豁免恢复进入 Active：状态矩阵单测覆盖；实现历史来自实际启动过的 Impl 会话记录。
- 无新增通知状态迁移：task.blocked / notified 的迁移属于 M-03，本轮不实现。

### 4. 输入域边界

- after 缺省、空数组、单元素、8 个不同 id、8 个重复 id（去重后单个）、9 个重复 id（按原始项数拒绝）均执行，通过。
- null、字符串、含数字的数组拒绝为参数错误；空串 id、前导零 T-0001、未知 T-9999 按不存在拒绝，通过。
- 日期、闰日、跨年不适用：after 的输入域是任务编号列表，不包含日期。

## 负向对照与还原

逐次临时改动生产实现，使用同一项目测试命令，均退出 101，且失败原因与设计预期一致：

- continue 改 break：`cargo test -p automed after_create_` 的跳过单测失败，C 未启动（4 通过、1 失败，lib 失败后 Cargo 不继续 e2e）；另跑 `cargo test -p automed --test end_to_end after_create_`，e2e 同样在 C 无运行会话处失败。
- 先创建、tick 后再保存依赖：`cargo test -p automed --test end_to_end after_create_`，B 的 started 为 true，期望 false，失败。
- 全部 Requeued 都受约束：`cargo test -p automed --lib after_create_requeued_`，Feature Implement 应放行而未放行，失败。
- 所有 Requeued 都不受约束：同一命令，Feature Design 无实现历史应阻塞却启动，失败。

日志：`.autome/output/m02-negative-{break,break-e2e,late-write,all-requeued,no-requeued}.txt`。这 5 次故障注入执行了 9 次用例（4 通过、5 预期失败），不混入最终正向验收的 10 个不同用例计数。

临时改动全部还原后重新完成上述三条最终验收命令；最终 `git diff --check` 无错误。没有留存故障注入到项目代码。
