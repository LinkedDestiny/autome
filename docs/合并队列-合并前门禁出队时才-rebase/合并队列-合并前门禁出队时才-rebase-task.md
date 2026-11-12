# 任务：合并队列——合并前门禁出队时才 rebase，通过后在同一段临界区里合并

本任务的规则见 `docs/合并队列-合并前门禁出队时才-rebase/protocol/loop-protocol.md` 与
`docs/合并队列-合并前门禁出队时才-rebase/protocol/session-protocol.md`。

- 设计文档：`docs/合并队列-合并前门禁出队时才-rebase/合并队列-合并前门禁出队时才-rebase.md`
- 任务卡（用户写的，第一输入；与本文件不一致时以任务卡为准）：
  `docs/合并队列-合并前门禁出队时才-rebase/合并队列-合并前门禁出队时才-rebase-card.md`
- 任务编号：T-70；任务整理日期：2026-11-13
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree，没有其它独立仓库，状态块不写 `repos:`）

## 1. 需求原文

> 合并队列：合并前门禁出队时才 rebase，通过后在同一段临界区里合并，别让每次合并把其它任务的门禁结果作废

任务卡正文比这句话具体得多。下面 §2–§5 照任务卡整理，§6–§8 是调研得出的约束、现状与待设计回答的问题。

## 2. 背景

T-61 之后本仓库用 `[gate] run = "before_merge"`（完整门禁每个任务只在合并前跑一次），T-47 让宿主门禁同一时间
只跑 1 个（`[gate] concurrency` 缺省 1，FIFO 排队）。两者叠加出一个新的浪费：**合并前门禁的结果只对「跑的那一刻的
main」有效，而任何一个任务合并都会让 main 前进。**

2026-11-13 05:31 的现场（任务卡原文）：T-46 停在「等你合并」（门禁已过），T-43 的合并前门禁正在跑，T-49、T-44 的
合并前门禁在排队，四个分支都基于同一个 main。此时无论先合并谁，其余三个的结果（已出的、在跑的、在排队的）全部过期：

- 排队中的门禁仍会在旧基线上跑完（每次 5–11 分钟），通过后按合并 → 内核发现分支落后 → 重新排队 rebase → 再排一次门禁；
- 它重跑期间只要又有别的任务合并，再作废一次。

四个任务最坏要跑约 10 次合并前门禁，理想情况只需 4 次。操作者当时只能手工串行化：`autome stop` 掉排队的三个，
合并一个，再逐个 `autome rerun <任务> rebase`。任务一多这就是新的「门禁抢占」。

## 3. 目标（任务卡「要做什么」，共八条）

1. **出队时才 rebase。** 合并前门禁（需求 Loop 的 MergeGate、Bug Loop 的 MergeGate）在宿主任务队列里轮到自己时，
   先检查任务分支是否落后默认分支；落后就在这时 rebase（沿用现有 rebase 步骤与冲突处理：有冲突照旧退回实现 / 修复轮，
   并让出门禁位置），然后才在 rebase 后的提交上跑门禁。入队时不再提前 rebase，或入队时 rebase 了、出队时发现又落后就
   再 rebase 一次——**口径由设计轮定准**，要求是「门禁开始跑的那个提交一定基于当时最新的默认分支」。
2. **已授权合并的任务，门禁通过后立刻合并，中间不让别的合并插队。** 任务带着 `merge_requested`（用户已经按过
   `autome merge`、因分支落后被送回 rebase 的那种）走到合并前门禁时，「出队 rebase → 门禁 → 合并」作为**项目内**的一段
   临界区：这段时间里同一项目不开始别的合并，也不让另一个合并前门禁在它之前完成合并。门禁通过直接进 `Merging`，
   不会再报落后。
3. **预先授权合并。** `autome merge <任务>` 允许在任务到达合并卡点之前就按下（任务处于实现、审计、复盘、rebase、
   合并前门禁、排队等任何未完成且未失败的状态）：记下 `merge_requested`，任务走到合并卡点时不停，直接进合并。
   `autome status` / `task.get` 显示「已预先同意合并」；提供撤销（`autome merge <任务> --cancel` 或等价 RPC）。
   设计卡点不受影响：没批准设计的任务按 merge 只记授权、不跳过设计批准。
4. **项目级自动合并（可选开关）。** `.autome/config.toml` 新增 `[merge] auto = true`（缺省 false）：打开后每个任务
   通过合并前门禁就直接合并，等价于每个任务都预先授权。没配 `[gate]` 时打开要给一条配置警告（没有门禁把关的自动
   合并），不报错。README 写明取舍。
5. **停在合并卡点的任务，结果过期要看得见。** 任务在「等你合并」而默认分支已经前进时，`autome status` / `task.get` /
   `autome ls` 标明「门禁结果基于旧的 main（落后 N 个提交），合并时会重新 rebase 并再跑一次门禁」，让用户知道按下去
   不是立刻合并。
