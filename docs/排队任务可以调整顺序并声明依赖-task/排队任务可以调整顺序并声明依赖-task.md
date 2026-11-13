# 排队任务的依赖声明与顺序调整：`--after`、`autome after`、`autome queue move`，出队跳过依赖未完成的任务

status: 已完成
design-round: 1/15
implementation-round: 8/30
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

任务文件：`docs/排队任务可以调整顺序并声明依赖-task/排队任务可以调整顺序并声明依赖-task-task.md`
任务卡：`docs/排队任务可以调整顺序并声明依赖-task/排队任务可以调整顺序并声明依赖-task-card.md`

下文「需求 n」指任务文件 §3 第 n 条，「验收 n」指 §4，「约束 n」指 §6，「问题 n」指 §7。

## 背景与现状

以下是设计轮在基线 `016f6a9` 上核对过的代码现状，行号为约数。

- **出队。** `store.queued_tasks`（store.rs:1429）取项目全部任务，按 `queued_seq IS NOT NULL, queued_seq, created_at`
  排序后只留 `Queued` / `Requeued`。`fill_slots_reporting_errors`（scheduler.rs:1706）逐项目按这个顺序对每个任务
  `advance(SlotAvailable)`，槽位满时 `break`。
- **`queued_seq`。** `mark_queued`（store.rs:1078）给它赋 `MAX(queued_seq)+1`，跨项目全局递增。调用它的地方只有三个：
  `create_task` 与 `convert_task`（状态为 `Queued` 时），以及 `task_start`（dispatch.rs:1606，无条件调用）。
  任务离开排队后不清这一列。`Requeued` 沿用第一次排队时的值，所以恢复的任务排在新任务前面。
  没有任何转换会回到 `Draft`（只有测试直接写它），所以待开始任务的 `queued_seq` 永远是 `NULL`。
- **创建。** `create`（dispatch.rs:1562）依次调 `prepare_task`（分配全局 `T-N` id）、`store.create_task`（事务）、
  写 `task.created`、`scheduler::tick`。tick 在同一次调用里启动任务。`create_task` 在仓库里有 61 处调用。
- **状态。** `TaskState` 有 `Draft / Queued / Requeued{resume} / Active{node} / Paused{resume} / Stopped{at} /
  Failed{at,reason} / Done / Cancelled`，其中 `is_terminal` 只含 `Done`、`Cancelled`。需求 Loop 在
  `AwaitDesignApproval` 之前的节点是 `Intake / Design / Review / Adjudicate`。Bug Loop 的节点是 `Node::BUG` 的 7 个。
  等待节点（`Await*`、`NotABug`）不占槽位。
- **归档。** `task_archive`（dispatch.rs:2170）只接受 `Done` 的任务。`convert_task` 会把原任务置为 `Cancelled` 并归档。
- **读者。** `dashboard.get`（dispatch.rs:2377）只有 `waiting`、`running` 两组，排队任务不出现在任何一组。读这个方法的
  CLI `status`、桌面 `dashboard.js`、网页 `notifier.ts` 都是按键名取值，所以新增一个键不会影响它们。
  `autome ls`（main.rs:530）把每个项目 `project.get` 的任务平铺成一张表，`--json` 输出任务对象数组。
- **网关。** `write-gate.ts` 的 `ALLOWED_WRITE_OPS` 与 `ARRAY_FIELDS` 就是任务卡说的「两份白名单」。不在
  `ARRAY_FIELDS` 里的数组参数会被 `validateShape` 拒绝。测试「长度上限钉死…全部 op 正路径可达」（write-gate.test.ts:517）
  会对每个放行的 op 用 `specific[op] ?? {}` 走一遍正路径。
- **测试替身。** e2e 的 `settle` 用 `has_pending_work` 判断有没有活。它把任何 `queued` 任务都算作「还有活」
  （end_to_end.rs:47）。所以一个永久被阻塞的排队任务会让 `settle` 在 90 秒后超时。
- **迁移测试。** v4→v5 测试（store.rs:3252）先把库降回 v4 再跑 `migrate`，这时 v6 块会在已经有新表的库上再跑一次。

## 目标与非目标

目标（对应需求 1–5）：

1. 任务可以声明「在哪些同项目任务之后」。创建时就能带上，之后可以在设计批准前改或清空。非法的依赖在写入前拒绝。
2. 出队时跳过依赖没有全部到 `Done` 的受约束排队任务（`Queued`，及设计期的 `Requeued`），但不改它的位置。依赖被取消或失败时写一次 `task.blocked`。
3. 排队与待开始的任务可以移到队首、某任务之前或之后。待开始的任务开始时按移动后的位置入队。
4. `after` / `blocked_by` 出现在 `task.get` / `project.get` / `dashboard.get`、`ls`、`status` 里，文本与 `--json` 一致。
5. Web 网关放行 `task.set_after`、`task.move`，并接受 `task.create` 的 `after`。

非目标：任务文件 §5「不在范围内」全部照办。另外三条：

- 依赖只在任务「还能改依赖」的排队状态起作用：`Queued`，以及停在设计期节点的 `Requeued`；实现期的 `Requeued` 不受约束（见方案 4.1）。
- 不改通知器规则（问题 11(f)，进 Backlog B-01）。
- 不改桌面端写白名单。

## 方案

### 1. 规则总览

