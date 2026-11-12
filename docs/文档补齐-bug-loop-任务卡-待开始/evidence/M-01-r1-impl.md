# M-01 · 实现轮 1/15

结论：M-01 验收通过，申请待审；仅补 technical-design §5.1 与 §5.6。没有前轮审计文件，也没有需要逐字移植的审计回归检查。本轮不改代码、测试、README 或其它里程碑。

## 命令与结果

- 验收命令（修改前、修改后相同）：`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-01`。
- 修改前：`M-01 FAIL 77/103`；修改后：`M-01 PASS 103/103`，验收脚本退出码 0。原始输出：`.autome/output/m01-before.txt`、`.autome/output/m01-after.txt`。
- 检查总数 103，上一轮设计基线 103，差 **0**；通过数增加 26，来自新增出处路径、5.6 位置和 24 个原本缺失的 §5 关键词。没有删检查、改脚本或缩小区间。
- `cargo test -p autome-domain --lib`：384 个测试，384 通过、0 失败、0 忽略、0 过滤；日志 `.autome/output/m01-domain.txt`。此前任务记录没有领域库独立运行的计数基线，不能冒充上一轮全量基线；本轮未改源码、测试或 Cargo 配置，测试定义增减为 0。
- 全量 `sh scripts/ci-local.sh` 本轮未手动执行：按设计由宿主在实现轮后运行，M-03 显式执行并报告三套测试基线。本轮的领域库测试不等同于全量门禁。

## 标待审之前的自审清单

### 1. 情形表逐行

逐条读取 D4 的代码锚点与实现上下文，并对照新增正文；T-01..T-36 共 36 条如下。表内的“通过”表示文档语义与源码一致，不把 grep 命中当作运行验收。

