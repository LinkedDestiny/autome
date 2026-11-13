# 合并队列：合并前门禁出队时才 rebase，通过后在同一段临界区里合并

status: 实现中
design-round: 1/15
implementation-round: 4/25
current-milestone: M-04
current-milestone-reopens: 0
convergence-mode: normal
next-action: 实现 M-04：过期显示与合并队列的输出（D5 渲染、D7）

任务文件：`docs/合并队列-合并前门禁出队时才-rebase/合并队列-合并前门禁出队时才-rebase-task.md`
任务卡（第一输入）：`docs/合并队列-合并前门禁出队时才-rebase/合并队列-合并前门禁出队时才-rebase-card.md`

## 背景与目标

### 背景

`[gate] run = "before_merge"`（T-61）让完整门禁每个任务只在合并前跑一次，T-47 让宿主门禁按账本 FIFO 排队、缺省同一时间只跑 1 个。
两者叠加后，合并前门禁的结果只对「跑的那一刻的 main」有效，而任何一个任务合并都会让 main 前进：

- 今天的 rebase 发生在 `Rebase` 节点，也就是**进队之前**。排队 5–30 分钟后才开跑的门禁，跑的是排队前的基线；
- 排在前面的任务一合并，后面所有已出、在跑、在排的结果全部过期，按合并时再走 T-10 的「落后 → 自动 rebase → 再排门禁」；
- 代码层面还有一个同 tick 的竞态：任务 X 的门禁在 tick N 被收走并转到 `Merging`，它的作业文件在同一 tick 里就不再占名额
  （`gate_queue::occupies` 要求任务仍停在该门禁节点），于是同一 tick 里排在后面的 Y 立刻开跑——此时 X 还没合并（`Merging`
  在 tick N+1 才执行，`perform` 对 `RunCoreStep` 什么都不做），Y 的门禁注定作废。只把 rebase 挪到出队时修不了这一条。

2026-11-13 05:31 的现场：4 个任务最坏约 10 次合并前门禁，理想 4 次；操作者只能手工 `stop` + 逐个 `rerun rebase`。

### 目标（编号沿用任务文件 §3）

- G1 出队时对齐：合并前门禁（需求与 Bug 的 `MergeGate`）开跑那一刻的提交，一定以当时的默认分支为祖先。
- G2 合并临界区：已授权的任务「出队对齐 → 门禁 → 合并」是项目内一段临界区，期间本项目不开始别的合并，也不让另一个合并前
  门禁在它之前完成合并；门禁通过直接进 `Merging`，不再报落后。
- G3 预先授权：`autome merge <任务>` 在合并卡点之前也可按，记下授权；`--cancel` 撤销；设计批准卡点不跳过。
- G4 项目级 `[merge] auto = true`（缺省 false），等价于每个任务都预先授权；没配 `[gate]` 时给配置警告，不报错。
- G5 过期可见：停在「等你合并」而默认分支已前进时，`task.get` / `autome status` / `autome ls` 标明落后 N 个提交、按合并会重新
  rebase 并再跑一次门禁。
- G6 顺序：等合并的先后按进入合并前门禁队列的先后（T-47 的 FIFO，`gate.queued` 的 seq）。
- G7 事件与 `autome run` 输出。
- G8 README 与 `docs/development/technical-design.md`。

### 非目标

- 不做合并列车（叠在一起只跑一次门禁、失败二分）——进 Backlog（B-01）。
- 不改 `[gate] concurrency` 的语义；不改 MainCheck 沿用规则（T-60）；不改设计批准卡点；不改 `[merge] after`（T-43）、
  `[setup]`（T-27）的语义；不改 `scripts/ci-local.sh`；不改 `.autome/` 下任何内容；`[merge] auto` 是否在本仓库打开由用户决定。
- 协议正文不改（理由见「方案 · 文档与协议」）。
- `apps/desktop`、`apps/web` 只在新增字段 / 事件让它们现有的校验或测试变红时做最小改动，不做新界面。
- 不管住「本项目合并之外」推进默认分支的东西（用户自己在 main 上提交、别的工具推送）：临界区只约束 Autome 自己的合并，
  T-10 的落后回退保留为兜底（D8）。

## 方案

### 总览

一句话：**保留 `Rebase` 节点不变，在 `MergeGate` 真正开跑前再对齐一次；已授权的任务一进入 `MergeGate` 按进队先后取得资格并拿项目合并锁（早于准备、准入与对齐），拿锁到
`Merging` 结束之间，本项目别的合并前门禁不开跑、别的 `Merging` 不执行。** 授权改成只由用户（以及 T-10 的落后路径）改变的
账本记录，再加项目级开关。所有状态都由账本事件 + 任务状态推导，不新增表。

新增两个模块，按分层放：

- `crates/autome-domain/src/merge_queue.rs`（纯逻辑）：授权来源的计算、临界区「持有 / 释放」判定、`task.merge` 在各状态下的
  语义（`merge_press`）、配置警告。无 I/O。
- `crates/automed/src/merge_queue.rs`（账本与 git）：读授权、找本项目锁的持有者、写锁 / 解锁 / 等待事件、算落后提交数。
  `gate_queue.rs`、`scheduler.rs`、`dispatch.rs` 调它。

### D1 rebase 的口径：入队前 rebase 一次，出队时落后再 rebase 一次

回答开放问题 1。保留 `Rebase` 节点（`after_the_loop` / `Retro` 之后照旧进它，冲突照旧回实现轮），**另在 `run_gate` 的
`MergeGate` 分支里加「出队对齐」**。理由：没配 `[gate]` 的需求任务没有 `MergeGate`，仍要靠 `Rebase` 节点 rebase；保留它
还让 §7.2 之外大批断言「`Rebase` → `MergeGate`」事件顺序的既有测试不必改。代价只是一次本地 rebase（主线没动时是空操作）。

`run_gate` 在 `node == MergeGate` 且作业不存在时，新顺序如下（其余门禁节点不变）：

1. **合并资格**（纯函数 `merge_queue::turn`，输入：本项目持锁者、已授权者里 seq 更早的「排队者」、本任务是否已授权、本任务的 seq）：
   - 本项目有**别的**任务持锁（D3）→ 等锁：不沿用、不准备、不准入，见 D4，返回。未授权任务只受这一条约束。
   - 本任务已授权，且本项目还有**别的**已授权任务排在它前面——「排队者」指停在 `MergeGate`、不持锁、最后一条队列事件是
     `gate.queued` 的已授权任务，前面指 seq 更小；本任务还没有 seq 时，任何排队者都在它前面——→ 同样等（等的是排在前面
     的那个），返回。这一条让已授权任务按进入合并队列的先后（G6）拿锁，沿用路径也不例外。
   - 本任务已授权，没有持锁者也没有排在前面的排队者 → **立刻拿锁**：写 `merge.locked { node: "merge_gate" }`（已持锁则不
     重复写），然后继续。拿锁发生在沿用判断、`[setup]` 准备、宿主准入、出队 rebase **之前**，所以准备命令的异步等待和 rebase
     之后的重跑都在锁内进行，后来的任务拿不到锁。