| 事项 | 规则 |
|---|---|
| 依赖何时起作用 | 出队时，对「受约束的排队任务」起作用：`Queued`，以及 `resume` 节点属于 `PRE` 的需求任务 `Requeued`（这两类恰好都是用户还能改依赖的）。`resume` 在 `Implement` 及以后的 `Requeued`、Bug 任务的 `Requeued` 不受约束 |
| 依赖何时算满足 | 依赖任务的状态是 `Done`，与是否归档无关 |
| 阻塞原因 | 依赖为 `Cancelled` → `cancelled`；为 `Failed` → `failed`；其它未完成状态没有原因（`null`） |
| 何时写 `task.blocked` | 受约束的排队任务的某个依赖带阻塞原因，且这个原因没有通知过 |
| 谁能改依赖 | 需求任务在设计批准之前（含等批准），且从未开始过实现轮；Bug 任务只在开始之前 |
| 谁能移动 | `Queued`、`Requeued`、`Draft`；参照任务同样可以是这三类（含没移动过的 `Draft`） |

### 2. 数据：store v6（问题 2；约束 3、5）

**表。** 新增 `task_after` 表，不在 `tasks` 上加列。这样不用改 `TaskRecord`，也就不用改它的 14 处字面量构造（大多在现有测试里）。
也不用改 `create_task` 的 61 处调用。v6 块照约束 5 写成单个事务：

```sql
BEGIN;
CREATE TABLE IF NOT EXISTS task_after (
    task_id  TEXT NOT NULL REFERENCES tasks(id),
    after_id TEXT NOT NULL REFERENCES tasks(id),
    ord      INTEGER NOT NULL,
    notified TEXT,              -- 最近一次已写 task.blocked 的原因；NULL = 未通知
    PRIMARY KEY (task_id, after_id)
);
PRAGMA user_version = 6;
COMMIT;
```

v6 块用 `IF NOT EXISTS`，是因为 v4→v5 迁移测试会把已经是 v6 的库降回 v4 再迁移。这样那条测试只需要改版本号断言
（见方案 10）。旧任务迁移后没有任何行，也就是没有依赖。`queued_seq` 不动，所以队列顺序和迁移前一致。

**store 新接口**（名字供实现参考，实现可以微调，但职责不变）：

- `create_task_after(task, brief, after: &[String])`：和 `create_task` 在同一个事务里写任务、写依赖。
  原来的 `create_task` 改成调用它并传 `&[]`。约束 3 要求的「同一事务、在 tick 之前落库」就落在这里。
- `task_after(id) -> Vec<String>`：按 `ord` 排序。
- `set_task_after(id, &[String])`：在事务里按新列表同步：新列表里没有的边删除；仍保留的边**保留 `notified` 并更新 `ord`**；新增的边 `notified` 为 `NULL`。
  这样重复设置相同的列表、调整声明顺序、增加或删除其它依赖，都不会让保留边的通知记录丢失，也就不会重复写 `task.blocked`（约束 9）。
  清空依赖就是删除全部边。
- `project_after(project_id) -> BTreeMap<id, Vec<id>>`：环检测要用。
- `blocked_notice(task, on) -> Option<String>`、`set_blocked_notice(task, on, Option<&str>)`。
- `queued_seq(id) -> Option<i64>`。
- `has_impl_session(task_id) -> bool`：`sessions` 表里是否存在该任务 Impl 角色的会话记录（`kind` 的字符串值实现时核对）。会话记录不随重跑删除，所以它是「这个任务是否开始过实现轮」的可核验历史事实（见 §3 `check_editable`）。
- `queue_order(project_id) -> Vec<TaskRecord>`：取 `Queued`、`Requeued` 和 `queued_seq` 非空的 `Draft`。
  排序键和 `queued_tasks` 一样。
- `reorder_queue(project_id, ordered: &[String])`：在一个事务里，令 `base = MAX(queued_seq)`（全表），然后按给定顺序
  依次赋 `base+1+i`。只改本项目给定的这些行，别的项目的 `queued_seq` 一个也不动（问题 9）。

### 3. 校验：新模块 `autome_domain::deps`（问题 3、12；约束 4；验收 4）

纯函数，不做 I/O，放在 `crates/autome-domain/src/deps.rs`。

- `MAX_AFTER = 8`。内核和网关用同一个上限（问题 11(d)）：网页、CLI 能给的依赖一样多。
- `check_after(task_id, project_id, proposed, lookup) -> Result<Vec<String>, DepError>`。
  `lookup(id) -> Option<(project_id, after)>` 由内核从 store 构造。检查顺序如下：
  1. 先按**输入项数**判断：超过 8 项就拒绝（含重复项；网关限制的也是数组长度，两端口径一致，问题 11(d)）。
  2. 再去重，保留首次出现的顺序。
  3. 每个 id：是自己就拒绝；`lookup` 为空就是「不存在」；项目不同就是「跨项目」（问题 12：id 是全局的，
     所以这两种拒绝分开写）。
  4. 从每个新依赖出发沿 `after` 边做 DFS，能回到自己就是成环，报出完整路径。

  指向 `Done`、`Cancelled`、`Draft` 的任务都接受。指向 `Done` 的依赖立刻满足；指向 `Cancelled` 的依赖会在出队时
  按 §4 被阻塞并写事件。任务卡只列了四种拒绝，所以这里不再加别的拒绝。