| 陈述 | 核对内容 | 结果 |
|---|---|---|
| T-01 | `Draft`（界面「待开始」）不占槽位、不算等用户、不是终态、没有节点 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/task.rs 的 `fn draft_holds_no_slot_does_not_await_the_user_and_is_not_terminal` |
| T-02 | 只有 `Draft` 接受 `Start`，转为 `Queued`；其它状态拒绝「只有待开始的任务可以开始」 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/task.rs 的 `Err(reject("只有待开始的任务可以开始"))` |
| T-03 | `Draft` 可以直接取消；暂停被拒「任务尚未开始」，会话、门禁等其它触发也都被拒 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/task.rs 的 `fn a_draft_refuses_every_other_trigger` |
| T-04 | 任务类型 `kind` 只有 `feature`（默认）与 `bug` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/task.rs 的 `pub enum TaskKind` |
| T-05 | 走 Bug Loop 的条件：`kind = bug`，且已记过 `bug.loop_started` 事件或仍处于 `Draft`/`Queued` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `pub fn uses_bug_loop` |
| T-06 | 需求任务原样走 §5.3 的表；已经停在需求节点的旧 bug 任务也继续走需求表 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_legacy_bug_tasks_already_in_feature_nodes_keep_the_feature_table` |
| T-07 | 七个节点与线上名：`locate`、`repro_check`、`fix`、`fix_gate`、`fix_review`、`await_info`、`not_a_bug` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/task.rs 的 `Node::ReproCheck => "repro_check"` |
| T-08 | 界面标签：定位、复现检查、修复、修复门禁、复核、等你补充信息、等你决定（不是 Bug） | 通过：对应 §5.1 / §5.6，出处为 crates/autome-cli/src/render.rs 的 `"await_info" => "等你补充信息"` |
| T-09 | `locate`、`fix` 由实现角色跑会话，`fix_review` 由审计角色跑；`repro_check`、`fix_gate` 是内核门禁步骤；`await_info`、`not_a_bug` 等用户、不占槽位 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_nodes_have_wire_names_roles_and_flags` |
| T-10 | Bug 任务要求审计角色开启：`Start` 或分到槽位时审计关闭即拒「Bug 任务需要独立复核，请启用审计角色后启动」 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `Bug 任务需要独立复核，请启用审计角色后启动` |
| T-11 | `Queued` 分到槽位 → `locate`；此时记 `bug.loop_started` | 通过：对应 §5.1 / §5.6，出处为 crates/automed/src/scheduler.rs 的 `.append_event("bug.loop_started", task_id, json!({}))?;` |
| T-12 | `locate` 结束：status 已定位 → `repro_check`；无法复现 → `await_info`；不是 Bug → `not_a_bug` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `BugStatus::CannotReproduce => Transition::active(Node::AwaitInfo, Action::None),` |
| T-13 | 会话文档校验：`locate` 以外的轮次 status 必须是已定位；`复核` 字段只有 `fix_review` 轮不得是待复核，其它轮必须是待复核；不符即 `Failed(Protocol)` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `Some("复核 字段不符合当前轮次")` |
| T-14 | `repro_check` 只跑复现命令（取 `bug.located` 事件里的命令）；超时取 `[gate] timeout_minutes`，未配置门禁时 30 分钟 | 通过：对应 §5.1 / §5.6，出处为 crates/automed/src/scheduler.rs 的 `_ => repro.to_owned(),` |
| T-15 | 退出码非 0、不是 126/127、且未超时，才算复现成功 → 开修复轮；退出码 0、126、127 或超时（门禁超时退出码 124）都不算复现，原因记为 `exited_zero` / `not_runnable` / `timed_out` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `![126, 127].contains(exit_code)` |
| T-16 | 同上的测试出处 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced` |
| T-17 | 超时退出码常量为 124 | 通过：对应 §5.1 / §5.6，出处为 crates/automed/src/gate.rs 的 `pub const TIMED_OUT: i32 = 124;` |
| T-18 | 第一次未复现 → 回 `locate`，注入命令、退出码、原因与日志尾；连续第二次未复现 → `await_info`（等你补充信息） | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `if progress.repro_misses >= 1 {` |
| T-19 | 「连续」的计数 = 最近一次 `task.replied` 之后的 `bug.repro_missed` 事件数，所以 reply 之后重新计数 | 通过：对应 §5.1 / §5.6，出处为 crates/automed/src/scheduler.rs 的 `.count_events_after(&task.id, "bug.repro_missed", "task.replied")?,` |
| T-20 | 同上的测试出处 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_second_consecutive_miss_waits_for_info` |
| T-21 | `fix_gate` 与 `MergeGate` 跑复现命令（取 `bug.reproduced` 事件里的）；项目配置了 `[gate]` 时跑 `( 复现命令 ) && ( 门禁命令 )`，否则只跑复现命令——合并前的两道门禁总带复现命令；`MainCheck` 只跑项目门禁 | 通过：对应 §5.1 / §5.6，出处为 crates/automed/src/scheduler.rs 的 `format!("( {repro} ) && ( {} )", gate.command)` |
| T-22 | `fix` 结束总进 `fix_gate`，与是否配置 `[gate]` 无关 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_fix_always_runs_the_fix_gate` |
| T-23 | `fix_gate` 通过 → `fix_review`；失败 → `fix`，注入门禁失败（命令与日志尾） | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `GateOutcome::Passed if *node == Node::FixGate => {` |
| T-24 | `fix_review` 复核通过 → `Rebase`（不经 `Retro`）；打回 → `fix`，注入打回 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_review_pass_goes_to_rebase_without_retro` |
| T-25 | `Rebase` 成功总进 `MergeGate`；冲突 → `fix`，注入冲突文件 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_rebase_always_runs_the_merge_gate` |
| T-26 | `MergeGate` 失败 → `fix`；通过后与需求任务共用 `AwaitMerge` 以后的尾段 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_merge_tail_is_shared_with_the_feature_loop` |
| T-27 | 每开一个 `fix` 会话（含手动重跑）记一条 `bug.fix_round_started`；上限 = 任务的预算，未设时取 `[bug] fix_rounds`；正常自动转换（门禁失败、复核打回、rebase 冲突、合并门禁失败、定位完成后首次开修复）在开下一修复轮前检查，已开轮数达到上限即 `Failed(FixBudget)` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `let limit = ctx.budget_n.unwrap_or(ctx.config.bug.fix_rounds);` |
| T-28 | 预算失败后追加 n 轮（n > 0）：上限加 n，直接开 `fix` | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_extend_resumes_fix_with_a_larger_limit` |
| T-29 | 停在 `await_info` / `not_a_bug` 时收到非空补充信息 → `locate`；空文本拒「补充信息不能为空」；不在这两个节点时拒绝 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_reply_is_refused_when_empty_or_not_waiting` |
| T-30 | 两个等待节点可以取消，不能暂停、停止或追加轮次 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend` |
| T-31 | 失败或停止的 Bug 任务只能从 `locate`、`fix`、`fix_review`、`rebase` 重跑 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `Bug 任务只能从定位、修复、复核或 rebase 重跑` |
| T-32 | `fix_gate` / `MergeGate` 同因失败熔断后，从同一门禁节点 rerun 或追加轮次都丢弃旧门禁结果重跑；追加轮次同时抬高修复轮上限 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn same_failure_bug_rerun_and_extend` |
| T-33 | 停在 `repro_check` / `fix_gate` 的任务恢复后重新排队，丢弃被杀的门禁结果；会话崩溃、文档不可解析、缺文件、守卫失败的处理与需求会话相同 | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_stopped_bug_gates_discard_runs_and_requeued_bug_nodes_reenter` |
| T-34 | Bug 文档字段：status（已定位 / 无法复现 / 不是 Bug）、fix-round、复现命令、根因、复核（待复核 / 通过 / 打回） | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_doc.rs 的 `"不是 Bug" => BugStatus::NotABug,` |
| T-35 | Bug 会话的 prompt 模板是 `protocol/prompts/locate.md`、`fix.md`、`fix-review.md`，规则在 `protocol/bug-protocol.md`，编译进 `automed` | 通过：对应 §5.1 / §5.6，出处为 crates/automed/src/protocol/mod.rs 的 `prompts/locate.md` |
| T-36 | 手动 `rerun … fix` 不经上述预算检查：任务因 `FixBudget` 失败后重跑 `fix` 仍直接开修复会话，只计入已开轮数，不再触发 `Failed(FixBudget)`（上限仍是 T-28 追加后的值） | 通过：对应 §5.1 / §5.6，出处为 crates/autome-domain/src/bug_loop.rs 的 `fn bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only` |

V-06：读取 `gate_failure::counts`（只计 Gate/MergeGate/FixGate）、`streak`（熔断之后重新从 1 计）、配置默认常量 3 与解析范围 1..=10；与既有 §8.1 一致，保留原文，无删除行例外。§8.1 的“从零计数”表示旧连续计数清空，新一次失败计为 1。

### 2. 失败分支

本任务交付文档，失败分支以源码匹配臂逐项核对，并运行已有领域测试，不启动真实任务或访问账本。

| 分支 | 核对/已有测试 | 结果 |
|---|---|---|
| 非草稿 Start、草稿其它触发 | `start_is_refused_for_every_state_that_is_not_a_draft`、`a_draft_refuses_every_other_trigger` | 通过，正文保留两条拒绝原文 |
| 审计关闭、执行角色关闭 | `apply_for` 的 Start/SlotAvailable 拒绝与 `session` 的 Config 失败；领域库覆盖 | 通过，不声称可跳过独立复核 |
| 会话崩溃、不可解析、缺文件、守卫失败 | `bug_loop_session_errors_fail_like_feature_sessions` | 通过；对应 Failed(SessionCrashed/Protocol) |
| status/复核字段不符 | `apply_for` 的 invalid 检查及领域库测试 | 通过，转换表写明轮次约束 |
| 未复现、超时、不可执行 | `bug_loop_exit_zero_returns_to_locate_with_the_tail`、`bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced`、`bug_loop_second_consecutive_miss_waits_for_info` | 通过 |
| fix_gate / MergeGate 失败、复核打回、rebase 冲突 | `apply_for` 各 `start_fix` 匹配臂及同名行为测试 | 通过，均有预算检查 |
| 预算耗尽、零追加与溢出 | `bug_loop_fix_budget_runs_out_after_the_configured_rounds`、`bug_loop_extend_resumes_fix_with_a_larger_limit`、`apply_for` 的 checked_add | 通过 |
| 空 reply、错误节点 reply、等待节点暂停/停止/追加 | `bug_loop_reply_is_refused_when_empty_or_not_waiting`、`bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend` | 通过 |
| 非法重跑节点、熔断后重跑 | `bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only`、`same_failure_bug_rerun_and_extend` | 通过；明确手动 fix 绕过预算的现状 |
| 外部服务不可达 | 不适用：本里程碑无外部服务调用或相应文档承诺 | 未增加虚构分支 |

### 3. 全部状态迁移

逐行对照新增转换表与 `apply_for`：Queued→Locate；Locate 的三种 status 出口；三个会话节点的协议/崩溃出口；ReproCheck 的成功、首 miss、第二 miss；Fix→FixGate；FixGate 通过/失败；FixReview 通过/打回；Rebase 成功/冲突；MergeGate 通过/失败；两个等待节点各自 reply/取消及暂停、停止、追加拒绝；FixBudget 追加；Failed/Stopped 的四个合法重跑节点及非法节点拒绝；两个熔断门禁的失败、同门禁 rerun、extend；Stopped(ReproCheck/FixGate) 恢复并重新入队。另核 Draft 的 Start/Cancel/其它触发。

结果：全部对应源码匹配臂。已有领域库 384 个测试运行包含上述 Bug Loop、Draft、共享合并尾段和熔断测试；调度器 `advance`、`perform`、`bug_progress`、`run_gate` 的事件写入、计数与命令选择为源码核对，未声称本轮端到端执行它们。新增节点表的七行也分别核对线上名、角色、界面标签和槽位标志。

### 4. 输入域边界

- 复现退出码：0→未复现；126/127→不可执行、不算复现；超时标志为 true→未复现。已有领域测试覆盖这些值和 1/125/128/255/-1 成功复现对照。124 是 `gate::TIMED_OUT` 的宿主常量；源码判定依赖超时标志，文档没有把非超时的 124 一概排除。
- miss 计数：0 与 1、u32::MAX 已有测试覆盖；源码 `bug_progress` 确认以最近 task.replied 为界。
- 修复预算：已有测试覆盖达到上限与追加 0、溢出；手动重跑路径另核不调用 start_fix。`fix_rounds` 的 1/20 是 M-02 配置文档范围，本轮未添加范围承诺；读取配置解析确认闭区间 1..=20。
- `same_failure_limit`：核对解析闭区间 1..=10 与既有默认 3；V-06 保留。
- 正文 65536 字节：不适用，本轮不推进 M-02 的任务卡正文输入域。日期、前导零、空列表等不适用，本轮没有声明这些输入域。

## 修复说明与限制

新增 Draft 段落和 Bug Loop 小节；保留所有原有行与章节号。源码引用复核时纠正了两个草稿符号名为 `advance`、`perform`。预算绕过为既有代码行为，设计 Backlog B-07 已记录，本轮不修代码。没有新增审计缺陷回归用例，因为没有上一轮审计。

补充配置边界检查：`cargo test -p automed --lib config_io::tests::` 退出码 0，40 通过、0 失败、0 忽略、608 过滤（明确为配置子集，不是全量）；日志 `.autome/output/m01-config.txt`。上一轮未记录该子集计数，测试定义增减为 0；包含 same_failure_config_bounds 的 1/10 边界。
