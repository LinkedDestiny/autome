# 「不是 Bug」改建成需求：`task.convert` RPC 与 `autome task convert`

status: 设计中
design-round: 0/15
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

- 任务文件：`docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome-task.md`
- 任务卡正文：`docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome-card.md`

下文「A-xx」指任务文件 §4 的验收条目，「约束 n」指 §6 第 n 条，「问题 n」指 §8 第 n 条。代码位置以基线 `42c90a5` 为准。

## 背景与现状

定位轮写「不是 Bug」后，任务停在 `active/not_a_bug`（`bug_loop.rs:227`）。这个节点不占并行名额（`task.rs:117` `occupies_slot`），
接受 `Cancel` 与 `Reply`。用户要把它改成需求，目前只能 `autome cancel` 加 `autome task new --kind feature --brief <卡片>`，
但卡片正文只在内核库里（`task_briefs` 表），没有命令能导出，`render.rs:113` 的提示没法照着做。

已核实的相关代码事实：

- `create`（`dispatch.rs:1431`）：slug 是 `unique_slug(slugify(request))`，候选集合含所有任务（包括已取消的）；`title = first_line(request)`，
  之后没有任何代码改写标题（`store.rs` 里没有 `set_task_title`）。`store.create_task` 在一个事务里写任务行与 brief；
  然后写 `task.created {project_id}`，不是草稿时立刻 `scheduler::tick`。
- `task_cancel`（`dispatch.rs:1878`）= `stop_running_session` + `task_trigger(Cancel)`。`task_trigger` 先 `apply_trigger`，再 `tick`。
  `apply_trigger` → `advance` 依次做 `resolve_config`、状态机检查、`set_task_state(Cancelled)`、`task.updated` 事件，
  最后 `perform(RunCancel)` → `cancel_task`（`scheduler.rs:3596`）：把文档目录复制到 `<doc_root>/.archive/<slug>`，
  强制删除 worktree 与分支，再 `set_task_archived`。文件系统步骤的错误都被忽略（`let _`）。
- 附件：`TaskRecord.attachments` 存的是源文件绝对路径；第一次开会话时 `start_session`（`scheduler.rs:2439`）调
  `write_task_inputs` 把它们复制到 worktree 的 `<doc_dir>/attachments/<文件名>`。所以停在 `not_a_bug` 的任务，worktree 里一定有一份副本，
  取消时这份副本跟着文档目录一起被归档。
- `store.rs` 没有删除任务的方法；`next_task_id` 取「行数」与「最大编号」两者的较大值，所以删掉一行不会让编号重复。
- 网页端的事件消费者：`notifier.ts:39` 用 `matchEvent` 按 `NOTIFICATION_RULES` 匹配事件，没匹配上就跳过；`levelOf` 对未知 kind 返回
  `undefined`。新增的事件类型不会让它出错。
- 网关测试 `write-gate.test.ts:553` 会对 `ALLOWED_WRITE_OPS` 里每个 op 拿 `specific[op] ?? {}` 断言能通过，所以新 op 如果有必填字段，
  必须在 `specific` 里登记正例。桌面端 `packaging.test.js:185` 只检查桌面端自己的白名单，本任务不碰它。
- T-37 在 `main`（`b4ea14b`）上仍未合并：`write-gate.ts` 里没有 `task.start`、`task.reply`。

## 目标与非目标

**目标**：一次调用完成「用同一份正文新建需求任务，并取消原 Bug 任务」，并且要么两件事都做成、要么都不做（约束 4）。
内核方法、CLI 命令、网页网关三处都能调用它（A-01…A-07）。

**非目标**：W7 按钮与 `apps/web/app` 页面；协议 prompt 与 `protocol/`；需求改建成 Bug；从 `not_a_bug` 以外的状态改建；
`apps/desktop`；T-37 的内容；崩溃恢复时清理半截状态（见「风险」R1）；`autome-domain` 不改（`Cancel` 在 `not_a_bug` 已合法）。

## 方案

