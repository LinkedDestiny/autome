# 任务：等你合并时默认分支已前进，按合并即自动 rebase、过门禁再合并

本任务的规则见 `docs/用户按合并时如果默认分支已经前进-任务自动/protocol/loop-protocol.md` 与
`docs/用户按合并时如果默认分支已经前进-任务自动/protocol/session-protocol.md`。

- 设计文档：`docs/用户按合并时如果默认分支已经前进-任务自动/用户按合并时如果默认分支已经前进-任务自动.md`
- 任务整理日期：2026-11-10
- 来源：用户的一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md` 第 6 节的 **K0b「合并时自动 rebase」**。

## 1. 需求原文

> 用户按合并时如果默认分支已经前进，任务自动 rebase、跑合并前门禁再合并，不再卡在等你合并

## 2. 目标与背景

**背景（参考文档 K0b 原文）。** 第 1 轮 T-8、T-9 同时等合并，先合并 T-8 后，T-9 的合并被拒
（「不在最新的 main 之上，需要先 rebase」）并退回等你合并；而等你合并的任务不能停止也不能重跑，
只能手工 rebase。

**目标。** 任务停在「等你合并」（`await_merge`）时，用户按合并（`task.merge`，命令行 `autome merge <任务>`，
桌面端的合并按钮都走这一个方法）；如果这时默认分支已经前进、任务分支不在它最新提交之上，内核自己走
rebase →（配置了门禁时）合并前门禁 → 合并，不再退回等你合并、不需要任何手工步骤。

## 3. 验收标准

以下六条逐字取自参考文档 K0b 的「验收」。设计文档的里程碑必须覆盖每一条；用户在设计定稿时会对照
这六条批准或驳回。

1. 在等你合并时按合并，如果任务分支（工作区项目是任一成员仓）不在默认分支最新提交之上，就自动走 rebase → 合并前门禁（配置了门禁时）→ 合并，中途不再停下来等你；
2. 默认分支没动时，行为和现在完全一样，现有测试不改就能通过；
3. 合并这一步因为「不在最新提交之上」被拒（按下合并到真正合并之间 main 又动了），走同一条自动路径；因为主工作树有冲突的未提交改动被拒，仍然退回等你合并；
4. 自动 rebase 冲突、合并前门禁失败，都按现有路径退回实现；修好后再次停在等你合并；
5. e2e：两个任务同时等合并，先合并 A，再合并 B，B 自动 rebase、过门禁、合并，两个都完成，全程没有手工步骤；
6. 事件里记录这次自动 rebase（如 `merge.rebased`），命令行 `autome run` 能打印出来。

补充说明（从需求原文和调研得出，不是新范围）：

- 第 5 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、
  真实 rebase 与 merge，只把模型换成替身）。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
  「过门禁」意味着该 e2e 的项目要配置 `[gate]`。
- 第 1 条的「中途不再停下来等你」：用户按的那一下合并，就是对这次合并的授权；自动路径走通后直接合并，
  不再回到 `await_merge` 要求再按一次。
- 第 3 条区分的是两类被拒：「不在最新提交之上」→ 自动路径；主工作树有冲突的未提交改动 → 仍退回等你合并。
  其余被拒原因（例如主工作树不在默认分支上）维持现状，即退回等你合并。
- 第 4 条「修好后再次停在等你合并」：退回实现之后走现有的实现 → 门禁 → 审计 → … → rebase → 合并前门禁 →
  等你合并，用户需要再按一次合并——这是现有路径，不在本任务里改。
- 工作区项目：只要任一成员仓不在其默认分支最新提交之上，就走自动路径；rebase 照现有 `rebase_all` 的做法
  覆盖所有成员仓。注意 `merge_all` 不是原子的（可能一部分仓已合并、某个仓被拒），设计要说明这种部分合并后
  自动路径的行为，且不得产生重复的合并提交。

## 4. 范围

**改动目录：只限 `crates/`。**（参考文档 K0b 的「目录」一栏；与它并行的 W2 改的是 `apps/web/server/`，
越界会和并行任务冲突。）

在范围内：

- `crates/autome-domain`：任务状态机中 `await_merge` 按合并、`merging` 被拒之后的转换，以及区分被拒原因所需的类型；
- `crates/automed`：合并前检查、`merge_all` / `rebase_all` 与调度器的衔接、事件写入、`task.changes`（如需要），
  以及相应单元测试与 e2e；
- `crates/autome-cli`：`autome run` 打印新事件那一行（`render.rs` 的 `event_line`）。

不在范围内（不要做）：

- `apps/`、`protocol/`、`plugins/`、`scripts/`、`.github/`、`docs/development/` 下的任何改动；
- 让「等你合并」可以停止、重跑（背景里提到的现状，但不是本需求）；
- 自动合并——没有用户按合并就不合并；
- 主工作树脏、不在默认分支等其它合并前置条件的处理方式；
- 任务类型、任务卡、Bug Loop（K1、K2）；
- 顺手重构、改名、整理与本需求无关的代码。

## 5. 硬性约束

1. **不新增 JSON-RPC 方法、不改已有方法的参数。** 桌面端的读写白名单（`apps/desktop/src/{ipc-gate,write-gate}.js`）
   与 `dispatch.rs` 的 `READ_METHODS` 由测试交叉核对，而本任务不能改 `apps/`。合并仍然只走 `task.merge`。
2. **不改变已有的对外形状。** 任务状态、节点名（`node_label` 一一对应的 `as_str()` 取值）、事件、`task.changes`
   的已有字段：可以新增字段或新增事件种类，不能改已有字段的名字和取值含义——除非设计文档证明这是本需求必须的，
   并逐条列出受影响的读者（桌面端 `apps/desktop/renderer/`、命令行 `render.rs`）。见 §6 关于 `mergeable` 的事实。
3. **默认分支没动时零行为变化**（验收 2）：现有单元测试与 e2e 不修改就要通过。新增测试可以，改已有测试不行；
   若确有必要改，设计文档要逐条列出理由，且理由不能是「新行为与旧断言不符」——那恰恰违反验收 2。
4. **门禁语义不变。** 合并前门禁（`merge_gate`）仍然跑在 rebase 之后的 head 上，与上次通过的 sha 相同则沿用
   （技术设计 §8.1 的现有规则）；没有配置 `[gate]` 时跳过门禁。不得因为是「用户已按过合并」而跳过或放宽门禁。
5. **槽位规则不变。** `await_merge` 不占槽位；`rebase`、`merge_gate`、`merging` 是占槽位的内核步骤
   （`Node::occupies_slot`）。自动路径不得越过项目的并行上限，设计要写明槽位已满时按合并会怎样。
6. **不动用户的主工作树。** rebase 只在任务 worktree 里做；主工作树只在真正合并时被 `git merge` 改动，
   前置检查失败时用户文件原样不动（需求 T-07）。
7. **不能产生重复合并提交，不能丢合并。** 已合并进默认分支的仓照现有逻辑跳过（`merge_all` 用 `is_ancestor` 判断）。
8. **旧账本照常可读。** 账本 schema 当前是 v4（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`），计划中的 K1
   会升到 v5。本任务最好不改 schema；若必须改，设计文档要写明理由和对 K1 的影响。停在 `await_merge`、
   `merging` 的已有任务记录在新 `automed` 下都要能加载并按新行为工作。
9. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过；宿主门禁命令是
   `sh scripts/ci-local.sh`，实现轮之后由 Autome 自己跑。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

- **被拒的源头。** `crates/automed/src/git.rs` 的 `merge_task_branch`（约第 711 行起）按顺序检查：
  主工作树有冲突的未提交改动 → `Blocked`；主工作树不在默认分支 → `Blocked`；
  `!is_ancestor(default_branch, task_branch)` → `Blocked { detail: "{task_branch} 不在最新的 {default_branch} 之上，需要先 rebase" }`；
  `git merge` 本身失败 → `merge --abort` 后 `Blocked`。三类原因都只是一个 `detail` 字符串，**目前没有机器可读的区分**。
- **状态机。** `crates/autome-domain/src/task.rs`：
  - `(Active{AwaitMerge}, Trigger::Merge)`（约第 969 行）：有本停顿点新纳入的 Backlog 项时回实现
    （`start_impl`），否则 → `Active{Merging}` + `RunCoreStep{Merging}`；
  - `(Merging, CoreStepResult::Blocked{..})`（约第 1039 行）→ 一律回 `AwaitMerge`，`Action::None`；
  - `(Rebase, Ok)` → 配置了门禁则 `MergeGate`，否则 `AwaitMerge`；`(Rebase, Conflict)` → 实现，注入
    `Inject::RebaseConflict{files}`；
  - `(MergeGate, Passed)` → `AwaitMerge`；`(MergeGate, Failed)` → `back_to_impl`；
  - `CoreStepResult` 只有 `Ok`、`Conflict`、`Blocked{detail}`、`Failed{detail}` 四种（约第 417 行）。
  - 现有单元测试 `a_blocked_merge_returns_to_the_stopping_point_rather_than_failing`、
    `merging_with_only_ignored_items_proceeds_to_merge`、
    `merging_with_an_item_decided_while_waiting_here_still_goes_back_to_implement` 覆盖这一段。
- **调度器。** `crates/automed/src/scheduler.rs`：`rebase_all`（约第 2207 行）对每个成员仓
  `git::rebase(worktree, base)`；`merge_all`（约第 2250 行）代码仓在前、文档仓最后，已合并的跳过，
  每个仓写一条 `merge.repo` 事件，有被拒的就汇总成 `Blocked`；第 1985–1986 行把 `Rebase`、`Merging`
  派给这两个函数。`task.merge` 在 `dispatch.rs` 第 397 行，走 `task_trigger(…, Trigger::Merge)`。
