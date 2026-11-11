# Bug 修复 Loop：定位、复现检查、修复、门禁、复核（K2）

status: 实现中
design-round: 1/15
implementation-round: 5/30
current-milestone: M-05
current-milestone-reopens: 0
convergence-mode: normal
next-action: 推进 M-05 需求零变化与全量门禁

任务文件：`docs/按计划-4-2-节实现-bug-修复-loop/按计划-4-2-节实现-bug-修复-loop-task.md`。
本任务的规则见 `docs/按计划-4-2-节实现-bug-修复-loop/protocol/loop-protocol.md` 与
`docs/按计划-4-2-节实现-bug-修复-loop/protocol/session-protocol.md`。

下文用 **A1–A7** 指任务文件 §3 的七条验收标准，**C1–C12** 指 §5 的硬性约束，**Q1–Q11** 指 §7 的待定问题。
代码位置以基线 `81a4394` 为准。

## 背景与现状

K1 已合并：内核认识 `TaskKind { Feature, Bug }`、任务卡正文（账本 `task_briefs`，第一次会话前写进
`<doc_dir>/<slug>-card.md` 并提交）、`Draft` 与 `task.start`；命令行 `reply` 映射到 `task.reject`。
**`kind = bug` 的任务今天仍跑需求 Loop**：`Queued + SlotAvailable` 一律进 `Intake`（`task.rs:813`）。

调研确认、并直接决定了下面方案形状的事实：

- **F1 协议体积只剩 130 字节。** `protocol eval` 第 1 层把 `loop-protocol.md` 与 `session-protocol.md` 合计限制在
  `SIZE_BUDGET_BYTES = 24576`（`autome-domain/src/protocol.rs:52`，`SIZED_FILES` 只含这两份）；现在合计
  19323 + 5123 = 24446 字节。Bug 文档规范写不进这两份文件。
- **F2 `Node::ALL` 被既有测试按「需求 Loop 的节点集合」使用。** `requeued_dispatches_all_thirteen_nodes` 断言非等待节点
  恰好 13 个；`requeued_stopped_preserves_the_original_node` 只排除三个门禁节点；把新节点加进 `ALL` 会让这些
  测试因迭代集合变化而失败（C2 不允许）。
- **F3 需求转换表的会话结束分支对未知节点 `unreachable!()`**（`task.rs` `SessionEnded` 分支末尾）。Bug 节点绝不能
  落进这张表。
- **F4 门禁结果不带退出码。** `GateOutcome::Failed { head, command, tail }`；超时由 `gate.rs` 写成退出码
  `TIMED_OUT`。复现检查需要区分「真的失败」与「超时 / 命令不存在」。
- **F5 会话种类按角色映射节点的地方有三处**：`Reenter`（`scheduler.rs:1334` `node.role()` → `SessionKind::Role`）、
  冷却改道（`scheduler.rs:1523` `Node::ALL.find(role)`）、守卫（`guards.rs:106` 按 `round.role` 要证据文件）。定位与
  修复都用「实现」配置，必须有自己的会话种类，否则会被当成实现轮。
- **F6 注入（inject）跨崩溃重试与暂停恢复保留**：`session.started` 事件与 `task.resume_input` 存了 inject，
  `Reenter` 与重试按它重放（`scheduler.rs:1370–1385`）。
- **F7 合并前门禁会复用同一提交上已通过的 `gate.result`**（`scheduler.rs:2113`，按 sha 判断）。
- **F8 三个既有测试钉住了 K1 的过渡行为「Bug 任务跑设计循环」**：`end_to_end.rs`
  `task_card_a_drafted_bug_waits_then_starts_with_its_card_committed_before_the_first_session`、`cli.rs`
  `task_card_a_brief_piped_on_stdin_creates_a_bug_draft_that_starts_with_its_card`、`cli.rs`
  `task_card_reply_rejects_the_design_with_the_text_verbatim` 的 `bug` 一半。它们与本任务目标直接冲突（见方案 §11）。
- **F9 真实账本里可能已有 K1 创建、正在跑需求 Loop 的 Bug 任务**（C4：已安装的 `autome` 与本机账本共用）。
  升级后它们不能被半路改走 Bug Loop。
- **F10 版本号被两处既有测试钉为 v1**：`protocol/mod.rs` `the_version_in_force_is_the_newest_changelog_heading_over_the_seed`
  与 `end_to_end.rs` `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`。

## 目标与非目标

**目标**：`kind = bug` 的任务从开始起走参考文档 §4.2 的 Bug Loop（定位 → 复现检查 → 修复 → 门禁 → 复核 →
rebase → 合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）；`kind = feature` 的任务行为与今天逐字节一致。
A1–A7 各有里程碑覆盖（见「里程碑说明」）。

**非目标**：

- `apps/`（W5 阶段映射、W7「改为需求」按钮与修复轮显示）、`plugins/`、`scripts/`、`.github/`、`.autome/`、
  `docs/development/`、`docs/plans/`。
- 新角色；按类型的看板列；需求 Loop 的任何行为变化；`autome ls` 类型标记等 K2 之后的 Backlog。
- 工作区项目沿用现有的成员仓库机制，Bug 文档带可选的 `repos:` 行（§2、§7），不新增工作区能力。
- 修改需求 Loop 的协议正文、既有 prompt、`brief-map.toml`：一字不改（F1、C2）。
- 账本迁移：所有新信息落在事件与已有 JSON 列里，`SCHEMA_VERSION` 保持 5（C4）。

## 方案

### 1. 节点与转换（Q1、Q2、A1）

**新增 7 个 `Node` 取值，不复用 `Implement`/`Gate`/`Audit`。** 理由：复用就得让需求转换表按 `kind` 分支，等于在
需求路径里动刀（C2）；独立节点让 W5 可以只看节点名映射阶段，不必同时看类型。

| 线上取值 | 中文 | 性质 | 角色配置 | 占槽 | 等用户 | §4.3 阶段 |
|---|---|---|---|---|---|---|
| `locate` | 定位 | 会话 | 实现 | 是 | 否 | 定位 |
| `repro_check` | 复现检查 | 内核步骤（门禁作业） | — | 是 | 否 | 定位 |
| `fix` | 修复 | 会话 | 实现 | 是 | 否 | 修复 |
| `fix_gate` | 修复门禁 | 内核步骤（门禁作业） | — | 是 | 否 | 修复 |
| `fix_review` | 复核 | 会话 | 审计 | 是 | 否 | 复核 |
| `await_info` | 等你补充信息 | 人工停顿 | — | 否 | 是 | 定位 |
| `not_a_bug` | 等你决定（不是 Bug） | 人工停顿 | — | 否 | 是 | 定位 |

`rebase`、`merge_gate`、`await_merge`、`merging`、`main_check`、`cleanup` 两条 Loop 共用（阶段「合并」），`Done`
为「完成」。阶段映射只写在本文档，由 W5 落到 `apps/`。

- `Node::ALL: [Node; 15]` **不变**，文档注释改为「需求 Loop 的全部节点」；新增 `Node::BUG: [Node; 7]`。
  `Node::parse` 先查 `ALL` 再查 `BUG`。新变体追加在枚举末尾，既有节点的排序不变（F2）。
- `Node::role()`：`locate`/`fix` → `Role::Impl`，`fix_review` → `Role::Audit`。`is_core_step`、`is_gate` 加入
  `repro_check`、`fix_gate`；`awaits_user` 加入两个等待节点，`occupies_slot` 随之排除它们。
- 「不是 Bug」**单独成节点**（Q2）：界面要给出不同的提问（补信息 vs 做决定）。两个等待节点允许的操作相同：
  `cancel`；`reply`（回到定位，补充信息随行，见 §5）；暂停、停止、追加轮次一律拒绝（沿用「等待用户的节点没有会话」
  的既有规则）。「改为需求」由用户用已有的 `task.create`（`kind = feature`、同一份 `brief`）新建，再取消本任务；
  内核不做特殊处理。

**转换表放在新模块 `autome-domain/src/bug_loop.rs`，需求转换表不动。**

- 入口 `bug_loop::apply_for(kind, state, trigger, ctx, progress, merge_requested)`。`kind` 走需求表时，函数体只有一行
  `task::apply_with_merge_request(state, trigger, ctx, merge_requested)`。调度器只调这个入口。
- `BugProgress { fix_round: u32, repro_misses: u32 }` 由调度器从账本事件算出后传入（§3、§4），不往 `Context` 加字段，
  所以既有测试里 5 处 `Context { … }` 字面量不用改。