### 1. 内核 `task.convert`（A-01，`dispatch.rs`）

参数是 `{task_id: string, kind: "feature", start?: bool}`。派发表在 `"task.cancel"` 那一行之后加一行 `"task.convert" => task_convert(ctx, p)`，
不进 `READ_METHODS`（约束 3），因此 `handle_command` 会让它全程持有驱动锁。

**第 1 步：预检。** 按下面的顺序检查，任何一项不过就返回错误，不写任何东西：

1. `task_id`：用 `str_param` 取，缺了报 `InvalidParams`。
2. `kind`：必填，只能是字符串 `"feature"`。缺了报 `InvalidParams`「缺少 kind：目前只能改建成需求（feature）」；
   其它值报「只能改建成需求任务：kind 必须是 feature」（问题 4）。
3. `start`：可以不传，传了必须是布尔值，否则报「start 必须是布尔值」。
4. `get_task`：任务不存在时报 `NotFound`（沿用 `StoreError::NotFound` 的映射）。
5. `kind != Bug`：报 `TransitionRejected`「只有 Bug 任务可以改建成需求：<id> 是需求任务」。
6. `state != Active{NotABug}`：报 `TransitionRejected`「只有停在『不是 Bug』等你决定的任务可以改建：<id> 现在是 <state>」。
   其中 `<state>` 取状态的串化形式，例如 `draft`、`queued`、`active/locate`、`active/await_info`、`paused`、`failed`、`cancelled`、`done`。
   已归档的任务一定处于终态，所以也落在这一条里（问题 9）。
7. 项目不是 active：报 `TransitionRejected`「项目已移除」，与 `task_create` 一致。
8. 附件：逐个检查 `Path::is_file`。缺失时报 `TransitionRejected`「附件不存在：<源路径>；原任务里的副本在 <worktree>/<doc_dir>/attachments/<文件名>，
   放回原处后重试」，只有副本存在时才带分号后面那半句（问题 2）。

**第 2 步：建新任务，但不写事件。** 把 `create` 里组装 `TaskRecord` 与 `create_task` 的部分抽成一个内部函数，供 `create` 与 `task_convert`
共用，现有 `task.create` 的行为不变（约束 6）。新任务的字段这样取：

- `kind = Feature`，`state = Draft`。
- `request` 取原任务的 `request`，`title` 直接取原任务的 `title`（与 `first_line(request)` 等值，但写明是沿用）。
- `slug` 照常从 `request` 派生并去重。原任务仍在任务列表里，所以新 slug 是 `<原 slug>-2` 这一类（问题 1）。
  原任务归档到 `<doc_root>/.archive/<原 slug>`，新任务的文档目录是 `<doc_root>/<新 slug>`，两者不会冲突。
- `attachments`、`doc_refs` 原样复制原任务记录里的值，指向同一批源文件。
- `brief` 取 `store.task_brief(原 id)` 的值，原样传入，`None` 时也是 `None`；不 trim，不重新校验。
- `doc_root` 用当前配置重新解析，与 `create` 相同。

**第 3 步：取消原任务。** 先调 `stop_running_session(原 id)`，再调 `scheduler::apply_trigger(原 id, Cancel)`；
这里**不**调 `task_trigger`，所以中途不会 tick。`not_a_bug` 节点正常情况下没有会话和门禁在跑，但走同一条路径没有坏处，
也与 `task.cancel` 的清理完全一致：归档文档、删除 worktree 和分支、置归档标记（问题 7）。

**失败补偿（约束 4）。** 如果 `apply_trigger` 返回错误，先重新读原任务：

- 原任务还不是 `Cancelled`：说明状态没落库，调新增的 `store.discard_task(新 id)` 删掉新任务，再把原错误返回给调用方。
- 原任务已经是 `Cancelled`：说明错误出在 `perform` 之后的步骤（例如 `set_task_archived`）。这时取消已经生效，
  用 `tracing::warn` 记一笔，继续执行第 4 步。