- `check_editable(id, kind, state, impl_started) -> Result<(), DepError>`（问题 3）。设 `PRE = {Intake, Design, Review, Adjudicate,
  AwaitDesignApproval}`。`impl_started` 由内核用 `store.has_impl_session(id)` 算出再传入，纯函数不做 I/O：
  - **先看历史。** 需求任务 `impl_started == true` 时一律拒绝（文案「已开始实现轮」），不论当前状态。
    原因：`Failed` / `Stopped` 经 `RerunFrom` 可以回到 `Intake / Design / Review / Adjudicate`
    （task.rs:1027 起的分支，调度层没有禁止），当前节点属于 `PRE` 不能证明「还没有开始过实现」。
    实现→停止→从 Design 重跑后，`Active{Design}`、之后的 `Paused` / `Stopped` / `Failed` / `Requeued` 包装状态都被这一条挡住。
  - 再看当前状态。需求任务可以修改的状态：`Draft`、`Queued`；`Requeued{resume}`、`Active{node}`、`Paused{resume}`、`Stopped{at}`、
    `Failed{at,..}` 中节点属于 `PRE` 的那些。其中 `AwaitDesignApproval` 就是任务卡说的「等你决定（设计卡点之前）」。
  - 节点在 `Implement` 及以后的，拒绝。
  - Bug 任务只有 `Draft`、`Queued` 可以修改。Bug Loop 没有设计阶段，开始即进入定位与修复。
  - `Done`、`Cancelled` 拒绝。

  对正在运行（`Active`）或暂停、停止、失败的设计期任务改依赖，只改记录：`after` / `blocked_by` 照常显示，但它不在排队，
  所以暂时对调度没有作用；之后若它因暂停/恢复回到 `Requeued{PRE}`，新依赖就会在出队时生效（见 §4.1）。这和任务卡「依赖只控制什么时候开始」一致。
- `block_reason(state) -> Option<BlockReason>`：`Cancelled` 返回 `cancelled`，`Failed` 返回 `failed`，其它返回 `None`。
  另有 `is_satisfied(state) = (state == Done)`。
- `reposition(order, task, Placement::{Top, Before(id), After(id)}) -> Result<Vec<String>, DepError>`。
  参照任务就是自己时拒绝；不在 `order` 里时拒绝。`order` 由调用方准备，调用方负责把没移动过的待开始任务先追加进去（见 §5 `task.move`）。
- 拒绝文案都是一句中文，不含换行，沿用 `rejected(...)` 返回：

  | 情况 | 文案 |
  |---|---|
  | 自引用 | 任务不能依赖自己（T-5） |
  | 不存在 | 依赖的任务 T-99 不存在 |
  | 跨项目 | 依赖的任务 T-3 属于另一个项目，依赖只能指向同一项目的任务 |
  | 成环 | 这样设置会形成依赖环：T-5 → T-3 → T-5 |
  | 超过上限 | 依赖最多 8 个，给了 9 个 |
  | 已开始实现轮 | T-5 已经开始实现轮（实现），依赖只能在设计批准之前修改 |
  | Bug 已开始 | Bug 任务 T-5 已经开始，依赖只能在开始之前修改 |
  | 已结束 | T-5 已经结束（已取消），不能再改依赖 |
  | 移动参照是自己 | 不能以任务自己为参照移动 |
  | 参照不在队列 | T-9 不在队列里：只能参照同项目排队中或待开始的任务 |

### 4. 出队与 `task.blocked`（问题 4、5、6；约束 1、2、7、9）

**4.1 哪些排队任务受约束（问题 4）。** 受约束的集合 = 用户还能改依赖的排队任务，即 `check_editable` 通过的 `Queued` 与 `Requeued`：
`Queued`，以及 `resume` 在 `PRE`、且从未开始过实现轮的需求任务 `Requeued`。
任务卡的「排队任务」对外就包括 `Requeued`（它对外仍显示排队），所以设计期暂停→恢复排队的任务，在设计期新增的依赖要生效，不能被忽略。

`resume` 在 `Implement` 及以后的 `Requeued`（以及 Bug 任务的 `Requeued`）不受约束。理由是避免死锁：
需求任务进入实现后依赖才被取消，任务被挡住，而 `check_editable` 拒绝在实现期改依赖，用户没有办法解开。
受约束集合与「可改依赖」集合一致，所以**被挡住的任务用户总能用 `autome after … --none` 解开**，不存在死锁。
这些任务在设计期的依赖，在它们第一次出队（`Queued`）时已经被检查过；Bug 任务的依赖只能在开始前设置，同理。

判断「受约束」所需的 `impl_started` 由 `fill_slots` 对有依赖的 `Requeued` 任务调 `store.has_impl_session` 取得，没有依赖的任务不查。

**4.2 `fill_slots` 的改动。** 每个项目分两步，二者都在拿到 `queued_tasks` 之后：

1. **通知。** 这一步与槽位无关，所以槽位满的项目也会做。对每个**受约束**（§4.1）且有依赖的排队任务，逐个依赖计算
   `block_reason`：
   - 原因为 `r`，且 `notified != r`：写 `task.blocked`，payload 为
     `{"task", "on", "reason", "project_id"}`，subject 是被阻塞的任务；然后 `notified = r`。
   - 没有原因，但 `notified` 非空：把它清空。这样依赖从失败重跑、之后又失败时，会再通知一次。

   同一个 `(任务, 依赖, 原因)` 只写一次，连续多少个 tick 都不会再写（约束 9）。
2. **出队。** 依次处理，遇到受约束且有任一依赖不满足的任务就 `continue`（不是 `break`）。不碰它的
   `queued_seq` 和状态。后面的任务照常 `advance`（约束 1）。

