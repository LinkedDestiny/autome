# M-03 · 实现 #3

本轮仅推进 M-03：`task.set_after`、`task.blocked` 与 `dashboard.get.queued`。上一轮审计 M-02 通过，无退回缺陷或必须逐字迁入的审计检查。新增测试保留在项目原有 Rust 测试体系，未修改已有 e2e 用例。

## 实现说明

- 创建和修改共用依赖校验；修改先检查实现会话历史和当前状态，再检查依赖图，全部通过后才持久化。记录 `task.after_set`，调用 tick，返回 task / started / queue_position。
- 通知扫描先于槽位判断，覆盖 Queued 和受约束的 Requeued；按边保存原因去重，无原因时清空通知标记。仍有依赖的任务不改状态、不改队列序号，继续检查后面的任务。
- dashboard 新增按项目、项目内真实队列顺序的 queued 组，带位置和完整任务字段，原 waiting / running 分组逻辑不变。

## 标「待审」之前的自审清单

### 1. 情形表逐行

| 设计条款 / 情形 | 检查与结果 |
|---|---|
| §3/§5 已开始实现、实现后停止再从 Design 重跑 | `after_blocked_set_rejects_without_side_effects`：当前 Implement 拒绝；持久 Impl 会话存在时 Stopped Implement / Active Design 拒绝，原依赖与事件数不变 |
| §3 需求 / Bug 可改状态表 | `deps::feature_editable_state_table`、`deps::bug_editable_state_table` 遍历全部节点与五类包装状态；需求分别检查有无实现历史；通过 |
| §5 Draft、Queued、Active Design、AwaitDesignApproval 可改；设计期暂停、停止、失败、恢复排队可改 | `after_blocked_set_accepts_design_states_and_clears` 经 RPC 逐项设置，状态不变，事件记录去重后的列表；通过 |
| §3 自引用、不存在、跨项目、成环、超限 | RPC 拒绝用例逐项验证中文单行信息、依赖不变、不新增 after_set 事件；域层同时覆盖三节点环；通过 |
| §5 清空依赖及事件 | Draft 清空记录 after_set；真实 e2e 清空 Queued 依赖后，同次 RPC 的 tick 启动任务，started=true、queue_position=null；通过 |
| §4 满槽、连续 tick、重复设置、增加/删除其它边、调整顺序 | `after_blocked_cancelled_is_notified_once_even_with_full_slots`：Queued / Requeued Design 分别跑 3 次，再经 RPC 设置同表、增边、换序、删其它边，始终仅 1 条通知，payload 四字段正确，状态与 queued_seq 不变 |
| §4 Failed → 重跑 → Failed | `after_blocked_failure_resets_on_rerun_and_notifies_again`：写第 1 条、恢复 Active 后清标记、再失败写第 2 条；原因变 cancelled 写第 3 条；Done 清标记；通过 |
| §4 Draft / Active / Paused / Stopped 依赖 | `after_blocked_passive_dependencies_do_not_emit_notices`：继续阻塞但不通知；通过 |
| §4 仅 Done 满足，完成后归档仍满足；不受约束的 Requeued | 回归 `after_create_done_is_the_only_satisfied_dependency` 与 `after_create_requeued_checks_design_and_implementation_history`；通过 |
| §6 dashboard 排队组 | `after_blocked_dashboard_preserves_project_queue_order`：两个项目分别逆创建顺序排队，Queued 与 Requeued 按 queued_tasks 顺序返回，位置从 1 起，cancelled 状态与原因正确；通过 |
| §4 取消后解除的真实流程 | `after_blocked_cancelled_dependency_holds_b_until_cleared`：真实 Git/worktree/包装脚本，取消 A 后 B 留队、通知 1 条；清空 B 后真实会话启动；通过 |

### 2. 失败分支

- 已实现、终态、非法依赖图与输入类型拒绝：上述 RPC / 域层测试通过；环拒绝前原列表保留，after_set 事件数为零。
- 缺少 after、null、字符串、非字符串数组：RPC 拒绝；空数组是合法清空，已单独验证。
- 拒绝授权：不适用，本里程碑不新增授权边界；静态检查及测试确认新方法不在 READ_METHODS，沿用现有写锁路径。
- 读盘失败、外部服务不可达、超时：不适用，M-03 未设计新分支，沿用 Store 错误传播与已有 tick；本轮不声称验证宿主故障恢复。