6. **队列里等合并的顺序**：按进入合并前门禁队列的先后（FIFO），与 T-47 的队列一致；T-53（排队任务调整顺序）如果
   已合并，沿用它的顺序口径，不另造。（整理时 T-53 **尚未**合并，见 §7.6。）
7. **事件与输出**：新增或复用事件说明「出队时 rebase 到 main @ sha」「门禁通过，直接合并（已授权）」；`autome run`
   打可读的一行。
8. **文档**：README Host gate 一节与 `docs/development/technical-design.md` 写明合并队列的行为、临界区范围、
   `[merge] auto` 的取舍。协议正文不需要改；如果设计认为要改，按 `protocol/CHANGELOG.md` 的规矩发新版本并带独立的
   `## protocol/vN` 标题。

## 4. 不做什么（任务卡原文，硬性范围边界）

- 不做「合并列车」（把几个任务叠在一起只跑一次门禁、失败再二分）——先把串行队列做对；留 Backlog。
- 不改 `[gate] concurrency` 的语义；并发大于 1 时临界区仍然保证同一项目的合并串行。
- 不改 MainCheck 的沿用规则（T-60）。
- 不改设计批准卡点。

调研补充的边界（同样不要做，不是新范围）：

- 不改本仓库的 `.autome/config.toml`，不改 `.autome/` 下任何内容（`AGENTS.md`）。`[merge] auto` 是否在本仓库打开，
  由用户合并后自行决定。
- 不改 `[merge] after`（T-43）与 `[setup]`（T-27）的语义，不改 `scripts/ci-local.sh`。
- 不改 `protocol/` 源文件，除非设计明确论证必须改（见目标 8）。
- `apps/desktop`、`apps/web` 的界面不在任务卡要求内（任务卡只点了 `autome status` / `task.get` / `autome ls` /
  `autome run`）。只有当新增的事件、字段或 RPC 让它们现有的穷举校验 / 白名单 / 测试变红时，才做让其不坏的最小改动。

## 5. 验收标准（逐字取自任务卡）

设计文档的里程碑必须逐条覆盖；用户在设计定稿时会对照它们批准或驳回。

1. e2e：三个任务同时走到合并前门禁、都已预先授权合并（或 `[merge] auto = true`）→ 三个都合并，门禁脚本恰好被调用
   3 次；每次调用时的 HEAD 都以当时的默认分支为祖先；没有任何一次「按合并时默认分支已前进」的回退。
2. e2e：任务 A 停在合并卡点（门禁已过），任务 B 合并使 main 前进 → `task.get` / `status` 显示 A 的结果已过期与落后
   提交数；对 A 按合并 → rebase + 门禁 1 次 → 合并。
3. e2e：预先授权 → 任务走完后不停在合并卡点直接合并；撤销授权后停在卡点；设计未批准时预先授权不跳过设计卡点。
4. e2e：出队 rebase 遇到冲突 → 退回实现轮（Bug 为修复轮），门禁位置让给下一个任务，冲突解决后重新排队；授权保留。
5. e2e：临界区内（A 正在「门禁 → 合并」）对 B 按合并，B 不会在 A 之前完成合并；A 门禁失败时临界区释放，B 继续。
6. 单测：`[merge] auto` 的解析、缺省、无 `[gate]` 时的警告；`save_project` 不改写用户的 `[merge]`（与 T-43 的
   `[merge] after` 共存，键不冲突）。
7. 驱动重启后队列顺序、授权与临界区状态从账本恢复，不丢、不死锁（有测试）。
8. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿，前两条由实现轮在会话里跑并记录退出码。

补充说明（从任务卡和调研得出，不是新范围）：

- 「e2e」指 `crates/automed/tests/end_to_end.rs` 这一套：真实 Git 仓库、真实 worktree、真实包装脚本、真实门禁后台
  作业，只把模型 CLI 换成替身。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
- 第 1 条「门禁脚本恰好被调用 3 次」要用不依赖时序猜测的证据断言（门禁脚本自己记下的调用记录、账本里
  `node = merge_gate` 的 `gate.started` / `gate.result`）。合并后的 MainCheck 也会调用同一条门禁命令——它不在本任务
  范围内、沿用规则不变（T-60：合并提交的树与通过 MergeGate 的树相同时写 `gate.reused` 不再跑）；设计须说明「恰好
  3 次」怎样与 MainCheck 区分，**不能靠关掉 MainCheck 来凑数**。「每次调用时的 HEAD 以当时的默认分支为祖先」须由
  门禁脚本在被调用的那一刻自己记录（HEAD 与默认分支的 sha），事后用 `git merge-base --is-ancestor` 之类断言。
- 第 1 条「没有回退」对应现有事件：授权被分支落后触发时写的 `merge.auto {active: true, repos, detail}`（渲染为
  「按合并时默认分支已前进（…），自动 rebase 后再合并」）以及 `Merging` 得到 `CoreStepResult::Behind`。该场景里这条
  路径出现次数必须为 0。
