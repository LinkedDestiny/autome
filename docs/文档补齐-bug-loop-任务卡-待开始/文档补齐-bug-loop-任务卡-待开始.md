# 文档补齐：Bug Loop、任务卡与待开始、reply 及 [bug]/[setup]/same_failure_limit 配置

status: 实现中
design-round: 1/15
implementation-round: 3/15
current-milestone: M-03
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-03 重跑全量门禁，核实 Electron SIGABRT 与本地监听 EPERM 环境阻塞解除并比对用例总数后报待审

任务文件：`docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始-task.md`（目标、范围、硬性约束与验收标准以它为准；下文 `任务 §n` 指它的小节，`要求 n` 指任务 §3 的条目，`验收 n` 指任务 §4 的条目，`约束 n` 指任务 §5 的条目）。

## 背景与现状

基线：分支与 main 的 merge-base `6fb7ef1`（任务文件写的 `cf522fb` 是其后的协议固定提交，两份目标文档在两者上相同）。以下事实均由本轮在基线代码上读出，出处见 D4–D6 的锚点。

- **README.md（399 行，英文）。** `### Host gate, retries and cooldowns` 的 TOML 示例与说明**已经写了** `[gate] same_failure_limit`（默认 3、1–10、计数节点 gate/merge_gate/fix_gate、`autome rerun` / `autome extend` 之后重新计数）与 `[setup]`（Preparation 条目）。经核对与代码一致（D6）。**没有** `[bug]`、`fix_rounds`。`## The command line` 的示例没有 `--kind`、`--brief`、`--draft`、`task start`、`reply`；`rerun`、`extend` 只在末段散文里出现，`autome rerun` / `autome extend` 两个字面只出现在 Host gate 一节。
- **docs/development/technical-design.md（360 行，中文）。** §5.1–§5.5 只覆盖需求 Loop，没有 `Draft`、没有 Bug 节点；§8.1 已有 `[setup]` 宿主准备、`same_failure_limit` 熔断（含 `fix_gate`、`repro_check` 不参与），经核对与代码一致（D6）；§9 已有 `[setup]` 校验规则，没有 `[bug]`；§14 命令清单没有 `task.start`、`task.reply`。全文没有 `kind`、任务卡、`fix_rounds`。
- **代码事实摘要。** Bug Loop 的转换全部在 `crates/autome-domain/src/bug_loop.rs` 的 `apply_for`；七个 Bug 节点在 `task.rs` 的 `Node`；复现检查、修复门禁、合并前门禁的命令拼装在 `crates/automed/src/scheduler.rs` 的 `run_gate`；复现未中计数与修复轮计数由 `scheduler.rs` 的 `bug_progress` 从事件表数出；任务卡正文与 `draft` 由 `dispatch.rs` 的 `task_create` 校验，正文存 `task_briefs` 表，首个会话前由 `write_task_inputs` 写成 `<slug>-card.md` 并提交；`task.reply` 对需求任务等同 `task.reject`。
- **对门禁的影响。** 本轮 `grep` 过 `crates/`、`apps/`、`scripts/`：没有任何源码或测试读取仓库根 `README.md` 或 `docs/development/technical-design.md`（命中的只有 `protocol/README.md` 的 `include_str!` 与两处指向 `docs/plans/` 的注释）；`scripts/ci-local.sh` 不检查文档。改这两份文档不应改变任何测试结果，以 M-03 的全量门禁为证。

## 目标与非目标

**目标。**

- G1（要求 1）README 的命令行一节补 `task new --kind bug --brief - --draft --json`、`task start`、`reply`、`extend`、`rerun`；配置一节补 `[bug] fix_rounds`；`[setup]`、`[gate] same_failure_limit` 已有且正确，只核对不重写。
- G2（要求 2）technical-design §5 加 Bug Loop 的节点与转换（含三点：126/127 不算失败、连续两次未复现进入等你补充信息、合并前门禁总带复现命令），§5.1 补 `Draft`；§8 加任务卡正文、附件与 `Draft`；§9 补 `[bug] fix_rounds` 的解析规则；§14 补 `task.start`、`task.reply` 与 `task.create` 的新参数。
- G3（要求 3）新增的每一条行为陈述都在 D4/D5 有一行「陈述 → 代码文件 + grep 锚点」，审计轮逐行核对（验收 2）。

**非目标。**（约束 1、2）

- 不改代码、测试、usage 文本；发现的代码问题进 Backlog。
- 不改两份文档的已有行（新增全部是追加的行；见 D7 的「删除行 = 0」检查），不改已有章节号，不动 `## The Loop` 的流程图，不补任务卡没点名的主题（§14 的其它漂移、§4 领域模型、§3.3 示例）——这些进 Backlog。
- technical-design 不重复写 `same_failure_limit`：§8.1 已有且正确（D6 的 V-06）。

## 设计

### D1 落点总表