- 需求表的入口 `apply_with_merge_request` 加一条防御，**位置在通用控制触发器（`Start`、`Cancel`、`Pause`、`Stop`、
  `Resume`，函数开头那段 `match trigger`）之后、`match (state, trigger)` 节点表之前**：状态里的节点属于
  `Node::BUG`，并且不是 `(Requeued, SlotAvailable)` 时直接 `Err`。需求任务永远不会处于这些节点，行为不变。
  - 通用控制段只依赖 `awaits_user`、`role()`、`is_enabled`，对 Bug 节点天然成立，所以 Bug 任务的取消（含两个等待节点）、
    暂停、停止、恢复都由它处理，Bug 表不重写。`Resume` 的 `DiscardGateRuns` 只认需求的三个门禁节点，所以
    `Stopped{repro_check / fix_gate} + Resume` 由 Bug 表先处理（上表倒数第二行）。
  - `(Requeued, SlotAvailable)` 的通用分支只用 `role()` 与 `is_core_step`（Bug 节点的取值见上），出队行为与需求相同，
    所以放行；其余一切节点表分支（含 F3 的 `unreachable!()`）Bug 节点都到不了。
- **旧 Bug 任务保持需求 Loop（F9）**：`bug_loop::uses_bug_loop(kind, state, started)` 为真当且仅当 `kind = Bug`，并且
  `state` 是 `Draft`/`Queued`，或者账本里已有本任务的 `bug.loop_started` 事件（`started`）。K1 期间已经开跑的 Bug 任务
  没有这个事件，继续走需求表直到结束。

新增类型（都是只增）：

- `Trigger::BugSessionEnded { outcome: BugSessionOutcome }`：`Ok { doc: Box<BugDoc> }`、`Crashed`、`Unparseable`、
  `MissingArtifact`、`GuardFailed`，处理方式与需求会话相同（崩溃先走既有重试，其余 → `Failed{Protocol}`）。
  用新触发器而不给 `SessionOutcome` 加变体，是为了让需求表一行不改：它的兜底分支会拒绝陌生触发器。
- `Trigger::ReproDone { head, command, exit_code, timed_out, tail }`：复现检查的结果，判定在领域层（§4）。
- `Trigger::Reply { info }`。
- `Action::StartBugSession { step: BugStep, inject: Option<Inject> }`，`BugStep { Locate, Fix, FixReview }`。
- `Inject::ReproNotFailing { head, command, exit_code, why, tail }`、`Inject::FixRejected`。修复门禁失败沿用
  `Inject::GateFailed`，rebase 冲突沿用 `Inject::RebaseConflict`。
- `FailureReason::FixBudget { limit }`。

Bug 转换表（未列出的组合拒绝；`start_fix` 见 §3）：

| 当前 | 触发 | 去向 / 动作 |
|---|---|---|
| `Queued` | `SlotAvailable` | `locate`，`StartBugSession{Locate}`。实现角色关闭 → `Failed{Config}` |
| `locate` | 会话结束，`status: 已定位` | `repro_check`，`RunCoreStep` |
| `locate` | 会话结束，`status: 无法复现` | `await_info` |
| `locate` | 会话结束，`status: 不是 Bug` | `not_a_bug` |
| `repro_check` | `ReproDone`，判为复现 | `start_fix(None)`：首次以 `[bug] fix_rounds` 设预算 |
| `repro_check` | `ReproDone`，判为未复现，且 `repro_misses + 1 < 2` | `locate`，注入 `ReproNotFailing` |
| `repro_check` | `ReproDone`，判为未复现，且 `repro_misses + 1 ≥ 2` | `await_info` |
| `fix` | 会话结束 | `fix_gate`，`RunCoreStep`（无论有没有 `[gate]`） |
| `fix_gate` | `GateDone` 通过 | 审计角色开启 → `fix_review`；关闭 → `rebase`（沿用 C-05 的「关掉评测方就跳过」） |
| `fix_gate` | `GateDone` 失败 | `start_fix(GateFailed)` |
| `fix_review` | 会话结束，`复核: 通过` | `rebase`（没有复盘） |
| `fix_review` | 会话结束，`复核: 打回` | `start_fix(FixRejected)` |
| `rebase` | 成功 | `merge_gate`（Bug 任务总跑，见 §4） |
| `rebase` | 冲突 | `start_fix(RebaseConflict)` |
| `merge_gate` | 失败 | `start_fix(GateFailed)` |
| `merge_gate` 通过、`await_merge`、`merging`、`main_check`、`cleanup` | — | 委托需求表，与需求任务相同，`merge_requested` 规则沿用 |
| `await_info` / `not_a_bug` | `Reply`，正文非空 | `locate`，`StartBugSession{Locate}` |
| `Failed{FixBudget{limit}}` | `ExtendBudget{extra > 0}` | `fix`，预算改为 `limit + extra` |
| `Failed` / `Stopped` | `RerunFrom{locate / fix / fix_review / rebase}` | 对应节点；其它节点拒绝 |
| `Stopped{repro_check / fix_gate}` | `Resume` | `Requeued` + `DiscardGateRuns` |
| 取消、暂停、停止、恢复（上一行除外）、`Requeued + SlotAvailable` | — | 委托需求表：控制段在防御之前处理，出队分支被防御放行 |

### 2. Bug 文档与状态块（Q3）

路径 `<doc_dir>/<slug>-bug.md`，`doc_dir` 取 `TaskRecord::doc_dir()`，新增 `TaskRecord::bug_doc()`，不硬编码 `docs/`。

```text
# <一句话标题>

status: 已定位
fix-round: 0/3
复现命令: cargo test -p automed some_test
根因: 一句话
复核: 待复核
repos: 仓库A, 仓库B
```

`repos` 是**可选**的第六个键，只有工作区项目的 Bug 文档才写（§7）；单仓库项目省略这一行。

状态块之后是五个二级标题小节：「现象」「复现」「根因分析」由定位轮写，「修复说明」由修复轮写，「复核意见」由复核轮写。

解析规则（`autome-domain/src/bug_doc.rs`，纯函数）：