没有依赖时第 1 步什么都不写、不多做查询，第 2 步走原路径。出队顺序、`queue_position`、事件的种类与顺序都和现在一样（约束 7）。

**4.3 归档与失败（问题 5、6）。** 满足条件只看 `Done`。理由有两条：

- 能被归档的任务只有 `Done`（dispatch.rs:2170）。
- 状态不是 `Done` 却已归档的，只有被取消或改建的任务，它们的原因已经是 `cancelled`。

所以任务卡里的「归档」不构成独立的阻塞，「依赖到已完成就自然轮到它」在任何情况下都成立。

依赖从 `Failed` 经过 `rerun` / `extend` 最终走到 `Done` 时，被阻塞的任务自然出队。`Stopped`、`Paused`、`Draft` 是用户
自己主动造成的停顿，不写事件，但会出现在 `blocked_by` 里，带着当前状态。依赖完成不触发任何别的动作（约束 2）。

### 5. RPC（问题 1、9；约束 3、4、8）

**参数名（问题 1）。** 任务一律用 `task_id`。理由：现有全部任务方法、Web 网关对 `task.start` / `task.convert` 的校验、
CLI 的 `trigger` 路径都用 `task_id`，多一个 `task` 会让网关和面板各写一套。事件 payload 照任务卡用 `task`。

- **`task.create`** 增加可选参数 `after: [id]`。
  - 不是字符串数组时，用 `bad_params` 拒绝，文案「after 必须是任务编号列表」。
  - 在 `prepare_task` 分配 id 之后、写库之前，调 `check_after`。被拒绝时不留下任务（约束 4）。
  - 校验通过就用 `create_task_after` 落库，之后才 tick（约束 3）。
  - 依赖非空时，`task.created` 的 payload 加上 `"after"`。依赖为空时 payload 和现在一样。
- **`task.set_after { task_id, after: [id] }`**，写方法，不进 `READ_METHODS`（约束 8）。
  - 依次检查 `check_editable`（`impl_started` 取自 `store.has_impl_session`）、`check_after`，然后 `set_task_after`。传空数组就是清空。
  - 写 `task.after_set {after}` 事件。现有写方法都留事件，桌面端和面板靠事件刷新。
  - 最后调 `scheduler::tick`。它和 `task.create` / `task.start` 一样：清空依赖、而且有空槽位时，在这次调用里就启动
    （验收 2 说的「下一个 tick」就是这一次）。
  - 返回 `{task, started, queue_position}`。
- **`task.move { task_id, before?, after?, top? }`**，写方法（问题 9）。
  - `before`、`after`、`top: true` 必须恰好给一个，否则用 `bad_params` 拒绝，文案「before、after、top 必须且只能给一个」。
  - 被移动的任务必须是 `Queued`、`Requeued` 或 `Draft`，否则拒绝，文案「只有排队或待开始的任务可以移动」。
  - 参照任务必须同项目，且处于 `Queued`、`Requeued` 或 `Draft`（含没移动过的 `Draft`，任务 §7 问题 9）；违反时按 §3 的文案拒绝。
    跨项目、运行中、已结束的参照合并成「不在队列里」这一种拒绝。
  - 步骤：取 `queue_order` 的 id 列表；被移动的任务和参照任务里，凡是没移动过的 `Draft`（`queued_seq` 为空，不在列表里），
    **先按「参照任务在前、被移动任务在后」的次序追加到列表末尾**（等同于它们各自 `task start` 时的队尾位置）；
    再用 `reposition` 算出新顺序，最后 `reorder_queue` 落库。一次事务内完成，所以两个都没移动过的待开始任务
    `queue move B --before A` 直接成功：落库后 B 在 A 之前，二者的 `queued_seq` 都非空，从此都是「移动过的待开始任务」。
  - 不调 tick：移动不改变任何任务能否出队，有空槽位的话它们早就开始了。
  - 写 `task.moved {task, project_id, before, after, top, queue_position}` 事件。
  - 返回 `{task, queue_position, order: [ids]}`。
  - 允许把任务移到它自己的依赖前面：出队时依赖规则优先，它会被跳过。
- **`task.start`**（问题 9）：`queued_seq(id)` 为空时才调 `mark_queued`。为空就是没移动过，照旧排到队尾（约束 7）；
  非空（被移动过，或曾被当作参照而落了位置）就保持已有位置，按这个位置入队。

### 6. 可见（问题 7、8；约束 10）

- **`task_json`**（`task.get`、`project.get`、`dashboard.get` 共用）新增两个字段：
  - `after: [id]`：声明的依赖，按声明顺序。
  - `blocked_by: [{id, state, reason}]`：`after` 里状态不是 `Done` 的那些。`state` 是线上的 `TaskState`，`reason` 取
    `"cancelled"`、`"failed"` 或 `null`。

  没有依赖的任务两个字段都是 `[]`，现有字段不变。
- **`project.get` 的任务对象**（`ls` 的数据来源，N1）另加 `queue_position`：每个项目只调一次 `queued_tasks`，
  `Queued` / `Requeued` 的任务给 1 起始的下标，其它任务为 `null`。`task.get` 本来就带这个字段，取值规则相同。注意 `task.changes` 已经有一个同名但不同义的 `blocked_by`，
  它在另一个对象里，两者不冲突。