| 文档 | 落点 | 方式 | 内容（行号见 D4/D5） | 里程碑 |
|---|---|---|---|---|
| technical-design | §5.1 末尾 | 追加一段 | `Draft` 状态与 `Start`/取消（T-01..T-03） | M-01 |
| technical-design | §5.5 之后、`## 6.` 之前 | 新小节 `### 5.6 Bug Loop（kind = bug）` | 适用范围、节点表、转换表、三点、预算、熔断、重跑（T-04..T-35） | M-01 |
| technical-design | §8.1 之后、`## 9.` 之前 | 新小节 `### 8.2 任务卡正文、附件与待开始（Draft）` | 建卡参数、存储、落盘提交、开始（T-40..T-51） | M-02 |
| technical-design | §9 末尾（`## 10.` 之前） | 追加一段 | `[bug] fix_rounds`（T-52、T-53） | M-02 |
| technical-design | §14「命令：」段之后 | 追加一段 | `task.start`、`task.reply`、`task.create` 的 `kind`/`brief`/`draft`（T-54..T-56） | M-02 |
| README | `## The command line` 末尾（`## The protocol` 之前） | 追加段落 + 一个 `sh` 代码块 | 建 Bug 任务、任务卡、草稿、`--json`、start/reply/rerun/extend（R-01..R-11） | M-03 |
| README | Host gate 一节 TOML 示例 `[setup]` 块之后 | 追加 `[bug]` 两行 | `fix_rounds = 3`，注释写默认与 1–20（R-12） | M-03 |
| README | Host gate 一节 Preparation 条目之后 | 追加一个条目 `**Bug fix rounds.**` | 修复轮预算、extend、bug 门禁带复现命令（R-12..R-14） | M-03 |

新增标题只有两个，逐字为 `### 5.6 Bug Loop（kind = bug）` 与 `### 8.2 任务卡正文、附件与待开始（Draft）`。新增内容里除这两行外不得有以 `#` 开头的行（代码块里的注释写在行尾），否则 D7 的标题检查会报。

### D2 代码位置怎么呈现（任务 §8 待定 2）

- **technical-design（开发者文档）：行内出处。** §5.6、§8.2、§9 新段、§14 新段里每一段或每一行表格以 `出处：` 结尾，写「文件路径 + 符号名」（函数、枚举变体、常量、测试名），如 ``出处：`crates/autome-domain/src/bug_loop.rs` 的 `apply_for`、测试 `bug_loop_second_consecutive_miss_waits_for_info` ``。不写行号（约束 4）。§5.1 追加段同样。
- **README（面向用户，英文）：不放源码路径。** 对照关系只在本设计 D5 的 R 表里，审计轮按 R 表逐行核对。
- **陈述与锚点的约束。** 新增文本里的每一条行为陈述必须对应 D4/D5 的一行；表外的行为不写（约束 3）。实现轮确需表外陈述时，先在表里补一行（锚点必须能被 D7 解析），在证据文件里写明理由，再写进文档；不得改动既有行的陈述。过渡句、指路句（如「需求任务的转换见 §5.3」）不算行为陈述。
- 代码里的中文字面量（错误信息、标签）引用时保持原样（约束 5）。

### D3 写法要点

- **§5.6 的结构。** 依次：适用范围（T-04..T-06、T-10）→ 节点表（节点、线上名、界面标签、谁来跑、是否占槽位；T-07..T-09）→ 转换表（与 §5.3 同列：当前 · 触发 · 条件 · 下一；T-11..T-33）→ 「三点」各一段加粗小标题，逐字包含 `126`、`127`、`124`、`连续两次`、`等你补充信息`、`( 复现命令 ) && ( 门禁命令 )`、`MergeGate`（T-15..T-21）→ 修复轮预算（T-27、T-28、T-36）→ Bug 文档字段与 prompt 模板（T-34、T-35）。
- **「失败」一词的口径。** 任务卡写「复现命令退出码 126/127 不算失败」「连续两次复现检查失败」。代码里复现检查要的是命令**失败**（退出码非零）才算复现成功，所以文档要把两个口径说清：126/127 与超时、退出码 0 一样**不算复现成功**（`why` 分别是 `not_runnable`、`timed_out`、`exited_zero`）；「连续两次复现检查失败」指连续两次未复现。
- **reply 的两种语义**（任务 §8 待定 5）：Bug Loop 任务停在 `await_info` / `not_a_bug` 时，`reply` 以非空补充信息回到定位；其它节点的 Bug 任务被拒；需求任务的 `reply` 在派发层被换成 `task.reject`（意见原样）。README 与 §14 都按此写。
- **README 的 Bug Loop 介绍只写一段**，放在命令行一节里 `--kind bug` 的说明处（不改 `## The Loop`）：一句话流程 locate → reproduce → fix → fix gate → review → rebase → merge gate → merge，以及需要开启 audit 角色。

### D4 陈述对照表：technical-design

列：ID · 落点 · 陈述（文档必须表达的事实）· 代码文件 · grep 锚点（`grep -F` 能在该文件里找到的字面）。D7 的脚本按列读这张表，单元格里不得出现竖线。

