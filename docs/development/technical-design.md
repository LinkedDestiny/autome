# Autome 2.0 技术设计

版本：v3　日期：2026-09-15　状态：定稿

- 本文替代 `2026-09-13-autome-2.0-greenfield-development-plan.md` 中 §3–§8、§10–§18 的技术设计。冲突处以本文为准。
- 需求见 `2026-09-15-autome-2.0-requirements.md`；决策记录见 `2026-09-15-autome-2.0-project-task-ui-decisions.md`。
- 沿用 09-13 方案的边界：D1 在独立仓库 autome-v2 从零建设，不复用 1.x 代码；D2 Rust 内核 `automed` 是唯一状态权威；D3 Electron 只做界面；D9 本地、单用户、macOS。1.x 的 `.autome/` 布局、任务文件协议与角色划分作为约定沿用，代码重新实现。

## 1. 架构总览

```
┌──────────────── Electron ────────────────┐      ┌──────────── automed (Rust) ────────────┐
│ Renderer（九块屏幕，无业务状态）           │      │ 项目登记 · 任务状态机 · 调度器          │
│ Preload（白名单 IPC）                      │◀────▶│ Git 工作区 · 会话启动器 · 配置解析      │
│ Main（启动 sidecar，安全基线）              │ JSON-RPC│ 环境探测 · 技能盘点 · SQLite 台账        │
└───────────────────────────────────────────┘ stdio └────────┬──────────┬──────────┬─────────┘
                                                            │          │          │
                                                   git / worktree   osascript   文件系统
                                                            │      iTerm2 / Terminal   │
                                                            ▼          ▼          ▼
                                                   用户仓库      claude / codex    .autome · docs ·
                                                   .worktree/    会话进程          .worktree · ~/.autome
```

与 09-13 方案相比的三个结构性变化：

1. **会话在后台运行**，由启动包装脚本直接拉起，不开终端窗口。automed 通过包装脚本写出的退出标记感知会话结束。（原定在 iTerm2 可见终端里运行，2026-09-16 用户改为后台：一次 Loop 会起六个以上会话，每个都弹窗抢焦点，打断用户手上的事。包装脚本照样把 CLI 的 stdout/stderr tee 进会话日志，任务面板能打开，也可以 tail -f 跟实时输出。）
2. **任务状态从仓库文件推导**：设计文档头部状态块、里程碑三态、Backlog 与争议项是事实来源；SQLite 只存登记、索引、台账与用户表态。
3. **automed 独占节点转换**。Agent 不再自己启动下一个会话；每个会话结束后由 automed 依据状态块决定下一节点并启动。这使暂停、并行控制、角色开关、预算与停顿点全部可由内核执行。

## 2. 进程与生命周期

- Electron Main 启动 `automed` sidecar（单实例，framed JSON-RPC over stdio，沿用 autome-v2 现有实现）。sidecar 崩溃时 Main 重启它，Renderer 显示「内核重连中」。
- 会话进程由包装脚本 `.autome/skill/run_session.sh` 在终端中启动。脚本职责：写 pid 文件；执行 CLI；tee 输出到日志；结束后写退出标记（exit code、结束时间）。
- automed 用文件系统 watcher 监听 `.autome/output/sessions/`，并以 5 秒轮询兜底。退出标记出现即视为会话结束。
- 停止 = 读 pid 文件，向进程组发 SIGTERM，3 秒后 SIGKILL，随后写「已停止」标记。包装脚本以 `process_group(0)` 启动，自成进程组，所以这一枪打得到它拉起的 CLI；也因此在终端里对 `autome run` 按 Ctrl-C 不会连带杀掉会话。
- 应用重启后 automed 扫描所有项目的 `.autome/output/sessions/`：有 pid 且进程存活 → 会话仍在运行；有退出标记未消费 → 立即消费并推进；无标记且进程不存在 → 判定会话崩溃，按 §8.1 的重试策略处理，用尽后任务进入失败状态供用户选择。

## 3. 文件布局

### 3.1 仓库内（随仓库提交，除标注忽略者）

```
.autome/
  config.toml              项目 Loop 配置（角色、并行、轮次、系数、技能绑定）
  rules/*.md               项目规则，供 Agent 读取
  skill/
    run_session.sh         会话包装脚本（automed 调用，也可手工调用）
    session-protocol.md    会话须遵守的协议：不改 .autome、结束即退出、状态块格式
  output/                  忽略。sessions/<task>/<session-id>.{log,pid,exit}
.worktree/<slug>/          忽略。每个任务一个 worktree
.gitignore                 init 时追加 .worktree/ 与 .autome/output/
AGENTS.md                  Onboarding 起草；含指向 .autome/rules/ 的固定段
docs/agent-project-profile.md
docs/<slug>/               任务文档（任务分支上产生，合并后进入默认分支）
  <slug>-task.md · <slug>.md · <slug>-review.md · <slug>-adjudication.md · <slug>-audit.md · retro.md
  attachments/
docs/.archive/<slug>/      归档或取消的任务文档
.claude/skills/ · .agents/skills/   项目自定义技能，Autome 只读
```

1.x 的 `SCHEDULE.json` 不再沿用，其 autorun 语义由 automed 调度器承担。

### 3.2 全局与应用数据

```
~/.autome/config.toml                              全局默认（结构同项目配置）
~/Library/Application Support/Autome/autome.sqlite  项目登记、任务索引、会话台账、用户表态、事件日志
~/Library/Application Support/Autome/logs/          automed 与 Electron 日志
```

### 3.3 配置文件

项目可选配置 `[budget] per_task_usd = 20`，金额范围 0.01–1000000 美元、最多两位小数；单任务累计超限只告警不停任务，省略则不启用预算告警。

```toml
# .autome/config.toml（项目）· 只写覆盖项；~/.autome/config.toml（全局）· 写全量默认
[loop]
parallel = 3            # 项目内并行上限，1..5
design_rounds = 15      # 设计循环上限
budget_factor = 5       # 实现预算 N = budget_factor × 初始里程碑数

[roles.plan]
enabled = true
runtime = "claude"      # claude | codex
model = ""              # 留空 = 用该 CLI 自己的默认模型
effort = "high"         # 留空 = 用该 CLI 的默认档位
skills = ["island-shop-conventions"]

[roles.review]
runtime = "codex"
model = ""
effort = "high"

[roles.adjudicate]
[roles.impl]
[roles.audit]

# 仅项目级：自动合并与合并后宿主命令
[merge]
auto = false               # 缺省 false；true 表示门禁通过后直接合并
after = "sh scripts/install-cli.sh"
after_timeout_minutes = 10  # 可省，缺省 10 分钟
```

## 4. 领域模型

```text
Project
  id · path · display_name · default_branch · parallel_limit
  onboarding(step 0..5 | skipped) · added_at · removed_at?

Task
  id(T-n) · project_id · slug · title · request_text · attachments[] · doc_refs[]
  state(见 §5) · node · design_round · impl_round · budget_n · convergence_mode
  milestones[{id, title, state(open|pending|done), reopen_count}]
  backlog[{id, text, decision(none|include|ignore)}]
  disputes[{id, text, sides, decision(none|a|b|custom(text))}]
  branch · worktree_path · created_at · completed_at? · merge_commit? · archived_at?

Session
  id · task_id · role · runtime · model · effort · skills[] · prompt
  started_at · ended_at? · exit_code? · log_path · outcome(见 §5.3)

RoleConfig { enabled, runtime, model, effort, skills[] }        × 5
ResolvedConfig = global ⊕ project（逐字段覆盖）· 校验结果 · 生效时间

Skill { name · source_dir · scope(global|project) · visible_to{claude, codex} }
SkillBinding { role · skill_name }                              存 config.toml

Environment { git, claude, codex, iterm2 : { present, version, path, login(ok|expired|n/a) } }
```

Task 的 `milestones`、`backlog`、`disputes`、轮次与 `convergence_mode` 由 automed 在每次会话结束后解析设计文档状态块得到，不由 Agent 通过 IPC 上报。

## 5. 任务状态机

### 5.1 状态

```text
Queued · Intake · Design · Review · Adjudicate · AwaitDesignApproval
Implement · Gate · Audit · Retro · Rebase · MergeGate · AwaitMerge · Merging · MainCheck · Cleanup · Done
Paused(resume_node) · Stopped(node) · Failed(reason) · Cancelled
```

界面上的节点线对应 Intake…Done；Queued、Paused、Stopped、Failed 以芯片叠加显示。Gate、MergeGate、MainCheck 只在项目配置了 `[gate]` 时经过（§8.1），节点线上不单占一格，显示在它所检查的那一步（实现、rebase、合并）上。

`Draft`（待开始）没有节点，不占槽位、不算等待用户，也不是终态。`task.start` 施加 `Start`，只有 Draft 接受并转为 `Queued`；其它状态拒绝「只有待开始的任务可以开始」。Draft 可直接取消；暂停拒绝「任务尚未开始」，会话、门禁等其它触发也被拒绝。出处：`crates/autome-domain/src/task.rs` 的 `draft_holds_no_slot_does_not_await_the_user_and_is_not_terminal`、`start_moves_a_draft_to_queued_without_an_action`、`start_is_refused_for_every_state_that_is_not_a_draft`、`a_draft_can_be_cancelled_directly`、`a_draft_refuses_every_other_trigger`。