2. **沿用判断**（`gate.reused`）：在原条件上**加一条「任务分支不落后」**（每个检出 `behind_count == 0`）。落后时不沿用，走下面
   的对齐与重跑。沿用通过时已授权任务早已持锁（第 1 步），`finish_gate` 后带着锁进 `Merging`。
3. `[setup]` 准备（`ensure_with_hold`，不变）。
4. 准入（`gate_queue::admit`，不变）：不放行就照旧写一次 `gate.queued` 并返回。**这一刻之前不 rebase**——排队期间 main 再怎么
   动都不碰工作树。持锁者也在宿主队列里按 seq 排队，锁不让它插队。
5. **出队对齐**：任一检出落后于自己的 base（`git rev-list --count <head>..<base>` > 0）→ `rebase_all(layout)`：
   - 冲突 → `advance(CoreStepDone { node: MergeGate, result: Conflict { files, detail } })`，见 D6；返回。没启动作业、没写
     `gate.started`，转换写下的 `task.updated` 让它的排队条目失效，名额当场让给下一个；转换离开临界区，锁随 `advance` 释放（D3）。
   - 干净 → 写 `merge.rebased { at: "dequeue", repos: [{repo, onto, base_sha, head}] }`；以新 head 重算 `(cwd, head, job)`，
     **从第 1 步再走一遍（至多一次）**：rebase 改了 `[setup].watch` 里的文件时，第 3 步的指纹比对会重跑准备命令（它同一 tick 内
     仍是队首，准备作业照常拿名额，随后靠 `continuation` 连占给门禁）；没改就直接到第 6 步。重走时已授权任务是持锁者，第 1 步
     放行；准备作业异步返回（`Ready::Pending`）期间锁一直在它手里，下一次进入时第 5 步会重新检查是否落后。
   - 不落后 → 不写任何事件。
6. `gate::start` + `gate.started`（不变）。

「门禁开始跑的那个提交一定基于当时最新的默认分支」由第 5 步与第 6 步在同一次 `run_gate` 调用里、中间不执行任何别的任务的步骤
保证；rebase 之后若准备命令异步返回，调用在第 3 步结束，下一次进入重新从第 1 步走到第 5 步，仍然是「对齐之后同一次调用里开跑」。
从第 1 步拿锁到合并之间 main 不被本项目合并推进，由 D3 的锁保证（已授权任务）；未授权任务没有锁，它的门禁结果可能在 AwaitMerge
时过期，由 G5 的显示告知。作业结束时的「head 已变 → 作废重跑」逻辑不变。

`(cwd, head)` 与 `discard_stale` 在第 5 步改 head 后按新 head 重算，旧 key 的作业文件本来就不存在（第 5 步只在作业不存在时到达）。

### D2 授权：账本记录 + 项目开关，只由用户与落后路径改变

回答开放问题 5、6、7。

**存储。** 继续用 `merge.auto` 事件记单个任务的授权（名字是历史遗留，与配置键 `[merge] auto` 同名不同物；文档与代码注释写明
「`merge.auto` 事件 = 单个任务的合并授权记录」）。payload 增加 `by`：

| 写入时机 | payload |
|---|---|
| 用户预先同意（`task.merge` 在非合并卡点） | `{active: true, by: "user"}` |
| 用户撤销（`task.merge {cancel: true}`） | `{active: false, by: "user"}` |
| `Merging` 报 `Behind`（T-10，不变） | `{active: true, by: "behind", repos, detail}` |

**有效授权**（纯函数 `merge_queue::authorization(last_record, project_auto) -> Option<AuthSource>`）：

- 最后一条记录 `active == true` → 已授权，来源 `user` 或 `behind`（老账本里没有 `by` 的 `active: true` 只可能来自 T-10，按 `behind`）；
- 最后一条记录 `active == false` 且 `by == "user"` → 未授权（用户撤销压过项目开关）；
- 其余（没有记录，或老代码写的不带 `by` 的 `active: false`）→ 看项目开关：`[merge] auto = true` 时已授权，来源 `project`。

**寿命。** 授权不再随「离开 rebase → 合并前门禁这一段」自动作废：

- 删掉 `advance` 里「原来有授权、转换不再携带 → 追加 `merge.auto {active: false}`」那一段；
- 域层 `apply_with_merge_request` 不再用 `in_merge_stretch` 过滤：传入的授权在任何状态都有效，它只在两处起作用——
  `(Rebase, Ok)` 且没配门禁、`(MergeGate, GateDone Passed)`，两处都走 `press_merge`；
- `Transition.merge_requested` 改名 `grants_merge_request`，含义改成「这次转换**授予**授权」，只有 `(Merging, Behind)` 为真；
  `advance` 只在它为真且之前未授权时写 `{active: true, by: "behind", …}`（与今天一致）；Bug Loop `apply_for` 里把授权原样
  抄进 `t.merge_requested` 的那一行删掉。

于是：rebase 冲突回实现轮、合并前门禁失败回实现轮、暂停 / 停止 / 恢复、失败后重跑，授权都保留（验收 4 要求的是第一种；
其余几种按同一口径——授权的含义是「它通过合并前门禁就合并」，与 `[merge] auto` 的行为一致）。只有用户撤销会清掉它；任务
完成或取消后它没有意义，不另写事件。

**与卡点的关系。** 授权只影响上面两处转换，所以设计批准卡点、Bug 的 `AwaitInfo` / `NotABug` 都照常停（验收 3）。`press_merge`
遇到「纳入」表态回实现轮（T-09）时授权保留，实现循环收尾后自动走完。`Merging` 得到 `Blocked`（主工作树脏 / 不在默认分支）
仍回 `AwaitMerge` 等用户再按（T-07），授权留着但不会自己重试。

**`task.merge` 的语义**（纯函数 `merge_queue::merge_press(state) -> MergePress`，派发层按结果执行）：

| 任务状态 | 结果 |
|---|---|
| `Active { AwaitMerge }` | `Trigger::Merge`，与今天相同 |
| `Active { Merging \| MainCheck \| Cleanup }` | 拒绝「已在合并」 |
| `Failed` | 拒绝「任务已失败：先重跑，再同意合并」 |
| `Done` / `Cancelled` | 拒绝「任务已终结」 |
| 其余全部（`Draft`、`Queued`、`Requeued`、`Paused`、`Stopped`、其它 `Active` 节点含设计批准卡点与 Bug 等人节点） | 预先授权：已授权则不重复写，返回「已预先同意」；否则写 `{active: true, by: "user"}` |