| ID | 落点 | 陈述 | 文件 | 锚点 |
|---|---|---|---|---|
| T-01 | §5.1 | `Draft`（界面「待开始」）不占槽位、不算等用户、不是终态、没有节点 | crates/autome-domain/src/task.rs | `fn draft_holds_no_slot_does_not_await_the_user_and_is_not_terminal` |
| T-02 | §5.1 | 只有 `Draft` 接受 `Start`，转为 `Queued`；其它状态拒绝「只有待开始的任务可以开始」 | crates/autome-domain/src/task.rs | `Err(reject("只有待开始的任务可以开始"))` |
| T-03 | §5.1 | `Draft` 可以直接取消；暂停被拒「任务尚未开始」，会话、门禁等其它触发也都被拒 | crates/autome-domain/src/task.rs | `fn a_draft_refuses_every_other_trigger` |
| T-04 | §5.6 | 任务类型 `kind` 只有 `feature`（默认）与 `bug` | crates/autome-domain/src/task.rs | `pub enum TaskKind` |
| T-05 | §5.6 | 走 Bug Loop 的条件：`kind = bug`，且已记过 `bug.loop_started` 事件或仍处于 `Draft`/`Queued` | crates/autome-domain/src/bug_loop.rs | `pub fn uses_bug_loop` |
| T-06 | §5.6 | 需求任务原样走 §5.3 的表；已经停在需求节点的旧 bug 任务也继续走需求表 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_legacy_bug_tasks_already_in_feature_nodes_keep_the_feature_table` |
| T-07 | §5.6 节点表 | 七个节点与线上名：`locate`、`repro_check`、`fix`、`fix_gate`、`fix_review`、`await_info`、`not_a_bug` | crates/autome-domain/src/task.rs | `Node::ReproCheck => "repro_check"` |
| T-08 | §5.6 节点表 | 界面标签：定位、复现检查、修复、修复门禁、复核、等你补充信息、等你决定（不是 Bug） | crates/autome-cli/src/render.rs | `"await_info" => "等你补充信息"` |
| T-09 | §5.6 节点表 | `locate`、`fix` 由实现角色跑会话，`fix_review` 由审计角色跑；`repro_check`、`fix_gate` 是内核门禁步骤；`await_info`、`not_a_bug` 等用户、不占槽位 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_nodes_have_wire_names_roles_and_flags` |
| T-10 | §5.6 | Bug 任务要求审计角色开启：`Start` 或分到槽位时审计关闭即拒「Bug 任务需要独立复核，请启用审计角色后启动」 | crates/autome-domain/src/bug_loop.rs | `Bug 任务需要独立复核，请启用审计角色后启动` |
| T-11 | §5.6 转换 | `Queued` 分到槽位 → `locate`；此时记 `bug.loop_started` | crates/automed/src/scheduler.rs | `.append_event("bug.loop_started", task_id, json!({}))?;` |
| T-12 | §5.6 转换 | `locate` 结束：status 已定位 → `repro_check`；无法复现 → `await_info`；不是 Bug → `not_a_bug` | crates/autome-domain/src/bug_loop.rs | `BugStatus::CannotReproduce => Transition::active(Node::AwaitInfo, Action::None),` |
| T-13 | §5.6 转换 | 会话文档校验：`locate` 以外的轮次 status 必须是已定位；`复核` 字段只有 `fix_review` 轮不得是待复核，其它轮必须是待复核；不符即 `Failed(Protocol)` | crates/autome-domain/src/bug_loop.rs | `Some("复核 字段不符合当前轮次")` |
| T-14 | §5.6 三点 | `repro_check` 只跑复现命令（取 `bug.located` 事件里的命令）；超时取 `[gate] timeout_minutes`，未配置门禁时 30 分钟 | crates/automed/src/scheduler.rs | `_ => repro.to_owned(),` |
| T-15 | §5.6 三点 | 退出码非 0、不是 126/127、且未超时，才算复现成功 → 开修复轮；退出码 0、126、127 或超时（门禁超时退出码 124）都不算复现，原因记为 `exited_zero` / `not_runnable` / `timed_out` | crates/autome-domain/src/bug_loop.rs | `![126, 127].contains(exit_code)` |
| T-16 | §5.6 三点 | 同上的测试出处 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced` |
| T-17 | §5.6 三点 | 超时退出码常量为 124 | crates/automed/src/gate.rs | `pub const TIMED_OUT: i32 = 124;` |
| T-18 | §5.6 三点 | 第一次未复现 → 回 `locate`，注入命令、退出码、原因与日志尾；连续第二次未复现 → `await_info`（等你补充信息） | crates/autome-domain/src/bug_loop.rs | `if progress.repro_misses >= 1 {` |
| T-19 | §5.6 三点 | 「连续」的计数 = 最近一次 `task.replied` 之后的 `bug.repro_missed` 事件数，所以 reply 之后重新计数 | crates/automed/src/scheduler.rs | `.count_events_after(&task.id, "bug.repro_missed", "task.replied")?,` |
| T-20 | §5.6 三点 | 同上的测试出处 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_second_consecutive_miss_waits_for_info` |
| T-21 | §5.6 三点 | `fix_gate` 与 `MergeGate` 跑复现命令（取 `bug.reproduced` 事件里的）；项目配置了 `[gate]` 时跑 `( 复现命令 ) && ( 门禁命令 )`，否则只跑复现命令——合并前的两道门禁总带复现命令；`MainCheck` 只跑项目门禁 | crates/automed/src/scheduler.rs | `format!("( {repro} ) && ( {} )", gate.command)` |
| T-22 | §5.6 转换 | `fix` 结束总进 `fix_gate`，与是否配置 `[gate]` 无关 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_fix_always_runs_the_fix_gate` |
| T-23 | §5.6 转换 | `fix_gate` 通过 → `fix_review`；失败 → `fix`，注入门禁失败（命令与日志尾） | crates/autome-domain/src/bug_loop.rs | `GateOutcome::Passed if *node == Node::FixGate => {` |
| T-24 | §5.6 转换 | `fix_review` 复核通过 → `Rebase`（不经 `Retro`）；打回 → `fix`，注入打回 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_review_pass_goes_to_rebase_without_retro` |
| T-25 | §5.6 转换 | `Rebase` 成功总进 `MergeGate`；冲突 → `fix`，注入冲突文件 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_rebase_always_runs_the_merge_gate` |
| T-26 | §5.6 转换 | `MergeGate` 失败 → `fix`；通过后与需求任务共用 `AwaitMerge` 以后的尾段 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_merge_tail_is_shared_with_the_feature_loop` |
| T-27 | §5.6 预算 | 每开一个 `fix` 会话（含手动重跑）记一条 `bug.fix_round_started`；上限 = 任务的预算，未设时取 `[bug] fix_rounds`；正常自动转换（门禁失败、复核打回、rebase 冲突、合并门禁失败、定位完成后首次开修复）在开下一修复轮前检查，已开轮数达到上限即 `Failed(FixBudget)` | crates/autome-domain/src/bug_loop.rs | `let limit = ctx.budget_n.unwrap_or(ctx.config.bug.fix_rounds);` |
| T-28 | §5.6 预算 | 预算失败后追加 n 轮（n > 0）：上限加 n，直接开 `fix` | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_extend_resumes_fix_with_a_larger_limit` |
| T-29 | §5.6 转换 | 停在 `await_info` / `not_a_bug` 时收到非空补充信息 → `locate`；空文本拒「补充信息不能为空」；不在这两个节点时拒绝 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_reply_is_refused_when_empty_or_not_waiting` |
| T-30 | §5.6 转换 | 两个等待节点可以取消，不能暂停、停止或追加轮次 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend` |
| T-31 | §5.6 转换 | 失败或停止的 Bug 任务只能从 `locate`、`fix`、`fix_review`、`rebase` 重跑 | crates/autome-domain/src/bug_loop.rs | `Bug 任务只能从定位、修复、复核或 rebase 重跑` |
| T-32 | §5.6 转换 | `fix_gate` / `MergeGate` 同因失败熔断后，从同一门禁节点 rerun 或追加轮次都丢弃旧门禁结果重跑；追加轮次同时抬高修复轮上限 | crates/autome-domain/src/bug_loop.rs | `fn same_failure_bug_rerun_and_extend` |
| T-33 | §5.6 转换 | 停在 `repro_check` / `fix_gate` 的任务恢复后重新排队，丢弃被杀的门禁结果；会话崩溃、文档不可解析、缺文件、守卫失败的处理与需求会话相同 | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_stopped_bug_gates_discard_runs_and_requeued_bug_nodes_reenter` |
| T-34 | §5.6 文档 | Bug 文档字段：status（已定位 / 无法复现 / 不是 Bug）、fix-round、复现命令、根因、复核（待复核 / 通过 / 打回） | crates/autome-domain/src/bug_doc.rs | `"不是 Bug" => BugStatus::NotABug,` |
| T-35 | §5.6 文档 | Bug 会话的 prompt 模板是 `protocol/prompts/locate.md`、`fix.md`、`fix-review.md`，规则在 `protocol/bug-protocol.md`，编译进 `automed` | crates/automed/src/protocol/mod.rs | `prompts/locate.md` |
| T-36 | §5.6 预算 | 手动 `rerun … fix` 不经上述预算检查：任务因 `FixBudget` 失败后重跑 `fix` 仍直接开修复会话，只计入已开轮数，不再触发 `Failed(FixBudget)`（上限仍是 T-28 追加后的值） | crates/autome-domain/src/bug_loop.rs | `fn bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only` |
| T-40 | §8.2 | `task.create` 的 `kind`：缺省 `feature`，只收 `feature` / `bug`，其它值拒 | crates/automed/src/dispatch.rs | `只能是 feature 或 bug` |
| T-41 | §8.2 | `brief`（任务卡正文）必须是字符串；去空白为空拒；超过 65536 字节拒 | crates/automed/src/dispatch.rs | `任务卡正文超过 64KB` |
| T-42 | §8.2 | 有 `brief` 没 `request` 时，标题取正文第一个 `# ` 行，没有则取第一非空行去掉前导 `#`；仍为空拒「任务卡正文缺少标题」；两者都给时 `request` 命名任务，正文原样保留 | crates/automed/src/dispatch.rs | `任务卡正文缺少标题：第一行写` |
| T-43 | §8.2 | `draft: true` 建为 `Draft`：不触发调度、不建 worktree，返回 `started: false`、`queue_position: null` | crates/automed/src/dispatch.rs | `fn a_draft_is_created_without_starting_or_a_queue_position_and_is_not_on_the_dashboard` |
| T-44 | §8.2 | 非草稿的 bug 任务在建卡时就检查审计角色；草稿不查，到开始时由 T-10 检查 | crates/automed/src/dispatch.rs | `if kind == TaskKind::Bug && !draft {` |
| T-45 | §8.2 | 正文原样存 SQLite 表 `task_briefs` | crates/automed/src/store.rs | `CREATE TABLE task_briefs (` |
| T-46 | §8.2 | `task.get` 返回 `brief`，没有正文时为显式 `null`；任务列表不带正文 | crates/automed/src/dispatch.rs | `fn archived_brief_remains_readable_and_lists_omit_it` |
| T-47 | §8.2 | 附件在建卡时逐个校验是文件，否则拒「附件不存在」 | crates/automed/src/dispatch.rs | `附件不存在：{a}` |
| T-48 | §8.2 | 首个会话前（建好任务检出后）写 `<doc_dir>/<slug>-card.md`（已存在不覆盖）与 `<doc_dir>/attachments/`（只在首次复制） | crates/automed/src/scheduler.rs | `fn write_task_inputs(` |
| T-49 | §8.2 | 两者以一条 `chore(autome): <任务 id> inputs` 提交进任务分支 | crates/automed/src/scheduler.rs | `&format!("chore(autome): {} inputs", task.id),` |
| T-50 | §8.2 | `task.start`：对任务施加 `Start`，记 `task.started` 事件，再调度一次；返回 `task`、`started`、`queue_position` | crates/automed/src/dispatch.rs | `fn task_start(ctx: &mut Ctx, task_id: &str) -> DispatchResult {` |
| T-51 | §8.2 | 草稿取消后归档，从未建过 worktree | crates/automed/src/dispatch.rs | `fn a_draft_is_cancelled_and_archived_without_a_worktree` |
| T-52 | §9 | 项目级 `[bug] fix_rounds`：缺省 3，必须是 1 到 20 的整数，否则「[bug] fix_rounds 必须是 1 到 20 的整数」 | crates/automed/src/config_io.rs | `[bug] fix_rounds 必须是 1 到 20 的整数` |
| T-53 | §9 | 默认值常量；`[bug]` 只在项目配置里有 | crates/autome-domain/src/config.rs | `pub const DEFAULT_FIX_ROUNDS: u32 = 3;` |
| T-54 | §14 | `task.reply {task_id, info}`：Bug Loop 任务 → 补充信息（空文本拒）；需求任务 → 等同 `task.reject`，`info` 作为驳回意见 | crates/automed/src/dispatch.rs | `Trigger::Reject { feedback: info }` |
| T-55 | §14 | `task.start {task_id}` 见 §8.2；它是写命令，不在 `READ_METHODS` 里 | crates/automed/src/dispatch.rs | `fn a_draft_is_refused_a_retro_and_task_start_is_not_a_read_method` |
| T-56 | §14 | `task.create` 另收 `kind`、`brief`、`draft`（§8.2） | crates/automed/src/dispatch.rs | `fn task_create(ctx: &mut Ctx, params: &Value) -> DispatchResult {` |

### D5 陈述对照表：README

| ID | 落点 | 陈述 | 文件 | 锚点 |
|---|---|---|---|---|
| R-01 | command line | `--kind bug` 建 Bug 任务，缺省是 feature | crates/autome-cli/src/main.rs | `params["kind"] = json!(kind);` |
| R-02 | command line | Bug 任务的一句话流程：locate、reproduce、fix、fix gate、review、rebase、merge gate、merge；需要开启 audit 角色 | crates/autome-domain/src/task.rs | `Node::FixGate => "fix_gate"` |
| R-03 | command line | `--brief <file>` 从文件读任务卡，`--brief -` 从标准输入读；必须是 UTF-8 | crates/autome-cli/src/main.rs | `任务卡正文不是 UTF-8 文本` |
| R-04 | command line | 只给 `--brief` 时任务标题取卡片第一个 `# ` 标题；空卡或超过 64 KB 被拒 | crates/automed/src/dispatch.rs | `任务卡正文缺少标题：第一行写` |
| R-05 | command line | `--draft` 只保存不开始，不占槽位，打印 `autome task start` 的提示 | crates/autome-cli/src/main.rs | `已存为待开始。用` |
| R-06 | command line | `--json` 输出 `task.create` 的原始结果：`task`、`started`、`queue_position`（草稿为 null） | crates/automed/src/dispatch.rs | `"queue_position": queue_position(ctx, &task)?,` |
| R-07 | command line | `autome task start <task>` 开始一个草稿，只接受草稿 | crates/autome-domain/src/task.rs | `只有待开始的任务可以开始` |
| R-08 | command line | `autome reply <task> "..."`：Bug 任务停在 `await_info` / `not_a_bug` 时带着补充信息回到定位；需求任务上等同 reject | crates/autome-cli/src/main.rs | `Bug 任务在等你补充信息/等你决定时用；需求任务等同 reject` |
| R-09 | command line | `autome rerun <task> <node>`：失败或已停止的任务从某节点重跑；Bug 任务只能从 locate、fix、fix_review、rebase（或熔断的那道门禁） | crates/autome-cli/src/main.rs | `"task.rerun_from"` |
| R-10 | command line | `autome extend <task> <n>`：给预算用完的任务追加 n 轮，n 为正整数；Bug 任务追加的是修复轮 | crates/autome-cli/src/main.rs | `轮数必须是正整数` |
| R-11 | command line | 程序化建卡的示例：`autome task new --kind bug --brief - --draft --json < card.md` | crates/autome-cli/src/main.rs | `std::io::stdin()` |
| R-12 | config | `[bug] fix_rounds = 3`：默认 3，1–20，写在项目 `.autome/config.toml` | crates/automed/src/config_io.rs | `[bug] fix_rounds 必须是 1 到 20 的整数` |
| R-13 | config | 每开一个修复轮计一次（含门禁失败、复核打回、rebase 冲突后的返工）；自动返工在开下一轮前检查，用完任务停下，`autome extend` 追加；手动 `autome rerun <task> fix` 不经该检查，只计入已开轮数 | crates/autome-domain/src/bug_loop.rs | `Transition::fail(at, FailureReason::FixBudget { limit })` |
| R-14 | config | Bug 任务的 fix_gate 与 merge_gate 跑复现命令，配置了 `[gate]` 时与它串联 | crates/automed/src/scheduler.rs | `format!("( {repro} ) && ( {} )", gate.command)` |

### D6 已有内容核对（只核对，不重写）

下列已有句子本轮已与代码核对一致。实现轮在 M-03（README，V-01..V-05）与 M-01（technical-design §8.1，V-06）证据里逐句复核 Host gate 一节的 gate 条目与 Preparation 条目、§8.1 熔断句；有不一致才改，改动的句子与理由写进证据，并按 D7 的例外规则处理删除行。

| ID | 落点 | 陈述 | 文件 | 锚点 |
|---|---|---|---|---|
| V-01 | README TOML | `same_failure_limit = 3` 是默认值 | crates/autome-domain/src/config.rs | `pub const DEFAULT_GATE_SAME_FAILURE_LIMIT: u32 = 3;` |
| V-02 | README TOML | 取值 1–10 | crates/automed/src/config_io.rs | `[gate] same_failure_limit 必须是 1 到 10 的整数` |
| V-03 | README gate 条目 | 计数节点是 `gate`、`merge_gate`、`fix_gate` | crates/autome-domain/src/gate_failure.rs | `pub fn counts(node: Node) -> bool {` |
| V-04 | README gate 条目 | 熔断之后的下一次运行从 1 重新计数（rerun / extend 之后 the streak starts over） | crates/autome-domain/src/gate_failure.rs | `!p.tripped` |
| V-05 | README TOML | `[setup] timeout_minutes` 缺省 30 | crates/autome-domain/src/config.rs | `pub const DEFAULT_SETUP_TIMEOUT_MINUTES: u32 = 30;` |
| V-06 | technical-design §8.1 | 熔断默认 3、整数 1–10，`repro_check` 与 `main_check` 不参与 | crates/autome-domain/src/gate_failure.rs | `matches!(node, Node::Gate` |

### D7 检查脚本与关键词清单（验收 1，任务 §8 待定 1）

关键词按「文档 + 小节区间」检查，不是全文 `grep`：基线上 `rerun`、`extend`、`[setup]`、`same_failure_limit` 已在 README 出现，全文 `grep` 没有鉴别力。区间用 `sed -n '/起始标题/,/下一标题/p'` 截取。

脚本还做五项结构检查：

1. **删除行 = 0**：两份文档相对 merge-base 的 `git diff --numstat` 删除数之和为 0（约束 2：只追加，不改已有行、不重排编号）。例外：D6 某行核对不符而必须改句时，实现轮在证据里列出被改的行与理由，审计轮确认后把这条检查判「已解释」；没有这种证据时删除行 ≠ 0 就是失败。
2. **新增标题行**：两份文档新增的以 `#` 开头的行只能是 D1 的两个标题。
3. **新小节位置**：`### 5.6` 在 `### 5.5 计数与预算` 与 `## 6. Git 工作区操作` 之间；`### 8.2` 在 `### 8.1 …` 与 `## 9. 配置解析` 之间。
4. **范围**：相对 merge-base 的改动与未跟踪文件，除本任务目录外只能是 `README.md` 与 `docs/development/technical-design.md`。
5. **锚点与出处**：D4/D5/D6 每一行的锚点都能在对应文件里 `grep -F` 到；technical-design 新增行里引用的每个 `crates/…​.rs` 路径都存在。

验收命令统一为（在 worktree 根目录执行，`M-0x` 换成里程碑号）：

`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-0x`

~~~sh check-docs
set -u
m=${1:-M-03}
slug=文档补齐-bug-loop-任务卡-待开始
dd=docs/$slug/$slug.md
td=docs/development/technical-design.md
rd=README.md
out=.autome/output
mkdir -p "$out"
base=$(git merge-base HEAD main)
n=0
t=0
pass() { t=$((t+1)); n=$((n+1)); }
fail() { t=$((t+1)); echo "FAIL $*"; }
has() {
  f=$1; a=$2; b=$3; shift 3
  sed -n "/$a/,/$b/p" "$f" > "$out/check-sec.txt"
  for k in "$@"; do
    if grep -qF -- "$k" "$out/check-sec.txt"; then pass; else fail "$f [$a] 缺 $k"; fi
  done
}
order() {
  f=$1; shift; prev=0
  for h in "$@"; do
    l=$(grep -nxF -- "$h" "$f" | head -1 | cut -d: -f1)
    if [ -z "$l" ] || [ "$l" -le "$prev" ]; then return 1; fi
    prev=$l
  done
}
h56='### 5.6 Bug Loop（kind = bug）'
h82='### 8.2 任务卡正文、附件与待开始（Draft）'

d=$(git diff --numstat "$base" -- "$rd" "$td" | awk '{s+=$2} END{print s+0}')
if [ "$d" = 0 ]; then pass; else fail "两份文档删除或改写了 $d 行"; fi
x=$(git diff -U0 "$base" -- "$rd" "$td" | grep -E '^\+#' | grep -vxF -e "+$h56" -e "+$h82")
if [ -z "$x" ]; then pass; else fail "多出的标题行：$x"; fi
x=$( { git -c core.quotePath=false diff --name-only "$base"; git -c core.quotePath=false ls-files --others --exclude-standard; } | grep -v "^docs/$slug/" | grep -vxF -e "$rd" -e "$td")
if [ -z "$x" ]; then pass; else fail "范围外改动：$x"; fi
grep -E '^\| [TRV]-[0-9]+ ' "$dd" > "$out/check-rows.txt"
while IFS='|' read -r _ id _ _ f a _; do
  f=$(printf '%s' "$f" | tr -d ' `')
  a=$(printf '%s' "$a" | sed -e 's/^ *`//' -e 's/` *$//')
  if grep -qF -- "$a" "$f"; then pass; else fail "锚点 $id 在 $f 找不到"; fi
done < "$out/check-rows.txt"
x=$(git diff -U0 "$base" -- "$td" | grep '^+' | grep -oE 'crates/[A-Za-z0-9_./-]+\.rs' | sort -u)
y=$(for p in $x; do [ -f "$p" ] || echo "$p"; done)
if [ -n "$x" ] && [ -z "$y" ]; then pass; else fail "出处路径缺失或不存在：[$y]"; fi

if order "$td" '### 5.5 计数与预算' "$h56" '## 6. Git 工作区操作'; then pass; else fail "5.6 不在 5.5 与 6 之间"; fi
has "$td" '^## 5\. ' '^## 6\. ' 'Draft' 'task.start' "$h56" 'kind' 'locate' 'repro_check' 'fix_gate' 'fix_review' 'await_info' 'not_a_bug' '126' '127' '124' '连续两次' '等你补充信息' 'bug.repro_missed' 'task.replied' '( 复现命令 ) && ( 门禁命令 )' 'MergeGate' 'fix_rounds' 'bug.fix_round_started' 'crates/autome-domain/src/bug_loop.rs' 'apply_for' 'crates/automed/src/scheduler.rs' 'run_gate'

if [ "$m" != M-01 ]; then
  if order "$td" '### 8.1 驱动锁、门禁、重试与冷却、表态租约' "$h82" '## 9. 配置解析'; then pass; else fail "8.2 不在 8.1 与 9 之间"; fi
  has "$td" '^## 8\. ' '^## 9\. ' "$h82" 'Draft' 'kind' 'brief' 'draft' '65536' 'task_briefs' '-card.md' 'attachments' 'inputs' 'task.start' 'autome task start' 'task.started' 'write_task_inputs' 'crates/automed/src/dispatch.rs' 'task_create' 'crates/automed/src/store.rs'
  has "$td" '^## 9\. ' '^## 10\. ' '[bug]' 'fix_rounds' '1 到 20' 'crates/automed/src/config_io.rs'
  has "$td" '^## 14\. ' '^## 15\. ' 'task.start' 'task.reply' 'task.reject' 'kind' 'brief' 'draft'
fi
if [ "$m" = M-03 ]; then
  has "$rd" '^## The command line' '^## The protocol' 'task new --kind bug --brief - --draft --json' '--kind' '--brief' '--draft' 'autome task start' 'autome reply' 'autome extend' 'autome rerun' 'feature' 'locate' 'fix_review' 'await_info' 'not_a_bug' 'queue_position' 'reject'
  has "$rd" '^### Host gate' '^## The command line' '[bug]' 'fix_rounds = 3' '1–20' '[setup]' 'same_failure_limit = 3' '1–10' 'fix_gate' 'merge_gate' 'autome extend'
fi

if [ "$n" = "$t" ]; then echo "$m PASS $n/$t"; else echo "$m FAIL $n/$t"; exit 1; fi
~~~

**检查条数**（锚点行 D4 53 + D5 14 + D6 6 = 73 条）：

- M-01：结构 5（删除行、标题、范围、出处路径、5.6 位置）+ 锚点 73 + §5 关键词 25 = **103**。
- M-02：M-01 的 103 + 8.2 位置 1 + §8 关键词 17 + §9 关键词 4 + §14 关键词 6 = **131**。
- M-03：M-02 的 131 + README 命令行 15 + Host gate 9 = **155**。

设计轮已在基线上跑过三个参数：锚点 73 条全部解析，删除行、标题、范围三项通过；`M-01 FAIL 77/103`（失败的是出处路径、5.6 位置与 §5 的 25 个关键词里的 24 个，`MergeGate` 基线已在 §5.1）、`M-02 FAIL 77/131`、`M-03 FAIL 84/155`（README 命令行区间缺 14 个、Host gate 区间缺 `[bug]`、`fix_rounds = 3`、`1–20`）。三个都在基线上红，说明检查有鉴别力。

### D8 关键词与验收 1 的对应

任务 §4 验收 1 点名的关键词全部落在 D7 的区间检查里：README——`--kind`、`--brief`、`--draft`、`task start`（`autome task start`）、`reply`、`extend`、`rerun`（命令行一节），`fix_rounds`、`[setup]`、`same_failure_limit`（Host gate 一节）；technical-design——Bug Loop（`### 5.6 Bug Loop（kind = bug）`）、`kind`（§5、§8、§14）、`Draft`（§5、§8）、`task.start`（§5、§8、§14）、`task.reply`（§14）、`fix_rounds`（§5、§9），三点（`126`/`127`、`连续两次` + `等你补充信息`、`( 复现命令 ) && ( 门禁命令 )` + `MergeGate`）。`same_failure_limit` 在 technical-design §8.1 已有（V-06），不新增。

## 风险

- **K1 陈述与代码不符**（最大风险，违反目标本身）。对策：D4/D5 每条陈述锚到具体符号，脚本保证锚点存在；审计轮逐行读锚点上下文核对语义（验收 2）。
- **K2 rebase 后代码变化使陈述过期。** 锚点检查每个里程碑都跑；rebase 后若锚点失效，审计轮退回对应里程碑。
- **K3 「失败」口径混淆**（任务卡的「126/127 不算失败」与代码的「命令失败才算复现」相反）。对策：D3 规定两种口径都写明。
- **K4 文档改动影响门禁。** 本轮已确认没有测试读取这两份文件；M-03 跑全量 `sh scripts/ci-local.sh`，并要求用例总数与基线相同。
- **K5 碰真实账本。** 本设计的所有陈述都来自源码，不需要运行 `autome`/`automed`；若实现轮确需运行，按约束 8 用隔离的 `AUTOME_HOME`/`AUTOMED_DB_PATH`。

## 验收与验证

- 每个里程碑用 D7 的同一个脚本，参数不同，检查累积（M-02 也重跑 M-01 的检查）。
- 验收 2（审计逐条核对）：审计轮对本里程碑涉及的 D4/D5/D6 行逐行打开锚点上下文，判断文档文字与代码一致，在审计证据里逐行写结论。
- 验收 3（门禁全绿）：宿主在每个实现轮后自动跑 `sh scripts/ci-local.sh`；M-03 的验收命令再显式跑一次并比对用例总数。
- 实现轮自审清单（协议「标待审之前」）在本任务的口径：情形表逐行 = D4/D5 逐行；失败分支 = 文档里写到的每条拒绝信息原样核对；全部迁移 = §5.6 转换表逐行对 `apply_for` 的匹配臂；域边界 = `fix_rounds` 1/20、`same_failure_limit` 1/10、正文 65536 字节、退出码 0/124/126/127。

### M-01 验收契约

最新证据：docs/文档补齐-bug-loop-任务卡-待开始/evidence/M-01-r1-impl.md · 审计通过 · 实现 #1（审计证据 evidence/M-01-r1-audit.md）

- 验收命令：`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-01`
- 预期：输出 `M-01 PASS 103/103`，退出码 0；基线上同一命令为 `M-01 FAIL 77/103`。
- 负向对照：把 Bug Loop 写成 `### 5.3.1` 或插到 §5.3 之前 → 「5.6 不在 5.5 与 6 之间」与「多出的标题行」失败；漏写退出码一点 → `缺 126`、`缺 127`；就地改写 §5.1 的状态列表行而不是追加段落 → 「删除或改写了 1 行」。

### M-02 验收契约

最新证据：docs/文档补齐-bug-loop-任务卡-待开始/evidence/M-02-r2-impl.md · 审计通过 · 实现 #2（审计证据 evidence/M-02-r2-audit.md）

- 验收命令：`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-02`
- 预期：输出 `M-02 PASS 131/131`，退出码 0（含 M-01 的 103 项）；基线上为 `M-02 FAIL 77/131`。
- 负向对照：把任务卡内容写进 §5 而不是 §8 → §8 区间 `缺 task_briefs`、`缺 -card.md`；把 `task.start, task.reply` 塞进 §14 原有的「命令：」行 → 「删除或改写了 1 行」；把 `[bug]` 规则写进 §8.1 而不是 §9 → §9 区间 `缺 1 到 20`。

### M-03 验收契约

最新证据：docs/文档补齐-bug-loop-任务卡-待开始/evidence/M-03-r3-impl.md · 文档检查通过，全量门禁环境阻塞，保持开放 · 实现 #3

- 验收命令：`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-03 && sh scripts/ci-local.sh > .autome/output/ci-local.txt 2>&1; echo "ci-local exit $?"; tail -25 .autome/output/ci-local.txt`
- 预期：输出 `M-03 PASS 155/155`（基线上为 `M-03 FAIL 84/155`）；`ci-local exit 0`，各步骤 ok；`cargo test`、`npm test`（desktop）、`apps/web` 测试的用例总数与基线相同（文档改动不增减用例），证据里写出这三个数和基线数。
- 负向对照：只在 Host gate 一节提 `autome rerun` / `autome extend`（即基线现状）→ 命令行区间 `缺 autome rerun`、`缺 autome extend`；在新代码块里写以 `#` 开头的注释行 → 「多出的标题行」；把 TOML 示例的 `same_failure_limit = 3` 改写 → 「删除或改写了 1 行」。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | technical-design §5.1 追加 Draft、新增 §5.6 Bug Loop（要求 2、3） | 0 | |
| M-02 | 已完成 | technical-design §8.2 任务卡与待开始、§9 [bug]、§14 命令（要求 2、3；前置 M-01） | 0 | |
| M-03 | 开放 | README 命令行与配置补齐、全量门禁（要求 1、3，验收 3；前置 M-02） | 0 | |

## 人工验收清单

- H-01 通读 README 新增的英文段落与示例命令，确认面向用户的措辞可以接受（不阻塞里程碑，合并前确认）。

## Backlog

- B-01 technical-design §14 的命令清单与 `dispatch.rs` 派发表还有其它出入（如 `task.retro`、`task.changes`、`project.probe`、`config.set_role`、`dashboard.get`、`events.since` 等），本任务只补点名的 `task.start`、`task.reply`。
- B-02 technical-design §4 领域模型的 Task 字段没有 `kind` 与任务卡正文。
- B-03 technical-design §3.3 配置示例只有 `[loop]`、`[roles.*]`，没有 `[gate]`、`[retry]`、`[setup]`、`[bug]`。
- B-04 CLI `USAGE` 末行只说门禁与重试在 `[gate]` / `[retry]` 里配置，没有提 `[setup]`、`[bug]`（代码，本任务不改）。
- B-05 README `## The Loop` 的流程图只画需求 Loop，没有 Bug Loop。
- B-06 README Host gate 一节开头说 "Three things the core does on its own"，其下已有四个条目（本任务再加一个），计数早已不符。
- B-07 手动 `autome rerun <task> fix` 不经修复轮预算检查（`bug_loop.rs` 的 `apply_for` 重跑分支直接 `session(BugStep::Fix…)`，不走 `start_fix`），超预算后仍能无限重跑；可能是代码缺陷，本任务按实际行为写文档（T-36、R-13），不修代码。来源 D1-P01。
- B-08 README R-04 只提第一个 `# ` 标题；T-42 已记录没有 `#` 标题时取第一非空行的回退，实现 M-03 时 README 可加一句回退说明。来源评审 S-01。
- B-09 M-03 验收命令里 `echo`/`tail` 在 `&&` 链之后，整条命令退出码可能来自 `tail`；实现轮应单独保存并核对检查与 CI 的退出码，以 `M-03 PASS 155/155` 与 `ci-local exit 0` 两行输出为准。来源评审 S-02。

## 争议项