- 只看第一个 `## ` 标题之前的行；代码围栏（```）内的行忽略。
- 键必须是 `status`、`fix-round`、`复现命令`、`根因`、`复核` 五个必填键或可选键 `repos` 之一，在行首；分隔符半角
  `:` 或全角 `：` 都接受（参考文档两种都用了），其后的空白去掉。五个必填键各出现恰好一次，`repos` 至多一次；
  `repos` 的值是逗号分隔的仓库名（半角、全角逗号都接受，去空白、去空项），解析成 `BugDoc.repos: Vec<String>`，缺省为空。
- `status` ∈ {`已定位`, `无法复现`, `不是 Bug`}；`复核` ∈ {`待复核`, `通过`, `打回`}；`fix-round` 形如 `k/N`（非负整数）。
- `status: 已定位` 时 `复现命令` 与 `根因` 必须非空且不是 `无`；其它状态可写 `无`。`复现命令` 是单行。
- 任何一条不满足 → `BugDocError`（指明键或行号）→ `Unparseable` → `Failed{Protocol}`（C5）。

各轮写什么，内核在会话结束后检查什么（不满足 → `GuardFailed` → `Failed{Protocol}`）：

| 字段 | 定位写 | 修复写 | 复核写 | 内核检查 |
|---|---|---|---|---|
| `status` | 三选一 | 保持 `已定位` | 保持 `已定位` | 修复、复核之后必须是 `已定位` |
| `fix-round` | `0/N` | 预算行给出的 `k/N` | 不改 | 只查格式。**以账本为准**，数值写错不判失败 |
| `复现命令` | 已定位时必填 | 不改 | 不改 | 内核执行的是复现检查记录下的命令（§4），文档后来的改动不生效 |
| `根因` | 已定位时必填 | 可修正 | 不改 | 定位之后必须非空 |
| `复核` | `待复核` | **重置为 `待复核`** | `通过` / `打回` | 定位、修复之后必须是 `待复核`；复核之后必须是二者之一 |
| `repos` | 工作区项目写（§7） | 可追加，不删 | 不改 | 工作区项目里 `已定位` 必须非空且都是成员名（§7） |

「修复必须重置 `复核`」是为了防止陈旧结论：合并前门禁失败退回修复后，如果上一轮的 `通过` 留在文档里，复核轮
即使什么也没写也会被当成通过。

规范写进**新文件 `protocol/bug-protocol.md`**，状态块示例与取值放在 kernel-contract 区 `bug-status-block` 里
（`regions()` 扫描所有协议文件，期望哈希从种子自动派生，`contract.toml` 自动生成，见 §10）。不写进
`session-protocol.md`/`loop-protocol.md` 的理由是 F1，而且需求任务的会话读的协议正文因此一字不变。

### 3. `fix-round` 的计数与预算（Q4、A5）

- **计数以账本为准**：调度器执行 `StartBugSession{Fix}` 时追加事件 `bug.fix_round_started { round, limit }`；
  `progress.fix_round` = 本任务这种事件的条数。崩溃重试与 `Reenter` 不经过这个动作，所以不计数。
- **上限**存在已有的 `tasks.budget_n` 列：第一次 `start_fix` 用 `with_budget(config.bug.fix_rounds)` 写入，`extend`
  写入 `limit + extra`。`task_json` 的 `budget_n` 对 Bug 任务因此就是修复轮上限；需求任务的含义不变。
  需求任务的 `implementation-round` 与 `compute_budget` 对 Bug 任务不适用。
- `start_fix(inject)`：`limit = ctx.budget_n.unwrap_or(config.bug.fix_rounds)`；`progress.fix_round ≥ limit` →
  `Failed{at: 当前节点, FixBudget{limit}}`；否则进入 `fix`。每一次进入修复都算一轮：复现之后、门禁失败、复核打回、
  rebase 冲突、合并前门禁失败、追加轮次之后。`RerunFrom{fix}` 是用户的显式操作，不查预算，但同样计一轮。
- **定位轮不设上限**：没有从修复回到定位的边。每次用户补充信息之后，最多跑两轮定位：未复现一次回到定位，
  连续第二次就停下等用户（§4）。所以定位环路总是止于人工停顿或修复。
- `[bug] fix_rounds`：只在项目配置里有，与 `[gate]` 一样。取 1 到 20 的整数，缺省为 3。
  - 领域层：`ProjectConfig.bug: Option<BugConfig>`，`ResolvedConfig.bug: BugConfig`，常量
    `DEFAULT_FIX_ROUNDS = 3`。
  - 读写：`config_io.rs` 的 `parse_project` / `save_project`，取值越界时报「`[bug] fix_rounds 必须是 1 到 20 的整数`」。
  - 不改本仓库的 `.autome/config.toml`（C1），测试用临时项目配置。

### 4. 复现检查与门禁的执行（Q5、Q6、A3）

三种 Bug 门禁都走 `run_gate` 与 `gate.rs` 的同一条路径（C6）：
- 用 `sh -c` 执行，工作目录是 `layout.cwd`，超时到点就杀掉整个进程组；
- 日志写到 `.autome/output/gates/<task>/<node>-<key>.*`；
- 发出 `gate.started` / `gate.result` 事件；
- HEAD 一旦移动，本次结果作废。

超时取 `[gate].timeout_minutes`，项目没配 `[gate]` 时取 `DEFAULT_GATE_TIMEOUT_MINUTES`（30）。不另设
`[bug]` 超时（Backlog B-06）。

- **复现检查 `repro_check`**：
  - 在定位轮留下的提交上（会话结束后 `sweep_commit` 已提交残留改动），执行 Bug 文档里的 `复现命令`。
  - 领域层判定「复现」当且仅当退出码非 0、没有超时、且不是 126/127（命令不可执行或不存在）。其余情况都判为
    「未复现」，`why` 取 `exited_zero` / `timed_out` / `not_runnable` 之一，原因写进注入与事件。
  - 调度器随后追加事件：判为复现时追加 `bug.reproduced { sha, command, exit_code }`，判为未复现时追加
    `bug.repro_missed { sha, command, exit_code, why, tail }`。
- **修复门禁 `fix_gate`** 在修复轮的提交上执行。项目配了 `[gate]` 时命令是 `( <R> ) && ( <[gate].command> )`，
  没配时只有 `<R>`。`R` 是最近一条 `bug.reproduced` 事件里记录的命令。复现命令先跑；两段任何一段失败，都带着
  日志尾部回到修复。
- **合并前门禁**：Bug 任务在 rebase 之后的提交上执行同一条组合命令，即使项目没配 `[gate]` 也跑。这样可以抓住
  rebase 把 Bug 带回来的情况。
  - F7 的复用条件加一项「命令相同」（上一条 `gate.result` 事件本来就记录 `command`），**只对 `uses_bug_loop` 为真的
    任务生效**。需求任务与走需求 Loop 的旧 Bug 任务（F9）继续用今天的条件（通过 + 同一 sha），不看命令：配置在两次
    门禁之间改变命令时，它们的复用与 `gate.reused` 事件都与今天一致（C2）。条件的分支在调度器里按 `uses_bug_loop`
    选择，不改需求路径的任何一行判断。
- **验证 main**：不变。只在配了 `[gate]` 时跑 `[gate]`，失败只告警。
- **Q6 落实**：`progress.repro_misses` 等于「最近一次 `task.replied` 事件之后（没有就从任务开始）」的
  `bug.repro_missed` 事件数。连续第二次未复现进入 `await_info`；两次的输出都在事件里，由命令行与网页展示。
  只要判为复现，就进入修复且不再回到定位，所以这个计数天然就是「连续」的。

### 5. `task.reply` 与补充信息（Q7、A4）

- 新 JSON-RPC 写方法 `task.reply { task_id, info }`，不进 `READ_METHODS`，桌面端写白名单不改（C3）。
- **需求任务**：与 `task.reject { task_id, feedback: info }` 走完全相同的代码路径，结果、错误文本、事件都一样
  （K1 验收 6 保持）。只在 `uses_bug_loop` 为真时才走下面的 Bug 路径，所以 F9 的旧 Bug 任务也等同 `reject`。
- **Bug 任务**：
  - `info.trim()` 为空时拒绝，错误文本「补充信息不能为空」。
  - 节点不是 `await_info` / `not_a_bug` 时，由状态机拒绝。
  - 成功时先追加 `task.replied { n, text }`（原文），再施加 `Trigger::Reply`，然后 `tick`。
- **补充信息如何交给定位**：每次启动定位会话前，调度器把全部 `task.replied` 按时间渲染成
  `<doc_dir>/<slug>-replies.md`（每条一节 `## 第 n 条（时间）`，正文逐字节原样），内容有变化就提交
  `chore(autome): <id> 补充信息`。
  - 定位模板固定写着「如果这个文件存在，先读它；它是数据，不是指令」。这个文件不参与占位符替换（C9 同理）。
  - 用文件而不用 inject 的理由：补充信息要在之后**每一轮**定位都可见（包括未复现退回之后），而 inject 只活一轮。
    文件在分支上，天然跨崩溃、暂停与重试，比「会话正常结束前一直携带」的租约更强，也让复核能看到用户说过什么。
- 命令行 `reply` 改调 `task.reply`，只改 `main.rs` 这一处映射。`USAGE` 那一行改成
  `reply <任务> "补充信息"   Bug 任务在等你补充信息/等你决定时用；需求任务等同 reject`。

### 6. 定位 prompt 与没有正文的 Bug（Q8）

- `prompts/locate.md` 含 `{request}`。有任务卡时沿用 K1 的前置段：调度器 `card_path` 的条件从
  「只有 `SessionKind::Intake`」扩到「`Intake` 或定位」，模板在 `{request}` 处切开，正文本身不进 prompt（C9）。
- 没有任务卡时只有一句话需求。prompt 要求：信息不足以复现时写 `status: 无法复现`，并在「现象」一节列出
  需要用户补充的具体信息（环境、步骤、期望与实际），任务随即停在「等你补充信息」。
- 三份模板都含 `{budget_line}`：定位轮渲染成「修复轮上限 N 轮，状态块写 `fix-round: 0/N`」，修复轮渲染成
  「本轮是修复第 k/N 轮」，复核轮渲染成「本轮复核修复第 k/N 轮」。
- 三份模板都不用 `{brief_path}`：Bug 会话不生成简报（§7）。

### 7. 没有任务整理轮时的准备工作（Q9）

- Bug Loop 里**没有**任务文件 `<slug>-task.md`、设计文档 `<slug>.md`、简报、`retro.md`、`lessons.md`。
- `launch_session` 照旧：
  - 第一个会话时建 worktree；
  - `freeze_protocol` 冻结 v2 副本，含 `bug-protocol.md`；
  - `write_task_inputs` 写入卡片与附件，并先于固定协议提交；
  - 定位会话前另写 `-replies.md`（§5）。