`task.merge {cancel: true}`（纯函数 `merge_queue::cancel_press(state)`，与 `merge_press` 同处）按状态限定边界：

| 任务状态 | 结果 |
|---|---|
| `Active { Merging \| MainCheck \| Cleanup }` | 拒绝「合并已经开始，无法撤销」（授权记录不动，不返回 `revoked`） |
| `Done` / `Cancelled` | 拒绝「任务已终结」 |
| 其余全部（含 `Active { AwaitMerge \| MergeGate \| …}`、`Failed`、`Paused`、`Stopped`） | 有效授权为真 → 写 `{active: false, by: "user"}`；否则拒绝「没有可撤销的合并授权」 |

撤销时若该任务正持锁（停在 `MergeGate`），同一次调用里追加 `merge.unlocked { reason: "cancel" }`：它不会再自动合并，不该继续挡住
别人。已在跑的门禁照常跑完，通过后因无授权停在 `AwaitMerge`（D3 表）。授权在 `MergeGate` 通过的那次转换里读取并决定去向：通过时
有授权 → `Merging`，之后撤销一律被上表拒绝；通过前撤销 → 停在卡点。所以「返回已撤销」与「随后仍被自动合并」在任何交错下都不会同时
出现（派发层写完后 `tick` 时，任务不可能是 `Merging` 且授权为假）。撤销不改节点，所以
不经过转换表——它和 `pending_decisions` 一样是账本上的用户意图记录；节点转换仍只由 `advance` 做。派发层在写完后照常 `tick`，
返回值是 `task_get` 的 payload 加 `"merge_request": "merging" | "authorized" | "already_authorized" | "revoked"`。不新增 RPC 方法，
两份写方法白名单不动。CLI：`autome merge <任务> [--cancel]`，帮助文本改为「合并到默认分支；未到合并卡点时记为预先同意，
`--cancel` 撤销」，按返回值打一行。

**`[merge] auto`。** `ProjectConfig.merge_auto: Option<bool>`、`ResolvedConfig.merge_auto: bool`（缺省 false），只在项目配置里认
（`[merge]` 本来就只在项目配置里有）。`parse_project` 在 `[merge]` 表里读 `auto`：非布尔报错「[merge] auto 必须是 true 或 false」；
与 `after`、`after_timeout_minutes` 同表共存。`save_project` 用 `toml_edit`，`[merge]` 表原样保留（加测试钉住）。运行中改开关：
有效授权每次现读配置，在途任务在下一个决策点（`MergeGate` 通过 / 无门禁的 `Rebase` 完成）按新值走；用户撤销过的任务不受影响。

**配置警告**（开放问题 7）。纯函数 `config::warnings(&ResolvedConfig) -> Vec<String>`：`merge_auto && gate.is_none()` 时返回
「[merge] auto = true 但没有配置 [gate]：需求任务 rebase 后直接合并，没有门禁把关（Bug 任务仍会跑复现命令）」。读路径现算、
不落库，所以天然不重复：`task.get` 与 `dashboard.get` 带 `config_warnings`（后者按项目），`autome status` 在顶部打印
「注意：项目 X：…」。不进 `ConfigViolation`（那会拒绝保存，任务卡要求不报错）。

### D3 项目合并临界区（锁）

回答开放问题 2、3、9。

**范围。** 起点：已授权任务在 `MergeGate` 里取得合并资格的那一刻拿锁（D1 第 1 步，早于沿用、准备、准入与出队 rebase，所以准备命令的
异步重跑和 rebase 之间不会有别的任务插进来）。终点：离开
`{Active MergeGate, Active Merging}` 的那次转换——正常情况就是 `Merging` 成功转到 `MainCheck` / `Cleanup`，此时 main 已经前进
完毕。MainCheck、Cleanup、`[merge] after` 不在临界区内：它们不再改 main，不必挡住别人。

**谁不持锁。** 未授权的任务门禁通过后停在 `AwaitMerge`，不持锁（否则会无限期卡住别人）。没配 `[gate]` 的需求任务没有
`MergeGate`，`Rebase` 到 `Merging` 只隔一个 tick 且没有昂贵的门禁要保护，也不拿锁；它的 `Merging` 仍服从别人的锁（D4）。

**存储与推导。** 两个事件：`merge.locked { node }`、`merge.unlocked { reason }`。任务 T **持有**本项目的锁，当且仅当：
T 未终结、T 最后一条 `merge.locked` / `merge.unlocked` 是 `merge.locked`、且 T 当前状态是 `Active { MergeGate }` 或
`Active { Merging }`（纯函数 `merge_queue::holds(lock_active, state)`）。`automed::merge_queue::holder(ctx, project_id)` 扫本项目未完成
任务求出它。

**释放。** `advance` 是所有转换的唯一入口：转换前持锁、`transition.next` 不在上述两种状态 → 追加
`merge.unlocked { reason: trigger_name }`（纯函数 `merge_queue::releases(lock_active, next)`）。持有判定里的状态条件是第二道
保险：任何绕过 `advance` 的状态变化（归档、改建）也不会留下永久占位。各情况：

| 持锁者遇到 | 去向 | 锁 |
|---|---|---|
| 门禁失败 / 熔断 / 准备命令失败 | `Implement`（Bug 修复轮）/ `Failed` | 释放 |
| 出队 rebase 冲突 | `Implement`（Bug 修复轮）/ `Failed` | 释放（锁在 D1 第 1 步已拿，转换离开临界区） |
| 用户撤销授权（持锁时） | 仍在 `MergeGate` | 释放（`cancel` 同次写 `merge.unlocked`）；门禁通过后无授权 → `AwaitMerge` |
| 暂停 / 停止 / 取消 | `Paused` / `Stopped` / `Cancelled` | 释放 |
| 作业消失（`Poll::Vanished`）、head 变了作废 | 仍在 `MergeGate`，下一 tick 重新准入、对齐、开跑 | 保留（自己不等自己） |
| 门禁通过、仍有授权 | `Merging` | 保留到合并结束 |
| 门禁通过、授权已被撤销 | `AwaitMerge` | 释放 |
| `[gate]` 中途被拿掉 | 需求任务 `MergeGate` 直接按通过处理 → 有授权进 `Merging` | 保留到合并结束 |
| `Merging` → `Ok` | `MainCheck` / `Cleanup` | 释放 |
| `Merging` → `Blocked` | `AwaitMerge` | 释放 |
| `Merging` → `Behind`（外部提交，D8） | `Requeued { Rebase }` | 释放 |
| `Merging` → `Failed` | `Failed` | 释放 |
| 驱动重启 | 账本与任务状态原样，下一 tick 按上面规则继续 | 从账本推导 |