- 第 2 条的「对 A 按合并 → rebase + 门禁 1 次 → 合并」就是今天 T-10 的自动路径；本任务要保证的是它在队列 / 临界区
  规则下仍然只跑 1 次门禁，并且之前的「过期」显示正确。
- 第 4 条的「授权保留」与现有行为相反：今天冲突或门禁失败退回实现轮时授权被丢弃，并有单测与 e2e 明确断言这一点
  （清单见 §7.2）。设计须逐个说明这些既有测试怎样改，不能悄悄删。
- 第 7 条需要「真的重启」的测试：现有样板是 `gate_queue_order_survives_a_core_restart`（e2e）与
  `gate_queue_project_limit_preserves_fifo_and_restart_order`（`scheduler_gate_queue_tests.rs`）。
- 第 8 条：`sh scripts/ci-local.sh` 是本项目的 `[gate] command`，本仓库是 `run = "before_merge"`，由 Autome 在合并前
  门禁里跑；会话里不要自己跑全量门禁，只跑 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`
  与里程碑直接相关的测试，并记录退出码。

## 6. 硬性约束

- **分层。** 纯类型、状态转换与校验放 `crates/autome-domain`（无 I/O、无 async、无 SQLite）；文件、进程、git、
  SQLite、事件放 `crates/automed`；`crates/autome-cli` 只解析参数与打印。
- **内核独占节点转换。** 所有状态变化经 `scheduler::advance`（先落库新状态，再执行动作），转换表在
  `task.rs::apply_inner` 与 `bug_loop.rs::apply_for`；不要在调度器里绕过转换表直接改任务状态。
- **账本是唯一事实来源。** 队列顺序、授权、临界区占用都必须能从账本（事件 / 表）和磁盘上的作业文件恢复；不得只存在
  进程内存里（验收 7）。现有做法：授权由最后一条 `merge.auto` 事件的 `active` 决定；门禁排队由最后一条队列相关事件
  是否为 `gate.queued` 决定，顺序取事件 `seq`。
- **不死锁、不永久占位。** 临界区的持有者被暂停、停止、取消、失败、门禁熔断、准备命令失败、作业消失
  （`Poll::Vanished`）、配置中途被改、驱动重启时，都必须有明确的释放或接续规则。
- **缺省行为的兼容性要说清楚。** `[merge] auto` 缺省 false；没有任何任务被预先授权、也没有并发合并时，用户看到的
  流程应与今天一致（rebase → 合并前门禁 → 等你合并 → 按合并）。若「出队时 rebase」让某些事件的先后或数量改变，
  设计须列出受影响的现有测试并逐个说明。
- **测试替身对齐真实接口**：e2e 用真实 git、真实 wrapper、真实 `sh -c`，只替换模型 CLI。
- **`.autome/` 不可写**；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下其它目录、不推送；超过 5 行的命令
  输出与临时验证产物写入 `.autome/output/`，不进任务文档目录。
- `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings` 必须干净。

## 7. 代码现状（2026-11-13 调研；行号为「约」，以符号名为准）

### 7.1 合并尾段的状态机（需求 Loop，`crates/autome-domain/src/task.rs`）

节点顺序（`Node` 枚举）：… → `Retro` → `Rebase` → `MergeGate` → `AwaitMerge` → `Merging` → `MainCheck` → `Cleanup`。
`Rebase`、`Merging`、`Cleanup` 与各门禁节点是内核步骤（`is_core_step`），由 tick 里的 `run_core_step` 执行。

- `after_the_loop`：实现循环收尾 → `Retro`（启用时）→ `Rebase`。
- `(Rebase, CoreStepResult::Ok)`：配了 `[gate]` → `MergeGate`；没配 → 授权时 `press_merge`，否则 `AwaitMerge`。
- `(Rebase, Conflict { files })` → `Implement`，注入 `Inject::RebaseConflict`。
- `(MergeGate, GateDone Passed)`：`merge_requested` 为真 → `press_merge`；否则 `after_gate` → `AwaitMerge`。
  `(MergeGate, Failed)` → `Implement`，注入 `Inject::GateFailed`。
- `(AwaitMerge, Trigger::Merge)` → `press_merge`：有「纳入」表态时回实现轮（`start_impl`），否则 → `Merging`。
  **`Trigger::Merge` 只在 `AwaitMerge` 被接受**，其它状态落到 `Err("当前状态不接受该操作")`。
- `(Merging, Behind { repos, detail })` → `TaskState::Requeued { resume: Rebase }`（T-10）；`(Merging, Ok)` →
  `MainCheck`（配了门禁）或 `Cleanup`；`(Merging, Blocked)` → 回 `AwaitMerge`。
- 授权的携带：`apply_with_merge_request(state, trigger, ctx, merge_requested)`。`in_merge_stretch` 只认
  `Rebase | MergeGate`（Active / Requeued / Paused）；`Transition.merge_requested` 仅在「`Merging` 报 `Behind`」或
  「已授权且下一状态仍在合并尾段」时为真。也就是说**今天的授权只活在 rebase → 合并前门禁这一小段里**，离开即失效。
- `TaskState::occupies_slot` / `Node::occupies_slot`：`Rebase`、`Merging`、门禁节点等占项目并行槽位；`AwaitMerge`、
  `Paused`、`Stopped`、`Failed`、`Requeued` 不占。

### 7.2 明确断言「授权被丢弃 / 不可预先授权」的既有测试（本任务会与之冲突）

`task.rs` 单测：`a_conflicting_requested_rebase_goes_back_to_implement_and_drops_the_request`、
`a_failed_merge_gate_on_a_requested_merge_goes_back_to_implement_and_drops_the_request`、
`the_request_survives_the_queue_and_a_pause_but_not_a_stop`、
`a_rebase_entered_from_anywhere_but_the_merge_carries_no_request`、
`without_a_request_rebase_and_merge_gate_still_stop_at_await_merge`、`behind_is_only_accepted_from_merging`。
`scheduler.rs` 单测：`a_conflicting_requested_rebase_goes_back_to_implement_and_clears_the_request`、
`a_rebase_nobody_asked_to_merge_records_no_merge_rebased`、
`with_every_slot_taken_the_requested_rebase_waits_in_the_queue_and_keeps_the_request`、
`a_requested_rebase_records_merge_rebased_and_merges_without_stopping`、
`a_merge_behind_main_is_requeued_for_a_rebase_and_main_is_untouched`。
e2e：`a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again`（断言门禁失败后
**再次停在** `await_merge`）、`two_tasks_waiting_to_merge_both_land_and_the_second_rebases_through_the_gate_by_itself`、
`main_moving_after_the_press_takes_the_same_automatic_path`。

### 7.3 Bug Loop（`crates/autome-domain/src/bug_loop.rs::apply_for`）

- 复核通过 → `Rebase`；`(Rebase, Ok)` → **总是** `MergeGate`（Bug 的合并前门禁不依赖 `[gate]`，命令是
  `( 复现命令 ) && ( 门禁命令 )`，没配项目门禁时只有复现命令），并把 `merge_requested` 原样带过去。
- `(Rebase, Conflict)` → 修复轮（`start_fix`，注入 `RebaseConflict`）；`(MergeGate, Failed)` → 修复轮（注入
  `GateFailed`）；`(MergeGate, Passed)` 与合并、落后等其余情况落回需求 Loop 的 `apply_with_merge_request`（合并尾段
  共用，见测试 `bug_loop_merge_tail_is_shared_with_the_feature_loop`）。
- Bug Loop 另有等人节点 `AwaitInfo`、`NotABug`。

### 7.4 调度器（`crates/automed/src/scheduler.rs`）

- `tick_locked` 的顺序：`gate_queue::reclaim` → 首次 tick `recover` → 收已结束的会话 → 启动到点的重试 →
  对每个 `Active` 且 `is_core_step` 的任务跑 `run_core_step`（按 `list_unfinished` 的 `created_at` 顺序）→
  `fill_slots_reporting_errors`（各项目按 `[loop] parallel` 给 `Queued` / `Requeued` 任务发 `SlotAvailable`）。
- `merge_requested(ctx, task_id)`（约 1623 行）：最后一条 `merge.auto` 事件的 `active == true`。`advance`（约 1637 行）
  在转换后维护它：原来有授权而转换不再携带 → 追加 `merge.auto {active: false}`；原来没有而 `Merging` 报 `Behind` →
  追加 `merge.auto {active: true, repos, detail}`。**注意事件名 `merge.auto` 与任务卡要新增的配置键 `[merge] auto`
  同名不同物**（前者是「单个任务的合并授权」的账本记录）。
- `run_core_step`（约 3076 行）：`Rebase` → `rebase_all(&layout)`；`Merging` → `merge_all`；门禁节点 → `run_gate`；
  `Cleanup` 前置主检出 `[setup]` 与 `[merge] after`。`Rebase` 成功且已授权时写
  `merge.rebased {repos: [{repo, onto, head}]}`。
- `rebase_all`（约 3708 行）：对 `layout.repos` 每个检出 `git::rebase(worktree, base)`，任一成员冲突 →
  `Conflict { files（工作区带成员名前缀）, detail }`。
- `merge_all`（约 3751 行）：非文档仓库在前、文档仓库最后；先对每个未并入的仓库 `git::check_merge`，**只有纯落后**
  （`BlockKind::Behind`，无其它阻塞）才返回 `CoreStepResult::Behind`；否则逐仓库 `git::try_merge`（`--no-ff`，消息
  `merge(autome): <id> <title>`），写 `merge.repo` 事件，全部成功后 `complete_task` 记合并提交。主工作树有冲突的
  未提交改动（`DirtyConflict`）或不在默认分支（`NotOnDefault`）→ `Blocked` → 回 `AwaitMerge`。
- `run_gate`（约 3256 行）：选门禁命令（Bug 节点现场拼）→ 取 `(cwd, head)`（MergeGate 为任务 worktree 与
  `gate_head(layout)`；工作区取各成员 head 的 sha256）→ `gate::discard_stale` → 若作业不存在：
  1. MergeGate 沿用：最后一条 `gate.result` 通过且 `sha == head`（Bug 还要求命令相同）→ 写 `gate.reused`，直接通过；
  2. `[setup]`：`setup::ensure_with_hold`（任务检出的准备命令与随后的门禁可连占同一名额）；
  3. 准入：`gate_queue::admit(ctx, task, project, node, "gate", None)`，不放行就写一次 `gate.queued` 并返回（任务
     留在原节点、继续占项目槽位）；
  4. `gate::start(&job, &cwd, &gate.command, env)`（环境变量 `AUTOME_TASK`、`AUTOME_HEAD`、`AUTOME_GATE_NODE`），
     写 `gate.started {node, sha, command, log}`。
  作业结束后：head 已变 → 作废重跑；写 `gate.result`（含熔断指纹 / 计数）；熔断 → `Trigger::GateSameFailure`；
  否则 `finish_gate` → `Trigger::GateDone`。**今天「轮到自己」的那一刻就是第 3 步 `admit` 返回 true 的那一刻；
  目标 1 的出队 rebase 要插在它与第 4 步之间或与之等价的位置，并处理 rebase 改变 head 后与第 1、2 步的关系。**
- `recover`（约 4059 行）：等人节点与内核步骤节点重启后原样保留，下一个 tick 重跑（要求幂等）。

### 7.5 宿主作业队列（T-47）

- 纯逻辑 `crates/autome-domain/src/gate_queue.rs`：`Waiter { task_id, seq, limit }`、`position`、
  `admission(held, waiters, task_id, limit)`（只有队首且 `held < limit` 才 `Go`）、`queued_after_event`。
- I/O `crates/automed/src/gate_queue.rs`：`waiters`（所有未完成任务里最后一条队列事件是 `gate.queued` 的，按 `seq`
  排序）、`held`（扫各任务 `.autome/output/gates/<task>/*.started` 作业文件判断占用）、`admit`、`continuation`、
  `reclaim`、`drop_on_error`。使排队失效的事件：`gate.started`、`setup.started`、`session.started`、
  `session.deferred`、`task.updated`、`gate.queue_dropped`。
- **队列是「一个账本内跨项目」的**（Gate / MergeGate / MainCheck / ReproCheck / FixGate 与准备命令共用名额，严格
  FIFO、无优先级）；而任务卡要的临界区是**项目内**的。两者范围不同，设计须分别说清。
- 读者：`task.get` / `dashboard.get` 的 `gate_queue_position`；CLI「排队等门禁（前面还有 k 个）」
  （`render::gate_queue_line`）。测试：`crates/automed/src/scheduler_gate_queue_tests.rs`（被 `scheduler::tests`
  include）与 e2e 的 `gate_queue_*` 一组（`queued_gate_world`、`queue_task`、`queue_tick`、`queue_wait`、
  `queue_release`、`queue_snapshot`）。

### 7.6 相关任务的合并状态（以 `main` 为准）

- 已合并：T-10（按合并时落后则自动 rebase → 门禁 → 合并）、T-43（`[merge] after`）、T-47（门禁排队）、T-60
  （MainCheck 沿用）、T-61（`before_merge`）。
- **未合并**：T-51（approve / merge / rerun 绕过并行上限）、T-53（排队任务调整顺序）、T-44。设计轮与每个实现轮
  开头应重新核对（`git log --oneline main | grep "merge(autome): T-5[13] "`）；T-53 若在本任务期间合并，按任务卡
  沿用它的顺序口径。T-51 未合并意味着今天 `Requeued { resume: Rebase }` 的任务要等项目并行槽位。
- 本任务分支基于 `ce6ec5f`（T-43 合并）；整理时 `main` 已前进到 `47a6580`（T-49 合并，领先 30 个提交）。会话不要
  自己 rebase——那是内核的步骤。

### 7.7 Git 层（`crates/automed/src/git.rs`）

`rebase(worktree, onto) -> RebaseOutcome::{Clean, Conflict { files, detail }}`、`is_ancestor(repo, ancestor, descendant)`、
`check_merge -> MergeCheck::{Ready, Blocked { kind: BlockKind::{DirtyConflict, NotOnDefault, Behind, MergeFailed}, detail }}`、
`try_merge -> MergeAttempt`、`head_sha`、`tree_id`、`second_parent`、`change_summary`、`commit_subjects`。
目前**没有**「分支落后默认分支多少个提交」的函数（目标 5 的 N 需要新增，例如 `rev-list --count`）。

### 7.8 配置（`crates/autome-domain/src/config.rs`、`crates/automed/src/config_io.rs`）

- `ProjectConfig` / `ResolvedConfig` 已有 `merge_after: Option<MergeAfterConfig>`、`gate`、`gate_run: GateRun`、
  `gate_concurrency`、`gate_reuse_for_main_check`、`gate_same_failure_limit`、`setup`；`resolve(global, project)` 合并。
- `parse_project`（约 244 行）里 `[merge]` 的解析（约 318 行）：不是表 → `"[merge] 不是一个表"`；`after` 非字符串 →
  报错；`after` 去空白后为空 → 等于没配；`after_timeout_minutes` 须为正整数。**今天 `[merge]` 里不认识的键被忽略**
  （测试用 `unknown = true` 验证）。`save_project`（约 514 行）用 `toml_edit`，不认识的表与键原样保留；样板测试
  `merge_after_section_survives_save_project`、`a_gate_and_a_retry_policy_are_read_from_the_project_file`。
- **仓库里没有现成的「项目配置警告」机制**：`ConfigViolation` 是保存前的硬校验（会拒绝保存），解析错误是硬错误，
  `project_alerts`（`store.raise_alert`）是运行期报警。任务卡第 4 条的「配置警告，不报错」落在哪、谁看得见，须由设计定。

### 7.9 RPC 与命令行

- `crates/automed/src/dispatch.rs`：`"task.merge" => task_trigger(ctx, task_id, Trigger::Merge)`（约 422 行）；
  `task_get`（约 1614 行）返回 `task`（`task_json`）、`gate_queue_position`、`queue_position`、`status_block`、
  `gate`（最后一条 `gate.result`）等；`task_changes`（约 2150 行）已按仓库算 `behind`（布尔）与 `rebase_first`；
  `dashboard_get` 供 `autome status`（不带任务）使用。
- 写方法白名单有**两份**：`apps/web/server/gates/write-gate.ts` 与 `apps/desktop/src/write-gate.js`（都已含
  `task.merge`）。新增 RPC 方法名而界面要用时两处都要加；只给 `task.merge` 加参数则不涉及。
- `crates/autome-cli/src/main.rs`：`"merge" => trigger(&mut c, &a, "task.merge", json!({}))`（约 178 行）；帮助文本
  `merge <任务>  合并到默认分支`（约 68 行）；`list`（`autome ls`，约 530 行，每行 `id slug 项目 state_line`）；
  `status`（约 555 行）与 `task_detail`（约 659 行，「状态 / 项目 / 门禁排队 / 需求 / 轮次 / 门禁」各行）。参数解析在
  `args.rs`。
- `crates/autome-cli/src/render.rs::event_line`：已有 `merge.auto`（「按合并时默认分支已前进（…），自动 rebase 后再
  合并」）、`merge.rebased`（「已自动 rebase 到 main @ sha」，T-23 修过单仓库缺仓库名的问题）、`gate.queued` /
  `gate.started` / `gate.result` / `gate.reused` / `gate.skipped`、`merge.after*`。`autome run` 逐事件打印这些行，
  `--json` 输出原始事件（T-46）。

### 7.10 权威文档的现状文字

- `README.md`「Host gate, retries and cooldowns」（约 107 行起）：配置示例块（含 `[gate]` 与 `[merge] after`）、
  「Host job queue」「The gate」（含 `before_merge` 一段）「Preparation」「After merge」各条；「The command line」
  一节有 `autome merge cart` 示例。
- `docs/development/technical-design.md`：§3.3 配置文件示例；§5.3 转换表（`Rebase` / `AwaitMerge` / `Merging` 各行，
  其中 `Rebase 无冲突 → AwaitMerge` 一行未写 MergeGate）；§5.6 Bug Loop；§6 Git 工作区操作；§8 调度器与 §8.1
  「宿主门禁」「合并前门禁模式」「合并后命令」各段；§14 IPC 接口。

## 8. 设计需要回答的开放问题（不预设答案）

1. **rebase 的口径。** 保留独立的 `Rebase` 节点（入队前先 rebase 一次、出队时落后再 rebase），还是把 rebase 收进
   MergeGate 的出队动作？出队 rebase 在 `run_gate` 里相对于「沿用判断（`gate.reused`）→ `[setup]` 准备 → `admit` →
   `gate::start`」的位置；rebase 改了 head 之后准备指纹与沿用判断是否要重做；怎样保证从 rebase 到 `gate::start` 之间
   默认分支不被**本项目的合并**推进。
2. **临界区的范围与实现。** 精确的起点与终点（出队 rebase 开始 → `Merging` 结束？是否含 MainCheck / Cleanup /
   `[merge] after`——任务卡写的是「出队 rebase → 门禁 → 合并」）。未授权的任务通过门禁后停在 `AwaitMerge`，它是否
   持有临界区（不持有才不会无限期卡住别人）。临界区内其它任务按合并时的行为：拒绝并提示、还是记下授权排队等待。
   `[gate] concurrency > 1` 时两个合并前门禁可以同时跑，怎样仍保证合并串行且后合并的那个不作废（任务卡：「不让另一个
   合并前门禁在它之前完成合并」）。占用状态存在哪（事件推导 / 新表），怎样从账本恢复。
3. **临界区的释放。** 持有者门禁失败、熔断、rebase 冲突、被暂停 / 停止 / 取消、准备命令失败、作业消失、
   `Merging` 得到 `Blocked`（主工作树脏 / 不在默认分支）或 `Failed`、`[gate]` 配置中途被拿掉、驱动重启——各自怎么
   释放或接续，保证不死锁（验收 5、7）。
4. **默认分支被本项目合并之外的东西推进**（用户自己在 main 上提交）。临界区管不住它；此时 `Merging` 仍可能报
   `Behind`，是否保留 T-10 的回退作为兜底，以及这与验收 1「没有回退」如何并存（验收 1 的场景里没有外部提交）。
5. **授权的生命周期。** 预先授权在哪些状态可以记（任务卡：任何未完成且未失败的状态，含 `Draft` / `Queued` /
   `Paused` / `Stopped` 吗）；哪些事件清除它（用户撤销、取消、失败、停止？）；rebase 冲突退回实现 / 修复轮后保留
   （验收 4 明确要求）；合并前门禁失败退回实现轮后保留还是丢弃（任务卡没有明说，现有 e2e 断言丢弃并再次停在
   `await_merge`）。存储是否继续用 `merge.auto` 事件，以及与配置键 `[merge] auto` 的命名区分。
6. **预先授权与其它卡点 / 分支的关系。** 设计批准卡点不跳过（任务卡明确）。Bug Loop 的 `AwaitInfo`、`NotABug`
   是否同样不跳过。`press_merge` 遇到「纳入」表态回实现轮（T-09）时授权是否保留。没配 `[gate]` 的需求任务没有
   MergeGate：预先授权 / `[merge] auto` 下是 rebase 后直接合并，临界区与「出队」在这条路径上怎么定义。
7. **`[merge] auto`。** 类型与缺省、是否只在项目配置里认（全局配置认不认）、与 `after` / `after_timeout_minutes`
   同表共存的解析与 `save_project` 保留；「无 `[gate]` 的配置警告」的载体（见 §7.8：没有现成机制）与触发时机、是否
   去重；单个任务能否在 `auto = true` 的项目里退出自动合并（撤销授权是否对它有效）；运行中切换开关对已在途任务的影响。
8. **过期显示（目标 5）。** 「落后 N 个提交」的计算位置（读路径上现算 git，还是 tick 时记账）与工作区项目（多个
   成员仓库）下 N 的口径；`task.get` 新字段的名字与形状、`dashboard.get` 是否同步带出；`autome ls` / `status` /
   `task_detail` 的文案；没配 `[gate]` 时只有「落后」没有「门禁结果」的说法；读路径调用 git 的开销与失败时的降级。
9. **队列顺序（目标 6）。** 「进入合并前门禁队列的先后」用哪个序号：今天任务只有在名额不足时才写 `gate.queued`，
   名额空闲时直接开始、没有入队事件。授权任务与未授权任务、MergeGate 与同队列里的 MainCheck / FixGate / 准备命令
   之间的先后是否受临界区影响（任务卡要求不改 `concurrency` 语义、与 T-47 的 FIFO 一致）。冲突退回后「重新排队」
   排在队尾。
10. **事件与输出（目标 7）。** 复用 `merge.rebased`（今天只在已授权时写）还是新增事件；「门禁通过，直接合并（已
    授权）」「已预先同意合并」「撤销授权」各用什么事件、payload 与 `autome run` 的一行文案；新事件是否影响
    `gate_queue` 的 `QUEUE_EVENTS`、指标（`task_metrics.rs`）、`apps/web` 通知器或桌面端的事件穷举。
11. **RPC 形状。** 撤销用 `task.merge` 加参数还是新方法（新方法要进两份写白名单才能被界面调用）；预先授权时
    `task.merge` 的返回与提示；`Trigger::Merge` 在非 `AwaitMerge` 状态的语义放在 domain 转换表里还是调度器 /
    派发层（注意「内核独占节点转换」与「账本是事实来源」两条约束）。
12. **工作区项目。** 多成员仓库下「落后」「rebase 到 main @ sha」「临界区」的口径（`rebase_all` 是逐成员的，
    `merge_all` 不是原子的，合并留一半时再按一次合并的既有行为要保持）。
13. **文档与协议。** README 与 technical-design 要改的具体小节（§7.10）；§5.3 转换表是否顺带补上 MergeGate；
    协议正文是否确实不用改（任务卡的判断是不用）。
14. **既有测试的处置。** §7.2 列出的每个测试是保留、改写还是替换，逐个给出理由。

## 9. 项目背景摘要（2026-11-13 整理时；任务分支 HEAD `babb7c7`，基于 `main` 的 `ce6ec5f`）

**仓库。** 单个仓库 `autome-v2`。Autome 2.0：一个 macOS 桌面应用加命令行（另有 Web 面板），把一句话需求在专用
worktree 里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 →（门禁）→ 审计 → 复盘 → rebase → 合并前
门禁 → 等你合并 → 合并 → 验证 main → 清理）推进成合并提交。Bug 任务走另一条 Loop（定位 → 复现检查 → 修复 → 修复
门禁 → 复核 → rebase → 合并前门禁 → …）。**内核独占节点转换**：会话只写文档与代码，下一个节点由 `automed` 读状态块
决定。本仓库正在用自己的 Loop 开发自己（launchd 代理跑已安装的 `autome` 二进制）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`；依赖 serde / serde_json / rusqlite /
  toml / toml_edit / sha2 / tracing 等）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O：`task.rs`（需求状态机）、`bug_loop.rs`（Bug 状态机）、
    `config.rs`（配置类型、合并与校验）、`gate_queue.rs`（FIFO 准入的纯逻辑）、`gate_failure.rs`（熔断指纹）、
    `merge_after.rs`、`setup.rs`、`status_block.rs`、`project.rs` 等。
  - `crates/automed` — 一切对外部世界的操作：`scheduler.rs`（tick、`advance`、`run_core_step`、`run_gate`、
    `rebase_all`、`merge_all`、`recover`）、`gate_queue.rs`（排队与占用）、`gate.rs`（后台作业 `Job`）、`git.rs`、
    `store.rs`（SQLite 账本、事件、报警）、`config_io.rs`（配置读写）、`dispatch.rs`（JSON-RPC）、`setup.rs`、
    `merge_after.rs`、`layout.rs`（`TaskLayout`，单仓库与工作区）、`brief.rs`、`launcher.rs`。测试在
    `crates/automed/tests/`（`end_to_end.rs`、`prompt_rendering.rs`、`real_cli.rs`、`golden/`）与各源文件的
    `#[cfg(test)]`（`scheduler_gate_queue_tests.rs` 被 `scheduler::tests` include）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析（`args.rs`）与输出
    （`main.rs`、`render.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`）；`apps/web` — React Router + Node 网关的面板（vitest、eslint、
  tsc）。各有一份写方法白名单。本任务预计不动（见 §4）。
- `protocol/` — Loop 协议源（编译进 `automed`，当前 `protocol/v4`）；`plugins/` — Claude 插件；`scripts/` —
  `ci-local.sh`、`install-cli.sh`、打包脚本。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 是 ADR；
  `docs/plans/` 是按日期的决策记录；`docs/<任务目录>/` 是各任务的文档。

**测试命令。**

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings      # 警告即错误
cargo test -p autome-domain                    # 状态机、配置、队列纯逻辑单测
cargo test -p automed --lib                    # 调度器、配置读写、门禁队列等单测
cargo test -p automed --test end_to_end        # 端到端（可加测试名过滤）
cargo test -p automed --test prompt_rendering  # 金样
cargo test --workspace --all-targets
npm test --prefix apps/desktop
npm run lint|typecheck|test|build --prefix apps/web
sh scripts/ci-local.sh                         # 以上全部，即本项目的 [gate] command
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记，只把模型换成替身；测试数据库由
`AUTOMED_DB_PATH` 各自隔离。常用工具：`World::new` / `w.configure(toml)` / `w.settle()` / `w.call(method, params)` /
`w.node(id)` / `w.state(id)`、`write_gate_script`、`wait_for_automatic_merge`、`gate_trace`、`assert_gate_calls`、
`queued_gate_world` 一组。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/` 下的任何内容；只在当前 worktree 内工作，不切分支、不操作 `.worktree/` 下的
  其它目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进
  `.autome/output/`，不要进任务文档目录。
- `.autome/rules/`：只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`（只读）：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`、
  `run = "before_merge"`、`timeout_minutes = 30`；`[retry] attempts = 2`、`backoff_seconds = 60`。即本任务自己的开发
  过程：实现轮 / 审计轮只跑里程碑验收与直接相关的测试，完整门禁由宿主在合并前跑一次。

**参考现场（同类任务的完整文档，可只读参照）。**

- T-10 自动 rebase：`docs/用户按合并时如果默认分支已经前进-任务自动/`
- T-47 门禁排队：`docs/宿主门禁按机器排队-gate/`
- T-61 `before_merge`：`docs/门禁只在合并前跑一次-gate-run/`
- T-60 MainCheck 沿用：`docs/合并提交的树与合并前门禁通过的树相同时跳过验证/`
- T-43 `[merge] after`：`docs/合并后钩子-merge-after/`
