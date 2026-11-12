# 宿主门禁按机器排队：`[gate] concurrency` 缺省 1，排队不计预算、超时从启动算

status: 已完成
design-round: 2/15
implementation-round: 7/30
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

任务文件：`docs/宿主门禁按机器排队-gate/宿主门禁按机器排队-gate-task.md`
任务卡：`docs/宿主门禁按机器排队-gate/宿主门禁按机器排队-gate-card.md`

文中「约束 n」「验收 n」「开放问题 n」分别指任务文件 §5、§3、§6 的编号。行号以基线 `ed2460d` 为准。

## 背景与现状

宿主作业（门禁与准备命令）都是 `gate::start` 起的后台作业，文件在 `<repo>/.autome/output/gates/<task>/<node>-<key>.{started,pid,exit,log}`。
`.started` 由 `gate::start` 写入当前秒数，`gate::poll_as` 用「现在 − `.started`」判超时；超时则杀进程组、在日志末尾追加
`=== Autome：{what}运行超过 N 分钟，已终止 ===`、写退出码 124。作业之间没有任何互斥，只有主检出准备命令（`setup-main`）
有一处「别的任务的 `setup-main` 存活就返回 `Pending`」。

启动宿主作业的地方一共三处，都经过两个函数：

- `scheduler::run_gate`（`run_core_step` 对五种门禁节点的分派）：在 `!job.exists()` 分支里依次做「MergeGate 同 sha 沿用
  （`gate.reused`）→ 门禁前准备（`setup::ensure`，worktree 或 verify 检出）→ `gate::start` → 写 `gate.started`」。
- `scheduler::start_session`：会话节点在起会话之前对任务检出调 `setup::ensure`；`Pending` 时
  `schedule_retry(…, "等待准备命令", counted = false)`，下个 tick 由 `start_due_retries` 再来。
- `scheduler::run_core_step` 的 `Cleanup`：对主检出调 `setup::ensure`（`setup-main`）。

`setup::ensure` 先收在途作业（收完删掉 `.started/.pid/.exit` 与 `.running` 标记），再按指纹决定是否要跑。门禁作业收完
之后 `.started` 保留（同一提交不重复跑），结果写进 `gate.result`。

2026-11-12 的实测（任务卡）：三个门禁并行，Rust `Test` 步骤从 51 秒变成 1192 秒，撞上 30 分钟超时被杀（124），
白白消耗一个修复轮次并留下一条 `main_gate_failed` 警报。

## 目标与非目标

目标：

- G1 一个 Autome 账本内，同一时刻最多运行 N 个宿主作业（门禁五种节点 + 准备命令三种检出），N 由 `[gate] concurrency`
  决定，缺省 1。
- G2 超出名额的任务停在原节点排队：不启动作业、不写 `gate.started` / `setup.started`、不产生作业文件、不动任何预算；
  轮到时才 `gate::start`，超时时钟从那一刻算。
- G3 排队顺序先到先得，可由账本事件与作业文件恢复，不加表、不改 schema。
- G4 `task.get`、`dashboard.get`、`autome status`、`autome run` 能看到「排队等门禁（前面还有 k 个）」。
- G5 内核杀掉的门禁超时，在注入文案、警报、简报与 CLI 里明确写「门禁超时」。

非目标（与任务文件 §4 一致）：不改 `ci-local.sh`、会话沙箱、`timeout_minutes` 的取值；不限制会话自己跑的 `cargo test`；
不加表；不改 `protocol/`、`plugins/`、`.autome/`；不做桌面端 / Web 面板的新展示；不重构无关代码；不给 MainCheck /
MergeGate 优先权（见方案 §4）。

## 方案

### 1. 计数范围：一个账本（约束 3）

名额的计数范围是**一个 `Ctx` 所持有的账本**：只看 `ctx.store.list_unfinished()` 里的任务，作业文件路径从各任务所属
项目的 `project.path` 推出（`gate::dir_for(project.path, task.id)`）。不扫全机进程，不读任何固定的全局路径。

依据：本仓库的门禁命令跑 `cargo test`，其中每个 e2e 用例 `World::new` 都有自己的 `autome_home`、自己的内存账本和
自己的临时仓库；它们起的内层门禁作业只出现在各自账本的项目目录下，外层真实账本看不到它们，反之亦然。所以外层门禁
不会等内层门禁，并行跑的测试之间也不互相排队。这一点由 e2e `gate_queue_counts_per_ledger_not_per_machine`
（两个 World 各自有一个门禁同时在跑、都没有 `gate.queued`）证明。

### 2. 名额：谁占着（约束 5、6）

「占用数」`held` 每次需要准入判定时现算，不存内存。对账本中每个未结束的任务 T，枚举 `gates/<T>/` 下的每个作业
（以 `.started` 为准的 stem），作业 J 占一个名额当且仅当：

- (a) `gate::running(J)` 为真——进程活着，或启动不足 10 秒（启动空窗）；或
- (b) J 已写退出标记、但 T 还没收它，且 T 处于 `Active`：
  - 准备作业（stem 以 `setup-` 开头）：`.started` 还在即未收（`ensure` 收完会删 `.started`）；
  - 门禁作业：T 为 `Active { node: X }`，J 的 stem 以 `X-` 开头，且 T 在节点 X 上最后一条 `gate.started` /
    `gate.result` 是 `gate.started`、其 `log` 等于 J 的日志路径；
  - 但 (b) **有期限（D2-P01）**：J 的退出标记早于「完成了内核步骤的上一次 tick 的开始时刻」时，(b) 不再成立（见下
    「持有名额的作业必须有不依赖业务路径的收束」）。

`Vanished`（没有退出标记、没有活进程、启动超过 10 秒）的作业不占名额。

**旧 head 的作业必须被收束（D1-P01）。** 非 MainCheck 的门禁作业按 head 前 12 位建 `Job`（Gate / MergeGate / ReproCheck /
FixGate），`run_gate` 每次只 poll 当前 head 的那一个；head 在两次 tick 之间变了，旧 head 的作业没有人再 poll，既不会被
收，也不会被 `job.clear()`——若它已退出且仍满足 (b)，就会永久占位，把后面的人全部锁死。因此 `run_gate` 对这四种节点
在做任何准入、`job.exists()` 判断之前，先做一次**清扫**：新增 `gate::discard_stale(repo, task, node, keep_key)`，对
`gates/<task>/<node>-*` 下 key 不等于当前 head key 的每个作业——进程还活着就杀进程组，随后连同 `.started/.pid/.exit/.log`
一并删除（已退出未收的、`Vanished` 的同样删除；它们的结果是针对已作废的 head，本来就要丢）。清扫之后，同一任务同一
节点最多剩当前 head 的一个作业，旧 head 的作业不可能继续占名额或继续跑。MainCheck 的 head 是固定的合并提交，不会变，
不需要清扫。清扫放在 `run_gate` 最前面，所以即使任务此刻因别的原因排队，也会先收束旧作业再判准入。

**持有名额的作业必须有不依赖业务路径的收束（D2-P01）。** 上面的“Active 任务必然继续 poll”并不总成立：`run_gate` 要先
成功 `gate_head` 才到清扫和 poll，门禁前准备会因 `[setup]` 被移除而不再进 `ensure`，会话前准备可能被冷却路由提前返回，
Cleanup 在配置缺失时跳过主检出准备。这些路径下，一个 `Active` 任务名下的作业既没有人超时处理、也没有人收。名额回收因此
不能只靠业务路径，每个 tick 在内核步骤**之前**做两件与任务状态、配置能否解析都无关的事（都不写事件、不收结果、不删文件、
不伪造门禁通过）：

1. **全任务超时回收**（D1-P04 的「空闲任务超时回收」推广到全部未结束任务，含 `Active`）：对 `list_unfinished` 里每个任务，
   枚举其 `gates/<T>/` 下 `gate::running` 为真的作业，按 `.started` 与超时调一次 `gate::poll_as`——它已有的行为是：未超时
   返回 `Running`；超时则杀进程组、追加终止行、写退出码 124。仍活着的作业因此最迟在超时后结束，不会只被“减计数”。
   超时值取该任务所属项目的有效配置（门禁 `[gate] timeout_minutes`、准备命令 `[setup] timeout_minutes`）；**配置被移除、
   读不到或解析失败时**，门禁取 30 分钟、准备取 `DEFAULT_SETUP_TIMEOUT_MINUTES`（30）——这两个缺省值本来就是配置的缺省，
   没有新增来源。对 `Active` 任务这是幂等的：回收先杀掉，随后 `run_gate` / `ensure` 照常 poll 到同一个 `Done(124)`，
   `gate.result` / `setup.result` 与现在逐字相同。