`discard_task(id)` 在一个事务里删除 `task_briefs` 与 `tasks` 里属于该 id 的行。如果这个任务已经有事件或会话，它返回 `Conflict`
拒绝删除，所以只能用来撤回一个刚建、还没人看到过的任务。

**第 4 步：写事件，然后排队或 tick。** 事件的写入顺序固定（问题 3、5）：

1. `task.created`，挂在新任务下，载荷 `{project_id, converted_from: <原 id>}`。普通创建仍然只写 `{project_id}`，两种载荷共存。
2. `task.converted`，挂在原任务下，载荷 `{from: <原 id>, to: <新 id>}`。

然后看 `start`：

- `start: true`：调现有的 `task_start(新 id)`。它做 `Start` 转换（`draft` → `queued`）、`mark_queued`、写 `task.started`、tick，
  结果与 `task.create` 不带 `draft` 时一样，有空位就立即开工。
- 不传或 `start: false`：只调一次 `scheduler::tick`。这一下 tick 对应 `task_cancel` 原本就有的那一下，新任务是草稿，不会被它启动。

不论哪一种，tick 都发生在原任务取消**之后**。

**返回值（问题 6）。** 形状与 `task.create` 对齐，多一个字段：
`{task: <新任务 task_json>, started: bool, queue_position, converted_from: <原 id>}`。
Outcome 里附带的事件只用现有类型：`task.created`（新任务）与 `task.updated`（原任务），这样桌面端和网页端的刷新路径都不用改。

### 2. CLI（A-02、A-05，`crates/autome-cli`）

- `task()` 新增子命令 `convert`。`c.resolve_task(a.need(2, "任务")?)` 把参数解析成 id，然后调
  `task.convert {task_id, kind: "feature", start}`；只有带 `--start` 时才传 `start: true`。
- `run()` 里 `task` 的选项白名单加上 `--start`。
- `--json` 原样打印 RPC 返回值，与 `task new --json` 一样；「新任务」就是其中的 `.task`。
- 不带 `--json` 时打印三行：
  1. `<新 id>  <新 slug>  <state_line>`
  2. `由 <原 id> 改建，<原 id> 已取消。`
  3. 草稿时打印 `已存为待开始。用 \`autome task start <新 id>\` 开始。`；否则沿用 `task new` 的那一句。
- `USAGE` 在 `task start` 下加一行：`task convert <任务> [--start]  把判定为「不是 Bug」的任务改建成需求（原任务取消）`。
- `render.rs:113` 的提示改成：`autome task convert {id} [--start]    或    autome cancel {id}；确认是 Bug：autome reply {id} "理由"`。
- `README.md` 里「`autome help` lists the rest」那一句的列表补上「turn a not-a-bug task into a feature」（问题 10）。不改别的文档。

### 3. 网页网关（A-03，`apps/web/server/gates/write-gate.ts`）

为了与 T-37 冲突最少（约束 5），只做三处局部改动，T-37 的任何内容都不顺手做：

- `ALLOWED_WRITE_OPS`：在 `'task.cancel',` 之后插入 `'task.convert',`。
- `validateOpSpecific`：在 `task.decide` 分支之后单独加一个 `if (op === 'task.convert')` 分支，不碰 T-37 会改的 `task.create` 分支。规则如下：
  - `task_id` 必须是非空字符串，否则报「task_id 必须是非空字符串」；
  - `kind` 必须严格等于 `'feature'`，否则报「kind 只能是 feature」；
  - `start` 只能是 `undefined` 或布尔值，否则报「start 必须是布尔值」。
- `write-gate.test.ts`：在 `specific` 对象里加一行 `'task.convert': { task_id: 'T-1', kind: 'feature' }`，另外追加一个独立的 `test(...)`。

`validateShape` 的通用限制（字符串上限 512、只允许标量）对这三个字段已经够用，不新增 `PROSE_FIELDS` 或 `ARRAY_FIELDS`。
实现时如果 T-37 已经进了本分支的基线，就在它的基础上加，规则不变。

## 待定问题的答案