- Bug 会话**不写简报**，`brief-map.toml` 不改。需要的协议只有 `bug-protocol.md` 一份（prompt 直接点名）；本轮
  特有的材料（失败尾部、打回）走 inject；复核意见写在 Bug 文档里。省下这一层，也避免为 `brief-map` 引入
  新的键而改它的解析（它对未知角色键报错）。
- `read_outcome` 对 Bug 会话读 `-bug.md`，用 `bug_doc::parse` 解析。文件不存在时报 `MissingArtifact`。
- 其它会去读设计文档的路径都必须容忍 Bug 任务没有设计文档。这些路径包括 `task.get`、`dashboard.get`、
  完成时的指标、取消时的归档；由 M-03 的完整 e2e 证明。
  - 手动复盘 `task.retro` 对 Bug 任务直接拒绝，错误文本「Bug 任务没有复盘」。
- **工作区项目**（Q9 的仓库选择，沿用 `working_repos` 的「已有检出 ∪ 文档 `repos:` 行」机制，不新增存储）：
  - 仓库集合改读 Bug 文档的 `repos:` 行：任务有 `-bug.md` 就读它，否则读设计文档（走需求 Loop 的旧 Bug 任务）。
  - 定位还没写过文档、也没有任何检出时，Bug 任务的初始集合是**全部成员仓库**，让定位轮能在任何一个成员里复现并提交
    回归测试。
  - 定位以 `已定位` 结束时，守卫要求 `repos` 非空且都是成员名（否则 `GuardFailed → Failed{Protocol}`）；通过后，内核
    移除没被点名、工作区干净且相对基线没有提交的成员检出（沿用 `git worktree remove` 与分支清理的既有函数），此后集合
    按 `repos` ∪ 已有检出只增不减，合并、rebase、清理与需求任务走同一条路径。文档仓库成员始终在集合里。
  - 工作区里复现命令的工作目录是工作区目录（`layout.cwd`），命令自己 `cd <成员>`；`bug-protocol.md` 写明这一点。
  - 单仓库项目不受影响：不读 `repos`、不移除检出。

### 8. 会话种类、守卫、冷却与 SAME-MODEL（A6）

- `SessionKind::Bug { step }`，线上形状 `{"kind":"bug","step":"locate|fix|fix_review"}`。
  - `role()` 依次返回 Impl / Impl / Audit，所以运行时、模型、技能都取「实现」或「审计」的配置，不新增角色（D6）。
  - `label()` 是「定位 / 修复 / 复核」。
  - 轮次按种类各自编号（`next_round`）。
- 新增 `SessionKind::for_node(node)`，替换 F5 的三处映射：
  - `Reenter` 与崩溃重试按节点得到会话种类；
  - 冷却改道对 `Bug{step}` 直接取 `step` 对应的节点；
  - 需求节点的映射结果与今天相同。
- **守卫**：Bug 会话不进 `guards::check`（那是证据文件、里程碑表、`retro.md` 的检查），改跑 §2 那张表的检查。
- **模板选择**：`prompts/locate.md`、`prompts/fix.md`、`prompts/fix-review.md`，取自任务冻结的协议副本。
- **SAME-MODEL**：
  - 配置层：`config::validate` 的 Audit↔Impl 配对不变。修复与复核用的就是这两份配置，所以「修复与复核不得同
    `runtime:model`」与审计/实现完全同一条规则。每次 `advance` 与 `run_gate` 都调 `resolve_config`，违反即
    `Failed{Config}`，Bug 任务走同一路径。
  - 冷却改道：`KEPT_APART` 按角色判断，`Bug{Fix}` / `Bug{FixReview}` 的 `role()` 正好是 Impl / Audit，所以
    改道时不会把复核挪到修复所在的 CLI，反过来也一样。M-03 有单测钉住。

### 9. 事件、`task_json` 与命令行（Q10）

- 新事件：
  - `bug.loop_started {}`（`Queued → locate` 时）；
  - `bug.located { status, command, root_cause }`；
  - `bug.reproduced { sha, command, exit_code }`；
  - `bug.repro_missed { sha, command, exit_code, why, tail }`；
  - `bug.fix_round_started { round, limit }`；
  - `bug.reviewed { round, verdict }`；
  - `task.replied { n, text }`。
- `task_json`：**只对 Bug 任务**追加 `fix_round`（整数，等于 `bug.fix_round_started` 的条数），上限仍看
  `budget_n`。需求任务的输出键集合与今天完全一样（C2、C3）。理由：W7 要显示「修复第 2 / 3 轮」，从事件推导只需
  一次计数查询。
- `render.rs`：
  - `node_label` 加 7 个中文名。
  - `next_step`：`await_info` 提示 `autome reply <任务> "…"` 或 `autome cancel <任务>`；`not_a_bug` 提示取消，或
    `autome task new --kind feature --brief <卡片>` 改建需求任务，确认是 Bug 时用 `reply` 说明理由。
  - `event_line` 覆盖上面 7 个新事件。
  - `FixBudget` 的失败文本是「修复轮次用完（上限 N）」，并附 `autome extend` 提示。

### 10. 协议 v2 与 eval（Q11、A2）

- 新文件：
  - `protocol/bug-protocol.md`：Bug Loop 概览、Bug 文档、Bug 文档状态块（kernel-contract `bug-status-block`）、
    各轮写什么、复现命令的要求（单行；宿主在 worktree 根目录用 `sh -c` 执行，工作区项目里是工作区目录，命令自己 `cd <成员>`，并在 `repos:` 点名成员；修复前必须失败、修复后必须通过；
    不依赖网络与时序）、修复的边界（最小改动，不改回归测试）、复核三查（根因是否站得住、回归测试是否有意义、
    改动范围是否最小）。
  - `protocol/prompts/locate.md`、`fix.md`、`fix-review.md`，每份都含「不要启动下一个会话」与 `{budget_line}`。
- `CHANGELOG.md` 末尾加 `## protocol/v2`，四个 `behavioral` 块：
  - 三份 prompt 各一块，`eval` 指向各自的用例；
  - `bug-protocol.md` 一块，`eval` 指向定位用例；
  - 每块 `evidence` 都是 `[web-dashboard-via-loop §4.2, 按计划-4-2-节实现-bug-修复-loop 任务文件]`，满足
    「两个任务」的计数规则；
  - `predicted_impact` 都是 `{metric: protocol_failures, direction: flat, scope: task, horizon: 3}`。
- 三个新 eval 用例，`case.yaml` 新增可选键 `prompt:`，指定模板名，缺省取 `role`：
  - `locate-proves-the-bug-with-a-failing-test`：`role: impl`，`prompt: locate`；
  - `fix-leaves-the-regression-test-alone`：`role: impl`，`prompt: fix`；
  - `fix-review-rejects-a-weakened-test`：`role: audit`，`prompt: fix-review`。
  - 静态门检查它们的形状与模板存在；行为用例要花钱，不在验收里跑（列入人工验收清单）。
- 静态门（`eval.rs` / `phrases.rs` / `case.rs`）新增：
  - 新常量 `BUG_PROMPTS: [&str; 3]` 与必含短语「不要启动下一个会话」。`ROLE_PROMPTS` 与 `EVERY_ROLE` 不动，
    需求模板的短语要求不变。
  - 三份 Bug 模板必须含 `{budget_line}`。
  - `case.yaml` 的 `prompt` 必须指向存在的模板。
  - 契约区由已有机制覆盖。
- `KNOWN_PLACEHOLDERS` 不需要新增。
- 种子与渲染：
  - `protocol/mod.rs` 的 `SEED` 加 4 个文件，`evals.rs` 的 `SEED` 加新用例文件。
  - 新金样 `locate.md`、`locate-card.md`、`fix.md`、`fix-review.md`，按 `tests/golden/README.md` 的纪律，与模板
    同一个提交。
- **不改**：`loop-protocol.md`、`session-protocol.md`、`brief-map.toml`、`README.md`，以及 8 份既有 prompt 与 8 份
  既有金样（M-05 用 `git diff --exit-code` 核对）。
- 已冻结在 v1 的进行中任务不受影响：它们读自己的副本，而需求相关契约区的哈希不变。

### 11. 既有测试改动清单（C2）

允许改动的既有测试如下。它们之外，任何既有测试的断言、输入、流程都不改；本方案不改 `Context`、`PromptSpec`、
`TaskRecord` 的字段，`ResolvedConfig`/`ProjectConfig` 的测试字面量已经用 `..`，所以没有「补字段」类的改动。
实现中若发现确需更多改动，先在证据文件里写明理由，再按本节格式补进来。