**不死锁。** ① 拿锁只发生在「本项目没有别的持锁者、也没有排在前面的已授权排队者」时（D1 第 1 步），判定与写 `merge.locked` 在同一次
调用里，所以任何时刻每个项目至多一个持锁者，不存在两个持锁者互等；② 持锁者自己从不等锁（第 1 步与 D4 都排除自己）；
③ 持锁者的进展只依赖它自己的准备作业、门禁作业（有超时，`reclaim` 在暂停时也会收）、宿主名额与一次同步的 `merge_all`：它在宿主
队列里按 seq 排队，挡在前面的只有别的项目（或本项目未授权任务）的作业，它们都会结束；作业消失时它要重新准入，而等锁的任务都被
移出了宿主队列（D4），不会挡在它前面；④ 等锁的任务不占宿主名额。⑤ 排在前面的排队者不可能卡住后面的：它在自己下一次被访问时，
只要没有持锁者就拿到锁；它若暂停、停止、失败、被撤销授权，就不再是排队者。中途被授权的任务（门禁已在未持锁状态下开跑）不补拿锁——
它通过后进 `Merging`，服从别人的锁（若作业消失重新进入 `MergeGate`，则按第 1 步正常取资格），这保住了①。

### D4 等锁：合并前门禁不开跑、`Merging` 不执行，且不堵宿主队列

- **`MergeGate` 等锁**（D1 第 1 步，两种情形：别人持锁、或已授权而前面还有更早的排队者）：若它最后一条队列事件不是 `gate.queued`，
  补写一条 `gate.queued`（占住 FIFO 位置，seq 就是它进入合并队列的时刻）；若它最后一条 `merge.waiting` 不晚于最后一条
  `task.updated`，写 `merge.waiting { node: "merge_gate", holder }`（`holder` 是持锁者，没有持锁者时是排在前面的那个任务；每段
  停留只写一次）。它仍占项目并行槽位（与今天排队等门禁相同）。第 1 步取资格用 seq 而不是 tick 内处理顺序，所以创建顺序与入队顺序
  相反、锁刚释放的 tick 里先处理到后入队者，都不会让它越过更早的排队者。
- **宿主队列排除等锁者**：`gate_queue::waiters` 跳过「停在 `MergeGate`、作业不存在、`merge_queue::turn` 结果为等」的任务（与
  第 1 步共用同一个判定）。否则它排在
  队首会让别的项目（以及 `concurrency > 1` 时同项目的别的门禁种类）全部等着——那就改了 `concurrency` 的语义。锁释放后它带着
  原来的 seq 回到队列，于是按进队先后（G6）轮到它。`gate_queue_position` 对等锁者为 `null`，界面改看 `merge.waiting_for`（D7）。
- **`Merging` 等锁**：`run_core_step(Merging)` 在 `merge_all` 之前查 `holder`；有别的持锁者 → 写一次
  `merge.waiting { node: "merging", holder }`，返回、不转换（下一 tick 再查）。被按合并的未授权任务、无门禁项目里有授权的任务、
  中途被授权的任务都走这里。锁释放后它照常 `merge_all`：main 没动就直接合并（验收 5 的「A 门禁失败，B 继续」）；main 动了就是
  T-10 的落后回退。

同 tick 的竞态因此关掉：X 门禁通过转 `Merging` 后仍持锁，同 tick 后面的 Y 在第 1 步就停下，直到 X 合并、转出 `Merging` 释放锁；
下一 tick Y 出队对齐到包含 X 的 main 再开跑。

**`[gate] concurrency > 1`**：两个合并前门禁可以同时跑的前提是它们不属于同一项目的「已授权 + 持锁」者。同项目里已授权的第二个
任务等锁；未授权任务的门禁可以在别人持锁之前开跑（第 1 步只拦开跑，不拦已在跑的），它通过后停在 `AwaitMerge`，结果是否过期由
G5 的显示告诉用户。合并本身任何时候都串行（D4 的 `Merging` 等锁 + 每个项目至多一个持锁者）。

**T-53 / T-51**：T-53（排队任务调整顺序）调研时未合并；T-51（`bf399db`，approve/merge/rerun 直接启动会话不看并行上限）已合并，
它改的是 `scheduler.rs` 里会话启动的并行名额与 `scheduler_gate_queue_tests.rs`，不改 `run_gate` 与 `waiters` 的排序。本设计只用
`waiters` 的 seq 排序，T-53 若改的是它的排序口径，本设计自动沿用；每个实现轮开头重新核对这两项在主线上的状态。

### D5 事件与 `autome run` 输出

回答开放问题 10。新增事件都不进 `gate_queue::QUEUE_EVENTS`（它们不改变排队条目：拿锁之后紧跟的 `gate.started` 本来就让条目失效）。

| 事件 | 何时 | `autome run` 一行（`render::event_line`） |
|---|---|---|
| `merge.rebased { at: "dequeue", repos: [{repo, onto, base_sha, head}] }` | 出队对齐真的 rebase 了 | `T-x 出队时已 rebase 到 main @ <base_sha 短>`（工作区按仓库列出） |
| `merge.rebased`（无 `at`，`Rebase` 节点、已授权时写，不变） | 同今天 | 同今天「已自动 rebase 到 …」 |
| `merge.locked { node }` | 拿锁 | `T-x 进入合并临界区：门禁通过即合并，期间本项目不开始别的合并` |
| `merge.unlocked { reason }` | 释放 | `T-x 离开合并临界区` |
| `merge.waiting { node, holder }` | 等锁 | `T-x 等 T-y 合并完成后再开跑门禁` / `T-x 等 T-y 合并完成后再合并` |
| `merge.direct { by }` | 授权使 `MergeGate` 通过（或无门禁的 `Rebase` 完成）直接进 `Merging`，在 `advance` 里写 | `T-x 门禁通过，直接合并（已授权：你预先同意了 / 项目开启了自动合并 / 按合并时分支落后）` |
| `merge.auto { active: true, by: "user" }` | 预先同意 | `T-x 已预先同意合并：走到合并卡点时直接合并` |
| `merge.auto { active: false, by: "user" }` | 撤销 | `T-x 已撤销预先同意合并` |
| `merge.auto { active: true, by: "behind" }` | 不变 | 不变 |

`--json` 输出原样事件（T-46，不变）。`task_metrics.rs`、`apps/web` 通知器、桌面端如果穷举事件种类而因新种类变红，实现轮按最小
改动让其不坏（只认不渲染）；不做新界面。

### D6 出队 rebase 冲突

域层新增两条转换（`is_core_step` 已含 `MergeGate`，`CoreStepDone` 的节点校验不变）：