2. **遗留的已退出作业不无限占位**：规则 (b) 只需要覆盖「同一个 tick 里 B 先于 A 被处理」这一个窗口。因此 (b) 以时间为
   界：核心在内存里记下「完成了内核步骤的上一次 tick 的开始时刻」`prev_tick`（被 `skipped` 的 tick 不算；core 重启后为空，
   此时 (b) 保持成立，下一个 tick 即有值，所以只多占一个 tick）；Active 任务名下已退出的作业，若其退出标记（`.exit`）的
   修改时间早于 `prev_tick`，说明上一个 tick 的步骤已经有机会收它而没有收（A 的步骤出错、跳过 `ensure`、`gate_head` 读不到、
   配置被移除……），此后 (b) 不再成立，名额释放。时间判据成立的依据：A 的步骤在上一个 tick 里观察到作业仍在 `Running`，
   则退出必然晚于该步骤、晚于 `prev_tick`，(b) 仍然成立，所以正常收尾路径的顺序保证（T1）不受影响。被释放的作业**文件
   仍在**，不删、不当作 `Vanished`：A 的业务路径恢复后照常收到真实退出码、写 `gate.result`（此时它可能晚于别的任务的
   `gate.started`，只丢失账本顺序的便利，没有错误结果）；永远没有人收的遗留文件（例如 `[setup]` 被移除后留下的准备作业文件）
   不占名额，也不影响任何结果，`[setup]` 再次出现时由 `ensure` 按现有逻辑收走。

这样，持有名额的作业在任何路径下都有界：仍在跑的最迟 `timeout_minutes` 后被杀，已退出的最迟再过一个 tick 释放。第 1 项
同时取代下面「暂停的任务」一段里的「空闲任务」限定。

规则 (b) 是为了让「第一个任务的 `gate.result` 先于第二个任务的 `gate.started`」在账本顺序上成立：一个 tick 内任务按
`created_at` 处理，若 B 排在 A 前面、A 的作业刚好结束还没被收，只看进程存活会让 B 先启动、A 后写结果。(b) 让 A 的作业
在被收之前一直占着名额。它只对 `Active` 任务成立、且有上面的期限，所以暂停、停止、失败的任务，以及业务路径不可达的 `Active` 任务，都不会因为一个没人收的已结束作业而永久占位。

由此逐条回答约束 5：

- 作业正常结束、失败、超时：收完后 (a)(b) 都不成立，名额释放。
- `Vanished`：不占名额；任务清掉作业文件后重新走准入（见 §3「重新入队」）。
- head 已变被作废（`job.clear()`）：文件没了，名额释放。
- 停止 / 取消：`stop_running_session` 已经 `gate::stop_all` + `discard_node("setup")`，进程死、文件删；取消的任务是终态，
  不在 `list_unfinished` 里。`discard_node`（恢复已停止的门禁、追加轮次、`rerun`）删文件，名额释放。
- 失败：失败任务不是 `Active`，(b) 不成立；它若还有活进程（例如准备作业在途时失败），(a) 让它如实占位直到进程结束——
  这是真实的机器负载。
- **暂停的任务**：作业仍在跑（现有行为，恢复后接着收同一作业，`paused_gate_resumes_at_gate_without_restarting_job`），
  按 (a) **继续占名额**，因为它确实在消耗机器，而限制并发负载正是本任务的目的。现有代码只 poll `Active` 任务的作业，
  暂停期间不会超时，一个挂死的作业会让后面的人无限等（D1-P04）。因此由上面的**全任务超时回收**覆盖（对暂停、停止、失败的任务同样每个 tick
  调一次 `gate::poll_as`）。回收**只做这一件事**，不收结果、不删文件、不写事件：作业被杀后 (a) 不再成立，(b) 因任务非 `Active`
  不成立，名额释放；任务恢复后沿用现有路径收到退出码 124，`gate.result` 照常带终止行（与「恢复时超时立即生效」的现有
  语义结果一致，只是提前到超时那一刻）。由此，任何在途作业——包括暂停任务的——最迟在 `timeout_minutes` 后结束
  （见 §4 的有限等待论证与风险 R3）。

### 3. 队列：谁先排、何时出队（约束 1、6，开放问题 10）

新增事件 `gate.queued`，主体是任务，载荷：

```text
{ "node": "<当前节点>", "job": "gate" | "setup", "checkout": "worktree" | "verify" | "main" | null, "position": <入队时的名次，1 起> }
```

- **入队**：任务走到「要启动一个新作业」的那一点、准入判定为「等」、且它当前没有有效排队项时，写一条 `gate.queued`。
  有有效排队项时不重复写（每个 tick 都会再判定一次，但只写一次事件）。
- **有效排队项**：任务最后一条属于下列种类的事件是 `gate.queued`：
  `gate.queued`、`gate.started`、`setup.started`、`session.started`、`session.deferred`、`task.updated`、
  `gate.queue_dropped`。
  也就是说，排队项在以下任一事件后失效：真正启动了作业（`gate.started` / `setup.started`）、不需要作业直接起了会话
  （`session.started`）、会话被限流冷却推迟（`session.deferred`，冷却期间不再来尝试）、任务状态有任何变化
  （`task.updated`：暂停、停止、取消、失败、通过、`rerun`、追加轮次……）、或内核判定它此刻无法推进而主动退队
  （`gate.queue_dropped`，见下）。
- **不可推进的队首自动退队（D1-P04）**：严格 FIFO 下队首若每个 tick 都在到达准入判定之前出错（例如读不到 git head），
  它不会出队，后面的人会被 `ahead != 0` 永久挡住。因此 tick 里 `run_core_step`（以及 `start_session` 经 `start_due_retries`
  / `fill_slots` 的路径）返回错误时，**同一处**检查该任务是否持有有效排队项，是则追加
  `gate.queue_dropped {node, error}`——排队项随即失效，后面的人顺位前移；任务下个 tick 若恢复正常会走到启动点，按「重新入队」
  排到队尾。错误本身照旧进 `report.errors`，不吞。有限等待保证针对**能走到准入判定的任务**；一个每个 tick 都在准入之前
  报错的任务不是「在等名额」，而是故障，它不再占位即可，不在保证范围内。
- **顺序**：有效排队项按其 `gate.queued` 的 `seq` 升序排列。`seq` 是账本自增主键，core 重启后原样可读，所以
  **重启后的排队顺序由事件推出，不加表、不改 schema**（约束 6，任务卡第 4 条）。
- **重新入队**：排队项失效之后再回到启动点（暂停后恢复、`rerun`、作业 `Vanished` 或 head 变了被作废后重跑、冷却结束
  再来）一律**算新排**，排到队尾。理由：排队项只表示「此刻在等」，保留旧位置需要再定义一套「离开多久仍算数」的规则，
  而排到队尾在 FIFO 下最多多等「当时队里的人数」个作业，有界。
- `record_gate_result` 的 `any_event_after(task, ["gate.started", "session.started"], seq)` 不变：`gate.queued` 不在
  这个列表里，不会被当成一次新运行；排队期间也不写 `gate.started`（约束 1）。

`position` 是**入队时的快照**。在严格 FIFO 下没有人能插到已在排队的任务前面（§4），所以一个任务前面的人只会减少，
快照是实际等待名次的上界，不会因后来的插队而变得误导（约束 10）；实时名次看 `task.get.gate_queue_position`。

### 4. 准入规则与顺序：严格先到先得，无优先（约束 7，开放问题 1）

纯函数放在 `autome-domain` 新模块 `gate_queue`（约束 12：只收参数，不做 I/O）：

```text
Waiter { task_id, seq, limit }
admission(held, waiters, task_id, limit) -> Go | Wait { position }
position(waiters, task_id) -> Option<usize>   // 1 起
```

