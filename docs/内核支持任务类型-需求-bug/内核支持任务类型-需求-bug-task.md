# 任务：任务类型（需求/Bug）、任务卡正文与待开始状态（K1）

本任务的规则见 `docs/内核支持任务类型-需求-bug/protocol/loop-protocol.md` 与
`docs/内核支持任务类型-需求-bug/protocol/session-protocol.md`。

- 设计文档：`docs/内核支持任务类型-需求-bug/内核支持任务类型-需求-bug.md`
- 任务整理日期：2026-11-10；任务 id T-13；基线提交 `f20fe14`
- 来源：用户的一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md` 第 6 节的 **K1「任务类型与任务卡」**
  （任务卡的内容与模板见该文档 §3.1，看板列见 §3.2，三个来源见 §3.3）。

## 1. 需求原文

> 内核支持任务类型（需求/Bug）、任务卡正文和待开始状态，命令行 task new 增加 --kind、--brief、--draft 并新增 task start 与 reply

## 2. 目标与背景

**背景（参考文档 §1–§3 的要点）。** Autome 要做一个按项目看任务卡的 Web 面板，并让需求开发和 Bug 修复走两条
不同的 Loop。任务卡有三个来源（Claude Code 等对话里整理后创建、网页上填写、其他 Agent 用命令行 `--json`
创建），落到内核里都是同一种任务卡：

> 任务卡 = 类型 + 标题 + 一份 Markdown 正文（下称「任务卡正文」）+ 可选附件。
> 正文由内核写进任务的文档目录 `docs/<slug>/<slug>-card.md`，随任务分支提交。它是任务整理轮（需求）和定位轮
> （Bug）的第一输入，排在一句话 request 之前。正文上限 64KB。

> 创建时可以选「创建并开始」或「存为待开始」。从 Claude 讨论里创建的卡，默认存为待开始，你在面板上看一眼再点开始。

对话插件（C1）会执行的命令行是：
`autome task new --brief - --kind bug|feature --draft -p <项目> --json`；其他 Agent 用 `--json` 拿到任务 id 后
`autome task start <id>` 开始。

**目标。** 内核（`crates/`）具备：任务类型、任务卡正文、待开始（`Draft`）状态，以及对应的 JSON-RPC 参数、
方法与命令行；任务整理轮把任务卡正文当作第一输入。**Bug Loop 本身（定位、复现检查、修复、复核）是 K2，不在
本任务里。**

## 3. 验收标准

以下七条逐字取自参考文档 K1 的「验收」。设计文档的里程碑必须覆盖每一条；用户在设计定稿时会对照这七条批准或驳回。

1. `Task.kind` 取 `feature` 或 `bug`，默认 `feature`；store 迁移到 v5，旧任务都视为需求；
2. `task.create` 接受 `kind`、`brief`（≤ 64KB）、`draft`；`brief` 写成 `docs/<slug>/<slug>-card.md` 并在第一个会话之前提交；
3. 新状态 `Draft` 不占槽位，`task.start` 让它进入排队或开始；`Draft` 可以直接取消；
4. 任务整理的 prompt 把任务卡正文列为第一输入，金样测试更新；
5. `project.get`、`dashboard.get`、`task.get` 都带 `kind`；
6. 命令行：`autome task new --kind bug --brief <文件|-> --draft`、`autome task start <任务>`、`autome reply <任务> "…"`（K2 之前对需求任务等同 reject）；集成测试覆盖从标准输入读取 brief；
7. 现有需求任务的 e2e 不做任何修改就能通过。

补充说明（从需求原文、参考文档和调研得出，不是新范围）：

- 第 2 条的 `docs/<slug>/` 指任务的文档目录，即 `TaskRecord::doc_dir()`（项目可配置文档根，工作区项目在文档仓里）；
  路径以 `doc_dir()` 为准，不要硬编码 `docs/`。
- 第 2 条「在第一个会话之前提交」：草稿任务在创建时还没有 worktree（worktree 在第一个会话时才建，见 §6），所以正文
  在 `task.start` 之前必须先存在某处；设计要写明存在哪、何时写进 worktree、何时提交。
- 第 6 条的集成测试指 `crates/autome-cli/tests/cli.rs` 这一类跑真实 `autome` 二进制的测试，「从标准输入读取 brief」
  要真的经由子进程的 stdin 喂进去。
- 第 6 条 `reply`：参考文档 §4.2 里它的最终用途是 Bug「无法复现」后补充信息（K2）。本任务只做到「对需求任务等同
  reject」；对 Bug 类型任务在 K2 之前怎么处理，由设计给出并说明理由（见 §7 待定问题）。
- 第 7 条的 e2e 指 `crates/automed/tests/end_to_end.rs`（真实 Git、真实 worktree、真实包装脚本，只把模型换成替身）。
  不传 `kind`、`brief`、`draft` 的创建，行为必须和现在完全一样。

## 4. 范围

**改动目录：`crates/`，以及 `protocol/prompts/`（只改任务整理模板对任务卡正文的引用）。**（参考文档 K1 的「目录」
一栏；同一轮并行的 Web 任务改的是 `apps/web/`，越界会和并行任务冲突。）

在范围内：

- `crates/autome-domain`：`TaskKind`（`feature` / `bug`）类型；`TaskState::Draft` 及其线上形状、
  `occupies_slot` / `awaits_user` 等分类、状态机里 `Draft` 的转换（开始、取消；其余触发器的拒绝）；
- `crates/automed`：
  - `store.rs`：schema v4 → v5 迁移（`kind`，以及设计决定需要持久化的正文/草稿信息），`TaskRecord` 新字段，
    旧任务读作 `feature`；
  - `dispatch.rs`：`task.create` 新参数与校验（`brief` ≤ 64KB）、`task.start`、`reply` 所需的方法（若设计决定新增）、
    `task_json` 带 `kind`；
  - `scheduler.rs`：草稿不进 `fill_slots`；任务卡正文写进任务目录并在第一个会话前提交（现有 `write_task_inputs`
    是自然的落点）；
  - `launcher.rs`：任务整理 prompt 的正文引用；
  - 相应单元测试、`tests/prompt_rendering.rs` 与金样 `tests/golden/prompts/intake.md`、`tests/end_to_end.rs` 新增用例；
- `crates/autome-cli`：`task new` 的 `--kind`、`--brief <文件|->`、`--draft`；`task start <任务>`；`reply <任务> "…"`；
  `render.rs` 对 `draft` 状态与新事件的显示；`USAGE` 帮助文本；`tests/cli.rs` 新增用例；
- `protocol/prompts/intake.md`：只改对任务卡正文的引用（见约束 6）。

不在范围内（不要做）：

- Bug Loop：定位、复现检查、修复、门禁、复核、Bug 文档状态块、`[bug] fix_rounds`、新的 prompt（这些是 K2）；
- `apps/`（桌面端、Web 面板，包括两套读写白名单）、`plugins/`（C1）、`scripts/`、`.github/`、
  `docs/development/`、`docs/plans/`；
- 网页附件上传、`~/.autome/web/` 下的任何东西（W 系列）；
- 看板列的聚合（「待开始 / 进行中 / 等你处理 / 已完成」是前端按状态归类的事，内核只给出状态和 `kind`）；
- 把 Bug 改建为需求、按类型区分进度阶段表（§4.3，W5/W7）；
- 顺手重构、改名、整理与本需求无关的代码。

## 5. 硬性约束

1. **零行为变化。** 不传 `kind`、`brief`、`draft` 时，`task.create` 的返回、事件、立即启动/排队的行为与现在完全一样；
   现有单元测试与 e2e 不修改就要通过（验收 7）。新增测试可以，改已有测试不行；若确有必要改（例如 `SCHEMA_VERSION`
   的断言 `assert_eq!(SCHEMA_VERSION, 4)`），设计文档要逐条列出理由，且理由不能是「新行为与旧断言不符」。
2. **已有对外形状只增不改。** 任务状态的线上形状（`TaskStateRepr`，`state` 字段的已有取值）、节点名、已有事件、
   `task_json` 的已有字段：可以新增字段、新增状态取值、新增事件种类，不能改已有字段的名字和含义。`Draft` 对外是一个
   新的 `state` 取值（建议 `"draft"`），不要借用 `queued`。
3. **不要让正在驱动本仓库的 Loop 停下。** 驱动 Loop 的是已安装的 `~/.local/bin/autome`（schema v4），它和本机的
   `~/.autome/state/automed.sqlite3` 共用账本；`store.rs` 遇到比自己新的 schema 会拒绝打开。所以：
   - 所有测试必须用隔离的账本（`AUTOMED_DB_PATH` / 临时 `AUTOME_HOME`，现有测试已经这么做）；
   - 实现与验证过程中**不得**用本任务构建出的 `autome` / `automed` 打开真实的 `~/.autome`（不要不带隔离地运行
     `cargo run -p autome-cli -- …` 或 `target/debug/autome`），否则真实账本会被迁移到 v5，正在跑的 Loop 立即停摆。
4. **迁移只进不退、不丢数据。** v5 迁移与已有 v1–v4 一样是单个事务、以 `user_version` 记录；v4 账本里所有任务
   （含 `Requeued{resume}`、`Paused`、`Failed` 等）迁移后照常可读，`kind` 为 `feature`。迁移要有测试。
5. **槽位规则。** `Draft` 不占槽位、不进 `fill_slots`、不出现在「等待我」（`awaits_user` 为假）。`task.start` 之后的
   排队顺序（按创建时间还是按开始时间）由设计决定并写明，且不得越过项目的并行上限。
6. **协议改动最小。** `protocol/prompts/intake.md` 只改对任务卡正文的引用，不动其它措辞。改了 prompt 模板，按
   `protocol/README.md`「怎么改」：跑 `cargo test -p automed --test prompt_rendering`，看过 diff 再用
   `AUTOME_UPDATE_GOLDEN=1` 接受，金样与模板改动放在同一个提交里；跑
   `cargo run -p automed -- protocol eval protocol/` 静态门。若静态门或测试要求同时在 `protocol/CHANGELOG.md` 加一节
   新版本，这超出了本任务的目录，设计文档必须把它列为「为满足门禁必须的最小越界改动」并说明理由，由用户在设计
   定稿时批准；能用内核填的占位符（如现有的 `{inputs}`）解决就不要动 CHANGELOG。K2 也会改协议并升版本，设计要
   说明两者如何不冲突。
7. **正文是数据，不是指令。** `brief` 原样写进 `-card.md`，不做模板渲染、不做占位符替换（正文里出现 `{request}`
   之类的文本不得被内核当占位符处理，也不得触发 `check_no_placeholders_left` 拒绝启动）；超过 64KB（按字节）拒绝，
   错误信息用中文。命令行 `--brief -` 读标准输入，`--brief <文件>` 读文件。
8. **不新增依赖。** 命令行的参数解析是手写的（`crates/autome-cli/src/args.rs`，`VALUED` 表），沿用它。
9. **新 JSON-RPC 方法写在派发表里，并判断是否属于 `READ_METHODS`。** `task.start`（以及若新增的 `task.reply`）都是写
   方法，不进 `READ_METHODS`。桌面端写白名单 `apps/desktop/src/write-gate.js` 是允许清单，本任务不改 `apps/`，
   所以桌面端暂时调不到新方法——这是预期的（桌面渲染进程将在 W11 删除，Web 面板在 W6/W7 接入）。
10. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过；宿主门禁命令是
    `sh scripts/ci-local.sh`，实现轮之后由 Autome 自己跑。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

- **任务记录与账本。** `crates/automed/src/store.rs`：`SCHEMA_VERSION = 4`（第 70 行）；`TaskRecord`（第 76 行起）
  有 `request`、`attachments`、`doc_refs`、`state`（`TaskState` 的 JSON）、`doc_root` 等字段，没有类型和正文；
  `migrate()`（第 219 行）逐版本一个事务块，v4 块在约第 401–450 行；`insert_task`（第 801 行）；`queued_tasks`
  （第 1201 行）只取 `Queued | Requeued`。测试里 `assert_eq!(SCHEMA_VERSION, 4)`（第 2221 行），并有「账本版本比
  二进制新时拒绝打开」的测试（约第 2799 行）。
- **状态机。** `crates/autome-domain/src/task.rs`：`TaskState`（第 177 行）有 `Queued`、`Requeued{resume}`、
  `Active{node}`、`Paused{resume}`、`Stopped{at}`、`Failed{at,reason}`、`Done`、`Cancelled`；线上形状由
  `TaskStateRepr`（第 205 行，`#[serde(tag = "state", rename_all = "snake_case")]`）决定，`Requeued` 对外是
  `queued` 带 `resume`。`TaskState::occupies_slot` / `awaits_user`（第 276、284 行）。`Trigger`（第 325 行）有
  `SlotAvailable`、`Approve`、`Reject{feedback}`、`Merge`、`Pause`、`Resume`、`Stop`、`Cancel`、`ExtendBudget`、
  `RerunFrom` 等，没有「开始」。`Reject` 只在 `await_design_approval` 有效。