### 3. 全部迁移

- 依赖列表：空→单项、多项（含 8 项）、重复去重、原列表→同表、增边、换序、删其它边、清空，均经上述用例执行。
- 通知标记：None→cancelled/failed；同原因→不再通知；failed→Active→None→failed；failed→cancelled；cancelled→Done→None，均执行并核对事件数。
- 任务调度：Queued / Requeued Design + 未满足→保持排队；Done→开始；清空→开始；实现期 / 有实现历史 / Bug Requeued→不受依赖约束，分别由新增用例和 M-02 回归覆盖。
- 可改性：域层对 Draft/Queued/Done/Cancelled、全部需求/Bug 节点 × Active/Paused/Stopped/Failed/Requeued、需求实现历史 true/false 逐项执行；RPC 验证历史来源确实来自 sessions。
- 多通道：不适用，本轮仅内核 RPC；CLI 与 Web 网关是 M-05 / M-06。

### 4. 输入域边界

- 空数组、单元素、8 个不同 id、8 个重复 id、9 个重复 id；缺失、null、字符串、数字数组；空字符串 id、前导零 id；全部执行，结果符合设计。
- 自身、同项目、跨项目、不存在、直接环与三节点环；依赖指向 Done / Cancelled / Draft 均覆盖。
- 日期、闰日、跨年：不适用，此接口无日期输入。最大值为列表 8 项，已验证两侧。

## 命令与计数

- `cargo test -p automed after_blocked_`：7 个通过（lib 6 + e2e 1）。
- `cargo test -p automed after_create_`：6 个通过（lib 5 + e2e 1）。
- `cargo test -p autome-domain deps::`：12 个通过，直接覆盖修改权限状态表、依赖图和原因判定。
- 正向验证共 **25 个不同用例**，失败 0、忽略 0。里程碑验收命令本身为 **13 个**，比上一轮 M-02 验收基线 10 个多 3：新增 after_blocked 7 个，保留 after_create 6 个，本轮未重跑先前 fill_slots/the_parallel_limit 共 4 个。加上直接相关域层 12 个后，总执行范围比上一轮多 **15**；不是新增 15 个测试，实际新增 **7** 个。
- 初次编译发现测试引用了错误的 ipc 模块路径和 Outcome 的 Debug 格式，已修正为 crate::ipc 和 reply；修正后全部用例通过。
- 原始输出：`.autome/output/m03-blocked.txt`、`m03-create.txt`、`m03-domain.txt`。仅运行了格式化写入 `cargo fmt --all`，未自行运行宿主快检或全量门禁。

## 负向对照

| 临时注入 | 项目回归检查 | 红结果 |
|---|---|---|
| 每个 tick 都写通知 | `cargo test -p automed --lib after_blocked_cancelled_is_notified_once` | 1 个用例失败 |
| set_after 不调 tick | `cargo test -p automed --test end_to_end after_blocked_cancelled_dependency` | 1 个用例失败 |
| 校验前写入 | `cargo test -p automed --lib after_blocked_set_rejects` | 1 个用例失败 |
| 不检查实现历史 | 同上 | 1 个用例失败 |
| 设置依赖先清空导致丢失 notified | `cargo test -p automed --lib after_blocked_cancelled_is_notified_once` | 1 个用例失败 |

这 5 次负向运行均以测试断言失败判定（非编译失败），不增加不同用例总数。每次独立注入后自动还原；脚本与原始日志在 `.autome/output/m03-negative.rb`、`m03-negative-*.txt`。

## 还原后复验与结论

全部注入还原后，按验收契约顺序再次运行 `cargo test -p automed after_blocked_ && cargo test -p automed after_create_`，退出码 0：7 + 6 = 13 个全部通过。日志：`.autome/output/m03-final-blocked.txt`、`m03-final-create.txt`。域层源码未改，前述 12 个结果仍对应最终代码。`git diff --check` 通过。

四项自审执行完毕，M-03 可标「待审」，由独立审计决定是否关闭。全量门禁、宿主快检及 H-01/H-02 人工验收未运行；未访问真实账本，所有运行验证使用测试隔离目录。