- 任务在 `waiters` 里：`ahead` = 排在它前面的有效排队项个数；不在（新来的）：`ahead = waiters.len()`。
- `Go` 当且仅当 `ahead == 0` 且 `held < limit`；否则 `Wait { position: ahead + 1 }`。
- `limit` 是**该任务所在项目**的有效名额（§7）。

这是严格的 FIFO：队首没轮到时，后面的人即使所在项目名额更大也不能越过它。因此：

- **不饿死**：队首只等 `held < limit`；队首在等时没有任何新作业能被准入（所有人都在它后面）。前提有两个，都由本设计
  保证而不是假设：(1) 每个在跑的作业都会结束——正常路径下 `Active` 任务的作业由 `run_gate` / `ensure` 的 poll 在 `timeout_minutes`
  内杀掉，其余任务（含暂停、业务路径不可达的 `Active` 任务）的作业由 §2 的全任务超时回收杀掉、已退出的遗留作业由 §2 的 (b) 期限释放，旧 head 的作业由 §2 的清扫收束，升级时遗留的旧
  作业同理（§10）；(2) 队首能推进——走不到准入判定的队首由 §3 的 `gate.queue_dropped` 退队。在这两个前提下，队首至多再等
  `held` 个作业结束，第 k 个排队者至多再等有限个作业。仍然成立的人为例外只有：用户自己把任务暂停且不恢复——暂停使
  `task.updated` 让它出队，它不再是排队者。
- **「前面还有 k 个」与实际启动顺序一致**：没有越位，启动顺序就是排队顺序。
- 名额 N ≥ 2 时，同一 tick 里排在后面的任务可能比队首先被处理而判「等」，下一个 tick 才启动——只差一个 tick。

**MainCheck / MergeGate 不优先。** 理由：(1) 任务卡把优先权留给设计轮，前提是不饿死、k 与实际顺序一致；严格 FIFO 两条
都直接成立，带优先级就要另加老化规则，k 也会因为后来的高优先级作业而变大，和 `gate.queued.position` 的快照语义冲突；
(2) 排队中的 MergeGate 只推迟这一个任务的合并，不挡别的任务；MainCheck 的结果只决定是否报警，晚几分钟不影响任何
任务的推进；(3) 11-12 的事故是并发造成的变慢，FIFO 下每个作业独占机器，单个门禁从 23 倍变回基线，等待时间可预期。
用户主动按下合并后的等待体验进 Backlog（B-01）。

### 5. 接入点：逐种作业（约束 4、13，开放问题 5）

准入在 `automed` 新模块 `gate_queue.rs` 里做：`held(ctx)` 按 §2 现算；`waiters(ctx)` 对每个未结束任务取其最后一条
§3 所列种类的事件，挑出有效排队项并读出各自项目的 `limit`；`admit(ctx, task, project, node, job, checkout)` 调纯函数，
判「等」且没有有效排队项时写 `gate.queued`。

**读不到作业目录不等于没有作业（D2-P02）。** `held(ctx)` 对每个未结束任务 T 的探测结果是三值，不再把读取失败按 0 计：

- `read_dir(gates/<T>)` 成功：按 §2 枚举；
- 返回 `NotFound`，且 T 所属项目的 `project.path` 本身存在、可读：没有作业（`gate::start` 才建目录），记 0；
- 其它情形（`project.path` 不存在或不可读、权限错误、I/O 错误），但 T 的账本事件能**证实没有在途作业**——T 没有任何
  `gate.started` / `setup.started`，或其最后一条 `gate.started` 之后已有 `gate.result`、最后一条 `setup.started` 之后已有
  `setup.result`——记 0，不报错；
- 其余：**未知占用**。此时 `held` 视为已满（等于该次判定的 `limit`），任何任务的准入都判「等」（照常写 `gate.queued`、
  排队、不动预算），直到该目录可读或该任务结束。内核同时向本 tick 的 `report.errors` 追加一条 tick 级信息：
  `无法确认项目 {名字} 的宿主作业（{原因}）：任务 {id} 可能仍有作业在运行，已暂停准入新的宿主作业；请恢复该目录，或取消该任务`。
  它不属于任何任务的步骤，所以不触发 `gate.queue_dropped`；已在跑的作业的收取路径不经过准入，不受影响。
  这是有意的保守：不扫全机进程就无法确认的活进程，宁可多等，也不让并行数超过 N。恢复路径是用户可达的（目录恢复可读、或取消
  该任务——取消后任务不在 `list_unfinished`），不是永久锁死；而「路径存在、只是没有作业」这一常见情形不受影响。

各作业是否占名额、何时占、何时放：

- **准入点 = 每一个「要起进程」的点，不是某个函数的入口（D1-P02）。** 宿主作业只在两处被起：`setup::ensure` 里起准备
  进程之前、`run_gate` 里 `gate::start` 之前。两处各有一个 `hold_or_admit` 判定：
  - 若本次调用刚**收完**了同一任务、同一检出的准备作业（`ensure` 的收尾分支，作业文件已删），且该作业满足**接续资格**，
    则视为名额一直在手，直接起下一个进程，不判准入；
  - 否则走 §4 的准入，「等」就返回（`ensure` 返回 `Pending`，`run_gate` `return Ok(())`），任务停在原节点。
  **接续资格**：被收的准备作业必须是**持续占着名额**的——任务是 `Active`，且任务账本里最后一条 `setup.started` 之后没有任何
  `task.updated`（暂停 / 恢复 / 停止 / 追加轮次都会写 `task.updated`），且退出标记未过 §2(b) 的占位期限；
  已过期的准备作业名额已释放，恢复收取时同样必须重新准入。这是一个保守的判据：暂停期间作业若已退出，名额
  已被 §2 释放，别人可能已经占了；恢复后哪怕作业文件还在，也**不得**据此启动后续作业，必须重新判准入、排到队尾。
  暂停期间作业仍存活的情形没有问题：它按 (a) 一直占着名额，恢复后的 `ensure` 看到作业存活只返回 `Pending`，不起新进程，
  不判准入；它退出之后被收那一次，因为 `task.updated` 已存在而失去接续资格，要重新判准入——代价是多等一次队，
  收益是不会超额。已被 §2 释放的作业，其结果仍可收（`Done` 的退出码照常生效），只是**之后**的新进程不再免准入。
  这条规则覆盖：门禁前准备 → 门禁（worktree / verify）、会话前准备里「收完旧作业后指纹又变而重跑准备」、`setup-main`
  收完旧作业后重跑，三处同一个判据。
- **Gate / MergeGate / MainCheck / ReproCheck / FixGate**（`run_gate`）：先做 §2 的旧 head 清扫；在 `!job.exists()`
  分支里，`gate.reused` 判断**之后**才可能需要准入。占名额从准备作业或门禁作业启动起，到门禁结果被收为止。
- **门禁前的准备（worktree / verify）与随后的门禁连着占同一个名额**：准入只在这次门禁运行的**第一个**要起的进程前判一次
  （无准备配置、或准备指纹未变时就是 `gate::start` 之前；要跑准备时就是 `ensure` 起准备之前）；准备作业在途由准备作业
  占名额；`ensure` 收完准备作业返回 `Ready` 时，同一次调用里立刻 `gate::start`，凭接续资格不再判准入。理由：各排一次
  会让任务在准备和门禁之间被插队，准备的成果（依赖已装好）白等，且同一个任务在两个作业之间被拆开排两次队。
- **会话前的准备（worktree，`start_session`）**：`ensure` 在「需要跑且此刻没有在途作业」时判准入；「等」返回 `Pending`，
  现有代码随之 `schedule_retry(…, "等待准备命令", counted = false)`——不计崩溃重试次数。准备跑完之后起会话，会话不占名额。
  **Bug Loop 的修复轮次在此处不提前记（D1-P03）**：见 §6。
- **合并后主检出的准备（`setup-main`，`Cleanup`）**：同上，在 `ensure` 里判准入。顺序是「指纹 → 现有 `setup-main` 互斥
  → 准入 → 启动」：被互斥挡住的任务既不占名额也不写排队项，所以互斥与名额之间没有循环等待（约束 13）。
