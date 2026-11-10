# 任务：暂停或停止的任务恢复时回到停下时的节点

本任务的规则见 `docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/protocol/loop-protocol.md` 与
`docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/protocol/session-protocol.md`。

- 设计文档：`docs/修复已停止或已暂停的任务恢复时从任务整理重新开始/修复已停止或已暂停的任务恢复时从任务整理重新开始.md`
- 任务整理日期：2026-11-10
- 来源：用户的一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md` 第 6 节的 **K0「恢复从原节点继续」**。

## 1. 需求原文

> 修复已停止或已暂停的任务恢复时从任务整理重新开始的问题，让它回到停下时的节点

## 2. 目标

一个处于「已暂停」或「已停止」的任务，用户执行恢复（`task.resume`，命令行 `autome resume <任务>`，
桌面端的「恢复」按钮都走这一个方法）之后，应当从**停下时所在的节点**继续，而不是回到任务整理
（`intake`）从头再来。

## 3. 验收标准

以下三条逐字取自参考文档 K0 的「验收」。设计文档的里程碑必须覆盖每一条；用户在设计定稿时会对照
这三条批准或驳回。

1. 停在实现、审计、门禁时恢复，都回到原节点；
2. 恢复时槽位已满就排队，轮到时仍回原节点；
3. 新增 e2e 覆盖上面两点。

补充说明（从需求原文和调研得出，不是新范围）：

- 「停在……时」同时指两种停法：暂停（`Paused`）与停止（`Stopped`），两者都必须满足第 1、2 条。
- 第 3 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、
  真实包装脚本、只把模型换成替身）。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
- 门禁指三个门禁节点中的「实现后门禁」`gate`；`merge_gate`、`main_check` 不在第 1 条点名之列，
  但修复后它们不得出现比现在更差的行为。

## 4. 范围

**改动目录：只限 `crates/`。**（参考文档 K0 的「目录」一栏；与它并行的任务改的是别的目录，
越界会和并行任务冲突。）

在范围内：

- `crates/autome-domain`：任务状态机里 `Resume` 与排队、出队的转换；
- `crates/automed`：调度器出队时决定启动哪个节点、账本读写任务状态、`task.resume` 的派发，以及相应测试；
- `crates/autome-cli`：只在恢复后的显示文字确实需要随之调整时才动。

不在范围内（不要做）：

- `apps/`、`protocol/`、`plugins/`、`scripts/`、`.github/` 下的任何改动；
- 任务类型、任务卡、待开始状态、Bug Loop（那是计划里的 K1、K2）；
- 暂停、停止、取消本身的语义（需求 T-08：暂停在当前会话跑完后生效、停止立刻结束会话），以及失败面板上的
  「从某节点重跑」（`rerun_from`）、「追加轮次」（`extend_budget`）；
- 顺手重构、改名、整理与本缺陷无关的代码。

## 5. 硬性约束

1. **不改变已有的对外形状。** 任务状态以 JSON 存进账本、经 JSON-RPC 发给桌面端与命令行，判别字段是
   `state`（`queued` / `active` / `paused` / `stopped` / …）。桌面端渲染层
   （`apps/desktop/renderer/lib/labels.js`、`screens/task.js`、`screens/project.js`）按这个字段分支，
   而本任务不能改 `apps/`。可以新增字段，不能改已有字段的名字和取值。
2. **旧账本必须照常读得出来。** 已经存在的 `{"state":"queued"}` 记录（新建、尚未开始的任务）和已有的
   `paused` / `stopped` 记录，在修复后的 `automed` 下都要能加载，并且行为正确：新建任务出队时仍从任务整理开始。
   账本当前 schema 版本是 4（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`）；计划中的 K1 会把它升到 v5。
   本任务若必须改 schema，要在设计文档里写明理由和对 K1 的影响。
3. **排队语义不变。** 恢复不得越过项目的并行上限；恢复后的任务和别的排队任务一样按队列轮到（技术设计
   §8「恢复时若槽位不足则排队」）。等待用户的两个停顿点（`await_design_approval`、`await_merge`）不占槽位，
   这一点不变。