1. `protocol::tests::the_version_in_force_is_the_newest_changelog_heading_over_the_seed`：`"protocol/v1"` → `"protocol/v2"`。
   理由：A2 要求协议版本加一，这一行钉的就是版本号本身；同一测试里的哈希断言不动。（M-02）
2. `end_to_end.rs` `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`：
   `starts_with("protocol/v1@")` → `"protocol/v2@"`。理由同上；上一行「等于 `current().to_wire()`」的断言不动。（M-02）
3. `end_to_end.rs` `task_card_a_drafted_bug_waits_then_starts_with_its_card_committed_before_the_first_session`：
   - 第 1 步替身改为写 Bug 文档 `status: 无法复现`，保留原来那句「卡片已提交」的前置检查，删掉第 2–4 步；
   - 期望节点 `await_design_approval` → `await_info`；
   - 其余断言不变：卡片字节、inputs 先于固定协议提交、prompt 里卡片路径先于需求、`task.get`/`project.get`/
     `dashboard.waiting[0]` 的 `kind`。
4. `cli.rs` `task_card_a_brief_piped_on_stdin_creates_a_bug_draft_that_starts_with_its_card`：
   - 在 `task_card_prepare` 之后用新增的 Bug 文档替身覆盖第 1 步，期望节点改为 `await_info`；
   - 卡片字节断言不变；
   - `task_card_prepare` 本身不改，其他测试还在用它。
5. `cli.rs` `task_card_reply_rejects_the_design_with_the_text_verbatim`：循环 `["feature", "bug"]` → `["feature"]`；
   移除旧 Bug 分支须与 M-03 启用新 Bug 路径配套，避免全量门禁仍要求新 Bug 走需求 Loop。
   新 Bug reply 由 M-04 新测试 `bug_loop_reply_returns_a_bug_to_locate_with_the_text_verbatim` 验证；M-04 前该项尚未覆盖。

6. `protocol::eval::tests::with_behavioral_entry` 追加的假想版本 v2 改为 v3，
   `a_changed_run_covers_the_new_cases_and_the_three_baselines` 查询同步改为 v3；真实种子已有 v2，避免同名版本遮蔽夹具条目，断言不变。（M-02）
7. `protocol::eval_run::tests::case` 为新增的可选字段填写 `prompt: None`，旧用例仍使用 role 模板。（M-02）

第 3–5 条的理由：这三处断言钉的是 K1 的**过渡**行为（任务文件 §2：「K1 里 Bug 类型的任务仍跑需求 Loop」），
而本任务的目标与 A1 正是替换这一行为。它们测的是 `kind = bug`，不在 C2「需求任务零行为变化」的范围内。保留它们，
就等于要求 Bug 任务继续跑设计循环，与任务目标直接矛盾。所以这里的理由不是「新行为与旧断言不符」，而是「任务
明令废止了旧行为」。这三处断言里测 K1 约定的部分（卡片、提交顺序、`kind` 读出）全部保留。（第 3、4 条在 M-03）

### 12. 待定问题索引

Q1 → §1；Q2 → §1；Q3 → §2；Q4 → §3；Q5 → §4；Q6 → §4（落实）；Q7 → §5；Q8 → §6；Q9 → §7；Q10 → §9；Q11 → §10。

## 风险与验证安排

- **R1 正在驱动本仓库的 Loop 被打断（C4）。**
  - 没有账本迁移，`SCHEMA_VERSION` 仍是 5。
  - 已在跑的 K1 Bug 任务按 `uses_bug_loop` 留在需求 Loop（M-01 单测；M-03 e2e
    `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it`）。
  - 所有验收命令都是 `cargo test` / `clippy` / `fmt` / `protocol eval`，测试一律用 `AUTOMED_DB_PATH` 或临时
    `AUTOME_HOME`；任何轮次都不得不带隔离地运行 `target/debug/autome` 或 `cargo run -p autome-cli`。
- **R2 需求任务的行为被悄悄改变（C2）。** 四层证明：
  - `apply_for(Feature, …)` 对状态 × 触发器的穷举与需求表逐项相等（M-01）；
  - `Node::ALL` 不变（M-01）；
  - 需求协议、8 份 prompt、8 份金样与基线逐字节相同（M-05 `git diff --exit-code`）；
  - 既有测试只改 §11 的 5 条（M-05 核对 diff）。
- **R3 替身和调用方对齐，而没有和真实接口对齐**（本仓库四次同源缺陷）。
  - e2e 的 Bug 文档替身由一个辅助函数生成：它从种子里 `bug-protocol.md` 的 `bug-status-block` 契约区读出键与
    分隔符的写法，再填值。协议一改，替身随之变或直接失败。
  - M-02 有单测，用内核解析器解析契约区里的示例。
  - 替身提交的回归测试是真实的 shell 脚本，由宿主真实执行；e2e 还断言定位 prompt 点名了 `-bug.md` 与
    `bug-protocol.md`。
- **R4 复现命令不稳定，或「失败」不是 Bug 造成的。**
  - 超时、126、127 不算复现；连续两次未复现停下等人（§4）。
  - 「测试失败是因为编译错误」这类情况由复核检查；prompt 明确要求复核在修复前的提交上看失败原因。
- **R5 宿主执行模型写的命令。** 与会话本身在 worktree 里执行命令的信任级别相同：只在任务 worktree 里执行，有超时，
  进程组隔离，日志留存。不扩大面。
- **R6 修复轮削弱回归测试让复现命令通过。** 由两道防线挡：
  - 内核执行的是复现检查记录下的命令，文档里改命令无效；
  - prompt 禁止修改回归测试，复核必须核对回归测试相对定位提交的 diff。
  - 机械守卫纳入 M-06（用户裁定 B-03），修复轮改动定位提交中的回归测试文件时拒绝进入门禁。
- **R7 Bug 节点进入需求表触发 `unreachable!()`。** 需求表在通用控制段之后有防御（§1），M-01 用例
  `bug_loop_feature_table_rejects_bug_triggers_and_never_enters_bug_nodes` 覆盖；同一用例断言控制触发器与
  `(Requeued, SlotAvailable)` 在 Bug 状态上放行，Bug 取消不会被防御挡住。
- **R8 协议体积。** Bug 规范放进新文件，不进 `SIZED_FILES`（F1）。`protocol eval` 在 M-02 和 M-05 各跑一次。

验证安排：
- 新测试统一带 `bug_loop` 字样。领域层的解析器测试在 `bug_doc::tests`，配置测试以 `bug_fix_rounds` 开头，便于按名
  过滤计数。
- 基线里没有任何测试名含 `bug_loop`、`bug_doc`、`bug_fix_rounds`。K1 的
  `kind_wire_names_are_feature_and_bug_and_the_default_is_feature` 含 `bug_` 但不含这三个词。
- M-01 的实现轮开工前先跑一次 `cargo test --workspace`，把基线用例总数写进证据文件，供 M-05 核对。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 领域层：Bug Loop 节点与转换表、Bug 文档解析、[bug] fix_rounds | 0 | domain-bug |
| M-02 | 已完成 | 协议 v2：bug-protocol.md、三份 prompt、eval 静态门与渲染 | 0 | protocol-bug |
| M-03 | 已完成 | 调度器：Bug Loop 主路径与创建到合并的完整 e2e | 0 | scheduler-bug |
| M-04 | 已完成 | task.reply、补充信息、不是 Bug、修复预算与命令行显示 | 0 | rpc-cli-bug |
| M-05 | 开放 | 需求任务零变化与全量门禁 | 0 | regression |
| M-06 | 开放 | B-03：保护定位提交中的回归测试文件 | 0 | regression-guard |

## 里程碑说明

### M-01 范围

最新证据：docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-01-r1-impl.md · 验收通过，待审 · 实现 #1

- 覆盖：A1（领域转换）、A3/A4/A5 的转换部分、A6（角色映射）；C2、C5。前置：无。
- 改动：
  - `autome-domain`：`task.rs`（§1 的新变体、`Node::BUG`、标志位、需求表在通用控制段之后的防御）、`session.rs`（`BugStep`、
    `SessionKind::Bug`、`for_node`）、新文件 `bug_loop.rs` 与 `bug_doc.rs`、`config.rs`（`BugConfig`）。
  - `automed`：`config_io.rs`（`[bug]` 的读写）。
  - `automed`、`autome-cli` 里因新增枚举变体导致编译不过的穷尽 `match`，只补最小分支：拒绝或原样透传，行为留给
    M-03/M-04。需求路径上不得出现 `panic`。