- **不启动作业的路径不排队**：`[gate]` 已从配置移除（直接 `Passed`）、MergeGate 同 sha 沿用（`gate.reused`）、工作区
  MainCheck 没有单一合并提交（`gate.skipped`）都在准入之前返回；准备指纹未变（`Ready::Ready`）的会话前准备与主检出准备
  不走准入；工作区跳过准备（`setup.skipped`）根本不调 `ensure`；verify 准备失败后跳过 MainCheck（`gate.skipped`）时，
  准备作业已占过名额、收完即放，门禁不再启动。注意：门禁节点上「准备指纹未变」仍要排队，因为随后要启动门禁。
- **排队不卡调度**：排队只是 `run_core_step` 返回 `Ok(())`，同一 tick 里其它任务的内核步骤照常执行，`tick` 不报错。
  排队中的任务仍停在门禁节点，**继续占项目的并行槽位**（`slots_in_use`）：它没有结束，放出槽位只会让更多会话与
  `cargo test` 叠加到正在排队的机器上，这与本任务的目的相反（约束 13）。

### 6. 预算与超时（约束 1、2，验收 2、3）

核对过的全部计数与它们在排队期间为何不动：

- 实现轮次（状态块 `implementation-round`、`ImplBudget`、`task.budget_n`）：只在会话结束后由状态块与 `advance` 改变；
  排队任务不起会话、不 `advance`。
- 设计轮次（`design-round`、`DesignBudget`）：同上。
- Bug Loop 修复轮次（`FixBudget`、`task.get.task.fix_round` = `bug.fix_round_started` 事件数）：**现状是
  `perform(StartBugSession { step: Fix })` 先写 `bug.fix_round_started`、再调 `start_session`**，而会话前准备的准入在
  `start_session` 里面——FixReview 打回后若准备命令或指纹变了、被名额挡住，轮次会在没有任何 Fix 会话、没有 `setup.started`
  的情况下先被花掉（D1-P03）。设计改为**轮次在真正起会话时才记，且只记一次**：
  - `perform` 对 Fix 不再直接写 `bug.fix_round_started`，改写一条标记事件 `bug.fix_round_requested {round}`（新增、纯增量，
    只表示「这一轮修复已被内核申请，尚未起会话」，不计入 `fix_round`、不进 `bug_progress`）；
  - `start_session` 对 `SessionKind::Bug { step: Fix }`，在会话前准备返回 `Ready`（含准备被名额挡住后的重试到期再来）之后、
    真正启动会话之前，检查该任务最后一条属于 {`bug.fix_round_requested`, `bug.fix_round_started`} 的事件：
    是 `bug.fix_round_requested` 才写 `bug.fix_round_started {round, limit}`（`round` = 此刻已记入的轮次 + 1，`limit` 同
    现状），否则不写。因此：排队、冷却推迟、等待在途准备期间轮次不动；等待后真正启动只记一次；崩溃重试（会话已
    `session.started`、最后一条是 `bug.fix_round_started`）和暂停 / 恢复都不会重复扣；重试路径 `redispatch_with` 直接调
    `start_session` 也走同一判据，不依赖 `perform` 再次被调用；
  - 副作用：冷却推迟（`session.deferred`）期间的 Fix 轮次也改为启动时才记，这是同一判据的自然结果；`bug.fix_round_started`
    的负载与 `fix_round` 的读法不变，旧账本里的旧事件照常读。读者：`event_line` 对 `bug.fix_round_requested` 不新增
    文案（落入 `_ => None`，不输出，不崩），`autome run` 的用户只会看到轮次真正开始的那一条；`gate.queue_dropped`
    同样不输出（用户已经从 `gate.queued` 看到排队，退队不需要单独一行）。
  - 实现期若发现某个旧用例因轮次写入时机后移而失败，按任务文件 §3 逐条记理由；预计 `bug_loop_*` e2e 都在会话启动后才
    计数，不受影响。
- 门禁同因失败的连续计数（`gate.result.same_failures` / `tripped`）：只在 `record_gate_result` 写 `gate.result` 时变；
  排队不写 `gate.result`，`gate.queued` 也不进 `any_event_after` 的列表（§3）。
- 崩溃重试次数（`session_retries.attempts`、`task.get.retry.attempts`）：会话前准备排队走 `counted = false` 的重试，
  `attempts` 加 0；门禁节点排队不碰重试表。
- 会话计数与用量（`task.get.sessions`、`so_far`、`session.started` 事件数）：排队不起会话。

e2e 断言的字段（M-05）：`task.get` 的 `task.budget_n`、`task.fix_round`（Bug 任务）、`status_block.impl_round`、
`status_block.design_round`、`retry`、`so_far`、`sessions` 的长度；账本里该任务的 `session.started`、`gate.result`、
`bug.fix_round_started` 事件数，以及 `Store::retry_attempts`。在入队那一刻取一次快照，排队期间多跑若干 tick 后再取一次，
直到它真正启动作业前最后取一次，三次相等。

超时：排队期间不调 `gate::start`，所以没有 `.started`，时钟不存在；轮到时 `gate::start` 写当时的秒数，`poll_as` 从那时算。
准备作业同理。不新增任何其它时钟来源。

### 7. 配置（约束 8，开放问题 3、4）

- **全局** `~/.autome/config.toml`：新增 `[gate] concurrency`，可省，缺省 1（`DEFAULT_GATE_CONCURRENCY = 1`）。
  `GlobalConfig` 新增 `gate: GlobalGateConfig { concurrency }`（`#[serde(default)]`）。`parse_global` 读 `[gate]`：不是表报
  `[gate] 不是一个表`；`concurrency` 不是 1 到 10 的整数（0、负数、小数、字符串、11）报
  `[gate] concurrency 必须是 1 到 10 的整数`。全局 `[gate]` 里的其它键忽略（与 `parse_global` 忽略未知表的做法一致）。
  `save_global` **不写 `[gate]` 表**：它用 `toml_edit` 原地改 `loop` / `ui` / `roles`，`[gate]` 原样保留；各 `config.set_*`
  都是「从磁盘读 → 改 → 写」，所以用户手写的值不会被界面保存冲掉。
- **项目** `.autome/config.toml`：`[gate] concurrency` 可选，取值与报错同上。`ProjectConfig` 新增独立字段
  `gate_concurrency: Option<u32>`（与 `gate_same_failure_limit` 同样不放进 `GateConfig`）。
  只写 `concurrency` 不写 `command` 的 `[gate]` 表：**视为没有门禁命令、只设名额**——表里有 `concurrency` 时缺 `command`
  不报错；没有 `concurrency` 时缺 `command` 仍报原来的「[gate] 需要 command…」（现有测试
  `a_gate_without_a_command_is_refused_and_an_empty_one_means_none` 不变）。`save_project` 不碰 `[gate]`，原样保留。
- **项目值超过全局：取小，不报错。** `resolve` 产出 `ResolvedConfig.gate_concurrency = min(project.unwrap_or(G), G)`
  （`#[serde(default)]` 为 1）。理由：项目文件提交在仓库里、跟着仓库到每台机器，全局文件属于某一台机器；用户在自己机器上
  把全局调小，不应让所有写了更大值的仓库配置失效、任务全部因「配置无效」停摆。取小同样满足「不得超过全局」。
- **项目值在「跨项目计」下的语义**：项目 P 的任务只有在**全账本**占用数 `held < limit(P)` 时才启动。例：全局 3、P 写 1，
  则 P 的门禁只在没有任何别的宿主作业在跑时才启动；全局名额对其它项目不变。注意这只是「P 的作业启动时机」的约束，
  不是持续独占：P 的作业开跑之后，只要 `held < 全局名额`，别的项目的作业仍可启动（D1-N02）。
- **上限 10**（开放问题 4）：宿主作业是完整测试套件，10 个并行已远离「排队」场景；与 `same_failure_limit` 同一写法与上限。
- 缺省即 1（验收 4）：旧配置文件不需要迁移；没有全局文件时 `GlobalConfig::default()` 的值就是 1。

### 8. 对外形状（约束 10、11，开放问题 2）

两者都做，各管一件事：

- 事件 `gate.queued`（§3）记录「何时入队、入队时第几」，进 `autome run` 的事件流；是排队顺序的来源。
- `task.get` 新增顶层字段 `gate_queue_position`：有效排队项的实时名次（1 起），不排队时为 `null`。
  `dashboard.get` 的 `waiting` / `running` 每一项新增同名字段（排队任务停在门禁节点，在 `running` 里）。