- **`dashboard.get`**（问题 7）新增 `queued` 组，按项目、按出队顺序列出 `Queued` / `Requeued` 的任务。条目的形状和
  `running` 一样，另带 `queue_position`。`waiting`、`running` 不变。现有读者都是按键名取值，见「背景与现状」。
- **事件**：`task.blocked`、`task.moved`、`task.after_set`。

### 7. CLI（问题 8、10）

- `args.rs` 的 `VALUED` 增加 `--after`、`--before`（变为 12 项）。`--top`、`--none` 是开关。
- **`task new … --after T-41[,T-47]`**。每个名字先经 `resolve_task` 解析（id 或 slug 唯一前缀），再作为 `after` 放进
  同一次 `task.create`（约束 3）。允许重复写 `--after`，也允许逗号分隔。
- **`autome after <任务> <列表> | --none`** 调 `task.set_after`。列表和 `--none` 同时出现、或两者都没有，按用法错误退出码 2。
- **`autome queue move <任务> --before X | --after X | --top`** 调 `task.move`。三者不是恰好一个时，退出码 2。
  `queue` 只有 `move` 这一个子命令，其它写法一律是用法错误。`--after` 在 `task new` 下表示依赖，在 `queue move` 下表示位置。
  每个动词允许哪些开关本来就是在 `run` 里按动词列的，所以不会混淆。
- **`ls` 文本（问题 8）。** 不在排队的任务照原样、原顺序输出。`Queued` / `Requeued` 的任务从原位置移到末尾的
  「排队（按出队顺序）」段：按项目分组，按 `project.get` 任务对象里的 `queue_position` 排序。行格式和原来一样，被阻塞的行（`Queued` 且 `blocked_by`
  非空）在末尾追加 `  ← 等 T-41（已取消）`，多个依赖用顿号连接。
  - 没有排队任务时不出现这一段，输出和现在一样。
  - 费用脚注仍然是最后一行。`cost_cli_ls_appends_cost_and_a_footnote` 断言的「行首是 id、行尾是费用、末行是脚注」都不受影响。
  - 被阻塞的任务留在原位标注，不排到可出队任务之后：它的 `queued_seq` 没变，解除后就从这个位置出队。
    `queue_position` 仍然是它在 `queued_tasks` 里的下标。
  - `ls --json` 仍是任务对象数组，每个对象多了 `after` / `blocked_by`，表达的是同一份数据。
- **`status <任务>`**：`after` 非空时，在「状态」行后加一行 `  依赖      T-41（已完成）、T-47（已取消）`。
- **`status`（不带任务）**：`queued` 非空时，在「运行中」后加「排队」段。每行是 `id slug 排队 · 第 n 位`，
  被阻塞的行末尾带「← 等 …」。
- 短状态名由 `render::short_state` 给出：待开始 / 排队 / 运行中 / 等你决定 / 已暂停 / 已停止 / 失败 / 已完成 / 已取消。
- `render::event_line` 为三个新事件各加一行文案：
  - `T-52 被阻塞：等 T-41（已取消）`
  - `T-52 移到队首` / `移到 T-44 之前` / `移到 T-46 之后`
  - `T-52 依赖改为 T-41、T-47` / `T-52 依赖已清空`
- `USAGE` 在「开始」段补一行 `after <任务> <任务,…> | --none`，在 `task new` 行补 `[--after 任务,…]`；在「卡点与控制」段
  补 `queue move <任务> --before X | --after X | --top`。现有的 `USAGE` 断言只检查「包含」，所以不受影响。

### 8. Web 网关（问题 11）

- (a) 两份白名单就是 `ALLOWED_WRITE_OPS` 和 `ARRAY_FIELDS`。
  - `ALLOWED_WRITE_OPS` 加 `task.set_after`、`task.move`。
  - `ARRAY_FIELDS` 加 `'task.set_after': ['after']`，并把 `task.create` 改成 `['attachments', 'doc_refs', 'after']`。
- (b) 照 `task.start` / `task.convert` 的写法，在 `validateOpSpecific` 里加分支：
  - `task.set_after`：`task_id` 是非空字符串；`after` 是数组，最多 8 个（`MAX_AFTER_IDS = 8`），每项匹配 `^T-\d+$`。
  - `task.move`：`task_id` 是非空字符串；`before` / `after` / `top===true` 恰好一个；`before` / `after` 匹配 `^T-\d+$`。
  - `task.create` 的 `after`：规则与 `task.set_after` 相同。
- (c) `task.create` 的 `after` 放行。理由：内核和 CLI 都支持，网关不放行的话面板以后无法创建带依赖的任务，到时还得再开一个任务。
- (d) 内核同样限 8 个，见 §3。
- (e) 往测试的 `specific` 表补两项正路径（见方案 10）。新的拒绝文案都是中文。
- (f) 通知器不改。`task.blocked` 的 payload 已经含 `task`、`on`、`reason`、`project_id`，足够写规则，进 B-01。

### 9. `task.convert` 与文档（问题 13；任务 §5 随行文档）

- `task.convert` 不继承、也不改指依赖。依赖原任务的排队任务会看到「等 T-x（已取消）」，并收到一次 `task.blocked`。
  用户用 `autome after … --none` 或改指新任务来解除。
- `docs/development/technical-design.md`：
  - §8 第 307 行「排队按 created_at 升序」改为 `queued_seq` 顺序、跳过依赖未完成的受约束排队任务、可移动。
  - §13 补 `task_after` 表与 v6。
  - §14 补 `task.set_after`、`task.move`、`task.create.after` 与三个新事件。