- 新测试（`autome-domain` `bug_loop::tests`，29 个）：
  1. `bug_loop_nodes_have_wire_names_roles_and_flags`
  2. `bug_loop_feature_node_list_is_unchanged`（`Node::ALL` 的 15 个取值与顺序，`BUG` 与之不相交）
  3. `bug_loop_feature_kind_matches_the_feature_table_exactly`（`Node::ALL` × {Active, Paused, Stopped, Requeued,
     Failed 各原因} × 全部触发器，`apply_for(Feature)` 与 `apply_with_merge_request` 逐项相等）
  4. `bug_loop_feature_table_rejects_bug_triggers_and_never_enters_bug_nodes`（同时断言：Bug 状态上的
     `Cancel`/`Pause`/`Stop`/`Resume` 不被防御挡住，`(Requeued, SlotAvailable)` 放行，其余触发器返回 `Err` 而非 panic）
  5. `bug_loop_legacy_bug_tasks_already_in_feature_nodes_keep_the_feature_table`
  6. `bug_loop_a_queued_bug_starts_at_locate`
  7. `bug_loop_impl_disabled_fails_with_config_at_start`
  8. `bug_loop_located_runs_the_repro_check`
  9. `bug_loop_cannot_reproduce_and_not_a_bug_wait_without_a_slot`
  10. `bug_loop_reply_returns_to_locate_from_both_waiting_nodes`
  11. `bug_loop_reply_is_refused_when_empty_or_not_waiting`
  12. `bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend`（`Cancel` 经需求表的通用控制段得到
     `Cancelled + RunCancel`；`apply_for(Bug)` 与 `apply_with_merge_request` 在这些输入上结果相同）
  13. `bug_loop_reproduced_starts_fix_round_one_with_the_configured_budget`
  14. `bug_loop_exit_zero_returns_to_locate_with_the_tail`
  15. `bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced`
  16. `bug_loop_second_consecutive_miss_waits_for_info`
  17. `bug_loop_fix_always_runs_the_fix_gate`
  18. `bug_loop_failed_fix_gate_returns_to_fix_with_the_tail`
  19. `bug_loop_passed_fix_gate_goes_to_review_or_to_rebase_when_audit_is_off`
  20. `bug_loop_review_pass_goes_to_rebase_without_retro`
  21. `bug_loop_review_reject_returns_to_fix`
  22. `bug_loop_fix_budget_runs_out_after_the_configured_rounds`（门禁失败、复核打回、合并前门禁失败三条路径在
      `fix_round == limit` 时都进入 `Failed{FixBudget}`）
  23. `bug_loop_extend_resumes_fix_with_a_larger_limit`
  24. `bug_loop_rebase_always_runs_the_merge_gate`
  25. `bug_loop_rebase_conflict_and_merge_gate_failure_return_to_fix_not_implement`
  26. `bug_loop_merge_tail_is_shared_with_the_feature_loop`
  27. `bug_loop_session_errors_fail_like_feature_sessions`
  28. `bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only`
  29. `bug_loop_stopped_bug_gates_discard_runs_and_requeued_bug_nodes_reenter`（含 `Active{fix} + Pause`、
      `Paused{fix} + Resume`、`Requeued{locate / repro_check / fix / fix_review} + SlotAvailable`）
- 新测试（`autome-domain` `bug_doc::tests`，8 个）：
  - `reads_all_five_fields_with_either_colon`
  - `status_values_are_exactly_three`
  - `a_missing_or_duplicate_field_names_the_key`
  - `located_requires_a_command_and_a_root_cause`
  - `fix_round_must_be_k_over_n`
  - `review_values_are_exactly_three`
  - `lines_after_the_first_section_or_inside_a_fence_are_ignored`
  - `repos_is_optional_at_most_once_and_read_as_a_name_list`
- 新测试（`autome-domain` `config::tests`，2 个）：
  - `bug_fix_rounds_default_to_three`
  - `bug_fix_rounds_project_override_resolves`
- 新测试（`automed` `config_io::tests`，3 个）：
  - `bug_fix_rounds_are_read_from_the_bug_section`
  - `bug_fix_rounds_outside_one_to_twenty_are_refused`
  - `bug_fix_rounds_survive_save_project`

### M-01 验收契约

- 验收命令：`cargo test -p autome-domain bug_loop && cargo test -p autome-domain bug_doc && cargo test -p autome-domain bug_fix_rounds && cargo test -p automed --lib bug_fix_rounds && cargo test -p autome-domain && cargo build --workspace --all-targets`
- 预期：前四条依次 29、8、2、3 passed、0 failed（基线均为 0 个用例），合计 42 个新用例；第五条全部通过，`task::tests` 的既有用例一个不少；第六条编译通过。
- 负向对照：`apply_for(Feature)` 让 `Queued + SlotAvailable` 进 `locate` 时第 3 号用例失败；超时被算作复现时第 15 号用例失败；预算判断写成 `>` 而不是 `≥`（多跑一轮修复）时第 22 号用例失败；解析器只认半角冒号时 `reads_all_five_fields_with_either_colon` 失败；把 Bug 节点加进 `Node::ALL` 时第 2 号用例与既有的 `requeued_dispatches_all_thirteen_nodes` 失败；把防御放回函数第一行（控制段之前）时第 4、12、29 号用例失败（`not_a_bug` 上的取消得到 `Err`）。

### M-02 范围

最新证据：docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-02-r2-impl.md · 已完成 · 审计 #2

- 覆盖：A2；C7、C9。前置：M-01。
- 改动：
  - `protocol/`：新文件 `bug-protocol.md` 与 `prompts/{locate,fix,fix-review}.md`；`CHANGELOG.md` 加 `## protocol/v2`；
    三个 `evals/<case>/`。
  - `automed/src/protocol/`：`mod.rs` 与 `evals.rs` 的 `SEED`；`eval.rs`、`phrases.rs`、`case.rs`、`eval_run.rs`
    （`prompt:` 键）。
  - `automed/src/launcher.rs`：
    - 为 `SessionKind::Bug` 选模板；
    - 定位的卡片前置段；
    - `ReproNotFailing` 与 `FixRejected` 的注入文本；
    - Bug 的预算行。
  - `tests/prompt_rendering.rs` 与 4 份新金样。
  - §11 第 1、2 条。
- 新测试（`automed --test prompt_rendering`，5 个）：
  - `bug_loop_locate_prompt_matches_its_golden`
  - `bug_loop_locate_prompt_with_a_card_matches_its_golden`（以 `locate` 金样结尾，前置段点名卡片路径）
  - `bug_loop_fix_prompt_matches_its_golden`
  - `bug_loop_fix_review_prompt_matches_its_golden`
  - `bug_loop_bug_prompts_leave_no_placeholder`
- 新测试（`automed` `launcher::tests`，3 个）：
  - `bug_loop_repro_not_failing_inject_names_the_command_exit_code_why_and_tail`
  - `bug_loop_fix_rejected_inject_points_at_the_review_section`
  - `bug_loop_gate_failed_inject_on_a_fix_round_carries_the_tail`
- 新测试（`automed` 的 `protocol` 各模块，测试名以 `bug_loop_protocol_` 开头，6 个）：
  - `bug_loop_protocol_seed_carries_the_bug_protocol_and_three_prompts`
  - `bug_loop_protocol_contract_example_parses_with_the_kernel_parser`
  - `bug_loop_protocol_bug_status_block_is_a_kernel_contract_region`
  - `bug_loop_protocol_eval_requires_budget_line_in_every_bug_prompt`
  - `bug_loop_protocol_eval_requires_the_no_relay_phrase_in_bug_prompts`
  - `bug_loop_protocol_case_prompt_key_must_name_an_existing_template`
  - `bug_loop_protocol_eval_runs_the_selected_bug_template`（替身 runner 证明实际选中 Bug 模板并填充文档路径，不调用模型）

### M-02 验收契约