- 现有字段一概不动：`queue_position` 仍只表示项目并行槽位的名次；`gate` 仍是最后一条 `gate.result`；`gate.started` /
  `gate.result` / `gate.reused` / `setup.*` 的字段不变。不新增 JSON-RPC 方法、不改参数，`READ_METHODS` 与桌面端两份白名单
  不变。
- 不新增 `TaskState` / `Node` 取值：任务停在原门禁节点（或会话节点），排队只是附加信息，所以 `state_line`、`node_label`、
  `labels.js`、`task.js`、Web 任务卡都无需改动即可正确显示（约束 11）。桌面端不按事件种类分派，未知事件不会让它崩。
- CLI：`render.rs` 新增 `gate_queue_line(position)` → `排队等门禁（前面还有 {position-1} 个）`；`autome status` 的运行中
  列表在该任务行末尾追加 `· 排队等门禁（前面还有 k 个）`；`autome status <id>` 增加一行 `门禁排队  …`；`event_line` 新增
  `gate.queued` → `{task} 排队等门禁（{节点}，入队时前面还有 k 个）`。

### 9. 门禁超时文案（约束 9，开放问题 8）

**「门禁超时」的定义**：作业被内核因超时杀掉。判据是 `poll_as` 追加的那行
`=== Autome：门禁运行超过 N 分钟，已终止 ===` 出现在日志末尾（`gate::tail` 取的 60 行里一定包含它）。门禁命令自己用
`timeout(1)` 得到的 124 **不算**「门禁超时」：内核没有杀它，它自己的输出已经说明发生了什么，把它说成「门禁超时」会让
修复轮去找一个不存在的时限问题。判据做成 `autome-domain::gate_failure::kernel_timeout(tail) -> Option<u32>`（返回分钟数），
并由 `gate.rs` 的单元测试交叉核对：真实 `poll_as` 杀掉的作业，其 `tail` 能被它识别（防止两边各说各话）。

门禁超时会走到的每一条用户 / 会话可见文案（共九处），逐条改为在超时时明确写「门禁超时」：

1. `launcher.rs` 普通 Loop 的 `Inject::GateFailed` 注入：`…运行了项目门禁 {command}：门禁超时（运行超过 N 分钟，被内核终止），不是测试失败的结论。…`，
   并提示先看日志里卡在哪一步；非超时保持原文「没有通过」。
2. `launcher.rs` Bug Loop 的 `Inject::GateFailed` 注入：同上。
3. `scheduler.rs` MainCheck 失败的 `main_gate_failed` 警报：`…没有通过门禁 {command}：门禁超时（运行超过 N 分钟，被内核终止，退出码 124）…`。
4. `scheduler.rs` 审计简报的 `render_gate_line`：`未通过（退出码 124）` → `门禁超时（运行超过 N 分钟，被内核终止）`。
5. `render.rs` `event_line` 的 `gate.result`：同上（`gate.result` 载荷带 `tail`）。
6. `main.rs` `autome status <id>` 的 `门禁` 行：同上。
7. `render.rs` `failure()` 的 `gate_same_failure`：摘要等于 `gate_failure::TIMEOUT_SUMMARY`（新导出的常量，值仍是
   「门禁运行超时，被内核终止」）时写 `门禁超时：连续 N 次运行超过时限被内核终止（日志：…）`。

8. `launcher.rs` 的 `Inject::ReproNotFailing`（D1-P05）：现文案「复现检查没有失败……原因：{why}……请重新定位并提交稳定的
   回归测试」。`why == "timed_out"` 且 `kernel_timeout(tail)` 为 `Some(N)` 时改为：`复现检查门禁超时：提交 {head}，命令
   {command} 运行超过 N 分钟被内核终止，没有得出结论，不能据此断言未复现。输出末尾（原文）：…。请先看日志里卡在哪一步，
   再决定重新定位还是缩小复现命令。`；其余 `why`（含非内核 124 的 `timed_out`）保持原文。
9. `render.rs` `event_line` 的 `bug.repro_missed`（D1-P05）：现文案「未复现：{command}；{why}」。`why == "timed_out"` 且
   `kernel_timeout(tail)` 为 `Some(N)` 时写 `{subject} 门禁超时：{command} 运行超过 N 分钟被内核终止，本次不能据此断言未复现`
   并保留 `tail`；其余保持原文。`bug.repro_missed` 的负载已带 `why` 与 `tail`，不加字段；`scheduler.rs` 写事件的位置不动。

**不改任何 serde 形状**：`FailureReason`、`Inject`、`Trigger`、`GateOutcome` 都不加字段、不加种类——判据从已有的 `tail`
里读，旧账本里的旧事件不受影响，现有构造这些类型的测试也不用改。`gate_failure::fingerprint` 的入参与摘要不变
（scheduler 测试断言了摘要原文，指纹 id 也不能因升级而变）；它对「任何 124」都给超时指纹，这是现有行为，与第 7 条的
显示保持一致，统一判据进 Backlog（B-03）。这是**保留的兼容例外**：非内核的 124 在熔断摘要里仍被说成「被内核终止」，
与本节「门禁超时」的定义不同，本任务不动（D1-N03）。`ReproDone.timed_out`（任何 124）与 `why = "timed_out"` 的原因码、
serde 形状同样不动，第 8、9 条只在文案层按 `kernel_timeout(tail)` 区分。
桌面端的失败文案（`labels.js`）显示的是同一个摘要，已含「超时」，本任务不改（界面只核对不坏）。准备命令的超时文案
不在任务卡第 5 条范围内，进 Backlog（B-04）。

### 10. 重启与升级（约束 6，开放问题 7）

- 重启：`held` 来自作业文件 + 账本，排队顺序来自 `gate.queued` 的 `seq`，都不在内存里。`recover` 对内核步骤节点本来就
  跳过、交给下个 tick 重跑，排队任务下个 tick 照常判准入；会话前准备排队的任务有待到期的重试，`recover` 也跳过它。
- 升级瞬间已有多个旧版本并行启动的作业：它们都满足 §2 (a) 或 (b)，`held` 可能大于 N。**不杀、不判 `Vanished`**：
  收结果的路径（`job.exists()` 之后的 poll）完全不经过准入，照常收完；在 `held` 降到 N 以下之前不准入任何新作业，
  新来的全部排队。运行数超过 N 只是过渡，最多持续到这些旧作业结束。

### 11. 开放问题逐条结论

1. MainCheck / MergeGate 不优先，严格 FIFO；不饿死的论证见 §4。
2. 事件与字段都做：`gate.queued {node, job, checkout, position}` + `task.get.gate_queue_position`，`dashboard.get` 每项
   同名字段（§8）。
3. 项目值超过全局取小；项目值语义是「全账本占用数达到它就不再启动本项目的作业」；只写 `concurrency` 的 `[gate]`
   等于没有门禁命令、只设名额（§7）。
4. 上限 10（§7）。
5. 门禁前准备与门禁连占一个名额；`setup-verify` 属于 MainCheck 那次准入；`setup-main` 与会话前准备各自在 `ensure` 里
   判准入，`setup-main` 先过现有互斥再判准入（§5）。
6. 暂停任务的在途作业继续占名额，但由 §2 的全任务超时回收保证最迟 `timeout_minutes` 后释放；暂停期间已退出的作业不占名额、
   恢复后无接续资格；排队中被暂停 / 停止的任务因 `task.updated` 出队，恢复后重新排到队尾（§2、§3、§5）。
7. 升级时超额的旧作业照常收完，期间不准入新作业（§10）。
8. 九处文案、判据、不改 serde 形状、`timeout(1)` 的 124 不算（§9）。
9. 验收 3 的测法见「测试与验证」T3。
10. 顺序由 `gate.queued` 的 `seq` 推导；离队后再回来一律新排（§3）。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 排队判定纯函数与 concurrency 配置 | 0 | |
| M-02 | 已完成 | 内核排队：名额探测、gate.queued 与三处启动点接入 | 0 | |
| M-03 | 已完成 | 读者：task.get / dashboard.get 与 CLI 显示排队 | 0 | |
| M-04 | 已完成 | 门禁超时文案 | 0 | |
| M-05 | 已完成 | 端到端验收：排队、预算、超时、账本隔离、重启 | 0 | |
| M-06 | 已完成 | 文档与全量回归 | 0 | |