- **合并面板与桌面端。** `dispatch.rs` 的 `task_changes`（约第 1900 行）对每个仓报告
  `mergeable = 无冲突脏文件 && on_top`，不在最新之上时 `blocked_by: { kind: "needs_rebase" }`。
  桌面端 `apps/desktop/renderer/screens/task.js` 第 928、1266 行在 `!changes.mergeable` 时**禁用合并按钮**，
  第 980 行把 `needs_rebase` 显示为「任务分支还没有 rebase 到最新默认分支」。也就是说，按现状桌面端用户在
  默认分支前进后**按不到合并**——而本任务不能改 `apps/`。设计必须处理这一点，并遵守约束 2。
  命令行 `autome merge` 不看 `mergeable`，直接调 `task.merge`。
- **事件与命令行打印。** `crates/autome-cli/src/render.rs` 的 `event_line`（约第 167 行）按事件种类各写一行，
  不认识的种类返回 `None`（不打印）；`node_label` 把 `rebase` 显示为「rebase」，`merge_gate` 为「合并前门禁」，
  `await_merge` 为「等你合并」。
- **现有 e2e。** `crates/automed/tests/end_to_end.rs`：`a_task_runs_from_one_line_to_a_merge_commit`（约第 600 行）、
  `a_dirty_main_worktree_blocks_the_merge_and_leaves_the_users_file_alone`（约第 687 行）、
  `the_merge_panel_reports_what_the_core_would_actually_do`（约第 757 行）、
  `a_workspace_task_branches_in_every_repository_it_names_and_merges_them_all`（约第 2182 行）、
  `a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`（约第 2548 行）。
  调度器单测里有 `World::new("merge-blocked")`（约第 3686 行）。
- **K0 已合并。** `TaskState::Requeued{resume}`（外部仍是 `state: queued`，带 `resume`）是 K0 新增的，
  恢复会回到原节点。本任务若涉及排队，应与它一致。
- **权威需求与设计。** `docs/development/requirements.md` 第 154 行 T-07（合并前置条件：主工作树无未提交改动、
  任务分支已在最新默认分支之上；不满足时合并失败并提示，不改动用户文件）；`docs/development/technical-design.md`
  第 134、138 行（节点列表）、第 254 行（宿主门禁、MergeGate 在 rebase 后新 head 上跑、同 sha 沿用）。
  本任务改变的是「不在最新之上」这一条前置条件不满足时的**处理方式**（自动补齐，而不是失败并提示），
  不改变「合并时必须在最新之上」这一前置条件本身。

## 7. 项目背景摘要（2026-11-10 整理时）

**仓库。** 单个仓库 `autome-v2`（工作目录即本任务的 worktree）。Autome 2.0：一个 macOS 桌面应用加命令行，
把一句话需求在专用 worktree 里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 →
rebase → 合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）推进成合并提交。**内核独占节点转换**：会话只写设计文档，
下一个节点由 `automed` 读状态块决定。本仓库正在用自己的 Loop 开发自己（`docs/plans/2026-11-10-web-dashboard-via-loop.md`）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置覆盖与校验、设计文档解析、
    任务状态机（`task.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git（`git.rs`）、会话启动器（`launcher.rs`）、
    调度器（`scheduler.rs`）、宿主门禁（`gate.rs`）、协议与 eval、JSON-RPC 派发（`dispatch.rs`）、布局（`layout.rs`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析与输出（`render.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`），两套 IPC 白名单；本任务不动。
- `apps/web` — React Router + 自定义 Node 服务器的面板（vitest），由并行的 Web 任务开发；本任务不动。
- `protocol/` — Loop 协议源（编译进 `automed`）；本任务不动。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 三份 ADR；
  `docs/plans/` 按日期的决策记录。

**测试命令。**

```sh
cargo test                                   # domain、core、端到端
cargo test -p automed --test end_to_end      # 只跑端到端
cargo clippy --all-targets -- -D warnings    # CI 中警告即错误
cargo fmt --all -- --check
sh scripts/ci-local.sh                       # 与 .github/workflows/ci.yml 同序同参数的全量本地 CI（含桌面端与 Web）
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记、真实 rebase 与 merge，只把模型换成替身。
测试数据库由 `AUTOMED_DB_PATH` 各自隔离。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下的其它目录、
  不推送；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，
  不要进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：并行 2；`[gate] command = "sh scripts/ci-local.sh"`，超时 30 分钟；崩溃重试 2 次，退避 60 秒。
- 本仓库还带一份项目技能 `.claude/skills/autome-v2-simplify`（简化/清理用），本任务不需要。