### 5.2 角色与会话 prompt

**不沿用 1.x 的入口语句。** 1.x 用「execute 任务文件 additional task 3」这样的句子派发轮次，任务文件里对应有编号小节——那层间接存在是因为 Agent 要自己判断轮次并接力。2.0 由内核调度，内核知道这是哪一轮，直接说出来即可。

实测证明这层间接不能照搬：真实运行中生成的任务文件没有名为「Task 1」的小节，四个会话各自打开文件、找不到针对自己的内容，设计文档纹丝未动。

每个角色的 prompt 只说三件事：

| 角色 | 本轮是什么 | 读 | 写 |
|---|---|---|---|
| plan | 设计轮 | 任务文件、设计文档、上轮评审与裁决 | 设计文档（含里程碑表） |
| review | 评审轮 | 任务文件、设计文档 | `<slug>-review.md`（覆盖） |
| adjudicate | 裁决轮 | 任务文件、设计文档、本轮评审、既有裁决 | `<slug>-adjudication.md`（追加）+ 设计文档 |
| impl | 实现轮 | 任务文件、设计文档、上轮审计 | 代码、设计文档里程碑状态、`retro.md` |
| audit | 审计轮 | 任务文件、设计文档 | `<slug>-audit.md`（覆盖）+ 里程碑状态、`retro.md` |

规则本身不在 prompt 里，在任务文件内嵌的 Loop 协议全文里（`.autome/skill/loop-protocol.md`，由任务整理轮逐字复制进去）。

三条每个 prompt 都带的固定条款：完成后结束会话、不要启动下一个会话、结束前提交本轮改动。

### 5.3 转换表

| 当前 | 触发 | 条件（来自状态块） | 下一 |
|---|---|---|---|
| Queued | 槽位空出 | 按提交顺序 | Intake |
| Intake | 会话结束 | 任务文件存在、标题已生成 | Design |
| Intake | 会话结束 | 任务文件缺失 | Failed(protocol) |
| Design | 会话结束 | review 启用 | Review |
| Design | 会话结束 | review 关闭 | AwaitDesignApproval |
| Review | 会话结束 | — | Adjudicate |
| Adjudicate | 会话结束 | 裁决要求复审 且 d < design_rounds | Review（d 已加一） |
| Adjudicate | 会话结束 | 裁决判定定稿 | AwaitDesignApproval |
| Adjudicate | 会话结束 | status = 不可实现 | Failed(infeasible) |
| Adjudicate | 会话结束 | d ≥ design_rounds | Failed(design_budget) |
| AwaitDesignApproval | 用户批准 | 无未消费表态 | Implement（计算 N） |
| AwaitDesignApproval | 用户批准 | 有裁定或纳入 | Design（注入表态） |
| AwaitDesignApproval | 用户驳回 | 附意见 | Design（注入意见） |
| Implement | 会话结束 | audit 启用 | Audit |
| Implement | 会话结束 | audit 关闭 且 全部里程碑 ≠ open | Rebase |
| Implement | 会话结束 | audit 关闭 且 有 open 且 k < N | Implement |
| Audit | 会话结束 | 全部里程碑 done | Rebase |
| Audit | 会话结束 | 有未完成 且 k < N | Implement |
| Audit | 会话结束 | k ≥ N | Failed(impl_budget) |
| Rebase | 内核执行 | 无冲突，配了门禁 | MergeGate |
| Rebase | 内核执行 | 无冲突，未配门禁 | 已授权走合并入口，否则 AwaitMerge |
| Rebase | 内核执行 | 有冲突 | Implement（冲突修复 prompt） |
| MergeGate | 门禁结果 | 通过 | 已授权走合并入口（无纳入表态时 Merging），否则 AwaitMerge |
| MergeGate | 门禁结果 | 失败 | Implement（门禁失败 prompt；熔断时 Failed） |
| MergeGate | 内核执行 | MergeGate 出队冲突 | Implement（冲突修复 prompt；连续冲突或预算用尽时 Failed） |
| AwaitMerge | 用户点合并 | 无纳入表态 且 前置条件满足 | Merging |
| AwaitMerge | 用户点合并 | 有纳入表态 | Implement（新增里程碑，N 加 5 × 新增数） |
| AwaitMerge | 用户点合并 | 前置条件不满足 | AwaitMerge（提示原因） |
| Merging | 内核执行 | 成功 | 配了门禁则 MainCheck，否则 Cleanup |
| Merging | 内核执行 | 落后 / 其它阻塞 | Requeued(Rebase) / AwaitMerge |
| Cleanup | 内核执行 | — | Done |
| 任一运行态 | 用户暂停 | — | Paused（当前会话跑完后不再启动） |
| 任一运行态 | 用户停止 | — | Stopped（杀会话） |
| Failed | 用户追加轮次 | — | 回到失败前节点，上限加值 |
| Failed | 用户从节点重跑 | — | 目标节点，重置该节点之后的计数 |
| 任一非终态 | 用户取消 | — | Cancelled |

会话 `outcome` 由解析结果归类：`advanced`、`needs_rerun`、`design_final`、`milestone_done`、`reopen`、`protocol_error`、`crashed`。

### 5.4 状态块解析

automed 解析设计文档头部的固定字段：`status`、`design-round`、`implementation-round`、`current-milestone`、`current-milestone-reopens`、`convergence-mode`、`next-action`，以及里程碑小节中的三态标记、`## Backlog` 与 `## 争议项`。字段缺失或格式错误一次即判 `protocol_error`，任务进入 Failed(protocol)，用户可选从该节点重跑。解析器有表驱动单元测试与 1.x 真实任务文档回归样本。

### 5.5 计数与预算

- d 由裁决轮加一，k 由实现轮加一，均以状态块为准，automed 只校验单调性。
- N 在首次进入 Implement 时计算：`budget_factor × 初始里程碑数`，写入 Task。追加轮次与纳入 Backlog 修改 Task.budget_n，不改配置。

### 5.6 Bug Loop（kind = bug）

失败或已停止的 Bug 任务接受 `autome reply`，补充文字原样追加到 replies 文档并回到定位轮；满槽时按现有准入排队。已用修复轮次保留，上限设为 `max(原预算或配置 fix_rounds, 已用轮次 + 配置 fix_rounds)`，加法溢出拒绝并提示新建任务。`AwaitInfo` / `NotABug` 的 reply 不改变预算，需求任务的 reply 仍等同 reject。

任务类型 `kind` 为 `feature`（默认）或 `bug`。只有 kind 为 bug，且已记过 `bug.loop_started` 或仍处于 `Draft` / `Queued` 的任务走 Bug Loop；需求任务和已经停在需求节点、尚无该事件的旧 bug 任务继续走 §5.3。Bug 任务在 `Start` 或分到槽位时要求审计角色开启，否则拒绝「Bug 任务需要独立复核，请启用审计角色后启动」。出处：`crates/autome-domain/src/task.rs` 的 `TaskKind`；`crates/autome-domain/src/bug_loop.rs` 的 `uses_bug_loop`、`apply_for`、`bug_loop_legacy_bug_tasks_already_in_feature_nodes_keep_the_feature_table`。

| 节点 | 线上名 | 界面标签 | 执行方 | 占槽位 |
|---|---|---|---|---|
| Locate | `locate` | 定位 | 实现角色会话 | 是 |
| ReproCheck | `repro_check` | 复现检查 | 内核门禁 | 是 |
| Fix | `fix` | 修复 | 实现角色会话 | 是 |
| FixGate | `fix_gate` | 修复门禁 | 内核门禁 | 是 |
| FixReview | `fix_review` | 复核 | 审计角色会话 | 是 |
| AwaitInfo | `await_info` | 等你补充信息 | 等用户 | 否 |
| NotABug | `not_a_bug` | 等你决定（不是 Bug） | 等用户 | 否 |

节点表出处：`crates/autome-domain/src/task.rs` 的 `Node`；`crates/autome-cli/src/render.rs` 的 `node_label`；`crates/autome-domain/src/bug_loop.rs` 的 `bug_loop_nodes_have_wire_names_roles_and_flags`。

下表中自动开 `fix` 均先检查修复轮预算；会话结束行以文档校验通过为前提。转换出处：每行末尾的符号均位于 `crates/autome-domain/src/bug_loop.rs`。