- 需求：`(Active MergeGate, CoreStepDone { node: MergeGate, Conflict { files } })` → `Implement`，注入 `Inject::RebaseConflict { files }`
  ——与 `(Rebase, Conflict)` 完全相同（复用同一个 `start_impl` 路径）。这条必须写在通用的 `(n, Conflict) → Failed` 之前。
- Bug：`bug_loop::apply_for` 里 `(MergeGate, CoreStepDone Conflict)` → `start_fix`，注入 `RebaseConflict`，与 `(Rebase, Conflict)` 相同。
- `MergeGate` 的 `CoreStepDone` 其它结果（`Ok` / `Failed` / `Blocked` / `Behind`）维持拒绝或原有分支：门禁节点的正常结果仍走 `GateDone`。

`advance` 里「冲突连续计数 + 实现预算检查」那一段（今天只认 `node: Rebase`）扩成 `Rebase | MergeGate`：同一冲突文件集连续 3 次
仍判失败，预算用尽仍判 `ImplBudget`。授权按 D2 保留。冲突修好后任务走 审计 → 复盘 → `Rebase` → `MergeGate`，重新排到队尾。

### D7 过期显示与读路径字段

回答开放问题 8、12。

- git 层新增 `git::behind_count(repo, branch, base) -> Result<u32>`（`git rev-list --count <branch>..<base>`）。
- `task_json` 增加一个 `merge` 对象（终结任务为 `null`），于是 `task.get`、`dashboard.get`、`task.list`（`autome ls`）一起带出：

```json
"merge": {
  "authorized": true,
  "authorized_by": "user | project | behind | null",
  "holds_section": false,
  "waiting_for": "T-12 | null",
  "stale": null
}
```

- `stale` 只在任务停在 `Active { AwaitMerge }` 且落后提交数 > 0 时非空：`{ "behind": N, "gated": bool, "repos": [{"repo", "base", "behind"}] }`。
  读路径现算（只对停在合并卡点的任务调一次 git，代价是每个仓库一次 `rev-list --count`）；工作区按成员仓库各算一次，`behind` 是
  各仓库之和，`repos` 逐个列出；任一 git 调用失败 → `stale: null` 并 `tracing::warn!`，不让读失败。`gated` = 项目配了 `[gate]`
  或任务是 Bug（Bug 的合并前门禁总在）。
- CLI 文案（`render` 新增纯函数，`list` / `status` / `task_detail` 调用）：
  - `gated`：「门禁结果基于旧的 main（落后 N 个提交），合并时会重新 rebase 并再跑一次门禁」
  - 非 `gated`：「分支落后 main N 个提交，合并时会先 rebase」
  - 已授权且未终结：「已预先同意合并」（来源为 `project` 时「项目已开启自动合并」）
  - `waiting_for`：「等 T-y 合并完成」
  - `autome ls` 在 `state_line` 后追加最短的一段（`· 结果已过期（落后 N）`、`· 已同意合并`、`· 等 T-y 合并`）。

### D8 本项目之外推进默认分支

回答开放问题 4。临界区管不住用户自己在 main 上的提交。此时持锁者的 `Merging` 仍可能报 `Behind`，T-10 回退保留为兜底（释放锁、
`Requeued { Rebase }`、授权 `by: "behind"` 不重复写因为已授权）。验收 1 的场景里没有外部提交，回退次数断言为 0；验收 1 的测试
同时断言 `merge.auto { by: "behind" }` 与 `Merging → Behind` 都没有出现。

### 缺省行为与兼容性

`[merge] auto` 缺省 false、没有任何授权、没有并发合并时，用户看到的流程与今天一致（rebase → 合并前门禁 → 等你合并 → 按合并）。
可见的变化与受影响的既有测试（处置见「验收与测试 · 既有测试处置」）：

1. 不再写「离开合并尾段」的 `merge.auto {active: false}`。断言该事件或把它列进 `gate_trace` 期望的测试要删去这一项。
2. T-10 路径里合并前门禁失败、修好之后**自动合并**，不再第二次停在 `AwaitMerge`（用户已经按过合并；要停下就 `--cancel`）。
3. 合并前门禁在出队时发现落后会多一次 rebase，写 `merge.rebased { at: "dequeue" }`；沿用多一个「不落后」条件。
4. 有任务持锁时，本项目别的合并前门禁不开跑、`Merging` 不执行（只在有人被授权时出现）。

### 文档与协议

- `README.md`「Host gate, retries and cooldowns」：配置示例加 `[merge] auto = false` 一行；新增 **Merge queue** 一段（出队对齐、
  临界区范围、等锁不堵队列、预先同意与撤销、`[merge] auto` 的取舍——省下人工按合并 vs 没人看就进 main，无 `[gate]` 时的警告）；
  「The command line」一节的 `autome merge cart` 示例加 `--cancel`。
- `docs/development/technical-design.md`：§3.3 配置示例加 `auto`；§5.3 转换表补 `Rebase 无冲突 → MergeGate（配了门禁）`、
  `MergeGate 通过 → Merging（已授权）/ AwaitMerge`、`MergeGate 出队冲突 → Implement`；§5.6 Bug Loop 补出队冲突 → 修复轮；
  §8.1 新增「合并队列」小节（D1–D4、D8）；§14 `task.merge` 的 `cancel` 参数与返回值、`task_json.merge` 字段。
- 协议正文不改：会话看不到队列与锁，出队冲突复用 `RebaseConflict` 注入，提示词与金样不变。

## 风险

- R1 同 tick 顺序：锁的正确性依赖「转 `Merging` 后锁仍在」与「对齐和开跑在同一次调用里」，不依赖 tick 内任务的处理顺序。
  M-01 的 e2e 用「X 先建、同 tick 收走 X 再处理 Y」的顺序做负向对照（去掉锁检查必红）。
- R2 准备命令：出队 rebase 改了 `watch` 文件时要重跑准备命令。准备命令由别的任务连占名额的边角（准备跑完前 main 又被外部推进
  且改了 `watch`）会让它重新排到队尾——可接受，记 Backlog B-03。已授权任务从取资格起持锁，本项目的合并不会在这段间隙里推进 main；
  未授权任务没有锁，仍按 G5 显示过期。M-01 有专门用例钉住常见路径。
- R3 暂停后恢复的持锁者不会补拿锁：恢复后作业已存在，收结果时不持锁；若期间 main 前进，走 T-10 回退。可接受（D8 兜底），记 B-02。
- R4 读路径调 git：`task.list` 对每个停在合并卡点的任务一次 `rev-list --count`；失败降级为 `null`。
- R5 `apps/web` / `apps/desktop` 对任务 JSON 或事件种类的穷举：新增字段 / 事件若使其测试变红，实现轮最小修补；合并前门禁
  （`sh scripts/ci-local.sh`）会跑它们。
- R6 改名 `Transition.merge_requested` → `grants_merge_request` 触及多处域测试；集中在 M-02 一次改完，避免半新半旧的含义。