- **创建。** `crates/automed/src/dispatch.rs`：`task.create`（第 387 行）→ `task_create`（第 1310 行）要求非空
  `request`，校验附件存在，收 `doc_refs`；`create`（第 1350 行）由 `request` 生成 slug（`slugify` + `unique_slug`）与
  初始标题（`first_line`，40 字截断），状态 `TaskState::Queued`，写 `task.created` 事件，**随后立即
  `scheduler::tick`**，返回 `{task, started, queue_position}`。`task_json`（第 537 行）是 `project.get`（第 514 行）、
  `task.get`（第 1462 行）、`dashboard.get`（第 2060 行）共用的任务形状——给它加 `kind` 三处都会带上。
  `READ_METHODS`（第 2388 行）17 个，桌面端测试解析源码交叉核对。`task_cancel`（第 1761 行）先停会话再
  `Trigger::Cancel`。
- **命令行 `task new` 没有 request 时。** 参考文档 §3.3 给 C1 的命令是 `autome task new --brief - --kind … --draft
  -p <项目> --json`，**没有一句话需求**；而现在的 `task new` 和 `task.create` 都要求非空 request，slug 和初始标题也
  都从 request 来。设计要决定只给 `brief` 时 request、标题、slug 从哪里来（例如正文的一级标题 `# <标题>`，§3.1 两个
  模板都以它开头），以及两者都给时以谁为准。