| 当前 | 触发 | 条件 | 下一 |
|---|---|---|---|
| Queued | 分到槽位 | 审计开启 | locate；出处：`apply_for` |
| locate | 会话结束 | status = 已定位 | repro_check；出处：`apply_for` |
| locate | 会话结束 | status = 无法复现 | await_info；出处：`apply_for` |
| locate | 会话结束 | status = 不是 Bug | not_a_bug；出处：`apply_for` |
| locate / fix / fix_review | 会话结束 | 非定位轮 status 不是已定位；或 fix_review 的复核为待复核、其它轮的复核不是待复核 | Failed(Protocol)；出处：`apply_for` |
| locate / fix / fix_review | 会话结束 | 崩溃 / 文档不可解析 / 缺文件 / 守卫失败 | 与需求会话相同：崩溃为 Failed(SessionCrashed)，其余 Failed(Protocol)；出处：`bug_loop_session_errors_fail_like_feature_sessions` |
| repro_check | 复现检查结束 | 成功复现 | fix；出处：`apply_for` |
| repro_check | 复现检查结束 | 第一次未复现 | locate，注入命令、退出码、原因与日志尾；出处：`apply_for` |
| repro_check | 复现检查结束 | 连续第二次未复现 | await_info；出处：`apply_for` |
| fix | 会话结束 | 无论是否配置项目门禁 | fix_gate；出处：`bug_loop_fix_always_runs_the_fix_gate` |
| fix_gate | 门禁结果 | 通过 / 失败 | fix_review / fix（注入门禁命令与日志尾）；出处：`apply_for` |
| fix_review | 会话结束 | 复核通过 / 打回 | Rebase（不经 Retro）/ fix（注入打回）；出处：`apply_for` |
| Rebase | 内核结果 | 成功 / 冲突 | MergeGate / fix（注入冲突文件）；出处：`apply_for` |
| MergeGate | 内核结果 | 出队 rebase 冲突 | fix（注入 RebaseConflict，保留授权、释放锁和门禁位置；预算用尽或连续冲突则 Failed） |
| MergeGate | 门禁结果 | 失败 / 通过 | fix / 共用需求任务 AwaitMerge 以后的尾段；出处：`bug_loop_merge_tail_is_shared_with_the_feature_loop`、`apply_for` |
| await_info / not_a_bug | reply | 非空补充信息 | locate；空白文本拒绝「补充信息不能为空」，其它节点拒绝 reply；出处：`bug_loop_reply_is_refused_when_empty_or_not_waiting`、`apply_for` |
| await_info / not_a_bug | 用户操作 | 取消 / 暂停、停止、追加轮次 | Cancelled / 拒绝；出处：`bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend` |
| Failed(FixBudget) | 追加 n 轮 | n > 0（合法追加） | 上限加 n，直接开 fix；出处：`apply_for` |
| Failed / Stopped | rerun | locate、fix、fix_review、rebase | 从指定节点重跑；其它节点拒绝「Bug 任务只能从定位、修复、复核或 rebase 重跑」（同门禁熔断例外见下行）；出处：`apply_for` |
| fix_gate / MergeGate | 同因失败熔断 | 当前节点匹配 | Failed(GateSameFailure)；从同门禁 rerun 或追加轮次均丢弃旧门禁结果再跑，追加同时提高修复轮上限；出处：`same_failure_bug_rerun_and_extend`、`apply_for` |
| Stopped(repro_check / fix_gate) | Resume | — | Requeued，丢弃被杀的门禁结果，分到槽位后重新执行；出处：`bug_loop_stopped_bug_gates_discard_runs_and_requeued_bug_nodes_reenter` |

Queued 首次进入定位时记录 `bug.loop_started`。出处：`crates/automed/src/scheduler.rs` 的 `advance`。

**复现检查的退出码。** `repro_check` 只跑 `bug.located` 事件中的复现命令；超时使用 `[gate] timeout_minutes`，未配置门禁时为 30 分钟。命令非零退出、不是 `126` / `127` 且未超时，才算复现成功。所谓「126/127 不算失败」指它们不能证明 Bug 已复现：退出码 0、126、127 或超时都算未复现，原因分别为 `exited_zero`、`not_runnable`、`timed_out`；宿主门禁超时退出码为 `124`，判定同时看超时标志。出处：`crates/automed/src/scheduler.rs` 的 `run_gate`；`crates/autome-domain/src/bug_loop.rs` 的 `apply_for`、`bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced`；`crates/automed/src/gate.rs` 的 `TIMED_OUT`；`crates/autome-domain/src/config.rs` 的 `DEFAULT_GATE_TIMEOUT_MINUTES`。

**连续两次未复现。** 「连续两次复现检查失败」指未复现：第一次回定位，第二次进入「等你补充信息」。计数取最近一次 `task.replied` 之后的 `bug.repro_missed` 事件数，因此 reply 后重新计数。出处：`crates/autome-domain/src/bug_loop.rs` 的 `bug_loop_second_consecutive_miss_waits_for_info`；`crates/automed/src/scheduler.rs` 的 `bug_progress`。

**合并前门禁总带复现命令。** `fix_gate` 和 `MergeGate` 选取 `bug.reproduced` 事件中的复现命令；配置了项目 `[gate]` 时组合为 `( 复现命令 ) && ( 门禁命令 )`，否则只跑复现命令。`before_merge` 下 FixGate 未配 quick 时只跑复现，配了 quick 时跑 `( 复现命令 ) && ( quick 命令 )`。`MainCheck` 只选择项目门禁。出处：`crates/automed/src/scheduler.rs` 的 `run_gate`。

**修复轮预算。** 每开一个 fix 会话（含手动重跑）记一条 `bug.fix_round_started`。上限取任务预算，未设时取 `[bug] fix_rounds`；首次复现成功和门禁失败、复核打回、rebase 冲突、合并门禁失败等自动转换，均在开下一轮前检查，已开轮数达到上限即 `Failed(FixBudget)`。追加轮次增加上限并恢复 fix。手动 `rerun … fix` 同样经 `start_fix` 检查预算；已开轮数达到上限时保持 `Failed(FixBudget)`，不启动会话，须用 `autome extend <任务> <n>` 追加修复轮。预算内重跑仍计入已开轮数。出处：`crates/automed/src/scheduler.rs` 的 `bug_progress`、`perform`；`crates/autome-domain/src/bug_loop.rs` 的 `start_fix`、`apply_for`、`bug_loop_extend_resumes_fix_with_a_larger_limit`。

Bug 文档包含 `status`（已定位 / 无法复现 / 不是 Bug）、`fix-round`、`复现命令`、`根因`、`复核`（待复核 / 通过 / 打回）。会话模板为 `protocol/prompts/locate.md`、`protocol/prompts/fix.md`、`protocol/prompts/fix-review.md`，规则为 `protocol/bug-protocol.md`，编译进 automed。出处：`crates/autome-domain/src/bug_doc.rs` 的 `parse`；`crates/automed/src/protocol/mod.rs` 的 `SEED`。

**定位阶段累计保护。** 普通定位和修测试共用 `scheduler/test_fix.rs` 的 `regression_set`：
以任务最早定位会话的各仓库 `locate_heads` 为基线，非 HEAD 祖先时回退到与默认分支的 merge-base；
累计测试、当前会话测试与上一快照路径取并集。普通定位仍只冻结新增测试块，不冻结产品辅助代码。
未定位会话留下的测试即使没有快照也会纳入。普通定位为空时先走复现判定，真正复现后才拒绝；
直接重跑 fix 而无快照时，启动会话前现算 `source: rerun_fix` 快照，空集或读盘失败转 `Failed(Fix, Protocol)`，不启动修复会话。

**修正回归测试。**

冻结的 `prompts/fix.md` 含 `<!-- regression-test-defect -->` 时启用修测试路径。
`bug.test_fix_requested` 记录声明会话、来源轮次、次数与上限（2）；
`bug.test_fix_settled` 记录 accepted / needs_fix 和基线、HEAD 退出码。
`task.get.task.test_fix` 暴露 used、limit、open；CLI 事件流与定位状态显示此模式。
声明轮不能改受保护测试；修测试定位轮只允许任务文档、既有整文件测试与支持文件、
新增测试及源文件的测试段，命令点名既有产品脚本不授予写权限。缺陷四项必须非空，
定位结束必须删除声明。新的保护快照包含先前快照和定位阶段累计测试改动。
成对检查仍占用一个 repro 作业，基线取 merge-base，覆盖修正后的测试和 support，
内联测试保留基线产品部分。缺少显式命令依赖或无法构造基线按未复现处理，
不能借无关构建失败接受测试。基线红、HEAD 绿进入 FixGate / FixReview；HEAD 红
正常计修复轮；基线不红沿用两次未复现转 AwaitInfo 的规则。临时检出检查结束后移除。

## 6. Git 工作区操作

所有 Git 命令由 automed 以固定二进制、净化环境执行，禁用 credential helper，不触碰远端。

| 操作 | 命令 | 备注 |
|---|---|---|
| 识别默认分支 | `symbolic-ref refs/remotes/origin/HEAD`，无远端时取 `init.defaultBranch`，再无则当前 HEAD | 结果存 Project |
| 初始化 | `git init -b <default>`（非 Git 目录）；追加 .gitignore；写 .autome | init commit 在 Onboarding 完成或跳过后创建 |
| 创建任务 | `git worktree add .worktree/<slug> -b autome/<slug> <default>`；写 `docs/<slug>/attachments/`；在任务分支提交「chore(autome): T-n inputs」 | 任务分支上的提交由 Autome 与 Agent 共同产生 |
| 会话提交 | Agent 在 worktree 内按协议自行提交；会话结束后 automed 兜底提交 worktree 内剩余的未提交改动 | 兜底提交只作用于该任务自己的 worktree，提交信息标注为「未提交的剩余改动」 |
| rebase | `git -C .worktree/<slug> rebase <default>` | 冲突时 `rebase --abort`，记录冲突文件，转 Implement |
| 合并前置 | 主工作树中**本次合并会改动的文件**没有未提交改动；任务分支 `merge-base --is-ancestor <default> autome/<slug>` | 不满足即拒绝。判据不是「工作树干净」——界面改配置会留下未提交的 `.autome/config.toml`，按前者会让此后每次合并都被拒 |
| 合并 | 在项目根 `git merge --no-ff autome/<slug> -m "merge(autome): T-n <title>"` | 主工作树随之更新 |
| 清理 | `git worktree remove .worktree/<slug>`；`git branch -d autome/<slug>` | 失败时保留并提示 |
| 取消 | 复制 `docs/<slug>/` 到项目根 `docs/.archive/<slug>/`；`worktree remove --force`；`branch -D` | 复制结果留在主工作树，不提交 |
| 归档 / 恢复 | 项目根 `git mv docs/<slug> docs/.archive/<slug>` 及反向 | 不提交 |