1. `request` 沿用原任务的 `request`，`title` 沿用原任务的 `title`；slug 去重后带后缀，原任务的 slug 继续占位；归档目录与新文档目录不冲突。见方案 1 第 2 步。
2. 附件复制的是源路径，并且要求源文件仍然存在；缺失就拒绝改建，并在错误信息里给出 worktree 副本的位置。A-04 断言两处：
   新任务记录里的源路径都是文件；`start: true` 开工后，新 worktree 的 `<doc_dir>/attachments/<文件名>` 存在，且字节与源文件相同。
3. 顺序是：预检 → 建新任务（草稿，不写事件）→ 取消原任务 → 写 `task.created`、`task.converted` → 排队或 tick。
   取消失败时删掉新任务；tick 在取消之后。
4. `kind` 必填，只接受 `"feature"`，错误文案见方案 1 第 1 步。
5. 两条事件的挂载位置与载荷见方案 1 第 4 步。网页端通知器会跳过未知事件（`notifier.ts:39`），不需要改。
6. 返回 `{task, started, queue_position, converted_from}`；CLI 的 `--json` 打印整个返回值，新任务是其中的 `.task`。非 `--json` 的输出见方案 2。
7. 完全复用 `task_cancel` 的两步，只是省掉中间那一下 tick。`not_a_bug` 正常情况下没有会话和门禁在跑，复用同一路径是为了清理行为与 `task.cancel` 一致。
8. 网关校验 `task_id` 是非空字符串、`kind === 'feature'`、`start` 是布尔值或不传；改动分三处局部插入，见方案 3。
9. 拒绝的情形：
   - 非 Bug 任务：`TransitionRejected`。
   - 不在 `not_a_bug` 的 Bug 任务（任何状态，包括已归档）：`TransitionRejected`，错误信息里带当前状态。
   - 任务不存在：`NotFound`。
   - 项目已移除：`TransitionRejected`。
   - 附件缺失：`TransitionRejected`。
   - 参数错误：`InvalidParams`。
10. `USAGE` 加一行，`render.rs` 改提示，`README.md` 列表补一项，见方案 2。

## 风险与验证安排

- **R1 崩溃窗口。** 如果进程恰好死在第 2 步提交之后、第 3 步落库之前，会留下一个没有 `task.created` 事件的草稿，原任务仍停在 `not_a_bug`。
  这时用户看到的是「多了一个草稿」，可以直接取消它，原任务也照样能再改建一次。这个窗口只有两次 SQLite 提交之间那一段，本任务不做崩溃恢复。
  先建草稿、取消成功后才排队，就是为了让这种残留不会自己开工。
- **R2 取消失败的补偿。** 这条路径没法靠真实故障稳定触发，所以 M-01 用 SQLite 触发器注入。做法是测试另开一个 `rusqlite::Connection`
  打开 `ctx.store.path()`，建一个 `BEFORE UPDATE ON tasks WHEN OLD.id = <原 id>` 的 `RAISE(ABORT)` 触发器，
  让 `set_task_state` 失败，再断言新任务已被撤回。`store.rs:3055` 已经用过同样的手法。
- **R3 与 T-37 的文本冲突。** 改动只有一行白名单、一个独立分支、`specific` 里一行和一个新测试。哪一方后合并，冲突都只在相邻行，rebase 时手工合并即可。
- **验证体系**：全部使用仓库已有的测试：dispatch 单测、`end_to_end.rs` 的 `World` 夹具（真实 Git 与 worktree，模型用替身）、
  `cli.rs`（真实二进制）和 vitest。宿主门禁 `sh scripts/ci-local.sh` 每个实现轮都会跑。
- **基线**：本设计轮在 `0024026` 上跑 `cargo test --workspace` 的结果是 11 个测试目标、1193 passed、0 failed、1 ignored；
  原始输出在 `.autome/output/plan1-baseline-cargo.txt`。
  `write-gate.test.ts` 在基线上有 35 个顶层 `test(...)`。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 内核 task.convert（预检、建新、取消、补偿、事件）与端到端用例 | 0 | convert-core |