- **worktree 与输入提交。** `crates/automed/src/scheduler.rs` 的 `launch_session`（约第 1586 行）在第一个会话时才建
  worktree（「a queued task should not hold one」），`first_time` 时调 `write_task_inputs`（第 1977 行）复制附件到
  `<doc_dir>/attachments/` 并 `git::commit_paths(…, "chore(autome): {id} inputs")`；随后 `freeze_protocol`
  （第 1787 行）把协议复制到 `<doc_dir>/protocol/`。`fill_slots`（第 891 行）对 `queued_tasks` 依次
  `advance(…, Trigger::SlotAvailable)`。
- **任务整理 prompt。** 模板 `protocol/prompts/intake.md`，经 `include_str!` 编进 `automed`
  （`crates/automed/src/protocol/mod.rs` 第 55 行）。`launcher.rs::intake_prompt`（第 588 行）填 `{doc_dir}`、
  `{request}`、`{inputs}`（附件与参考文档两段，由内核拼）、`{slug}`、`{design_rounds}`，填完用
  `check_no_placeholders_left` 检查。模板里 `{request}` 在前、`{inputs}` 在后；验收 4 要求正文排在 request 之前。
  金样 `crates/automed/tests/golden/prompts/intake.md`，测试 `the_intake_prompt_renders_exactly_what_the_constant_did`
  （`tests/prompt_rendering.rs` 第 151 行）；`launcher.rs` 单测
  `the_intake_prompt_lists_attachments_and_doc_refs_when_present`（第 1699 行）。
  当前协议版本是 `protocol/CHANGELOG.md` 里的 `protocol/v1`（本任务已冻结在 v1）。