**集成复验的提交范围。** 需求实现轮在非空且全 `已完成` 的里程碑表上开始、结束后仍全关闭时，
按仓库用首次尝试的 `session.started.start_heads` 作 H0（简报提交之后取值，崩溃重试沿用），
兜底提交后的 HEAD 作 H1，B1 = `git merge-base H1 <slot.base>`。
列出范围为 `git rev-list --reverse --cherry-pick --right-only H0...H1 ^B1`，
消失范围将 `--right-only` 换为 `--left-only`。线性修复等价于 H0..H1；
rebase 用 patch-id 等价比较列出改写或新增提交，另列原分支中消失的补丁，排除默认分支和干净重放。
触及已知冲突文件的提交保守纳入，即使 patch-id 相同；列出或消失的提交至少一个修改任务文档
目录外路径才构成集成改动。消失提交也保留路径与统计，工作区路径带仓库名。
判定事实读取失败时进入 Failed(Config)，不当作空改动放行；仅 stat 等展示信息失败时保留范围并注明读取失败。

## 7. 会话启动器

### 7.1 启动

1. 解析生效配置，校验 SAME-MODEL 与技能可见性；不通过则任务进入 Failed(config)，不启动。
2. 组装 prompt：入口语句 + 绑定技能条款「本会话必须使用技能 X、Y」+ 冲突修复或用户意见附加段（如有）。
3. 组装命令：`run_session.sh <session-id> <role> <runtime> <model> <effort> "<prompt>"`，工作目录为任务 worktree。
4. 直接 spawn 包装脚本，stdio 全部指向 /dev/null，输出由包装脚本 tee 进会话日志。不开终端窗口、不抢焦点。（`launcher::LaunchMode` 仍保留 `Terminal` 模式，但产品里没有调用方。）
5. 写 Session 记录，状态进入对应节点。

### 7.2 CLI 参数映射

模型、Effort、自动执行模式的命令行参数按 CLI 版本可能变化，放在 adapter 表中，不硬编码在状态机里。安装或版本变化时用 `--help` 探测校正。自动执行模式沿用各 CLI 的非交互自动模式；会话工作目录限定在 worktree，不做 09-13 方案的沙箱资格体系。这是有意接受的风险，见 §17。

### 7.3 结束与解析

会话生命周期包括 `Running`、`Exited`、`Killed`、`Vanished`、`TimedOut { limit_minutes }`。超时终止确认进程组不存在后才收割，按崩溃策略计次重试；不解析被中断的文档，重派沿用逻辑轮号并注入超时说明与最后 40 行日志。


退出标记出现后：读取 exit code；exit 非零或日志空白判 `crashed`；否则解析状态块（§5.4）并按转换表推进。日志路径写入 Session，界面按需读取。

### 7.4 会话写入范围与越界还原

审计、复盘、Bug 复核只允许修改本任务文档目录。`sweep_commit` 后按会话 `scope_heads` 基线计算越界路径；首次越界先存 binary 补丁到主检出 `.autome/output/scope/<任务>/<会话>.patch`，工作区项目每仓库一份 `<会话>--<成员>.patch`。先持久登记 `session.scope_restore_begun`，再只还原越界路径、核对 `git apply --check`、提交并记 `session.scope_restored`；文档改动保持不变，不 reset 分支。启动恢复与会话启动前接续未完成意图；失败时路径级补偿到会话结束的树，记 `session.scope_restore_failed`，沿用协议失败与手工步骤并补「自动还原没有完成」。

熔断只看本任务最近一个已结束的同角色会话：有 begun 且无 failed 即为生效的还原，连续第二次越界不再还原，原失败详情追加「上一次已自动还原」。需求审计和 Bug 复核角色均为 audit，手动与流程内复盘均为 retro；实现/修复轮不打断，同角色一次干净会话打断。判断来自持久账本，重启与 extend 不清零；熔断轮没有 begun，因此手工处理后 rerun 的下一次越界重新算首次。

待处理由 begun 排除 failed 与 scope_patches_handled 推出，每次启动实现/修复轮写入简报或 prompt，并在 session.started 的 scope_patches 列出；会话正常结束且过守卫才记 session.scope_patches_handled，崩溃不消费。是否采纳由会话判断。需求最后审计/复盘仍有补丁时补实现 → 审计，不受实现预算拦截；Bug 复核按不通过退回修复，仍遵守修复预算。手动复盘不改变已终结任务状态，补丁留待人处理或 rerun 后交接。补丁轮沿用现有审计路径，与 T-68 的衔接标记为 session.started.scope_patches。

CLI run 展示还原、失败、已处理三种事件，begun 不打印；还原行含角色、轮次、总文件数、前 3 个路径及首份补丁（多份带份数），Bug 指向修复轮。task.get 顶层 scope_restore 返回最近 begun 载荷与账本时间 at、待处理补丁路径 pending；无记录 last 为 null，不改变 task 对象键。status 以本地时间显示最近还原记录与待处理/已处理；失败详情保留手工恢复步骤。prompt 与协议正文不变，边界不放宽。


### 里程碑关闭之后的交接轮

域上下文 `handoff_pending` 由 `handoff::pending` 从账本计算；目前来源是未处理的越界补丁，操作者备注接入后成为第二类来源。交接文案统一由 `handoff::sections` 写入需求实现轮简报或 Bug 修复 prompt。审计全部关闭里程碑、复盘结束时仍有交接工作则补实现 → 审计；Bug 复核通过时仍有补丁则按复核不通过退回修复，遵守修复预算。熔断仍按 §7.4 的同角色已结束会话口径计算。

全部里程碑已完成时，补丁轮只处理补丁，不改里程碑表或已完成里程碑的最新证据指针；证据写最后一个里程碑本实现轮的文件。产生目录外产品提交则由 T-68 记录 `integration.range`，触发标签 `scope_patch`（越界补丁交接），随后集成复验；仅文档改动仍走普通审计。会话正常收尾才记已处理，崩溃重试继续交接。

## 8. 调度器


- 每个项目一个信号量，容量为 `parallel_limit`。占用槽位的状态：Intake、Design、Review、Adjudicate、Implement、Audit、Rebase、Merging、Cleanup。AwaitDesignApproval、AwaitMerge、Paused、Stopped、Failed 不占槽位。
- 排队按 `queued_seq`、`created_at` 顺序，可用 `task.move` 调整；槽位释放后依次检查，跳过依赖尚未全部到 `Done` 的受约束任务，保留其队列位置，让后面的任务先走。约束适用于 `Queued`，以及需求任务在设计批准前节点的 `Requeued`；实现期和 Bug 的 `Requeued` 不受依赖约束。依赖取消或失败时任务继续排队，用户可修改或清空依赖。
- 单个任务同一时刻最多一个会话。
- 暂停在当前会话结束后生效；恢复时若槽位不足则排队。
- 一次 tick 的顺序：收已结束的会话 → 启动到点的重试 → 跑内核步骤（rebase、门禁、合并、清理）→ 填空槽位。

**集成复验。** `apply_guards` 在兜底提交与会话范围检查后记录 `integration.range`，携带首次实现
会话 ID、实现轮号、触发原因和各仓库提交范围。仅紧随该实现会话的审计使用它；后续实现会话使旧范围失效。
任务固定协议的审计模板含 `<!-- integration-audit -->` 标记时启用简报段、prompt 分支及强制集成审计守卫。
旧模板缺标记但已有 `integration.range` 时载荷带 `legacy: true`，内核在简报补充目录外提交范围、独立复跑验收与证据要求，
不改冻结的 prompt；此时缺证据的 `audit_made_no_progress` / `evidence_missing` 只提醒，v8 模板的强制行为不变。首次审计尝试记录指定证据路径的存在、mtime_ns 与长度，收尾比较是否更新，
防止实现轮预写证据绕过复验。无证据且表不动报 `audit_made_no_progress`；退回也必须有证据。
通过时表不动，沿原状态机进入复盘 / rebase / 合并卡点；退回则计 reopen，按原预算返回实现轮，
不新增节点或回边。所有需求实现轮另检查已完成里程碑的最新证据指针：改指向新的实现证据时，内核仅还原受影响里程碑的指针行
（保留其它正文、开放里程碑与代码围栏，多出的指针删除），做独立提交并记 `guard.auto_fixed {code, milestones, commit, protocol, detail}`，
`autome run` 显示一行说明。v8 之前说明旧协议没有这条规则，v8 及以后说明违反指针规则；无法解析版本时也自动还原并明确说明。
文档无法识别、工作区不干净、读写或提交失败时仍报 `impl_closed_milestone`，提交失败尝试恢复会话原文和索引。
这是内核不改写会话文档的窄例外；实现轮擅自把里程碑关闭仍为错误，Bug Loop 不进入此路径。