| M-02 | 开放 | CLI autome task convert、帮助与 not_a_bug 提示 | 0 | convert-cli |
| M-03 | 开放 | 网页写网关放行 task.convert 与全量回归 | 0 | convert-web |

## 验收契约

M-01 覆盖 A-01、A-04，没有前置里程碑。M-02 覆盖 A-02、A-05，前置 M-01。M-03 覆盖 A-03、A-06、A-07，前置 M-01，并且要在 M-02 之后做全量回归。

### M-01 验收契约

- 验收命令：`cargo test -p automed task_convert && cargo test -p automed discard_task`
- 预期：`task_convert` 过滤共 8 个用例通过，`discard_task` 过滤 1 个通过。这 9 个都是新增用例，基线上两个过滤都是 0 个。

  `dispatch::tests` 下 7 个：
  1. `task_convert_rebuilds_a_not_a_bug_task_as_a_draft_feature`：
     - 新任务 `kind=feature`、状态 `draft`；
     - brief 逐字节相同（brief 含中文、尾随空格和 `{request}`）；
     - `request`、`title`、`attachments`、`doc_refs` 与原任务相同；
     - 原任务 `cancelled` 且已归档；
     - 两条事件的挂载与载荷正确，写入顺序为先 `task.created` 后 `task.converted`；
     - 返回值四个字段齐全；
     - `READ_METHODS` 不含 `task.convert`。
  2. `task_convert_with_start_queues_the_new_task`：新任务进入 `queued` 或 `active`，返回值 `started` 与 tick 的结果一致，并写了 `task.started`。
  3. `task_convert_rejects_every_state_other_than_not_a_bug`：覆盖 draft、queued、active/locate、active/await_info、paused、failed、cancelled、done 和已归档；
     每个都返回 `TransitionRejected`，错误信息含当前状态，任务数不变，原状态不变。
  4. `task_convert_rejects_a_feature_task`
  5. `task_convert_rejects_bad_params`：缺 `task_id`、`kind` 缺失、`kind` 为 `"bug"`、`start` 为 `"yes"` 时返回 `InvalidParams`；任务不存在时返回 `NotFound`。
  6. `task_convert_rejects_a_missing_attachment_without_touching_either_task`
  7. `task_convert_puts_back_the_new_task_when_cancelling_the_original_fails`：用 R2 的触发器注入失败，断言调用返回错误、没有新任务也没有它的 brief、
     原任务仍是 `not_a_bug`、没有写 `task.converted`。

  `end_to_end` 下 1 个：`bug_loop_not_a_bug_task_convert_rebuilds_it_as_a_feature_with_its_attachments`。
  - 用一个真实附件和一份 brief 建 Bug 任务。
  - 会话还在 `active/locate` 时调 `task.convert`，应被拒。
  - `settle` 到 `not_a_bug` 后，用 `start: true` 改建。新任务 `kind=feature`；`task.get` 返回的 brief 与原任务相同；
    记录里的附件路径存在；新 worktree 的 `<doc_dir>/attachments/<文件名>` 存在，且字节与源文件相同；原任务 `cancelled` 且已归档。
  - 对原任务再调一次 `task.convert`，应被拒。

  `store::tests` 下 1 个：`discard_task_removes_a_fresh_task_and_refuses_one_with_history`。

  现有的 `bug_loop_not_a_bug_waits_for_a_decision_and_can_be_cancelled_or_rebuilt_as_a_feature` 保留不动。
- 负向对照（任一种错误实现都会让验收变红）：

  | 错误实现 | 变红的用例 |
  |---|---|
  | 漏掉第 3 步取消 | 用例 1 和端到端（原任务不是 `cancelled`） |
  | 补偿里不调 `discard_task` | 用例 7（新任务残留） |
  | 只检查 `kind`、不检查节点 | 用例 3 的 await_info 一项通过了改建 |
  | 新任务的 brief 用 `trim` 后的值或重新拼的标题 | 用例 1 的逐字节比较 |
  | 没复制 `attachments` | 端到端 worktree 里没有附件 |
  | 把 `task.convert` 加进 `READ_METHODS` | 用例 1 |