## 里程碑说明

- **M-01** 结果：`autome-domain` 有 `gate_queue` 模块（`admission` / `position` / 排队项有效性判定）与 `concurrency` 的配置
  形状、`resolve` 取小；`config_io` 读写全局与项目的 `[gate] concurrency`、中文报错、两种保存都不丢这一项。
  覆盖：任务卡第 1 条、约束 7、8、12，验收 4（缺省 1）。前置：无。
- **M-02** 结果：`automed::gate_queue` 现算占用与排队；`run_gate`、`start_session` 的准备、`Cleanup` 的主检出准备三处
  接入准入（准入点是每个「要起进程」的点，接续资格见 §5）；`gate.queued` / `gate.queue_dropped` 事件；旧 head 作业清扫
  （`gate::discard_stale`）、全任务超时回收与 (b) 期限释放（D2-P01）、作业目录读取失败的未知占用（D2-P02）、不可推进队首的退队；Bug Loop 修复轮次改为起会话时才记
  （`bug.fix_round_requested` 标记）；暂停 / 停止 / 作废 / 升级 / 重启路径的名额与排队行为按 §2、§3、§5、§6、§10 实现。
  覆盖：任务卡第 2、4 条，约束 1–7、13、14。前置：M-01。
- **M-03** 结果：`task.get.gate_queue_position`、`dashboard.get` 每项同名字段；CLI `status` / `status <id>` / `run`
  显示排队与 `gate.queued`。覆盖：任务卡第 3 条，约束 10、11。前置：M-02。
- **M-04** 结果：`gate_failure::kernel_timeout` 与 `TIMEOUT_SUMMARY`；§9 九处文案（含 ReproCheck 的 `Inject::ReproNotFailing`
  与 `bug.repro_missed`）在超时时写「门禁超时」，非超时原文不变。
  覆盖：任务卡第 5 条，约束 9。前置：M-03（代码上只依赖 M-01，与 M-02、M-03 无代码依赖；按编号顺序做，契约里的
  用例总数以 M-03 完成时为基）。
- **M-05** 结果：`end_to_end.rs` 新增 6 个 `gate_queue_` 用例，覆盖验收 1、2、3、4 与约束 3、6；现有门禁 / 准备 e2e 一行不改
  全部通过。覆盖：验收 1–4，约束 3、6、14、15。前置：M-02、M-03、M-04。
- **M-06** 结果：README「Host gate, retries and cooldowns」补 `concurrency`；`technical-design.md` §8.1「宿主门禁」
  「宿主准备」补排队规则；`cargo test --workspace`、clippy、fmt、桌面端测试全绿。覆盖：验收 5、6，约束 15。前置：M-05。

## 里程碑验收契约

### M-01 验收契约

最新证据：docs/宿主门禁按机器排队-gate/evidence/M-01-r1-impl.md · 通过 · 审计 #1（证据 docs/宿主门禁按机器排队-gate/evidence/M-01-r1-audit.md）

- 验收命令：`cargo test -p autome-domain && cargo test -p automed --lib`
- 预期：`autome-domain` 共 394 个用例全部通过（基线 385 + 新增 9）；`automed --lib` 共 655 个全部通过（基线 648 + 新增 7）。
  新增用例名一律以 `gate_queue_` 或 `gate_concurrency_` 开头，可用这两个过滤器单独复跑：domain 的 `gate_queue_` 6 个
  （队首且有名额才放行、新来者排队尾、名额已满谁都不放、项目名额小于全局时队首挡住后面所有人、名次 1 起且与入队顺序一致、
  排队项在 `gate.started` / `setup.started` / `session.started` / `session.deferred` / `task.updated` 之后失效而 `gate.result`
  不使其失效）与 `gate_concurrency_` 3 个（缺省 1、项目覆盖、项目超过全局取全局）；`config_io` 里的 `gate_concurrency_` 7 个
  （全局缺省与读取、全局非法值 0 / -1 / 1.5 / "2" / 11 逐个拒绝且报 `[gate] concurrency 必须是 1 到 10 的整数`、项目读取、
  项目非法值拒绝、只写 `concurrency` 的项目 `[gate]` 等于没有门禁命令、`save_global` 保留手写值、`save_project` 保留手写值）。
  基线清单中的原有用例一个不少、一行不改。
- 负向对照：`resolve` 不取小（直接用项目值）时「项目超过全局取全局」失败；`admission` 用 `held + ahead < limit` 代替严格
  FIFO 时「项目名额小于全局时队首挡住后面所有人」失败；`save_global` 改成写回 `GlobalConfig` 默认值或删 `[gate]` 时
  「保留手写值」失败。

### M-02 验收契约

最新证据：docs/宿主门禁按机器排队-gate/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 验收命令：`cargo test -p automed --lib`（新增用例单独复跑：`cargo test -p automed --lib scheduler::tests::gate_queue_`）
- 预期：`automed --lib` 共 676 个用例全部通过（M-01 完成时 655 + 新增 21）；21 个新用例都在 `scheduler::tests` 里、名字以
  `gate_queue_` 开头——第二个门禁排队且不产生作业文件与 `gate.started`；门禁前准备与门禁连占
  一个名额，另一任务无法在两者之间插入；已结束未收的作业在被收之前占名额（B 先于 A 被处理也不能抢先）；升级时两个旧作业
  并行在跑，都被正常收完、不被杀、第三个任务等到两者都结束；排队中暂停 / 停止后出队、恢复后排到队尾；`stop_all` /
  `discard_node` / `Vanished` / head 变化都释放名额；暂停任务的活作业占名额、其已结束作业不占；不启动作业的路径（门禁被移除、
  `gate.reused`、工作区 `gate.skipped`、会话前准备指纹未变）不写 `gate.queued`；会话前准备排队走不计数的重试
  （`retry_attempts` 为 0）；主检出准备的互斥与名额不死锁；项目名额小于全局时的严格 FIFO；排队事件不打断
  `record_gate_result` 的同因计数；（D1 新增 5 个）head 在两次 tick 之间变化时旧 head 的作业（活的被杀、已退出未收的被删）
  不再占名额、新 head 照常准入；暂停期间已退出的准备作业在恢复后不免准入——另一任务已占满名额时，恢复的任务重新排到队尾，
  不与其并行；队首在准入之前持续出错时写 `gate.queue_dropped`、后面的任务顺位前移；暂停任务的挂死作业在 `timeout_minutes`
  后被回收（退出码 124、日志有终止行）、名额释放，恢复后收到同一结果；（D2 新增 4 个）`Active` 任务的 head 读取失败（或 `[setup]` 被移除）而其活作业无人 poll 时，全任务超时回收仍在
`timeout_minutes` 后杀掉它（退出码 124、日志有终止行）、名额释放，业务路径恢复后收到同一结果；同一情形下已正常退出但无人收的作业，
在上一个 tick 之后不再占名额（B 得以启动），而该作业文件仍在、A 恢复后照常收到真实退出码；`[setup]` 被移除后遗留的准备作业
文件不占名额；作业目录读取失败（项目路径被移走）而账本显示有未收尾的 `gate.started` 时，另一项目的准入判「等」并在 tick 报告
给出可恢复的信息，目录恢复后准入放行，而账本证实没有在途作业的项目目录读取失败不挡人；Bug Loop 的 Fix 会话前准备被名额挡住期间
  `bug.fix_round_started` 数不变、等待后真正启动只加 1、崩溃重试与暂停恢复不再加。基线清单中的原有用例（其中 `scheduler` 85 个、`setup` 11 个、`gate::` 11 个）一个不少、
  一行不改。
- 负向对照：准入放在 `setup::ensure` 之后（准备与门禁各排一次）时「门禁前准备与门禁连占一个名额」失败；占用只看
  `gate::running`、不含规则 (b) 时「已结束未收的作业在被收之前占名额」失败；把 `gate.queued` 加进 `any_event_after` 的列表时
  「排队事件不打断同因计数」失败；`held` 只数 `Active` 任务的作业时「暂停任务的活作业占名额」失败；不做旧 head 清扫时旧 head 作业用例失败；
  接续资格只看「准备作业文件还在」而不看 `task.updated` 时暂停恢复用例失败；`perform` 仍直接写 `bug.fix_round_started`
  时 Fix 轮次用例失败；不做全任务超时回收时挂死作业与业务路径不可达两个用例失败；(b) 不设期限时「无人收的已退出作业不占名额」失败；读取失败按 0 计时未知占用用例失败。