新增 error 级守卫若依赖某协议版本才写进 prompt 的规则，对更早版本的任务只能是提醒或自动修正。

### 8.1 驱动锁、门禁、重试与冷却、表态租约

**谁能推进账本。** 构建标识为祖先提交数（7 位）、提交时间与短 SHA；只按祖先数比较，时间不排序。同一提交的 CLI 与内核共用标识；无 Git 或浅克隆为 unknown。未知不能覆盖已知，两个不同 unknown 或同计数不同 SHA 按旧内核拒绝。所有 READ_METHODS 之外的方法都算推进。tick 与写命令先无锁只读比较，再在驱动锁内 claim，锁覆盖比较到最后一个副作用；取锁 I/O 失败不推进。检查先于 reclaim、recover、固定协议和启动会话。旧内核只读，拒绝不改变任务或新增 task.updated。AUTOME_ALLOW_OLDER_KERNEL=1 是显式逃生口，每进程最多一次，事务内降低标记并记 kernel.adopted（from/to/via/method）；新内核再来会抬高标记，旧进程随后被拒。

**宿主准备。** 项目 `[setup]` 在任务检出建好后的会话前、各门禁前和合并后 Cleanup 前检查准备指纹；MainCheck 的 detached 检出单独准备。指纹包含命令与 watch 文件内容（不可读取记 absent），成功才原子保存到 `.autome/output/setup/`，任务检出、verify 检出、主检出分别记录，主检出记录跨任务共享；新建检出清除旧记录。空 watch 只比较命令。复用门禁 wrapper、独立进程组与 pid/exit/started/log，tick 只轮询，重启接续，超时退出码 124。先收在途作业并记录启动指纹，再按最新输入判断。准备命令与门禁共用 `[gate] concurrency` 名额，任务检出、verify 检出、主检出均计入；指纹未变、未配置或工作区跳过准备时不排队。门禁前准备与随后的完整门禁在持续占位、期间无 task.updated 且退出标记未过占位期限时连占同一名额，否则后续进程重新准入；轻车道检查始终重新准入，不能继承准备命令的重车道名额。会话前准备排队不计崩溃重试或 Bug Loop 修复轮次，Fix 轮次到真正启动会话才记录。主检出准备先检查指纹和其它任务的主检出准备互斥，再申请名额，包含尚无 pid 的启动空窗。任务检出准备失败进入 Failed（命令、退出码、日志），不退回实现轮；verify 失败跳过 MainCheck 并报警，主检出失败报警后照常清理完成，均不撤销合并。暂停后恢复收同一作业，停止/取消杀进程并清准备标记。写 setup.started/result；工作区三个时机都跳过，每任务只写一次 setup.skipped；未配置时无准备事件或等待。

**合并后命令。** 项目 `[merge] after` 是 Cleanup 的前置子阶段，不新增节点：合并成功、MainCheck（若配置）通过、主检出准备处理完后，在清理任务 worktree / 分支之前，由宿主在 `project.path` 用 `sh -c` 执行；准备失败仍运行钩子。命令获得 `AUTOME_TASK` 与 `AUTOME_MERGE_COMMIT`（本任务完整合并 SHA），主检出可能已有后续合并。工作区、无单一合并提交、MainCheck 失败或跳过时只写一次 `merge.after_skipped`；Bug Loop 同样适用。

复用 `gate::Job` 独立进程组，文件为 `.autome/output/gates/<task>/after_merge-<sha12>.{log,pid,exit,started}`，tick 只轮询，core 重启接续；启动前等待其它任务在途的钩子或主检出准备（含尚无 pid 的启动空窗）。`after_merge.prepared` 标记准备段已处理，钩子等待或重入不重复准备；不改变 setup 自己的互斥规则。写 `merge.after_started {command, commit, log}` 与 `merge.after {ok, exit_code, command, commit, log, tail}`，CLI 打印开始、结果或跳过原因。非零退出或超时（缺省 10 分钟，杀进程组、退出码 124）仅报 `after_merge_failed`，失败警报与结果事件同事务写入；已有结果则不再执行，照常清理并进入 Done，不撤销合并。作业存在但开始事件缺失时补事件，不重启；作业消失则清文件、下次 tick 重试。暂停后恢复接同一作业；停止/取消杀进程并删作业文件，停止后恢复重新运行，取消不报失败。停留期间删除配置则直接清理，在途命令后台结束且不收结果。命令应可重复执行，只写 gitignore 产物或检出之外，避免弄脏主检出阻塞后续合并。

**驱动锁。** 推进 Loop 的进程不止一个：桌面端的 sidecar 和 `autome run` 都会 tick。两者打开同一个数据库，在它旁边的 `<db>.driver.lock` 上用 `flock` 轮流持锁。tick 拿不到锁就跳过这一轮（返回 `skipped`），写命令最多等 10 秒。这样两个驱动同时开着，也不会把同一个任务的会话启动两次。

**宿主门禁。** 项目配置 `[gate] command` 后，实现轮结束进入 Gate 节点：内核在任务 worktree 里用 `sh -c` 跑这条命令，跑在后台作业里（独立进程组，日志与退出标记写在 `.autome/output/gates/<task>/`），tick 只轮询，不阻塞 IPC。结果绑定当时的 head（工作区项目取所有仓库 head 的 sha256），作业结束时 head 已变则作废重跑。通过才进审计（或无审计时的下一步）；失败回实现轮，注入 `GateFailed{head, command, tail}`，这一轮豁免「必须有开放里程碑」的守卫，因为它修的是已待审的里程碑。rebase 之后在新 head 上再跑一次（MergeGate），与上次通过的 sha 相同且分支不落后才可沿用；失败同样回实现轮。合并之后在 `.worktree/_verify-<sha>` 的 detached 检出上对 main 跑最后一次（MainCheck）：合并已发生，撤不回，所以失败只写一条项目报警（`project_alerts`，仪表盘和 `autome status` 都显示，用户关闭），任务照常完成。默认 `[gate] reuse_for_main_check = true`：合并提交与最近通过 MergeGate 的提交树相同，且被合并的 head 与实际启动命令均匹配时，沿用该结果并写 `gate.reused`，不创建验证检出、不启动 MainCheck 作业；设为 `false` 则始终照常运行。无法确认通过记录、命令或树时也照常运行。同一任务同一 `gate` / `merge_gate` / `fix_gate` 节点连续相同失败指纹达到 `[gate] same_failure_limit`（默认 3，整数 1–10）时熔断，任务失败并附原因摘要与最近日志路径；同节点通过或指纹变化打断连续计数，修复环境后可用 `rerun` / `extend` 重跑，熔断后的新运行从零计数；`repro_check` 与 `main_check` 不参与熔断。每次实际执行的门禁写一条 `gate.result` 事件，审计轮简报的「宿主门禁」段引用它。

宿主作业按实际启动种类分两条独立 FIFO 车道：`full` 与准备命令走重车道（`[gate] concurrency`），`quick` / `repro` 走轻车道（`[gate] quick_concurrency`）。两键全局缺省均为 1、整数 1–10，项目覆盖由全局封顶；只写任一并发键不启用门禁命令。推荐按机器核数设置 quick_concurrency 为 1–2。按一个账本内所有项目同车道的在途作业数与当前项目有效限额比较，不扫描全机进程，也不限制会话内的命令。各车道内严格 FIFO，无节点优先级；排队保留原节点及项目并行槽位，不写作业文件或 started 事件、不消耗预算与失败/重试计数，超时从真正启动的 `.started` 算起。`gate.queued.position` 是入队时从 1 起的快照，`task.get` 与 `dashboard.get` 的 `gate_queue_position` 是同车道实时名次（非排队为 null），`gate_queue_lane` 为 heavy / light 或 null，不复用 `queue_position`；CLI 区分「排队等完整门禁（前面还有 k 个）」与「排队等快检或复现检查（前面还有 k 个）」，k = 名次 − 1。`gate.queued.lane` 持久化车道，缺字段的旧事件归重车道；占位按 `.started` 旁 `.kind` 的启动种类判定，未知归重车道。排队期间重算车道改变时重新进新车道队尾，已经启动的作业不换车道。有效排队项按事件 seq 恢复，启动、状态变化、会话冷却或 `gate.queue_dropped` 使其失效，再来排队尾；准入前出错的队首退队并报告错误。正常结束、失败、超时或作废后释放名额；暂停的活作业继续占位，每 tick 全任务超时回收，已退出未收作业有 tick 期限，避免永久占位。无法读作业目录且账本不能证明无在途作业时保守暂停新准入并报告恢复目录或取消任务的提示。升级时已有超额作业正常收取，不为限额杀掉它们。内核超时终止行在注入、警报与 CLI 中明确显示「门禁超时」；命令自行返回 124 而无终止行不使用该文案，但保留既有熔断指纹将任何 124 视为超时的兼容行为，其摘要仍按超时显示。