- **命令行。** `crates/autome-cli/src/main.rs`：`USAGE`（第 21 行）；每个动词允许的 flag 表（约第 103–117 行，
  `task` 允许 `--json --project --attach --doc`）；动词派发（约第 125 行起，`reject` 走 `task.reject` 带 `feedback`）；
  `task new`（约第 366 行）把 `positional[2..]` 拼成 request，附件做 `canonicalize`。`args.rs` 的 `VALUED`
  （第 10 行）列出带值的 flag，`-` 作为位置参数保留（第 37 行）。`render.rs::state_line`（第 40 行）不认识的
  状态原样输出；`event_line` 不认识的事件不打印。集成测试 `crates/autome-cli/tests/cli.rs`（跑真实二进制）。
- **桌面端（只读，不改）。** `apps/desktop/renderer/lib/labels.js` 的 `taskStatus` 对未知状态原样显示 `state.state`，
  所以新增 `draft` 不会让它崩；写白名单 `apps/desktop/src/write-gate.js` 不含新方法。没有桌面端测试解析
  `task.rs` / `store.rs`。
- **规格权威。** `docs/development/requirements.md` 第 130 行 T-01（输入：一句话需求 + 附件 + 文档引用）、第 133 行
  T-02（「提交即启动。创建后不再确认」）；`docs/development/technical-design.md` 第 133、164 行（`Queued` → 槽位空出
  → `Intake`）、第 302 行（命令清单）。`Draft` 是对 T-02 的有意例外（参考文档 §3.2 拍板），本任务不改这两份文档，
  设计文档里写明这一点即可。