- `README.md`「The command line」补 `after`、`queue move`、`task new --after`（英文）。

### 10. 现有测试的必要改动（验收 5）

| 位置 | 改动 | 理由 |
|---|---|---|
| store.rs `assert_eq!(SCHEMA_VERSION, 5)` | 5 → 6 | 任务文件点名的版本钉 |
| store.rs v4→v5 迁移测试 `assert_eq!(version, 5)` | 5 → 6 | 同上；降级语句不改，靠 `IF NOT EXISTS` |
| write-gate.test.ts「全部 op 正路径可达」的 `specific` 表 | 加 `'task.set_after': {task_id:'T-1', after:[]}`、`'task.move': {task_id:'T-1', top:true}` | 这条测试的设计就是「每个放行的 op 在表里登记正路径」。新 op 有必填参数，用 `{}` 走不通。这是登记新 op，不是迁就旧断言 |

除此之外，现有的需求与 Bug e2e、CLI 测试、调度单测一行不改。

### 11. 否决的做法

- **在 `tasks` 上加一列 JSON。** 要改 `TaskRecord` 和它的 14 处构造，`notified` 也没地方放。
- **重排用小数或间隔序号。** `queued_seq` 是 `INTEGER`。重编号只动本项目的行，更简单。
- **扫描账本去重 `task.blocked`。** 每个 tick 都要查事件。用 `notified` 列是 O(1)，而且可以随状态复位。
- **让所有 `Requeued` 都受依赖约束。** 实现期的 `Requeued` 会出现 §4.1 的死锁；所以只约束「还能改依赖」的那部分。
- **完全不约束 `Requeued`。** 设计期恢复排队的任务刚设置的依赖会被忽略，违反任务 §3.2。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 依赖校验纯函数与 store v6 迁移 | 0 | |
| M-02 | 已完成 | 创建即带依赖，出队跳过依赖未完成的任务 | 0 | |
| M-03 | 已完成 | 改依赖、阻塞事件与 dashboard 排队组 | 0 | |
| M-04 | 已完成 | task.move 调整队列顺序 | 0 | |
| M-05 | 已完成 | CLI：task new --after、after、queue move、ls 与 status | 0 | |
| M-06 | 已完成 | Web 网关放行与随行文档 | 0 | |

## 里程碑说明

新用例按里程碑分别使用唯一前缀（`deps::`、`deps_store_`、`after_create_`、`after_blocked_`、`queue_move_`、`deps_cli_`）。
设计轮已用 `git grep` 核对过，这些前缀在基线上匹配零个用例，所以过滤得到的数字就是新增数。
新用例的名字不得包含回归过滤用到的子串：`fill_slots_`、`task_start_queues_a_draft`、`the_parallel_limit`、
`task_card_a_started_draft`。否则回归命令的计数会被带偏。
新 e2e 用例不用 `settle`，因为被阻塞的排队任务会让它超时（见「背景与现状」）。它们用现有的 `scheduling_wait` 加自定义判据
（tick 后「没有运行中的会话，且指定任务处于指定状态」）。`has_pending_work` / `settle` 不改。

- **M-01**：覆盖需求 1 的校验与迁移、验收 4 与 5 的迁移部分；没有前置。可观察的结果：`deps` 模块与 v6 迁移通过单测。
  - `deps::` 12 个：自引用、不存在、跨项目、直接环、三节点环、超过 8 个、去重保序并接受 Done/Cancelled 依赖、
    全部拒绝文案是单行中文、需求任务的可改状态表（含 `impl_started == true` 时 `Active{Design}`、`Stopped{Design}` 等设计期状态也被拒绝）、Bug 任务的可改状态、`block_reason` 与 `is_satisfied`、`reposition` 的三种位置与两种拒绝。
    「超过 8 个」用例含 9 个相同 id 也被拒绝（按输入项数）。
  - `deps_store_` 5 个（`has_impl_session` 并入 `set_task_after` 那个用例，数量不变）：v5→v6 迁移后无依赖且队列顺序不变；`create_task_after` 因外键失败时整体回滚、不留任务；
    `set_task_after` 替换与清空：保留边的 `notified` 不变、新增边为空、删除边消失；`has_impl_session` 随 Impl 会话记录翻转；`reorder_queue` 不动别的项目的 `queued_seq`；`blocked_notice` 往返。
- **M-02**：覆盖需求 1（create）、需求 2（跳过）、需求 4（`task.get` / `project.get`）、验收 1、验收 4（经 RPC 拒绝无副作用）；
  前置 M-01。
  - `after_create_` 6 个：
    - dispatch：创建时自引用（用预测的 id）、不存在、跨项目都被拒绝，且任务数不变。
    - dispatch：`task.get` / `project.get` 带 `after` / `blocked_by`，无依赖时为 `[]`。
    - scheduler：被阻塞的任务 `queued_seq` 与 `queue_position` 不变，后面的任务先启动。
    - scheduler：依赖置为 `Done` 后放行。
    - scheduler：`Requeued{Design}`（无 impl 会话）受依赖约束、被跳过；`Requeued{Implement}` 与有 impl 会话的 `Requeued{Design}` 不受约束、照常出队。
    - e2e `after_create_b_waits_for_a_and_starts_once_a_is_done`（验收 1：`parallel=2`，A、C 的会话 sleep 占槽；
      A 经批准、合并到 `done` 后 B 自动开始；断言账本里 A 到 `done` 的 seq 小于 B 第一次进入 `active` 的 seq）。