**两级门禁。** 项目 `[gate] quick` 在 `before_merge` 下于每个实现轮后运行，Bug 修复轮跑复现命令和快检；`each_round` 忽略 quick。快检建议只放格式、lint、类型检查并控制在 1 分钟内，测试留给合并前完整门禁。完整命令是否包含快检内容由项目决定，Autome 不拼接；快检结果永不替代 MergeGate。`quick_timeout_minutes` 缺省 5 分钟（正整数），Bug FixGate 使用完整门禁超时。脚本通过 `AUTOME_GATE_KIND=quick|full|repro` 区分运行种类。快检使用独立轻车道，不等待其它任务的完整门禁。

**合并前门禁模式。** 项目 `[gate] run = "before_merge"` 把完整门禁次数从「每轮一次」降到「每个任务一次」，由宿主在 MergeGate 统一运行；代价是门禁问题与回归到合并前才暴露，失败会多退回一次实现轮，修好后再过 MergeGate。会话不再各自兜底：plan、impl、audit、fix、fix-review 五份模板按 `before_merge` 选段，宿主通过 `gate_line` 注入完整门禁命令及本轮不要自行运行的提示；设计轮不把全量门禁列为里程碑验收，实现、审计、修复、复核轮只运行里程碑验收或复现命令与直接相关的测试子集。未配 quick 时需求实现轮跳过 Gate，直接进入审计或无审计时的下一步；配了 quick 则进入 Gate 跑快检。Bug 修复轮在 FixGate 跑复现命令，配了 quick 时再跑快检；Bug MergeGate 仍跑复现命令与项目门禁。审计简报没有本轮完整门禁结果时，以及 Bug 复核 prompt 中，明确写「本项目门禁只在合并前运行，本轮没有宿主门禁结果」，不把旧结果或仅复现通过当成本轮门禁证据；有快检结果时明确注明本轮只跑快检及命令、退出码，不能当作完整门禁证据。任务停在 Gate 时切换到此模式且未配 quick，宿主清理该节点作业并写 `gate.skipped` 后继续；MergeGate 的失败重跑与合并后的 MainCheck 保持上述行为。缺省及显式 `each_round` 的行为不变。

**重试与冷却。** 会话结果为崩溃（非零退出、进程消失）且不是人叫停的，不推进，写一行 `session_retries`，退避 `backoff_seconds × 2^(n-1)` 后由 tick 重派，带着原来的注入。次数上限为 `[retry] attempts`（默认 2），用尽才 Failed。输出不可解析、缺产物、守卫失败不重试——那是协议问题。崩溃输出里能认出限流（429、rate limit、usage limit、overloaded）时，给这个 CLI 写一条冷却（10 分钟起，连续限流翻倍，最长 60 分钟；CLI 给了重置时间就用它），这次不计入次数。冷却期间要启动的角色，若改派到另一个 CLI 仍满足 SAME-MODEL 的分离（按 runtime 粒度判断），就改派并记 `session.rerouted`；否则任务留在原节点，等冷却结束。

**表态租约。** 用户对 Backlog 与争议项的表态，不再在会话启动前就记为已消费。启动带 `Decisions` 注入的会话时，把「已表态、未消费、未被别的会话带走」的条目标上 `carried_by = 会话`，prompt 与简报只列这批。会话正常结束才写 `consumed_by`；崩溃、守卫失败或被叫停，就清掉 `carried_by`，下一个会话重新带上。复盘关闭时的忽略项等没有会话可携带的路径，照旧直接消费。

**合并队列。** 保留入队前的 Rebase；需求与 Bug 的 MergeGate 在真正出队时重新检查各检出 `git rev-list --count <head>..<base>`，落后才 rebase。沿用结果除原有 SHA/命令条件外还要求不落后。已授权任务先按 `gate.queued` 的 seq 取得项目合并资格，写 `merge.locked {node}`，再检查沿用、准备、宿主准入与出队对齐。对齐改变准备输入时重跑准备，期间仍持锁，下一次启动门禁前重新检查基线；真正对齐写 `merge.rebased {at: "dequeue", repos: [{repo, onto, base_sha, head}]}`。

锁覆盖准备和等待宿主名额、出队 rebase、MergeGate、Merging，到离开 Active MergeGate / Merging 时写 `merge.unlocked {reason}`。MainCheck、Cleanup、`[merge] after` 在锁外。其它任务的 MergeGate 不开跑、Merging 不执行，写 `merge.waiting {node, holder}`；等锁者保留 FIFO seq，但从宿主可准入队列排除，`gate_queue_position` 为 null，不堵其它项目。持锁者仍遵守宿主准入限额；`concurrency > 1` 也不允许同项目第二个已授权任务越过锁。已在运行的未授权门禁不被中断，中途授权不补拿锁，其 Merging 仍等待已有持锁者。

门禁失败、熔断、准备失败、出队冲突、暂停、停止、取消，以及合并成功、阻塞、落后或失败离开临界区时释放锁；授权保留。撤销授权立即解锁，已启动的门禁照常完成，通过后停 AwaitMerge。作业消失或 head 改变而仍在 MergeGate 时保留锁并重新准入、对齐；移除项目门禁时需求任务按通过处理，有授权则持锁进入 Merging。持有关系由最后一条 `merge.locked` / `merge.unlocked` 与当前 Active 节点共同推导，重启从账本恢复队列、授权和锁，不依赖内存。未授权任务停在 AwaitMerge 不持锁；没有项目门禁的需求任务不拿锁，但 Merging 服从其它持锁者。外部工具或用户提交不受锁约束，仍由 Merging 的 Behind 路径释放锁、排队 rebase 后重试。

MergeGate 出队冲突复用 RebaseConflict，需求回 Implement、Bug 回 fix；同文件集连续三次冲突或预算用尽会失败。未启动门禁、不写 gate.started，状态变化让排队条目失效，名额让给下一任务；修好重新排队尾。出处：`scheduler.rs` 的 `run_gate` / `advance`、两个 crate 的 `merge_queue.rs` 与 `gate_queue.rs`。

项目 `[merge] auto` 仅接受布尔值，缺省 false，与 `after` 共存，`save_project` 保留整个表。开启可省人工确认，但通过的修改会未经最终人工检查进入 main；未配置 `[gate]` 时只给 `config_warnings` 警告，需求任务 rebase 后直接合并，Bug 仍跑复现命令。有效授权每次读取：最后一条 `merge.auto` 事件 active=true 表示 user/behind 授权；active=false 且 by=user 表示显式撤销，覆盖项目开关；无记录或旧版无 by 的 false 才回落到项目 auto。`merge.auto` 事件是单任务授权记录，不是配置开关。冲突、失败、暂停、停止、重跑均不自动清授权；设计批准、Bug 等人卡点照常停，Blocked 返回 AwaitMerge 等用户处理。直接合并写 `merge.direct {by}`，`autome run` 打印出队对齐和已授权直接合并信息。

### 8.2 任务卡正文、附件与待开始（Draft）

`task.create` 的 `kind` 缺省为 `feature`，只接受 `feature` / `bug`，其它值拒绝。可选的 `brief` 是任务卡正文，必须是字符串，去空白为空或超过 65536 字节均拒绝（分别报「任务卡正文必须是字符串」「任务卡正文不能为空」「任务卡正文超过 64KB」）。出处：`crates/automed/src/dispatch.rs` 的 `task_create`。

提供 `brief` 而没有 `request` 时，从正文第一个 `# ` 行取标题；没有这样的行则取第一非空行并去掉前导 `#` 与空白，结果仍为空时报「任务卡正文缺少标题：第一行写 `# 标题`」。两者都提供时由 `request` 命名任务，正文原样保留。出处：`crates/automed/src/dispatch.rs` 的 `task_create`、测试 `a_brief_without_a_request_takes_its_request_from_the_first_heading`、`with_both_the_request_names_the_task_and_the_brief_is_kept_verbatim`。

正文原样存入 SQLite 的 `task_briefs` 表；`task.get` 返回 `brief`，没有正文时显式为 `null`，任务列表不带正文。附件 `attachments` 在建卡时逐个检查是否为文件，否则报「附件不存在：<路径>」。出处：`crates/automed/src/store.rs` 的 `create_task`、`task_brief`；`crates/automed/src/dispatch.rs` 的 `task_create`、测试 `archived_brief_remains_readable_and_lists_omit_it`、`missing_brief_is_explicit_null`。

`draft: true` 创建 `Draft`（待开始），不触发调度、不建 worktree，返回 `started: false`、`queue_position: null`。非草稿 Bug 任务建卡时即检查审计角色是否开启；草稿到开始时再检查（§5.6）。出处：`crates/automed/src/dispatch.rs` 的 `create`、测试 `a_draft_is_created_without_starting_or_a_queue_position_and_is_not_on_the_dashboard`；`crates/autome-domain/src/bug_loop.rs` 的 `apply_for`。

`autome task start <任务>` 对应 `task.start`：施加 `Start`，只有 `Draft` 可转为 `Queued`，其它状态拒绝「只有待开始的任务可以开始」；记 `task.started` 后调度一次，返回 `task`、`started`、`queue_position`。草稿也可直接取消、归档，全程无需建立 worktree。出处：`crates/automed/src/dispatch.rs` 的 `task_start`、测试 `task_start_refuses_a_task_that_is_not_a_draft`、`a_draft_is_cancelled_and_archived_without_a_worktree`；`crates/autome-domain/src/task.rs` 的 `apply`；`crates/autome-cli/src/main.rs` 的 `USAGE`。