- 验收命令：`cargo run -p automed -- protocol eval protocol/ && cargo test -p automed --test prompt_rendering && cargo test -p automed --lib launcher::tests::bug_loop && cargo test -p automed --lib bug_loop_protocol && cargo test -p automed --lib protocol:: && cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`
- 预期：`protocol eval` 退出码 0 且报告版本 `protocol/v2`；`prompt_rendering` 全部通过，其中 `bug_loop` 用例 5 个、既有用例数与基线相同；第三条 3 passed，第四条 7 passed（本里程碑合计 15 个新用例）；`protocol::` 下全部通过（含改成 v2 的版本断言）；e2e 那一个用例 1 passed；全部 0 failed。
- 负向对照：从 `prompts/fix.md` 删掉 `{budget_line}` 时 `protocol eval` 退出码非 0 且 `bug_loop_protocol_eval_requires_budget_line_in_every_bug_prompt` 失败；契约区示例写成 `复现命令 = …` 时 `bug_loop_protocol_contract_example_parses_with_the_kernel_parser` 失败；定位不加卡片前置段时 `bug_loop_locate_prompt_with_a_card_matches_its_golden` 失败；改动任何一份既有 prompt 时 `every_role_prompt_renders_exactly_what_the_constants_did` 失败。

### M-03 范围

最新证据：docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-03-r4-audit.md · 审计通过 · 审计 #3

- 覆盖：A1（端到端节点序列）、A3、A6（运行时）、A7；C4、C5、C6。前置：M-02。
- 改动：`automed/src/scheduler.rs`：
  - 按 `uses_bug_loop` 选表，`bug.loop_started` 事件；
  - `StartBugSession` 与 `bug.fix_round_started`；
  - 从事件计算 `BugProgress`；
  - `read_outcome` 读 Bug 文档，Bug 守卫；
  - `run_gate` 支持 `repro_check` / `fix_gate` 与 Bug 任务的合并前门禁组合命令；复用条件的「命令相同」只在 `uses_bug_loop` 为真时生效，需求与旧 Bug 任务的复用判断一字不改；
  - `ReproDone`；
  - `SessionKind::for_node` 替换 F5 的三处；
  - 定位的 `card_path`；
  - Bug 会话不写简报；
  - `bug.located`、`bug.reproduced`、`bug.repro_missed`、`bug.reviewed` 事件。

  `store.rs` 只增加事件查询，不改表。还包括 §11 第 3、4 条，以及测试辅助函数（Bug 文档替身，按 R3 从契约区派生）。
- 新测试（`automed --test end_to_end`，8 个）：
  1. `bug_loop_a_bug_runs_from_creation_to_a_merge_commit`（A7）：
     - 带正文创建 `kind = bug`；
     - 替身定位提交一个真实失败的检查脚本和 Bug 文档；
     - 宿主复现检查失败 → 替身修复 → 修复门禁通过 → 替身复核 `通过` → rebase → 合并前门禁 → `await_merge` →
       `task.merge` → `Done`。
     - 断言：
       - 节点序列不含 `intake`/`design`/`audit`/`retro`；
       - 三个 prompt 分别是定位、修复、复核模板；
       - main 上的合并提交包含修复；
       - `.autome/output/gates/<task>/` 下有 `repro_check-*` 与 `fix_gate-*` 日志。
  2. `bug_loop_a_repro_command_that_passes_on_the_locate_commit_returns_to_locate`（A3 前半）：下一个 prompt 是定位，
     含「复现检查没有失败」、命令与日志尾部。
  3. `bug_loop_a_fix_whose_repro_command_still_fails_returns_to_fix`（A3 后半）：下一个 prompt 是修复，带门禁尾部；
     第二轮修复好之后进入复核。
  4. `bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate`：配置一个失败的 `[gate]`，复现命令通过，
     任务仍回到修复。
  5. `bug_loop_review_rejection_returns_to_fix_and_counts_a_round`：`task.get` 的 `budget_n` 为 3，账本里
     `bug.fix_round_started` 有 2 条。
  6. `bug_loop_a_crashed_fix_session_is_retried_without_counting_a_round`
  7. `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it`：直接在账本里放一个
     `Active{await_design_approval}`、没有 `bug.loop_started` 的 Bug 任务，批准后进入 `implement`。
  8. `bug_loop_a_malformed_bug_doc_is_a_protocol_failure`：`已定位` 但缺复现命令 → `Failed{Protocol}`；修复后没把
     `复核` 重置 → `Failed{Protocol}`，原因含 `复核`。
- 新测试（`automed` `scheduler::tests`，3 个）：
  - `bug_loop_cooldown_keeps_fix_and_review_on_different_runtimes`
  - `bug_loop_reenter_maps_bug_nodes_to_bug_sessions`
  - `bug_loop_merge_gate_does_not_reuse_a_pass_for_a_different_command`（Bug Loop 任务：同 sha、命令不同则重跑；
    同一输入下需求任务仍复用并发出 `gate.reused`——钉住 C2）

### M-03 验收契约

- 验收命令：`cargo test -p automed --test end_to_end bug_loop && cargo test -p automed --lib scheduler::tests::bug_loop && cargo test -p automed --test end_to_end task_card && cargo test -p autome-cli --test cli task_card_a_brief_piped_on_stdin_creates_a_bug_draft_that_starts_with_its_card && cargo test -p automed --test end_to_end`
- 预期：第一条 8 passed，第二条 3 passed（本里程碑合计 11 个新用例）；第三条全部通过（含按 §11 第 3 条修改的那个用例）；第四条 1 passed；第五条全部通过、用例数 = 基线 + 8；全部 0 failed。
- 负向对照：对需求任务也加「命令相同」条件时 `bug_loop_merge_gate_does_not_reuse_a_pass_for_a_different_command` 的需求半段失败；复现检查把退出码 0 当成「复现」时 `bug_loop_a_repro_command_that_passes_on_the_locate_commit_returns_to_locate` 失败；修复门禁只跑 `[gate]` 不跑复现命令时 `bug_loop_a_fix_whose_repro_command_still_fails_returns_to_fix` 失败（无 `[gate]` 时会直接放行到复核）；崩溃重试经过 `StartBugSession` 时 `bug_loop_a_crashed_fix_session_is_retried_without_counting_a_round` 失败；选表只看 `kind` 时 `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it` 失败。

### M-04 范围

最新证据：docs/按计划-4-2-节实现-bug-修复-loop/evidence/M-04-r5-impl.md · 验收通过，待审 · 实现 #5

- 覆盖：A4、A5，A1 的「需求任务的 `reply` 等同 `reject`」；C3、C8、C9。前置：M-03。
- 改动：
  - `automed/src/dispatch.rs`：`task.reply`；`task_json` 只对 Bug 任务加 `fix_round`；`task.retro` 拒绝 Bug 任务。
  - `scheduler.rs`：`-replies.md` 的生成与提交；工作区项目的仓库集合（§7：初始全部成员、`repos` 守卫、移除未点名的检出）；`bug_doc` 的 `repos` 键已在 M-01，`bug-protocol.md` 的工作区说明已在 M-02。
  - `autome-cli`：`main.rs` 的 `reply` 映射与 `USAGE`；`render.rs` 的节点、下一步、事件与失败文本。
  - `cli.rs` 新用例；§11 第 5 条。
- 新测试（`automed --test end_to_end`，6 个）：
  1. `bug_loop_cannot_reproduce_waits_for_info_and_a_reply_returns_to_locate_with_it`：
     - `await_info` 出现在 `dashboard.waiting`；
     - `task.reply` 之后，`-replies.md` 已提交，内容逐字节为原文（含双空格与多字节字符）；
     - 第二轮定位的 prompt 点名该文件；之后能继续到 `repro_check`。
  2. `bug_loop_not_a_bug_waits_for_a_decision_and_can_be_cancelled_or_rebuilt_as_a_feature`：
     - 暂停被拒；
     - 用同一份 `brief` 以 `kind = feature` 调 `task.create` 成功；
     - 原任务 `task.cancel` 后为 `cancelled` 并已归档。
  3. `bug_loop_two_repro_misses_in_a_row_wait_for_info`
  4. `bug_loop_fix_rounds_run_out_and_extend_adds_more`：
     - 临时项目配置 `[bug]\nfix_rounds = 2`；
     - 两轮失败的修复后进入 `Failed{FixBudget{2}}`；
     - `task.extend_budget` 1 轮后进入第 3 轮修复并通过。
  5. `bug_loop_task_get_reports_the_fix_round_for_bugs_only`：Bug 任务有 `fix_round`；需求任务的键集合与基线相同。
  6. `bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos`：沿用既有的
     `Workspace` 夹具（两个成员加文档成员）。
     - 定位会话开始时每个成员都有检出；替身在成员 a 里提交失败的回归测试，Bug 文档写 `repos: a`；
     - 复现检查在工作区目录执行并判为复现；成员 b 的检出被移除，进入修复时集合是 a 与文档成员；
     - 另一条分支：工作区 Bug 文档 `已定位` 却缺 `repos` → `Failed{Protocol}`，原因含 `repos`；
     - 合并尾段不重复证明，沿用既有工作区合并用例。