- **M-03**：覆盖需求 1（`set_after`）、需求 2（阻塞与事件）、需求 4（`dashboard.get`）、验收 2；前置 M-02。
  - `after_blocked_` 7 个：
    - dispatch：已开始实现轮的任务被拒绝，原依赖不变；「实现后停止、从 Design 重跑」后（有 impl 会话，当前 `Active{Design}`）修改依赖同样被拒绝，原依赖不变；成环被拒绝，原依赖不变。
    - dispatch：Draft、Queued、Active{Design}、AwaitDesignApproval 可改，清空后写 `task.after_set`。
    - dispatch：`dashboard.get` 的 `queued` 组按出队顺序列出，并带 `blocked_by` 和原因。
    - scheduler：依赖取消后连跑 3 个 tick 只有 1 条 `task.blocked`，payload 正确；再 `task.set_after` 同样的列表（或加上另一个依赖）后再 tick，事件数仍是 1；设计期 `Requeued{Design}` 的任务同理写出一条。
    - scheduler：依赖失败、重跑、再失败，共 2 条。
    - scheduler：依赖为 Draft 或运行中时不写事件。
    - e2e `after_blocked_cancelled_dependency_holds_b_until_cleared`（验收 2）。
- **M-04**：覆盖需求 3、验收 3；前置 M-02。
  - `queue_move_` 7 个：
    - dispatch：top / before / after 三种移动后的顺序，以及 `task.moved` 的 payload。
    - dispatch：位置参数为零个或两个、参照是自己、参照在别的项目、参照在运行中，都被拒绝且文案是中文。
    - dispatch：参照是没移动过的待开始任务——两个没移动过的待开始任务 `move B --before A` 成功，随后 `task start` A、B，B 先于 A 出队；`--after` 同理。
    - dispatch：别的项目的 `queued_seq` 不变。
    - dispatch：移动过的待开始任务开始后保持位置。
    - dispatch：没移动过的待开始任务开始后仍排到队尾。
    - e2e `queue_move_the_third_task_moved_to_top_starts_first`（验收 3：`parallel=1`，占槽任务的会话 sleep；
      三个排队任务把第三个移到队首；取消占槽任务后，先启动的是它，另外两个仍在排队）。
- **M-05**：覆盖需求 1、3、4 的 CLI 部分和验收 6；前置 M-03、M-04。
  - `deps_cli_` 7 个。
  - `tests/cli.rs` 4 个。它们用「依赖是待开始任务」的方式造出稳定被阻塞的排队任务，不需要占槽：
    - `task new --after`：用 id 和 slug 前缀各写一次，检查 `ls --json` 的 `after` / `blocked_by`，且账本里没有 `task.after_set`；自引用和不存在的依赖退出码为 1，stderr 是中文。
    - `after … --none`，以及列表与 `--none` 同时给或都不给时退出码为 2。
    - `queue move --top` 之后 `ls` 排队段的顺序和「← 等」标注；位置参数为零个或两个时退出码为 2。
    - `status <任务>` 的「依赖」行，以及 `status` 的「排队」段。
  - 单元测试 3 个：`USAGE` 包含新命令、`args` 解析 `--before` / `--after`、`event_line` 的三个新事件。
- **M-06**：覆盖需求 5 和随行文档；前置 M-03、M-04。
  - `write-gate.test.ts` 新增 3 个：`set_after` / `move` 放行且 `task_id` 必填；`move` 的位置参数恰好一个；
    创建与修改依赖的两处 id 列表在 8 个放行、9 个拒绝，非 `T-N` 拒绝；移动位置只接受单个编号。
  - 修改 `specific` 表，见方案 10。
  - 文档按方案 9 改。

## 验收契约

### M-01 验收契约

最新证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-01-r1-audit.md · 通过 · 审计 #1

- 验收命令：`cargo test -p autome-domain deps:: && cargo test -p automed --lib store::`
- 预期：第一条 12 个用例通过。第二条通过的用例数为 store 模块基线 52（设计轮按源码 `#[test]` 计数，实现轮以实际运行为准，
  记入证据）加 5，共 57，其中两条版本号断言已改为 6。
- 负向对照：环检测只查直接边时，「三节点环」用例失败。v6 块写成不带 `IF NOT EXISTS` 的 `CREATE TABLE` 时，v4→v5 迁移测试
  报 `table task_after already exists` 失败。

### M-02 验收契约

最新证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 验收命令：`cargo test -p automed after_create_ && cargo test -p automed fill_slots_ && cargo test -p automed --test end_to_end the_parallel_limit`
- 预期：`after_create_` 6 个通过（lib 5 个、end_to_end 1 个）。`fill_slots_` 2 个现有用例通过。`the_parallel_limit` 2 个现有用例通过（并发上限用例与 `task_card_a_started_draft_waits_behind_the_parallel_limit`）。
- 负向对照：
  - 跳过写成 `break` 时，「后面的任务先启动」用例与验收 1 的 e2e 失败（C 不会启动）。
  - `after` 改成在 tick 之后才落库（先创建、再写依赖）时，e2e 里 B 在创建时就被启动，断言「B 仍在排队」失败。
  - 所有 `Requeued` 都按依赖跳过时，「`Requeued{Implement}` 不受约束」用例失败；完全不约束 `Requeued` 时，「`Requeued{Design}` 受约束」用例失败。

### M-03 验收契约

最新证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-03-r4-audit.md · 通过 · 审计 #3