### M-03 验收契约

最新证据：docs/宿主门禁按机器排队-gate/evidence/M-03-r3-audit.md · 通过 · 审计 #3

- 验收命令：`cargo test -p automed --lib && cargo test -p autome-cli`（新增用例单独复跑：`cargo test -p automed --lib dispatch::tests::gate_queue_`、`cargo test -p autome-cli gate_queue_`）
- 预期：`automed --lib` 共 679 个全部通过（676 + 新增 3，都在 `dispatch::tests`：`task.get` 对排队任务给出
  `gate_queue_position` 且 `queue_position` 仍为 `null`；不排队时为 `null`；`dashboard.get` 的 `running` 项带同名字段）；
  `autome-cli` 共 42 个全部通过（基线 39 + 新增 3，都在 `render` 的测试里：`gate_queue_line` 的 k = 名次 − 1；
  `event_line("gate.queued")` 写出节点与「入队时前面还有 k 个」；`state_line` 对停在门禁节点的任务输出不变）；
  `READ_METHODS` 与桌面端白名单交叉核对的原有用例不变。
- 负向对照：把排队名次写进 `queue_position` 时第一条失败；`gate_queue_line` 直接用 `position` 当 k 时第二个 CLI 用例失败。

### M-04 验收契约

最新证据：docs/宿主门禁按机器排队-gate/evidence/M-04-r4-audit.md · 通过 · 审计 #4

- 验收命令：`cargo test -p autome-domain && cargo test -p automed --lib && cargo test -p autome-cli && cargo test -p automed --test prompt_rendering`（新增用例单独复跑：三个 crate 各加过滤器 `gate_timeout_`）
- 预期：`autome-domain` 共 397 个（394 + 3）、`automed --lib` 共 684 个（679 + 5）、`autome-cli` 共 46 个（42 + 4）、
  `prompt_rendering` 16 个（不变）全部通过。新增用例名以 `gate_timeout_` 开头：domain 3 个（识别 `poll_as` 的终止行并取出
  分钟数；识别不到普通失败输出；只有退出码 124 而无终止行不算）；automed 5 个
  （真实 `poll_as` 杀掉的作业其 `tail` 被 `kernel_timeout` 识别；两种 Loop 的注入在超时时含「门禁超时」、非超时仍含
  「没有通过」；`main_gate_failed` 警报含「门禁超时」；`render_gate_line` 对超时写「门禁超时」；`Inject::ReproNotFailing`
  在 `why = "timed_out"` 且 `tail` 带终止行时含「门禁超时」与「不能据此断言未复现」，`tail` 不带终止行的 `timed_out`
  与其它 `why` 仍是原文）；autome-cli 4 个（`event_line("gate.result")`、`failure()` 的 `gate_same_failure`、
  `autome status <id>` 门禁行用的同一渲染函数、`event_line("bug.repro_missed")` 对内核超时写「门禁超时」而对普通未复现不变）。
  非超时的原有文案用例一行不改。
- 负向对照：判据写成「退出码 == 124」时 domain 第 3 个用例失败；`gate.rs` 的终止行文案与判据不一致时 automed 第 1 个用例失败。

### M-05 验收契约

最新证据：docs/宿主门禁按机器排队-gate/evidence/M-05-r5-audit.md · 通过 · 审计 #5

- 验收命令：`cargo test -p automed --test end_to_end`（新增用例单独复跑：`cargo test -p automed --test end_to_end gate_queue_`）
- 预期：`end_to_end` 共 112 个用例全部通过（基线 106 + 新增 6，即 T1–T6，见「测试与验证」）；基线中的门禁 e2e
  （`gate` 过滤下 11 个，含任务文件 §7 点名的全部门禁用例）与准备 e2e（`setup_` 过滤下 16 个）一个不少、一行不改；
  `gate_queue_` 过滤单独跑时 6 个用例的总耗时不超过 60 秒（不真等一分钟超时）。
- 负向对照：准入失效（两个门禁并行启动）时 T1 的 `seq` 断言失败；在入队时就调 `gate::start` 或写 `.started` 时 T1 的
  「排队期间无作业文件」与 T3 的「B 以 0 结束」都失败；按全局固定路径或全机进程计数时 T4 失败；排队顺序只放内存时 T6 失败。

### M-06 验收契约

最新证据：docs/宿主门禁按机器排队-gate/evidence/M-06-r7-impl.md · rebase 后全量复验 · 实现 #7（关闭结论沿用审计 #6）

- 验收命令：`cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check && npm test --prefix apps/desktop && grep -n "concurrency" README.md && grep -n "concurrency" docs/development/technical-design.md`
- 预期：`cargo test --workspace` 全绿，`cargo test --workspace -- --list` 共 1289 个用例（rebase 后基线 1228 + 本任务新增 61：
  domain 12、automed lib 36、autome-cli 7、end_to_end 6）；clippy、fmt 无输出；桌面端 105 个用例、104 通过 1 跳过（与基线
  相同）；两条 `grep` 分别在 README 的「Host gate, retries and cooldowns」一节与 `technical-design.md` §8.1 的「宿主门禁」
  「宿主准备」段落各至少命中一行（基线两个文件都是 0 处）。
- 负向对照：README 未补 `concurrency` 时第一条 `grep` 无命中、命令非零退出；任何一个旧用例被删除或改名时 `--list` 与基线
  清单的对比少一个名字、总数对不上。

## 测试与验证

单元测试（M-01–M-04）按契约里的名字前缀组织，便于审计轮用过滤器复跑。e2e（M-05）全部用 `World`、真实 Git、真实
worktree、真实包装脚本与真实后台作业，只把模型换成替身（约束 14）；「两个任务同时到门禁」用项目 `[loop] parallel = 2` 的
两个真实任务构造。门禁脚本按 `$AUTOME_TASK` 写 `ready-<task>`、等 `release-<task>` 文件（沿用 `pause_prelude` 的做法），
由测试决定谁先放行；所有等待都用 `scheduling_wait` 轮询条件，不靠 `sleep` 后看一眼。

- **T1 `gate_queue_second_task_starts_its_gate_only_after_the_first_one_ends`**（验收 1、2、4）：不写任何 `concurrency`
  （缺省）。A 的门禁先启动并被挡住，B 到门禁后：B 有一条 `gate.queued`（`position` 1）、没有 `gate.started`、
  `gates/<B>/` 下没有任何文件；`task.get(B)` 状态仍是 `active / gate`、`gate_queue_position == 1`、`queue_position` 为 `null`；
  `dashboard.get` 的 B 项同名字段为 1。按 §6 列出的字段取三次快照并断言相等。每个 tick 之后断言账本内活作业数 ≤ 1。放行 A
  之后断言 A 的 `gate.result` 的 `seq` 小于 B 的 `gate.started` 的 `seq`，再放行 B，两者都照常往下走。
- **T2 `gate_queue_bug_loop_repro_check_waits_without_spending_a_fix_round`**（验收 2 的 Bug Loop 部分）：一个普通任务 A
  的门禁被挡住，一个 Bug 任务 B 走到 ReproCheck 排队；断言 B 的 `task.fix_round`、`bug.fix_round_started` 数、`retry` 在排队
  期间不变，A 结束后 B 的复现命令才有 `gate.started`。沿用 `bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate`
  的夹具。T2 只覆盖 ReproCheck 门禁的排队；Fix 会话前准备排队与修复轮次的关系（§6）由 M-02 的
  `scheduler::tests` 用例覆盖，不靠 T2。