任务检出建好后、首个会话前，`write_task_inputs` 把正文写成 `<doc_dir>/<slug>-card.md`（已有文件不覆盖），附件仅首次复制到 `<doc_dir>/attachments/`；这些输入以一条 `chore(autome): <任务 id> inputs` 提交进任务分支。出处：`crates/automed/src/scheduler.rs` 的 `write_task_inputs` 及其会话启动调用处。

## 9. 配置解析

项目级 `[merge]` 必须是表；`after` 缺省或去首尾空白后为空等于未配置，此时不校验超时；非字符串报错。非空命令的 `after_timeout_minutes` 缺省 10，必须是正整数，上限截到 `u32::MAX`，未知键忽略。领域类型为 `MergeAfterConfig { command, timeout_minutes }`，`ProjectConfig` / `ResolvedConfig` 的 `merge_after` 为可选值，只从项目复制、不从全局继承；保存其它配置时原样保留该表。未配置时不新增事件、作业或节点停留。

项目级 `[setup]` 使用 `command`、`watch`、`timeout_minutes`：必须是表，command 必须是字符串，去空白为空等于未配置（不再校验其余字段）。watch 缺省空列表，否则必须是字符串数组，每项去空白后非空、非绝对路径、不含 `..` 路径段；相对检出根目录。timeout_minutes 缺省 30，必须是正整数。保存其它配置时原样保留该表；生效配置从项目复制，不从全局继承。

- 生效配置 = 全局 `~/.autome/config.toml` 逐字段被项目 `.autome/config.toml` 覆盖。「恢复默认」即删除项目文件中的该字段。
- 保存前校验：runtime 合法；review ≠ plan、audit ≠ impl（比较 `runtime:model` 字面值）；每个绑定技能对该角色 runtime 可见；parallel 1..5。任一失败返回结构化原因，界面据此标红。
- automed 监听两份配置文件变化，新会话启动时读取最新生效值。运行中会话不受影响。
- 界面保存只写文件，不执行 git 提交。

项目级 `[bug] fix_rounds` 缺省为 3，必须是 1 到 20 的整数，否则报「[bug] fix_rounds 必须是 1 到 20 的整数」；`[bug]` 只在项目配置中提供，不从全局继承。出处：`crates/automed/src/config_io.rs` 的 `parse_project`；`crates/autome-domain/src/config.rs` 的 `DEFAULT_FIX_ROUNDS`、`resolve`。

## 10. Onboarding 与初始化

初始化（幂等）：创建 `.autome/` 骨架与 `config.toml`（仅 `[loop]` 空节，全部继承全局）；追加 .gitignore；AGENTS.md 不存在时创建含规则段的最小版本，存在时只追加标记段。

Onboarding 第 3 步以专用 prompt 启动 Claude Code 会话（工作目录为项目根，不建 worktree）：调研仓库、起草 `docs/agent-project-profile.md` 与 AGENTS.md；空目录时向用户提问。退出标记出现后界面进入第 4 步，在应用内展示两个文件供编辑（文本框），保存即写回。第 5 步从全局默认复制全量到 `config.toml` 供修改。完成或跳过后 automed 创建 init commit。

## 11. 环境探测与安装

| 项 | 探测 | 登录态 | 安装 |
|---|---|---|---|
| Git | `git --version`、路径 | 不适用 | `xcode-select --install` |
| Claude Code | `claude --version`、路径 | 调用其登录状态命令，按版本映射 | `npm i -g @anthropic-ai/claude-code` 或 Homebrew |
| Codex | `codex --version`、路径 | 同上 | `npm i -g @openai/codex` 或 Homebrew |
| iTerm2 | `/Applications/iTerm.app` 存在与版本 | 不适用 | `brew install --cask iterm2` |

安装与登录命令通过 osascript 在可见终端执行——这两条是用户主动按的、需要看到 sudo 与登录提示，所以与会话相反，必须可见。Homebrew 或 npm 缺失时先给出它们的安装命令。探测在应用启动、回到前台、安装或登录终端关闭后触发，结果缓存并推送事件。

## 12. 技能盘点与绑定

- 扫描四类目录（§3.1、需求 S-01），每个子目录含 `SKILL.md` 即为一个技能。`~/.claude/skills`、`.claude/skills` 对 Claude 可见；`~/.agents/skills`、`.agents/skills` 对 Codex 可见；同名出现在两侧则双可见。
- 绑定写入 `config.toml` 的 `roles.<role>.skills`。保存校验见 §9。
- 启动时把绑定技能写入 prompt（§7.1）。不做加载限制。

## 13. 持久化与恢复

迁移 v8 新增单行 kernel_mark（id=1、build_id、updated_at）。BEGIN IMMEDIATE 内比较并按需更新；同标识不写，没有记录时首次推进写入。原迁移不改。v8 会拒绝之后新连接的旧 schema 内核，但不能撤销迁移前已经打开的连接。首次上线必须先退出桌面端、面板、手动驱动并 bootout 常驻驱动，确认无旧进程，再安装 CLI（doctor 会迁移），随后 bootstrap 新驱动、重新打包桌面端并重启面板。

回退不得把 SCHEMA_VERSION 降回 7；回退行为须保留 v8 迁移与 kernel_mark。临时接管用 AUTOME_ALLOW_OLDER_KERNEL=1，不绕过 schema 检查。本任务之前的内核不能靠此开关打开 v8 账本。

SQLite 表：`projects`、`tasks`、`sessions`、`decisions`（用户对 Backlog 与争议项的表态，及携带 / 消费它的会话）、`events`（append-only，界面事件流与审计用）、`session_retries`（等待重派的崩溃轮次）、`runtime_cooldowns`（限流中的 CLI）、`project_alerts`（项目级报警，如合并后 main 门禁失败）。

schema v7 新增 `task_after(task_id, after_id, ord, notified)`：两个任务编号均外键引用 `tasks(id)`，联合主键为 `(task_id, after_id)`；`ord` 保留声明顺序，`notified` 记录已通知的阻塞原因。旧任务迁移后无依赖，原队列顺序不变；创建任务与依赖在同一事务内提交。

事实分工：任务文档与状态块是任务进度的事实来源；SQLite 保存登记、索引、台账与表态。两者不一致时以文件为准并记录一条 `integrity_warning` 事件。

启动恢复：加载 projects → 校验路径与 worktree 存在 → 对每个非终态 task 扫描会话目录（§2）→ 推进或标记失败 → 重建调度队列。

## 14. IPC 接口

沿用 autome-v2 的 framed JSON-RPC stdio 与版本协商。`autome core` 与 `automed` 共用 stdio 入口，stdout 只写帧。

`env.get.kernel` 含 build_id、origin（git/unknown）、ledger_build_id、may_advance、message，只读不更新标记。tick 保留布尔 skipped，新增 older_kernel（null 或 {message,kernel,ledger}）；锁忙与旧内核可区分。写命令拒绝码为 older_kernel，消息与 tick 相同；Web 映射 409。面板 outdated 状态保持可读、横幅显示原话并禁写，tick 降为每 10 个间隔一次，状态不变不重复日志。常驻 autome run 被拒时保持进程，只在原因变化时输出；恢复继续推进；--once/--until-idle 被拒退出 1。

`task.get.documents` 按任务卡、设计文档 / Bug 文档、任务文件、评审、裁决、审计、运行记录、复盘教训、证据的顺序列出存在的文件，证据按文件名排序。每项保留 name、label、path、absolute、size，并带 source（worktree / default_branch）。任务文档目录存在时从任务检出读取；目录不存在时，从项目主检出（工作区为文档仓库）的默认分支树过滤文件，且仅列磁盘上存在的文件。此时 status_block 从默认分支的 Git 内容解析，不受主检出的未提交内容影响；未合并且默认分支无文档时返回空清单和 null。Bug 文档使用 BugDoc 解析器。CLI status 显示合并后的里程碑及 Bug 文档路径。

命令：`project.add / list / get / remove / onboarding.step / onboarding.skip`、`task.create / list / get / approve / reject / merge / pause / resume / stop / cancel / decide / extend_budget / rerun_from / archive / restore`、`config.get / set / reset_field / validate`、`env.detect / install / login`、`skills.list / bind`、`session.log`。

补充命令：`task.start {task_id}` 是写命令，不在 `READ_METHODS` 中，语义见 §8.2；`task.create` 另收 `kind`、`brief`、`draft`（§8.2）。出处：`crates/automed/src/dispatch.rs` 的 `task_start`、`task_create`、测试 `a_draft_is_refused_a_retro_and_task_start_is_not_a_read_method`。

`task.merge {task_id, cancel?: bool}`：cancel 缺省 false，非布尔拒绝。在 AwaitMerge 走合并入口；其它未完成且未失败状态（含 Draft、排队、暂停、停止和设计卡点）记录预先同意，不跳过卡点。Failed 要先重跑；Active Merging / MainCheck / Cleanup 以及 Done / Cancelled 拒绝再次授权。`cancel: true` 对应 CLI `autome merge cart --cancel`，撤销有效授权；无授权、已终结或合并已开始时拒绝，Failed 可撤销。成功返回 task.get 投影加 `merge_request: "merging" | "authorized" | "already_authorized" | "revoked"`；`merging` 表示已执行合并入口，仍可能因纳入表态回实现轮，不保证已落地。出处：`dispatch::task_merge`、领域层 `merge_press` / `cancel_press`。