## 验收与测试

### 验证安排

- 每个实现轮：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`，加本里程碑的验收命令，退出码写进证据
  （验收 8 的前两条）。全量门禁由宿主在合并前跑，会话不跑。
- 新测试一律带里程碑前缀，验收命令按前缀过滤，前缀在调研时没有任何既有测试名命中：M-01 `merge_queue_`、M-02 `merge_auth_`、
  M-03 `merge_conflict_`、M-04 `merge_view_`。
- 调度器单测放新文件 `crates/automed/src/scheduler_merge_queue_tests.rs`，像 `scheduler_gate_queue_tests.rs` 一样被
  `scheduler::tests` `include!`。
- e2e 都在 `crates/automed/tests/end_to_end.rs`，真实 git / worktree / wrapper / `sh -c`，只替换模型 CLI。门禁脚本在被调用的那一刻
  把 `$AUTOME_GATE_NODE $AUTOME_TASK $(git rev-parse HEAD) $(git rev-parse main)` 追加到记录文件，事后逐行做
  `git merge-base --is-ancestor <main> <HEAD>`；脚本按 `release-<任务>` 文件放行（沿用 `queued_gate_world` 的写法），
  `AUTOME_GATE_NODE=main_check` 时直接放行，免得负向对照里 MainCheck 真跑时卡住。
- 「恰好 3 次」与 MainCheck 的区分：记录文件恰好 3 行且全是 `merge_gate`；账本里 `node = merge_gate` 的 `gate.started` 恰好 3 条、
  `node = main_check` 的 `gate.reused` 恰好 3 条、`node = main_check` 的 `gate.started` 为 0。MainCheck 沿用开关保持缺省（开），
  不关。

### 验收标准 → 里程碑

| 任务验收 | 覆盖 |
|---|---|
| 1 三任务恰好 3 次门禁、祖先、无回退 | M-01（`[merge] auto` 变体）、M-02（预先同意变体） |
| 2 过期显示 + 按合并后 rebase + 门禁 1 次 | M-04 |
| 3 预先同意、撤销、不跳过设计卡点 | M-02 |
| 4 出队冲突回实现 / 修复轮、让位、授权保留 | M-03（授权寿命在 M-02） |
| 5 临界区内按合并不插队、A 失败后释放 | M-01 |
| 6 `[merge] auto` 解析、缺省、警告、`save_project` | M-01（解析、缺省）、M-02（警告、`save_project`） |
| 7 重启恢复 | M-01 |
| 8 fmt / clippy / ci-local | 每个实现轮跑前两条；第三条由合并前门禁 |

### 既有测试处置（任务文件 §7.2 逐个）

| 测试 | 处置 | 理由 | 里程碑 |
|---|---|---|---|
| task.rs `a_conflicting_requested_rebase_goes_back_to_implement_and_drops_the_request` | 改名 `merge_auth_a_conflicting_requested_rebase_goes_back_to_implement`；回实现轮与注入的断言保留，`!merge_requested` 改为 `!grants_merge_request` | 转换不授予授权；授权保留由账本决定，在调度器用例里断言 | M-02 |
| task.rs `a_failed_merge_gate_on_a_requested_merge_goes_back_to_implement_and_drops_the_request` | 改名 `merge_auth_a_failed_merge_gate_on_a_requested_merge_goes_back_to_implement`，同上 | 同上 | M-02 |
| task.rs `the_request_survives_the_queue_and_a_pause_but_not_a_stop` | 改写为 `merge_auth_a_request_reaches_the_merge_after_a_stop_and_a_conflict`：经停止 / 恢复 / 冲突回实现后，带授权的 `MergeGate` 通过仍进 `Merging` | 「停止丢授权」与 D2 相反 | M-02 |
| task.rs `a_rebase_entered_from_anywhere_but_the_merge_carries_no_request` | 改写为 `merge_auth_only_a_behind_merge_grants_a_request`：遍历节点与触发器，只有 `(Merging, Behind)` 授予 | 原前提（授权只活在合并尾段）被 D2 取消 | M-02 |
| task.rs `without_a_request_rebase_and_merge_gate_still_stop_at_await_merge` | 保留（只随字段改名） | 未授权仍停在合并卡点 | M-02 |
| task.rs `behind_is_only_accepted_from_merging` | 保留 | 不变 | — |
| scheduler.rs `a_conflicting_requested_rebase_goes_back_to_implement_and_clears_the_request` | 改写为 `merge_auth_a_conflicting_requested_rebase_keeps_the_request`：最后一条 `merge.auto` 仍 `active: true`，没有新增 `active: false` | 验收 4 | M-02 |
| scheduler.rs `a_rebase_nobody_asked_to_merge_records_no_merge_rebased` | 保留 | `Rebase` 节点未授权不写 `merge.rebased` 不变 | — |
| scheduler.rs `with_every_slot_taken_the_requested_rebase_waits_in_the_queue_and_keeps_the_request` | 保留 | 不变 | — |
| scheduler.rs `a_requested_rebase_records_merge_rebased_and_merges_without_stopping` | 保留；若它数到「离开尾段的 `active: false`」，删去这一项 | D2 不再写它 | M-02 |
| scheduler.rs `a_merge_behind_main_is_requeued_for_a_rebase_and_main_is_untouched` | 保留（`merge.auto` 仍恰好 1 条，payload 多 `by`） | T-10 不变 | — |
| e2e `a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again` | 改写为 `merge_auth_a_gate_failing_after_the_automatic_rebase_keeps_the_request_and_merges_after_the_repair`：失败那次 `gate.result` 的 sha 仍等于 `merge.rebased` 的 head；修好后 B 自动合并、`merge.direct { by: "behind" }` 1 条 | 兼容性变化 2 | M-02 |
| e2e `two_tasks_waiting_to_merge_both_land_and_the_second_rebases_through_the_gate_by_itself` | 保留；期望里若含离开尾段的 `merge.auto`，删去 | 兼容性变化 1 | M-02 |
| e2e `main_moving_after_the_press_takes_the_same_automatic_path` | 同上 | 同上 | M-02 |
| 其它 `gate_trace` 期望（如 reuse-off 用例里第二个 `merge.auto`） | 删去离开尾段的那一个 `merge.auto` | 同上 | M-02 |

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 合并队列核心：`[merge] auto` 读取、出队对齐、项目合并临界区 | 0 | merge-queue |
| M-02 | 已完成 | 授权寿命与预先同意 / 撤销、配置警告 | 0 | merge-auth |
| M-03 | 已完成 | 出队 rebase 冲突回实现 / 修复轮并让位 | 0 | merge-conflict |
| M-04 | 开放 | 过期显示与合并队列的输出 | 0 | merge-view |
| M-05 | 开放 | README 与技术设计文档 | 0 | docs |

## 里程碑范围

### M-01 范围

最新证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r2-audit.md · 通过 · 审计 #1

- 结果：`[merge] auto = true` 的项目里，三个同时到达合并前门禁的任务各跑一次门禁、依次合并，没有落后回退；临界区内被按合并的
  任务等持锁者，持锁者门禁失败后它继续；重启后照旧。
- 覆盖：G1、G2、G4（读取与缺省）、G6；验收 1（auto 变体）、5、6（解析与缺省）、7。
- 前置：无。
- 内容：D1 全部（含第 1 步的合并资格 `merge_queue::turn`；出队冲突在 M-01 期间走转换表现有的通用分支、任务以 rebase 冲突失败；M-03 改成回实现 / 修复轮，M-01 的用例
  不制造冲突）；D2 中的有效授权读取与 `[merge] auto` 解析（寿命改造留 M-02）；D3、D4；D5 的事件写入（渲染留 M-04）；D8 的兜底不变。
  注：M-01 期间 `advance` 的自动清除仍在，但它的「原来有授权」只看账本记录、不看项目开关（否则 `auto = true` 的项目每次转换都会
  写一条 `active: false`）；它写的不带 `by` 的 `active: false` 按 D2 回落到项目开关，与 `[merge] auto` 变体不冲突。M-02 删掉它。

### M-02 范围

最新证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-02-r3-audit.md · 通过 · 审计 #2

- 结果：任意未完成未失败的任务可预先同意合并并在合并卡点不停；`--cancel` 后停；设计卡点不跳过；授权不再随离开尾段丢失；
  无门禁时有配置警告；`save_project` 保留 `[merge]`。
- 覆盖：G3、G4（警告、保存）；验收 1（预先同意变体）、3、6（警告、`save_project`），并为验收 4 的「授权保留」打底。
- 前置：M-01。
- 内容：D2 全部（`merge_press`、`cancel`、CLI `--cancel`、字段改名、删自动清除、`config::warnings`）；既有测试处置表里标 M-02 的各行。

### M-03 范围

最新证据：docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-audit.md · 通过 · 审计 #3

- 结果：出队 rebase 冲突时任务回实现轮（Bug 回修复轮），名额当场让给下一个，授权保留，修好后重新排队并自动合并。
- 覆盖：G1（冲突分支）；验收 4。
- 前置：M-01、M-02。
- 内容：D6。

### M-04 范围

- 结果：停在合并卡点而 main 已前进的任务在 `task.get` / `status` / `ls` 上显示落后 N 个提交与「合并时会重新 rebase 并再跑一次
  门禁」；按合并后 rebase + 门禁 1 次 + 合并；`autome run` 对新事件各打一行；`status` 显示配置警告。
- 覆盖：G5、G7；验收 2。
- 前置：M-01、M-02。
- 内容：D5 渲染、D7。

### M-05 范围

- 结果：README 与技术设计文档写明合并队列的行为、临界区范围、`[merge] auto` 的取舍与 `task.merge --cancel`。
- 覆盖：G8。
- 前置：M-01 – M-04（文档写实现后的真实行为）。
- 内容：「方案 · 文档与协议」所列各节。

## 里程碑验收契约

### M-01 验收契约

- 验收命令：`cargo test -p autome-domain merge_queue_ && cargo test -p automed --lib merge_queue_ && cargo test -p automed --test end_to_end merge_queue_`
- 预期：共 19 个用例通过（domain 4：`merge_queue_effective_authorization_follows_the_ledger_then_the_project`、`merge_queue_holder_needs_a_lock_and_a_section_node`、`merge_queue_leaving_the_section_releases_the_lock`、`merge_queue_turn_follows_the_holder_then_the_queue_order`；automed lib 11：`merge_queue_auto_is_read_and_defaults_false`、`merge_queue_auto_rejects_non_booleans`、`merge_queue_a_dequeued_gate_rebases_onto_the_moved_main_first`、`merge_queue_an_up_to_date_branch_is_not_rebased_at_dequeue`、`merge_queue_reuse_requires_an_up_to_date_branch`、`merge_queue_a_dequeue_rebase_that_changes_setup_inputs_reruns_setup_first`（断言重跑期间本任务持锁）、`merge_queue_a_second_merge_gate_waits_outside_the_host_queue_while_the_section_is_held`、`merge_queue_a_merge_waits_while_another_task_holds_the_section`、`merge_queue_every_exit_releases_the_section`、`merge_queue_a_pending_setup_after_the_dequeue_rebase_keeps_the_section`、`merge_queue_a_reusable_gate_does_not_jump_an_earlier_queued_task`；e2e 4：`merge_queue_three_auto_merging_tasks_run_the_gate_exactly_three_times`、`merge_queue_a_press_during_the_section_waits_and_continues_after_a_gate_failure`、`merge_queue_the_section_and_the_order_survive_a_core_restart`、`merge_queue_with_two_gate_slots_one_project_still_merges_one_at_a_time`）；三任务用例里记录文件恰好 3 行且每行 main 是 HEAD 的祖先，`merge.auto` 事件 0 条。
  新增的两个 lib 用例：`..._a_pending_setup_...` 让 B 出队 rebase 后的准备作业受控阻塞、C 已就绪，断言 B 全程持锁、C 在 B 合并前没有拿锁、
  `gate.started` 与合并的先后是 B 先 C 后；`..._a_reusable_gate_...` 安排「创建顺序与入队顺序相反、持锁者门禁失败、后入队者可沿用」，
  断言先入队的 B 先拿锁，C 的 `gate.reused` 与 `merge.locked` 都晚于 B 的 `merge.locked`。
- 负向对照：去掉 D1 第 1 步的锁检查（只留出队对齐）时，`merge_queue_three_auto_merging_tasks_run_the_gate_exactly_three_times` 失败——Y 在 X 转 `Merging` 的同一 tick 开跑，门禁调用多于 3 次或出现 `Merging → Behind` 回退（`auto = true` 变体里授权本来就有效，不会再写 `merge.auto`，所以不以该事件为证据）；去掉出队对齐（只留锁）时同一用例的祖先断言失败；去掉 `Merging` 等锁时 `merge_queue_a_press_during_the_section_waits_and_continues_after_a_gate_failure` 失败（A 门禁未结束 main 就前进了）；把拿锁推迟到出队 rebase 之后（旧顺序）时 `..._a_pending_setup_...` 失败；沿用路径绕过 `turn` 的 seq 检查时 `..._a_reusable_gate_...` 失败。

### M-02 验收契约

- 验收命令：`cargo test -p autome-domain merge_auth_ && cargo test -p automed --lib merge_auth_ && cargo test -p autome-cli merge_auth_ && cargo test -p automed --test end_to_end merge_auth_`
- 预期：共 17 个用例通过（domain 6：`merge_auth_merge_press_by_state`（同时覆盖 `cancel_press`）、`merge_auth_only_a_behind_merge_grants_a_request`、`merge_auth_a_request_reaches_the_merge_after_a_stop_and_a_conflict`、`merge_auth_config_warns_when_auto_has_no_gate`、`merge_auth_a_conflicting_requested_rebase_goes_back_to_implement`、`merge_auth_a_failed_merge_gate_on_a_requested_merge_goes_back_to_implement`；automed lib 6：`merge_auth_cancel_is_rejected_once_the_merge_has_started`（`Merging` 带授权时 `cancel` 被拒、授权记录不动、随后照常合并；`MergeGate` 持锁时 `cancel` 写 `merge.unlocked`）、`merge_auth_auto_and_after_survive_save_project`、`merge_auth_a_conflicting_requested_rebase_keeps_the_request`、`merge_auth_leaving_the_merge_tail_writes_no_revocation`、`merge_auth_cancel_overrides_the_project_switch`、`merge_auth_task_merge_rejects_failed_and_finished_tasks`；autome-cli 1：`merge_auth_merge_cancel_is_parsed`；e2e 4：`merge_auth_a_preauthorized_task_merges_without_stopping_and_a_cancelled_one_stops`、`merge_auth_preauthorizing_before_design_approval_does_not_skip_it`、`merge_auth_three_preauthorized_tasks_run_the_gate_exactly_three_times`、`merge_auth_a_gate_failing_after_the_automatic_rebase_keeps_the_request_and_merges_after_the_repair`）。
- 负向对照：保留 `advance` 里离开尾段的自动清除时，`merge_auth_a_gate_failing_after_the_automatic_rebase_keeps_the_request_and_merges_after_the_repair` 停在 `await_merge` 而失败，`merge_auth_leaving_the_merge_tail_writes_no_revocation` 也失败；把预先同意实现成对 `AwaitDesignApproval` 应用 `Approve` 时 `merge_auth_preauthorizing_before_design_approval_does_not_skip_it` 失败；`cancel_press` 接受 `Merging` 时 `merge_auth_cancel_is_rejected_once_the_merge_has_started` 与 `merge_auth_merge_press_by_state` 失败；`merge_press` 接受 `Failed` 时 `merge_auth_merge_press_by_state` 与 `merge_auth_task_merge_rejects_failed_and_finished_tasks` 失败。

### M-03 验收契约

- 验收命令：`cargo test -p autome-domain merge_conflict_ && cargo test -p automed --lib merge_conflict_ && cargo test -p automed --test end_to_end merge_conflict_`
- 预期：共 6 个用例通过（domain 2：`merge_conflict_a_dequeue_conflict_goes_back_to_implement`、`merge_conflict_a_bug_dequeue_conflict_goes_back_to_fix`；automed lib 2：`merge_conflict_a_dequeue_conflict_yields_the_queue_and_counts_the_streak`、`merge_conflict_bug_streak_and_budget_stop_before_launching_another_fix`；e2e 2：`merge_conflict_a_dequeue_conflict_returns_to_implement_keeps_the_request_and_merges_after_the_repair`、`merge_conflict_a_bug_dequeue_conflict_returns_to_the_fix_round`）；需求用例里第三个任务的 `gate.started` 早于冲突任务修好后的那次 `gate.started`。
- 负向对照：让 `(MergeGate, Conflict)` 落进通用分支变成 `Failed` 时两个 domain 用例与两个 e2e 用例失败；冲突后不让出名额（例如在 rebase 前先写 `gate.started` 或保留排队条目）时 `merge_conflict_a_dequeue_conflict_yields_the_queue_and_counts_the_streak` 失败。

### M-04 验收契约

- 验收命令：`cargo test -p automed --lib merge_view_ && cargo test -p autome-cli merge_view_ && cargo test -p automed --test end_to_end merge_view_`
- 预期：共 6 个用例通过（automed lib 3：`merge_view_behind_count_counts_commits_only_on_the_base`、`merge_view_task_get_reports_a_stale_await_merge`、`merge_view_dashboard_reports_authorization_waiting_and_warnings`；autome-cli 2：`merge_view_event_lines_for_the_merge_queue`、`merge_view_state_suffixes_for_ls_and_status`；e2e 1：`merge_view_a_stale_await_merge_shows_behind_and_one_gate_run_merges_it`）；e2e 里 `stale.behind` 等于测试自己用 `git rev-list --count` 独立算出的数，按合并后该任务 `node = merge_gate` 的 `gate.started` 恰好 1 条。
- 负向对照：`behind_count` 把范围写反成 `<base>..<branch>` 时 `merge_view_behind_count_counts_commits_only_on_the_base` 与 e2e 用例数字不符而失败；不限定 `AwaitMerge` 就输出 `stale` 时 `merge_view_task_get_reports_a_stale_await_merge` 失败。

### M-05 验收契约

- 验收命令：`grep -q '^auto = ' README.md && grep -q 'Merge queue' README.md && grep -q 'merge cart --cancel' README.md && grep -q '合并队列' docs/development/technical-design.md && grep -q 'merge.locked' docs/development/technical-design.md && grep -q 'MergeGate 出队冲突' docs/development/technical-design.md && echo docs-ok`
- 预期：输出 `docs-ok`，退出码 0（6 处检查全部命中）。
- 负向对照：README 没加 Merge queue 一段或技术设计没写临界区事件时，对应 `grep -q` 返回 1，命令不输出 `docs-ok`。

## 人工验收清单

- H-01 在装好的 `autome` 上找一个停在「等你合并」、main 已前进的任务，`autome status <任务>` 与 `autome ls` 的「结果已过期（落后 N 个提交）」文案读起来清楚、不挤行。
- H-02 Web 面板与桌面端打开带 `merge` 字段的任务卡、`autome run` 出现新事件时，界面不报错、不出现空白卡片。

## Backlog

- B-01 合并列车：把队列里几个已授权任务叠在一起只跑一次门禁、失败再二分（任务卡明确不做）。
- B-02 暂停后恢复的持锁者不补拿锁（R3）；需要时可在收结果时检查是否落后、落后则作废重跑。
- B-03 准备命令跑完前 main 被外部推进且改了 `watch` 文件时，任务重新排到队尾（R2）。
- B-05 持锁者在宿主队列里等名额、或等自己的准备作业时，同项目其它已授权任务都在等锁（D1 第 1 步的取舍：保证 FIFO 且不让对齐被插队）；若实测等待过长，可改为只在准入通过后拿锁并让准备命令在锁外重跑。
- B-04 界面（Web / 桌面）展示「已预先同意合并」「等 T-y 合并」与撤销按钮。

## 争议项