- **T3 `gate_queue_time_does_not_count_toward_the_gate_timeout`**（验收 3）：`timeout_minutes = 1`。A 的门禁被挡住永不放行，
  B 排队。排队期间：断言 `gates/<B>/` 下没有 `.started`；把 `gates/<B>/` 下此刻存在的任何 `.started` 改成 120 秒前（正确实现
  下是空操作，错误实现若在入队时写了时钟则 B 一启动就会被判超时）；把 A 的 `.started` 改成 61 秒前（与
  `a_gate_that_runs_too_long_is_killed_and_reported_as_timed_out` 同法）。继续 tick：A 以 124 结束且日志末尾有终止行
  （按 A 自己的启动时间计）；随后 B 启动，B 的 `.started` 不早于 A 的 `gate.result` 的 `at`；放行 B，B 的 `gate.result`
  退出码为 0 而不是 124——B 排在一个「跑了超过 1 分钟」的作业后面，等待时间却没有算进它的超时。A 的下一轮会话提示里含
  「门禁超时」（M-04 文案）。全程不真等一分钟。
- **T4 `gate_queue_counts_per_ledger_not_per_machine`**（约束 3）：两个 `World`（两个账本、两个临时仓库），各有一个任务的
  门禁被挡住；断言两者都有 `gate.started`、都没有 `gate.queued`。
- **T5 `gate_queue_holds_a_setup_command_behind_another_tasks_gate`**（约束 4）：项目配了 `[setup]`。A 的门禁被挡住时创建
  B，B 第一个会话前的准备排队：有 `gate.queued`（`job: "setup"`、`checkout: "worktree"`）、没有 `setup.started`、
  `retry.attempts == 0`；放行 A 后 B 的 `setup.started` 的 `seq` 大于 A 的 `gate.result`。
- **T6 `gate_queue_order_survives_a_core_restart`**（约束 6、任务卡第 4 条）：`parallel = 3`，A 的门禁被挡住，B、C 依次排队
  （名次 1、2）。用同一个 `Store` 新建一个 `Ctx`（丢弃旧 `Ctx` 的全部内存状态），首个 tick 走 `recover`；断言 B、C 名次不变、
  A 的作业没有被杀也没有被判 `Vanished`；放行 A 后 B 先于 C 启动（`seq`）。

旧用例（验收 4）：任务文件 §7 列出的门禁 / 准备单元测试与 e2e，以及 `scheduler`、`setup`、`gate::`、`config_io`、`dispatch`、
`autome-cli`、`prompt_rendering` 的全部原有用例，**一行不改**。本设计没有需要改动已有测试的地方：新增的配置字段在现有
测试里都经 `..Default::default()` 构造；不新增 serde 种类或字段到 `FailureReason` / `Inject` / `Trigger` / `GateOutcome`；
`setup_in_the_main_checkout_waits_while_another_tasks_main_setup_is_running` 里的「另一任务」`T-2` 不在账本里，不计入占用。
若实现期发现某个旧用例确需改动，按任务文件 §3 在证据文件里逐条写理由，且理由不能是「新行为与旧断言不符」。

基线：本轮在提交 `186e1fc` 上用 `cargo test … -- --list` 数过——`autome-domain` 385、`automed --lib` 648、`autome-cli` 39、
`end_to_end` 106、`prompt_rendering` 16、`cargo test --workspace` 合计 1201；桌面端 `npm test` 105 个（104 通过、1 跳过）。
契约里的总数都按「基线 + 前面里程碑的新增 + 本里程碑的新增」写。实现轮在 M-01 开工前重跑 `--list` 并把全量名字清单存到
`.autome/output/`、数字写进 M-01 的证据文件；若 rebase 带进了别的任务的用例，以重记的基线为准，各里程碑的**新增数不变**。

## 人工验收清单

- H-01 在本机全局 `~/.autome/config.toml` 不写 `[gate]` 的情况下，让 autome-v2 项目两个任务先后到门禁，在 `autome status`
  里看到后到的那个显示「排队等门禁（前面还有 0 个）」，前一个门禁结束后它才开始（`autome run` 事件流里先有前者的门禁结果，
  再有后者的门禁开始）。

## 风险与回退

- **R1 队首卡住挡住后面所有人。** 严格 FIFO 下，有效排队项的队首若每个 tick 都在准入之前报错（例如 git 读不到 head），
  会让后面的人一直等。已解决（D1-P04）：出错的同一处写 `gate.queue_dropped` 使它退队，后面的人顺位前移；错误照旧进
  `tick` 的 `errors`。残余：一个间歇性出错的任务每次恢复后重新排队尾，可能多等几轮——这是故障任务，不在有限等待
  保证范围内。会话前准备被冷却推迟时已由 `session.deferred` 出队。
- **R2 本任务自己的门禁仍按旧逻辑并行跑。** 实现期 Autome 跑的门禁命令是旧二进制，可能被别的任务挤到超时；按协议如实
  记录，不改 `ci-local.sh`、不调 `timeout_minutes`（约束 15）。
- **R3 暂停任务的作业占着名额。** 现有「暂停不 poll」行为与「活作业占名额」的组合会让挂死的门禁在暂停期间无限占位。
  已解决（D1-P04）：§2 的全任务超时回收（D2-P01 起覆盖 `Active` 任务）在每个 tick 对存活作业调一次 `gate::poll_as`，到点杀掉并写
  124，名额随之释放；恢复后收到的就是这个 124。残余：回收的时间精度是一个 tick；超时值取当前有效配置，配置在作业
  启动后被改过时以回收时刻的为准（与 `run_gate` 的 poll 同样的取法）。
- **R4 缺省从「不限」变成 1 会让多任务的门禁整体变慢。** 这是任务卡要求的缺省；需要并行的用户在全局写更大的值。
  现有 e2e 里两个任务都要过门禁的用例（`two_tasks_waiting_to_merge_both_land_and_the_second_rebases_through_the_gate_by_itself`
  等）在缺省 1 下只是串行，`settle` 会把队列跑空；M-05 的基线核对会暴露任何因此超时的旧用例。
- **R5 与并行任务的冲突。** 其它任务也在改 `scheduler.rs`、`task.rs`、`dispatch.rs`、`end_to_end.rs`。排队逻辑集中在新模块
  `automed::gate_queue` 与 `autome_domain::gate_queue`，三处启动点各只插几行，降低 rebase 冲突（约束 16）。
- **R6 未知占用与 (b) 期限的保守代价（D2）。** 作业目录读取失败且账本不能证实没有在途作业时，整本账本暂停准入新的宿主作业，直到
  目录恢复或该任务取消（tick 报告点名项目与任务）；(b) 的期限在 core 重启后第一个 tick 内不生效。两者都只会多等、不会超额，
  且用户可达。残余：项目目录被永久删除而任务既不取消又不结束时，账本会一直等到用户处理——这是有意取舍，不扫全机进程。
- **回退**：在全局写 `[gate] concurrency = 10` 即基本回到不排队的行为；代码回退是撤掉三处准入调用，新事件与新字段是纯增量，
  旧读者不受影响。

## Backlog

- B-01 用户按下合并后的 MergeGate 是否需要「插到队首、但每个普通门禁至多被越过一次」的优先规则；需要先有等待时长的
  实际数据。
- B-02 （已由 D1-P04 采纳并入 §2，不再是 Backlog。）
- B-07 旧用例 `stopping_a_task_kills_the_cli_the_wrapper_started` 读 `cli.pid` 时可能读到刚创建尚未写入的空文件（并发整套下复现），应等到内容非空再解析。
- B-03 统一「门禁超时」判据：`gate_failure::fingerprint` 与 `ReproDone.timed_out` 目前对任何 124 都按超时处理，与
  「内核终止行」判据不一致。
- B-04 准备命令超时（`FailureReason::Setup` 退出码 124）的文案同样写明「准备命令超时」。
- B-05 `dashboard.get` 顶层给出全账本的名额使用情况（`concurrency`、在跑几个、排队几个），`autome status` 显示一行汇总；
  桌面端 / Web 面板显示排队名次。
- B-06 全局 `[gate]` 里写了 `concurrency` 以外的键（例如 `command`）时给出提示，而不是静默忽略。
- B-07 实现期构造 T3 的负向对照时，宜针对 B 自己的入队时间（让 B 的 `gate.queued` 到真正启动之间的间隔超过 `timeout_minutes`，
  例如经测试夹具改写 B 的入队时钟），而不是只回拨 A 的 `.started`；`gate::start` 会 clear 并重写 `.started`，回拨提前写出的
  B 文件未必产生预测的超时（D1-N01）。