### M-02 验收契约

- 验收命令：`cargo test -p autome-cli task_convert`
- 预期：3 个用例通过，都是新增的，基线上为 0 个。`cli.rs` 的集成测试从 13 个变成 15 个。

  `render::tests` 下 1 个：`task_convert_is_the_next_step_for_not_a_bug`。它断言 `next_step` 对 `active/not_a_bug` 的输出含 `autome task convert <id>`，
  不再含 `task new --kind feature --brief`。

  `tests/cli.rs` 下 2 个：
  1. `task_convert_rebuilds_a_not_a_bug_task_from_the_command_line`：
     - 用 `bug_wait_step` 把 Bug 跑到 `not_a_bug`；
     - `status <id>` 的输出含 `autome task convert`；
     - `task convert <slug 前缀> --json` 返回的 `.task.kind == "feature"`、`.task.state.state == "draft"`、`.converted_from == <原 id>`；
     - 原任务变成 `cancelled`；
     - `autome --help` 与 `autome help` 的输出都含 `task convert`；
     - 再次 convert 时退出码为 1，stderr 含「不是 Bug」。
  2. `task_convert_with_start_queues_and_rejects_other_tasks`：
     - `--start` 时新任务不是 `draft`；
     - 对一个需求草稿 convert，退出码为 1；
     - `task convert` 不带参数时退出码为 2。
- 负向对照：

  | 错误实现 | 变红的用例 |
  |---|---|
  | 提示没改 | render 用例与 cli 用例 1 |
  | `--start` 没加进白名单 | cli 用例 2（退出码 2「不认识 --start」） |
  | `USAGE` 漏写 | cli 用例 1 的帮助断言 |
  | `--json` 只打印 `.task` | `.converted_from` 断言失败 |

### M-03 验收契约

- 验收命令：`npm ci --prefix apps/web && npm test --prefix apps/web -- test/write-gate.test.ts && npm test --prefix apps/web && cargo test --workspace`
- 预期：
  - `write-gate.test.ts` 36 个通过（基线 35 + 新增 1 个 `task.convert` 用例）。新用例的正例有四组，必须全部通过：
    - `{task_id:'T-1', kind:'feature'}`
    - `start: true`
    - `start: false`
    - 带 512 字符的 `task_id`

    反例有八组，必须全部被拒：
    - 缺 `task_id`
    - `task_id` 为空串
    - `task_id` 为数字
    - 缺 `kind`
    - `kind` 为 `'bug'`
    - `start` 为 `'yes'`
    - `start` 为 `1`
    - `task_id` 为 513 字符
  - `npm test --prefix apps/web` 全绿，0 failed，通过数比改动前多 1（实现轮先在改动前跑一次，把数记进证据）。
  - `cargo test --workspace` 全绿：1205 passed、0 failed、1 ignored。这是设计轮基线 1193 加上 12 个新用例（M-01 的 9 个与 M-02 的 3 个）；
    如果其它任务先合并进基线，以「改动前基线 + 12」为准。
- 负向对照（任一种错误实现都会让验收变红）：

  | 错误实现 | 变红的用例 |
  |---|---|
  | 只在白名单加 op、不加 `validateOpSpecific` 分支 | 新用例的 `kind:'bug'`、`start:'yes'` 反例 |
  | 加了分支但没在 `specific` 登记 | 第 553 行的「全部 op 正路径可达」 |
  | 把 `task.convert` 写进读门禁 | `write-gate.test.ts:506` 的交叉检查 |

## 人工验收清单

- H-01 在一个真实项目里把 Bug 任务跑到「不是 Bug」，执行 `autome task convert <任务>` 后用 `autome status` 确认：新需求任务是待开始，原任务已取消，任务卡正文与原卡一致

## Backlog

## 争议项