- **已合并的相邻任务。** K0（恢复回原节点，`Requeued{resume}`）与 K0b（合并时自动 rebase，事件 `merge.rebased`）
  已在 main 上。K2 会在本任务之后基于 `kind` 实现 Bug Loop。

## 7. 待定问题（设计必须给出答案）

1. 只给 `--brief` 不给一句话需求时，request / 标题 / slug 的来源；两者都给时的关系。
2. 草稿期间正文存在哪里（账本列、`$AUTOME_HOME` 下的文件，或其它），以及何时写进 worktree、何时提交。
3. `task.start` 之后的排队位置；对非草稿任务调用 `task.start` 的拒绝信息。
4. `reply` 是新方法（如 `task.reply`）还是命令行把它映射到 `task.reject`；对 Bug 类型任务在 K2 之前的行为
   （K1 里 Bug 类型的任务跑哪条 Loop——在 K2 之前只能是现有的需求 Loop，设计要写明）。
5. 草稿被取消时走哪条路径（它没有 worktree、没有分支、没有文档，归档步骤不能因此失败）。
6. 新事件（如 `task.started`）与 `task.created` 的载荷是否带 `kind` / `draft`，`autome run` 如何打印。
7. 任务整理 prompt 的改法：改模板还是只靠内核填的占位符；是否触发 CHANGELOG 版本（见约束 6）。

## 8. 项目背景摘要（2026-11-10 整理时）

**仓库。** 单个仓库 `autome-v2`（工作目录即本任务的 worktree）。Autome 2.0：一个 macOS 桌面应用加命令行，
把一句话需求在专用 worktree 里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 →
rebase → 合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）推进成合并提交。**内核独占节点转换**：会话只写设计文档，
下一个节点由 `automed` 读状态块决定。本仓库正在用自己的 Loop 开发自己（`docs/plans/2026-11-10-web-dashboard-via-loop.md`，
并行 2：本任务 K1 与 Web 任务同轮）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置、设计文档状态块解析、
    任务状态机（`task.rs`）、项目与 slug（`project.rs`）、协议版本（`protocol.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git（`git.rs`）、会话启动器与 prompt
    （`launcher.rs`）、调度器（`scheduler.rs`）、宿主门禁（`gate.rs`）、协议与 eval（`protocol/`）、JSON-RPC 派发
    （`dispatch.rs`）、布局（`layout.rs`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析（`args.rs`）与输出
    （`render.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`），两套 IPC 白名单；本任务不动。
- `apps/web` — React Router 8 + 自定义 Node 服务器的面板（vitest），由并行的 Web 任务开发；本任务不动。
- `protocol/` — Loop 协议源（`include_str!` 编译进 `automed`），改动规则见 `protocol/README.md`。
- `plugins/` — 插件（C1 的落点）；本任务不动。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 三份 ADR；
  `docs/plans/` 按日期的决策记录。

**测试命令。**

```sh
cargo test                                          # domain、core、命令行、端到端
cargo test -p automed --test end_to_end             # 只跑端到端
cargo test -p automed --test prompt_rendering       # prompt 金样；接受新金样加 AUTOME_UPDATE_GOLDEN=1
cargo test -p autome-cli --test cli                 # 命令行集成测试（真实二进制）
cargo run -p automed -- protocol eval protocol/     # 协议静态门
cargo clippy --all-targets -- -D warnings           # CI 中警告即错误
cargo fmt --all -- --check
sh scripts/ci-local.sh                              # 与 .github/workflows/ci.yml 同序同参数的全量本地 CI（含桌面端与 Web）
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记，只把模型换成替身。测试数据库由
`AUTOMED_DB_PATH` 各自隔离（见约束 3）。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下的其它目录、
  不推送；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，
  不要进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：并行 2；`[gate] command = "sh scripts/ci-local.sh"`，超时 30 分钟；崩溃重试 2 次，退避 60 秒。
- `protocol/README.md`「怎么改」：协议（含 prompt 模板）的改动规则；`crates/automed/tests/golden/README.md`：金样更新纪律。
- 本仓库还带一份项目技能 `.claude/skills/autome-v2-simplify`（简化/清理用），本任务不需要。