`task.get`、`dashboard.get`、`project.get.tasks` 的任务投影包含 `merge: {authorized, authorized_by, holds_section, waiting_for, stale}`，终结任务为 null。authorized_by 为 user/project/behind/null，waiting_for 为任务 ID 或 null。stale 只在 Active AwaitMerge 且落后时为 `{behind, gated, repos: [{repo, base, behind}]}`；工作区 behind 为各成员落后数之和，gated 表示配了项目门禁或为 Bug。Git 读取失败记 warning 并返回 stale=null，不阻断列表，也不代表已确认最新。`status` / `ls` 显示授权、等待与过期状态，有门禁时提示「合并时会重新 rebase 并再跑一次门禁」，无门禁则提示先 rebase。task.get 与 dashboard.get 另带 config_warnings（后者按项目），status 打印配置警告。

`task.create` 可携带 `after: ["T-41"]`；`task.set_after {task_id, after}` 替换依赖，空数组清空。最多 8 个输入项，去重保序，只接受同项目存在的任务，拒绝自引用和环；需求任务只允许在设计批准前且从未开始实现轮时修改，Bug 任务只允许开始前修改。

`task.move {task_id, before?, after?, top?}` 仅移动排队或待开始任务，`before` / `after` / `top: true` 恰好指定一个位置；参照必须是同项目的其它排队或待开始任务。重排只改变本项目队列；移动过的草稿在 `task.start` 时保留位置，未移动的草稿排到队尾。Web 网关放行这两个操作，要求非空 `task_id`，依赖列表最多 8 项，依赖及位置编号匹配 `^T-\d+$`。

`task.get`、`project.get` 和 `dashboard.get` 的任务对象带 `after` 与 `blocked_by`，后者列出未完成依赖的编号、状态和阻塞原因；`dashboard.get` 的 `queued` 组按出队顺序排列。这里的 `blocked_by` 表示任务依赖，与 `task.changes` 中表示文件变更阻塞的同名字段不同。`task.convert` 不继承或改指依赖，依赖原任务者会继续等待已取消的原任务，需用户清空或改指。

`task.reply {task_id, info}` 对走 Bug Loop 的任务表示补充信息：只有停在 `await_info` / `not_a_bug` 时可回到 `locate`，空白文本拒绝「补充信息不能为空」，其它节点拒绝；对需求任务则等同 `task.reject`，把 `info` 原样作为驳回意见。出处：`crates/automed/src/dispatch.rs` 的 `dispatch`（`task.reply` 分支）；`crates/autome-domain/src/bug_loop.rs` 的 `apply_for`、测试 `bug_loop_reply_is_refused_when_empty_or_not_waiting`。

事件：`project.updated`、`task.updated`、`session.started / ended`、`config.changed`、`env.changed`、`skills.changed`、`core.status`。

新增事件：`task.after_set` 携带更新后的 `after`；`task.moved` 携带任务、项目、位置参数及 `queue_position`；`task.blocked` 携带 `task`、`on`、`reason`、`project_id`。依赖为取消或失败时，同一阻塞原因只通知一次；恢复后再次失败可再次通知。

命令行 `autome`（`crates/autome-cli`）不走这条管道：它链接 `automed` 库，在本进程内用同一个 `dispatch::handle_command` 处理每条命令，打开同一个数据库，受同一把驱动锁约束（§8.1）。因此 CLI 与桌面端不存在两份实现；CLI 自己只有参数解析和输出格式。会话包装脚本用 `current_exe() render-stream` 渲染日志，所以 `autome` 与 `automed` 一样必须支持 `render-stream` 和 `protocol eval` 两个子命令。

## 15. Electron 工作台

- 沿用 autome-v2 的安全基线：contextIsolation、Renderer 无 Node、特权 `autome://` scheme、拒绝外部导航与权限请求。
- Renderer 只渲染 automed 推送的投影，不持有业务状态。屏幕与交互以设计稿为准。
- 外部动作（打开目录、打开编辑器、切到 iTerm2）经 Main 白名单执行。
- 一屏约束在 CI 用 Playwright 对 1512 × 944 逐屏测量。

## 16. 测试与质量门

| 门 | 内容 |
|---|---|
| T1 单元 | 状态块解析器（含 1.x 真实样本）；转换表表驱动测试；配置合并与校验 |
| T2 Git | 临时仓库上的 worktree 创建、rebase 干净与冲突、合并前置、合并、清理、取消、归档 |
| T3 启动器 | 用假 CLI 脚本验证包装脚本、pid、退出标记、崩溃判定、停止 |
| T4 IPC 与壳 | 命令与事件契约测试；Electron 安全基线冒烟 |
| T5 真实 CLI | 手工清单：Claude Code 与 Codex 各跑一个最小任务到合并 |
| T6 界面 | Playwright 一屏测量；关键交互脚本 |
| T7 端到端 | 需求文档 §6 的 10 个场景，用假 CLI 自动化，其中 1 与 3 另用真实 CLI 手工验证 |

## 17. 风险与对策

- **会话墙钟时长上限**。`[loop] session_minutes` 默认 120、至少 5 分钟，启动时冻结；旧会话按当前配置回退。tick 超限终止整个进程组，确认不了就保持运行、不重派；重试用尽才失败。电脑睡眠时间也计入，醒来可能立即超时。Onboarding 不登记会话、不受此上限约束；既有心跳兜底保持不变。


- **退出感知依赖包装脚本**。用户手动关闭终端标签会丢失退出标记。对策：pid 探活 + 日志 mtime 心跳，超过 10 分钟无心跳判崩溃。
- **CLI 参数漂移**。对策：adapter 表 + `--help` 探测，失败时环境页提示版本不支持。
- **自动执行模式的权限风险**。Agent 在 worktree 内仍可执行任意命令。对策：协议明示边界、`.autome/` 只读约定、日志留痕；后续版本再评估沙箱。
- **Agent 不写或写坏状态块**。对策：一次判协议失败并停下，不猜测；模板中状态块格式有示例。
- **并行任务共享输出目录**。对策：`sessions/<task>/` 分目录，文件名含 session id。
- **主工作树脏导致长期无法合并**。对策：待合并卡明示原因，提供「打开目录」。

## 18. 里程碑

| 阶段 | 内容 | 通过门 |
|---|---|---|
| M0（2 周） | 项目添加与初始化、跳过 Onboarding、配置读写与校验、环境探测、Electron 壳与项目 / 设置 / 环境屏 | T1、T4 全绿；添加空目录得到 init commit |
| M1（3 周） | 单任务全流程：启动器、状态块解析、转换表、设计定稿停顿、rebase、手动合并、清理、任务面板 | 场景 1、8、10 用假 CLI 通过；场景 1 用真实 Claude Code 手工通过 |
| M2（2 周） | 并行与排队、冲突修复路径、暂停 / 停止 / 取消、失败选项、Backlog 与争议项消费 | 场景 2、3、6、7、9 通过 |
| M3（2 周） | Onboarding 完整五步、技能盘点与绑定、一键安装、登录态与横幅 | 场景 4、5 通过；T5 清单通过 |
| M4（1–2 周） | 归档与恢复、仪表盘、打包与签名、T6 一屏测量进 CI | 全部 10 个场景通过；干净机器安装可用 |

## 19. 对 autome-v2 现有代码的处置

| 保留 | 删除或替换 |
|---|---|
| `automed` 的 SQLite 事件存储、framed JSON-RPC 循环与版本协商 | `autome-domain/project.rs` 八阶段状态机与 hold → 替换为 §4 的 Project |
| Electron Main 的安全基线与 sidecar 生命周期 | `project_intent.rs` |
| Codex / Claude adapter 骨架 → 改造为 §7 启动器与参数映射表 | contract、graph、replan、completion.evaluate 及其 IPC |
| `install-recipes/` → 作为 §11 安装表的数据来源 | ExecutionQueue / HarnessLease → 替换为 §8 项目信号量 |
| 测试基础设施与 CI 配置 | `playbooks/`、`profiles/`、`skill-policies/`、capability broker |

处置完成后更新 autome-v2 的 README 与 `docs/development/plan.md` 镜像，指向本文与需求文档。

### 操作者备注交接

`task.note` 在事件账本按序记 `task.note_added`；需求实现、Bug 定位及修复为收件轮。启动时 `session.started.notes` 固定本次编号集合，只有 Ok 收尾写 `task.note_delivered`，期间新来的备注留给下一轮。失败、超时和崩溃不标送达。
未送达备注与越界补丁共用 `handoff_pending`，末次审计/复核、复盘及合并前门禁通过时补实现/修复轮。`AwaitMerge` 要显式 `--reopen`，保留合并预授权；Bug 预算不足拒绝 reopen。已开始合并只记备注并警告。停止和失败任务由 rerun 的下一收件轮消费。
冻结审计模板含 `operator-notes` 标记才强制核对备注；审计启动固定待核对编号，通过关闭里程碑或集成复验后写 `task.notes_audited`。退回不确认，之后的审计仍列出。旧协议保持送达但不加强审计要求。