- 新测试（`automed` `dispatch::tests`，3 个）：
  - `bug_loop_reply_on_a_feature_task_is_exactly_reject`：在 `await_design_approval` 与其它状态下，结果、状态、事件都与
    `task.reject` 相同。
  - `bug_loop_reply_is_a_write_method_not_a_read_method`
  - `bug_loop_a_bug_task_is_refused_a_retro`
- 新测试（`autome-cli --test cli`，2 个）：
  - `bug_loop_reply_returns_a_bug_to_locate_with_the_text_verbatim`
  - `bug_loop_status_shows_the_bug_nodes_and_what_to_do_next`
- 新测试（`autome-cli` 单元，2 个）：
  - `bug_loop_render_labels_every_bug_node_event_and_failure`
  - `bug_loop_usage_no_longer_says_reply_equals_reject`

### M-04 验收契约

- 验收命令：`cargo test -p automed --test end_to_end bug_loop && cargo test -p automed --lib dispatch::tests::bug_loop && cargo test -p autome-cli bug_loop && cargo test -p autome-cli`
- 预期：第一条 14 passed（M-03 的 8 个 + 本里程碑 6 个）；第二条 3 passed；第三条 4 passed（集成 2 个 + 单元 2 个；本里程碑合计 13 个新用例）；第四条全部通过，含按 §11 第 5 条修改的 `task_card_reply_rejects_the_design_with_the_text_verbatim` 与既有的 `usage_lists_task_start_reply_and_the_new_flags`、`task_card_start_refuses_a_task_that_is_not_a_draft`；全部 0 failed。
- 负向对照：工作区 Bug 不点名 `repos` 却放行时用例 6 的第二条分支失败；`task.reply` 对需求任务走 `Trigger::Reply` 时 `bug_loop_reply_on_a_feature_task_is_exactly_reject` 失败；补充信息只放在 inject 里而不写文件时，用例 1 的「第二轮定位点名该文件」断言失败；`ExtendBudget` 在 Bug 任务上委托给需求表（会被拒绝）时用例 4 失败；对所有任务都输出 `fix_round` 时用例 5 失败。

### M-05 范围

- 覆盖：A1「需求任务的转换不变」的总体证明；C2、C3、C7、C11、C12。前置：M-04。
- 改动：不新增功能。只修复全量门禁暴露的问题，并在证据文件里逐条核对 §11（`git diff 81a4394 -U0` 在四个集成测试
  文件与各 `tests` 模块里的删除行，只能落在 §11 列出的允许改动项里）。
- `.autome/` 不在下面的 diff 范围里：它可能被 Autome 自己的初始化刷新。会话不改它这一点由 C1 与 AGENTS.md 约束，
  审计轮另用 `git log 81a4394.. --format=%s -- .autome/` 核对，只能出现 Autome 署名的提交。

### M-05 验收契约

- 验收命令：`cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check && cargo run -p automed -- protocol eval protocol/ && git diff --exit-code 81a4394 -- protocol/loop-protocol.md protocol/session-protocol.md protocol/brief-map.toml protocol/README.md protocol/prompts/intake.md protocol/prompts/onboarding.md protocol/prompts/plan.md protocol/prompts/review.md protocol/prompts/adjudicate.md protocol/prompts/impl.md protocol/prompts/audit.md protocol/prompts/retro.md crates/automed/tests/golden/prompts/adjudicate.md crates/automed/tests/golden/prompts/audit.md crates/automed/tests/golden/prompts/impl.md crates/automed/tests/golden/prompts/intake.md crates/automed/tests/golden/prompts/intake-card.md crates/automed/tests/golden/prompts/onboarding.md crates/automed/tests/golden/prompts/plan.md crates/automed/tests/golden/prompts/review.md apps/ plugins/ scripts/ .github/`
- 预期：全部退出码 0；`cargo test --workspace` 0 failed，用例总数 = M-01 证据里记录的基线总数 + 81（M-01 42、M-02 15、M-03 11、M-04 13；M-06 另加 4 个）；`git diff --exit-code` 无输出。
- 负向对照：改动任何一份需求 prompt、金样或需求协议正文（哪怕一个空格）时 `git diff --exit-code` 退出码 1；需求表的行为被改动时，M-01 的 `bug_loop_feature_kind_matches_the_feature_table_exactly` 与既有 `task::tests` 在 `cargo test --workspace` 中失败。

### M-06 范围

- 用户裁定 B-03 纳入；前置 M-03、M-04、M-05。按编号顺序在独立实现轮推进。
- 定位成功时，按仓库记录定位提交相对本次定位会话起点新增或修改的测试文件及定位后的 blob/mode，持久化到已有事件，不迁移账本。
- 保护集包含测试目录（`test/`、`tests/`、`__tests__/`）、常见 test/spec 文件名、复现命令直接引用的仓库内测试脚本；内联测试变更所在源码文件按整个文件保护。实现轮必须用项目真实夹具核定识别范围并在证据列明；不得把空保护集当作已保护。
- 每次修复完成、进入 fix_gate 前，按定位快照核对保护集；内容、模式改变、删除、重命名均拒绝，走 Bug GuardFailed/Protocol 路径并指出文件。跨重试、暂停、追加轮次仍使用相同快照；重新定位建立新的快照。需求与旧 Bug 不启用此守卫。
- 工作区按成员仓库分别取定位提交与保护集；新增或修改的非测试产品文件允许修复。保护的是完整测试文件，内联测试所在产品源码需要同时改动时本版会拒绝，应在定位轮把回归测试放入独立测试文件，不能静默放宽。
- 新 e2e 四项：`bug_regression_guard_rejects_modified_or_deleted_tests`（含重命名与弱化断言）、`bug_regression_guard_allows_product_only_fixes`、`bug_regression_guard_survives_retry_and_extend`、`bug_regression_guard_checks_each_workspace_member`；每项同时断言节点、失败原因与保护文件字节，正反向对照齐备。

### M-06 验收契约

- 命令：`cargo test -p automed --test end_to_end bug_regression_guard && cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`。
- 预期：专项 4 passed；全量在 M-05 基线上 +4，全部 0 failed。需求既有测试不修改。
- 负向对照：禁用守卫后，修改/删除/重命名测试的拒绝用例必须红；把保护集错误扩大到所有产品文件时，product-only 正向用例必须红。

## 人工验收清单


- 安装本任务构建的 `autome` 前，确认驱动本仓库的 Loop 当时没有运行中的会话；安装后，`autome status` 能列出全部既有任务，账本仍是 schema v5。
- 用一个真实的小 Bug 和真实模型跑一次 Bug Loop 到「等你合并」，检查三件事：定位轮写的 `-bug.md` 与回归测试是否像样；复现检查确实失败；修复没有动回归测试。
- （可选，要花钱）`cargo run -p automed -- protocol eval protocol/ --behaviour` 跑三个新用例，看行为是否符合预期。
- 在桌面端看到 Bug 任务的新节点名时原样显示、不崩溃（桌面端本任务不改）。

## Backlog

- B-01 W5 的阶段映射（定位 · 修复 · 复核 · 合并 · 完成）、W7 的「改为需求」按钮与「修复第 k / N 轮」显示，落在 `apps/`，不在本任务目录。
- B-02 工作区 Bug 的完整合并路径单独的 e2e（本任务只证明仓库选择与修剪，合并尾段沿用工作区既有用例）。
- B-03 已纳入 M-06：修复轮不得改动定位提交里新增或修改的回归测试文件。
- B-04 追加轮次之后的第一轮修复，带上最近一次失败的门禁或复核结论（目前要自己去读日志与 Bug 文档）。
- B-05 `autome ls` 显示任务类型与修复轮次。
- B-06 复现检查单独的超时配置 `[bug] repro_timeout_minutes`。
- B-07 给 Bug 会话生成简报（`brief-map.toml` 支持按 Bug 步骤取小节）。
- B-08 手动 `RerunFrom{locate}` 是否开启新的复现周期：目前 `repro_misses` 只按最近一次 `task.replied` 清零，手动重跑定位不清零（第 1 轮评审 N1）。

## 争议项