4. **新建任务的流程不变。** 新建任务仍然是 `排队 → 任务整理 → …`。现有 e2e 与单元测试除了直接断言
   「恢复后进入排队」这种被本缺陷定义为错误的行为之外，**不修改就要通过**；被修改的测试要在设计文档里逐条列出理由。
5. 恢复后重新进入的节点，所需的附带工作要做全：例如角色节点照常带上该节点应有的 prompt 输入；
   门禁节点要重新跑门禁（停止时宿主已经杀掉了在跑的门禁，见 `dispatch.rs` 的 `stop_running_session`
   与 `gate.rs` 的 `stop_all`），不能把一次被杀掉的门禁当成通过或失败。
6. 角色开关、SAME-MODEL 等现有校验在恢复路径上照常生效，不得因为「回到原节点」而被绕过。
7. 所有测试、`cargo clippy --all-targets`（警告即错误）、`cargo fmt --check` 通过；宿主门禁命令是
   `sh scripts/ci-local.sh`，实现轮之后由 Autome 自己跑。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

- `crates/autome-domain/src/task.rs`：
  - `TaskState` 中 `Paused { resume: Node }`、`Stopped { at: Node }` 都记着节点；`Queued` 不带节点。
  - `apply()` 里 `Trigger::Resume` 把 `Paused` / `Stopped` 一律转成 `TaskState::Queued`（约第 621–630 行，
    注释写的理由是「槽位可能已被占用」）。
  - `(TaskState::Queued, Trigger::SlotAvailable)` 一律转成 `Active { node: Intake }` 并 `StartIntake`
    （约第 636–638 行）。两者合起来，就是「恢复后从任务整理重新开始」。
  - `RerunFrom` 已有「按节点决定启动什么动作」的映射（约第 658 行起），涵盖角色节点、核心步骤。
- `crates/automed/src/scheduler.rs`：单元测试 `pause_and_resume_move_through_queued_rather_than_jumping_the_limit`
  （约第 3667 行）断言恢复后是 `Queued`，但没有断言出队后去了哪个节点。
- `crates/automed/tests/end_to_end.rs`：`stopping_a_task_leaves_it_resumable`（约第 998 行）只断言恢复后是
  `queued` 或 `active`，同样没有断言节点——这正是缺陷没被测出来的原因。
- `crates/automed/src/store.rs`：任务状态以 `serde_json::to_string(&task.state)` 存储；`task_counts` 把
  `Queued` 计为 `queued`。
- `crates/autome-cli/src/render.rs`：已暂停显示为「已暂停（会从 {节点} 继续）」——界面已经向用户承诺了回到原节点。
- 权威需求：`docs/development/requirements.md` T-08（运行中干预）；技术设计
  `docs/development/technical-design.md` 第 135、190、247 行附近（`Paused(resume_node)`、暂停语义、恢复时排队）。

## 7. 项目背景摘要（2026-11-10 整理时）

**仓库。** 单个仓库 `autome-v2`（工作目录即本任务的 worktree）。Autome 2.0：一个 macOS 桌面应用加命令行，
把一句话需求在专用 worktree 里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 →
rebase → 合并前门禁 → 合并 → 验证 main → 清理）推进成合并提交。**内核独占节点转换**：会话只写设计文档，
下一个节点由 `automed` 读状态块决定。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置覆盖与校验、设计文档解析、
    任务状态机（`task.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git、会话启动器（`launcher.rs`）、
    调度器（`scheduler.rs`）、宿主门禁（`gate.rs`）、协议与 eval、JSON-RPC 派发（`dispatch.rs`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析与输出。
- `apps/desktop` — Electron 外壳（纯 `node --test`），两套 IPC 白名单；本任务不动。
- `protocol/` — Loop 协议源（编译进 `automed`）；本任务不动。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 三份 ADR；
  `docs/plans/` 按日期的决策记录。

**测试命令。**

```sh
cargo test                                   # domain、core、端到端
cargo test -p automed --test end_to_end      # 只跑端到端
cargo clippy --all-targets                   # CI 中警告即错误
cargo fmt --all -- --check
sh scripts/ci-local.sh                       # 与 .github/workflows/ci.yml 同序同参数的全量本地 CI（含桌面端测试）
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