- 验收命令：`cargo test -p automed after_blocked_ && cargo test -p automed after_create_`
- 预期：`after_blocked_` 7 个通过（lib 6 个、end_to_end 1 个），`after_create_` 6 个仍然通过。
- 负向对照：
  - 每个 tick 都写 `task.blocked` 时，「3 个 tick 只有 1 条」用例失败。
  - `task.set_after` 不调 tick 时，验收 2 的 e2e 断言「清空后 B 开始」失败。
  - 校验放在写入之后时，「成环被拒绝，原依赖不变」用例失败。
  - `check_editable` 只看当前节点时，「实现后重跑 Design 仍被拒绝」用例失败。
  - `set_task_after` 先删后插并清空 `notified` 时，「同样列表再设置后事件数仍是 1」用例失败。

### M-04 验收契约

最新证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-04-r6-audit.md · 通过 · 审计 #4

- 验收命令：`cargo test -p automed queue_move_ && cargo test -p automed task_start_queues_a_draft && cargo test -p automed --test end_to_end task_card_a_started_draft_waits_behind_the_parallel_limit`
- 预期：`queue_move_` 7 个通过（lib 6 个、end_to_end 1 个），另外两条命令各 1 个现有用例通过。
- 负向对照：
  - 只给被移动的任务赋 `MAX+1`、不重编号时，`--top` 用例与验收 3 的 e2e 失败。
  - `task_start` 仍然无条件调 `mark_queued` 时，「移动过的待开始任务保持位置」用例失败。
  - 重编号用全表顺序而不是只动本项目时，「别的项目不变」用例失败。
  - 参照是没移动过的待开始任务时仍被拒绝，「参照是没移动过的待开始任务」用例失败。

### M-05 验收契约

最新证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-05-r7-audit.md · 通过 · 审计 #5

- 验收命令：`cargo test -p autome-cli deps_cli_ && cargo test -p autome-cli --test cli`
- 预期：`deps_cli_` 7 个通过（bin 单测 3 个、cli 集成 4 个）。`tests/cli.rs` 共 28 个通过，即基线 24 加新增 4，其中包括
  `cost_cli_ls_appends_cost_and_a_footnote`。
- 负向对照：
  - `ls` 排队段按 `created_at` 而不是 `queue_position` 排序时，`queue move --top` 用例失败。
  - 允许 `--none` 与列表同时出现时，退出码 2 的断言失败。
  - `task new --after` 写成「先创建、再调 `task.set_after`」时，账本里会多出一条 `task.after_set`。`task new --after` 用例
    断言没有这条事件，因此失败。

### M-06 验收契约

最新证据：docs/排队任务可以调整顺序并声明依赖-task/evidence/M-06-r8-audit.md · 通过 · 审计 #6

- 验收命令：`npm test --prefix apps/web -- write-gate && ! grep -n "排队按 created_at 升序" docs/development/technical-design.md && grep -q "queue move" README.md && grep -q "task.set_after" docs/development/technical-design.md`
  （`apps/web/node_modules` 不存在时先执行 `cd apps/web && npm ci`）
- 预期：`write-gate.test.ts` 47 个用例通过，即基线 44 加新增 3。三条 grep 检查都成立。
- 负向对照：
  - 漏加 `ARRAY_FIELDS` 时，「全部 op 正路径可达」用例因「task.set_after 的 after 不允许数组」失败。
  - 不限 8 个时，「9 个拒绝」用例失败。

## 测试与验证

- 每个里程碑只跑上面的子集。全量门禁（`sh scripts/ci-local.sh`，含 fmt、clippy、全部 e2e、web、desktop）由宿主在合并前运行，
  承担验收 5、7 的全量回归。
- **约束 6：** 所有手工核对都必须像 `tests/cli.rs` 那样把 `AUTOME_HOME` 指到临时目录。不得用本 worktree 构建的二进制打开
  `~/.autome`。
- 超过 5 行的输出写到 `.autome/output/`。

## 风险与回退

- **v6 迁移让旧二进制打不开账本**（约束 6）。合并并重装后，真实账本会迁到 v6，回退需要旧备份。由人工验收 H-01 在合并后确认。
  实现期不碰真实账本。
- **e2e 的时序。** 验收 1、3 依赖 sleep 占住槽位，沿用 `the_parallel_limit…` 已验证过的写法。判据用账本 seq 和状态，不用「sleep 后看一眼」。
- **`fill_slots` 每个 tick 多出的查询。** 只对有依赖的 `Queued` 任务做，依赖最多 8 个，可以忽略。
- **两个 `blocked_by`。** `task.changes` 和 `task_json` 里各有一个同名字段，含义不同。它们在不同的对象里，文档 §14 里写明。

## 人工验收清单

- H-01 合并并重装 autome 后，用已安装的 `autome ls` 打开真实账本（会迁到 schema 6），确认能正常列出、已有排队任务的先后与升级前一致
- H-02 有排队任务时打开桌面端首页与网页面板，确认页面照常显示（`dashboard.get` 多了 `queued` 组，界面未改）

## Backlog

- B-01 通知器增加 `task.blocked` 规则，依赖被取消或失败时提醒用户（问题 11(f)，任务卡只要求事件足以支撑提醒）
- B-02 面板上的拖动排序与依赖编辑（任务卡「不做什么」，W6/W7）
- B-03 `task.convert` 时提示或提供把依赖改指到新任务

## 争议项

（暂无）
