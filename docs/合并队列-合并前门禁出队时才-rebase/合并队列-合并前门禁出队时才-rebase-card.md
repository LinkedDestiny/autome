# 合并队列：合并前门禁出队时才 rebase，通过后在同一段临界区里合并，别让每次合并把其它任务的门禁结果作废

## 背景

T-61 之后本仓库用 `[gate] run = "before_merge"`，T-47 让宿主门禁同一时间只跑 1 个。两者叠加出一个新的浪费：**合并前门禁的结果只对「跑的那一刻的 main」有效，而任何一个任务合并都会让 main 前进**。

2026-11-13 05:31 的现场：T-46 停在「等你合并」（门禁已过），T-43 的合并前门禁正在跑，T-49、T-44 的合并前门禁在排队，四个分支都基于同一个 main。此时无论先合并谁，其余三个的结果（已出的、在跑的、在排队的）全部过期：

- 排队中的门禁仍会在旧基线上跑完（每次 5–11 分钟），通过后按合并 → 内核发现分支落后 → 重新排队 rebase → 再排一次门禁；
- 它重跑期间只要又有别的任务合并，再作废一次。

四个任务最坏要跑约 10 次合并前门禁，理想情况只需 4 次。操作者当时只能手工串行化：`autome stop` 掉排队的三个，合并一个，再逐个 `autome rerun <任务> rebase`。任务一多这就是新的「门禁抢占」。

## 要做什么

1. **出队时才 rebase。** 合并前门禁（需求 Loop 的 MergeGate、Bug Loop 的 MergeGate）在宿主任务队列里轮到自己时，先检查任务分支是否落后默认分支；落后就在这时 rebase（沿用现有 rebase 步骤与冲突处理：有冲突照旧退回实现 / 修复轮，并让出门禁位置），然后才在 rebase 后的提交上跑门禁。入队时不再提前 rebase，或入队时 rebase 了、出队时发现又落后就再 rebase 一次——口径由设计轮定准，要求是「门禁开始跑的那个提交一定基于当时最新的默认分支」。
2. **已授权合并的任务，门禁通过后立刻合并，中间不让别的合并插队。** 任务带着 `merge_requested`（用户已经按过 `autome merge`、因分支落后被送回 rebase 的那种）走到合并前门禁时，「出队 rebase → 门禁 → 合并」作为项目内的一段临界区：这段时间里同一项目不开始别的合并，也不让另一个合并前门禁在它之前完成合并。门禁通过直接进 `Merging`，不会再报落后。
3. **预先授权合并。** `autome merge <任务>` 允许在任务到达合并卡点之前就按下（任务处于实现、审计、复盘、rebase、合并前门禁、排队等任何未完成且未失败的状态）：记下 `merge_requested`，任务走到合并卡点时不停，直接进合并。`autome status` / `task.get` 显示「已预先同意合并」；提供撤销（`autome merge <任务> --cancel` 或等价 RPC）。设计卡点不受影响：没批准设计的任务按 merge 只记授权、不跳过设计批准。
4. **项目级自动合并（可选开关）。** `.autome/config.toml` 新增 `[merge] auto = true`（缺省 false）：打开后每个任务通过合并前门禁就直接合并，等价于每个任务都预先授权。没配 `[gate]` 时打开要给一条配置警告（没有门禁把关的自动合并），不报错。README 写明取舍。
5. **停在合并卡点的任务，结果过期要看得见。** 任务在「等你合并」而默认分支已经前进时，`autome status` / `task.get` / `autome ls` 标明「门禁结果基于旧的 main（落后 N 个提交），合并时会重新 rebase 并再跑一次门禁」，让用户知道按下去不是立刻合并。
6. 队列里等合并的顺序：按进入合并前门禁队列的先后（FIFO），与 T-47 的队列一致；T-53（排队任务调整顺序）如果已合并，沿用它的顺序口径，不另造。
7. 事件与输出：新增或复用事件说明「出队时 rebase 到 main @ sha」「门禁通过，直接合并（已授权）」；`autome run` 打可读的一行。
8. 文档：README Host gate 一节与 `docs/development/technical-design.md` 写明合并队列的行为、临界区范围、`[merge] auto` 的取舍。协议正文不需要改；如果设计认为要改，按 `protocol/CHANGELOG.md` 的规矩发新版本并带独立的 `## protocol/vN` 标题。

## 不做什么

- 不做「合并列车」（把几个任务叠在一起只跑一次门禁、失败再二分）——先把串行队列做对；留 Backlog。
- 不改 `[gate] concurrency` 的语义；并发大于 1 时临界区仍然保证同一项目的合并串行。
- 不改 MainCheck 的沿用规则（T-60）。
- 不改设计批准卡点。

## 验收标准

1. e2e：三个任务同时走到合并前门禁、都已预先授权合并（或 `[merge] auto = true`）→ 三个都合并，门禁脚本恰好被调用 3 次；每次调用时的 HEAD 都以当时的默认分支为祖先；没有任何一次「按合并时默认分支已前进」的回退。
2. e2e：任务 A 停在合并卡点（门禁已过），任务 B 合并使 main 前进 → `task.get` / `status` 显示 A 的结果已过期与落后提交数；对 A 按合并 → rebase + 门禁 1 次 → 合并。
3. e2e：预先授权 → 任务走完后不停在合并卡点直接合并；撤销授权后停在卡点；设计未批准时预先授权不跳过设计卡点。
4. e2e：出队 rebase 遇到冲突 → 退回实现轮（Bug 为修复轮），门禁位置让给下一个任务，冲突解决后重新排队；授权保留。
5. e2e：临界区内（A 正在「门禁 → 合并」）对 B 按合并，B 不会在 A 之前完成合并；A 门禁失败时临界区释放，B 继续。
6. 单测：`[merge] auto` 的解析、缺省、无 `[gate]` 时的警告；`save_project` 不改写用户的 `[merge]`（与 T-43 的 `[merge] after` 共存，键不冲突）。
7. 驱动重启后队列顺序、授权与临界区状态从账本恢复，不丢、不死锁（有测试）。
8. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿，前两条由实现轮在会话里跑并记录退出码。

## 参考

- `crates/autome-domain/src/task.rs`：`press_merge`、`merge_requested`、`(Node::Merging, CoreStepResult::Behind)` → `Requeued { resume: Rebase }`、`(Node::Rebase, Ok)` → `MergeGate`。
- `crates/autome-domain/src/bug_loop.rs`：Bug 的 rebase / MergeGate / 合并。
- `crates/automed/src/scheduler.rs`：`run_gate`、T-47 的宿主任务队列（`gate.queued`）、合并步骤。
- `crates/automed/src/git.rs`：`MergeCheck::Blocked { kind: Behind }`。
- T-43（`[merge] after` 钩子）、T-47（门禁排队）、T-60（MainCheck 沿用）、T-61（`before_merge`）、T-51（approve / merge / rerun 绕过并行上限）。
