# 任务：Bug Loop 的定位 / 修复 / 复核 prompt 列出任务附件与 `--doc` 参考

本任务的规则见 `docs/bug-loop-的定位-修复-复核/protocol/loop-protocol.md` 与
`docs/bug-loop-的定位-修复-复核/protocol/session-protocol.md`。

- 设计文档：`docs/bug-loop-的定位-修复-复核/bug-loop-的定位-修复-复核.md`
- 任务卡正文（第一输入，与本文件不一致时以它为准）：`docs/bug-loop-的定位-修复-复核/bug-loop-的定位-修复-复核-card.md`
- 任务整理日期：2026-11-12；任务 id T-41；基线提交 `83c80e9`
- 来源：用户的任务卡正文、一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`（§3.1 任务卡内容、§4.2 Bug Loop）。

## 1. 需求原文

> Bug Loop 的定位/修复/复核 prompt 列出任务附件与 --doc 参考

## 2. 背景与目标

**背景。** 参考文档 §3.1：「任务卡 = 类型 + 标题 + 一份 Markdown 正文 + 可选附件」，正文「是任务整理轮（需求）和
定位轮（Bug）的第一输入」。附件和 `--doc` 参考文档是任务卡的一部分。内核在第一个会话之前已经把附件复制进
`docs/<slug>/attachments/` 并提交，但只有需求 Loop 的任务整理轮 prompt 把附件与 `doc_refs` 渲染出来；
Bug Loop 的三个步骤（定位、修复、复核）的 prompt 都没有。带截图或日志的 Bug 卡，定位轮不知道有附件；
`--doc` 指定的参考文档同样被忽略。

**目标。** Bug Loop 的定位、修复、复核三个会话的 prompt 都列出本任务的附件（任务目录内的相对路径）与
`--doc` 参考文档；没有附件也没有参考文档时，prompt 里不出现空段落。

## 3. 要做什么（任务卡「要做什么」，逐条）

1. `bug_prompt` 复用整理轮的 `inputs` 渲染（附件相对路径列表 + 参考文档列表），三个 Bug 步骤的 prompt 模板
   （`protocol/prompts/locate.md`、`fix.md`、`fix-review.md`）加 `{inputs}` 占位；无附件、无参考时不输出空段落。
2. 协议版本号按规则递增（`protocol/`），`protocol/CHANGELOG.md` 记一条。
3. 测试：
   - `crates/automed/src/launcher.rs` 单测，有 / 无附件两种；
   - `prompt_rendering` 金样更新（`crates/automed/tests/golden/prompts/`）；
   - e2e：带附件与 `--doc` 创建 Bug 任务后，定位会话的 prompt 文件含附件路径与文档路径。

## 4. 不做什么

- 不改需求 Loop 的 prompt（`intake.md`、`plan.md`、`review.md`、`adjudicate.md`、`impl.md`、`audit.md`、`retro.md`、
  `onboarding.md` 的文本与渲染结果都不变；对应金样不应出现差异）。
- 不改附件上传与存储（`task.create` 的参数校验、账本里 `attachments` / `doc_refs` 的存法、
  `write_task_inputs` 复制与提交附件的行为）。
- 不改 Bug Loop 的状态机、转换、预算、守卫；不改 Web 面板、桌面壳、插件。

## 5. 验收标准

- A1 有附件或参考文档时，定位、修复、复核三个会话的 prompt 都列出附件路径与参考文档路径。
- A2 无附件且无参考文档时，三个 prompt 不多出空段落，也不残留 `{inputs}` 字面量。
- A3 `launcher.rs` 有覆盖「有 / 无附件」两种情况的单测。
- A4 `prompt_rendering` 金样已更新，差异只在三个 Bug 模板对应的金样里。
- A5 e2e：带附件与 `--doc`（`task.create` 的 `attachments` / `doc_refs`）创建 Bug 任务后，定位会话的 prompt 文件
  含附件路径与文档路径。
- A6 协议版本号已递增，`protocol/CHANGELOG.md` 有对应条目；`cargo run -p automed -- protocol eval protocol/`
  静态门通过。
- A7 `cargo test --workspace` 全绿；项目门禁 `sh scripts/ci-local.sh` 通过。
- A8 `plugins/autome` 的 eval 不受影响（`plugins/autome/` 下无改动；`sh plugins/autome/tests/run.sh evals` 仍通过）。

## 6. 硬性约束

- C1 范围以第 3、4 节为准，不扩大。
- C2 不修改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、不推送远端；
  不自行启动下一个会话。
- C3 超过 5 行的命令输出、临时验证产物写到 `.autome/output/`，不进任务文档目录。
- C4 协议的改动按 `protocol/README.md`「怎么改」的步骤走：改 `protocol/` 下的文件，在 `CHANGELOG.md` 末尾加一节新的
  `## protocol/vN`（每条改动一个块，写清 `kind`、`evidence`、`predicted_impact`）；`behavioral` 类改动必须带
  「改前红、改后绿」的 eval 用例，`evidence` 至少两个任务；`predicted_impact.metric` 只能取
  `TaskMetrics::METRIC_NAMES` 里的指标；`realized_impact` 不由提出者填。
- C5 改的是仓库根的 `protocol/`（协议的唯一来源，`include_str!` 编译进 `automed`）。
  **不要改** `docs/bug-loop-的定位-修复-复核/protocol/`——那是本任务固定的 protocol/v2 副本，也不要改其它任务目录下的协议副本。
- C6 不动被 `<!-- kernel-contract: … -->` 包住的区域。
- C7 模板里的每个占位符都必须被内核填掉，渲染后残留 `{…}` 是错误（`check_no_placeholders_left`）；
  用户文本（`request`）可能自带花括号，不能被当成占位符替换——沿用现有「先切 `{request}` 再替换」的做法。
- C8 金样用 `AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering` 接受，接受前确认差异正是想要的。
- C9 测试替身要与真实接口对齐：e2e 里附件的取值要和真实入口（命令行 `--attach` 传的是规范化后的绝对路径）一致，
  不要只用裸文件名让测试自己和自己对齐。

## 7. 整理时核对到的现状（基线 `83c80e9`）

任务卡里的行号有一处已漂移，以下是基线上的实际位置：

- `crates/automed/src/launcher.rs`
  - `PromptSpec` 已有 `attachments: &[String]`、`doc_refs: &[String]` 两个字段（约 360 行）。
  - `bug_prompt`（492–540 行）：按 `locate` / `fix` / `fix-review` 取模板，替换 `{doc_dir}`、`{slug}`、`{budget_line}`，
    以 `{request}` 切段后拼接；定位轮且有任务卡时在最前面加「任务卡正文是本任务的第一输入…」一段。没有用到
    `attachments` / `doc_refs`。
  - `intake_prompt`（678–731 行）：内联拼 `inputs` 字符串——附件段「用户提供的附件（已复制到任务目录）：」逐行
    `- {doc_dir}/attachments/{a}`，参考段「用户指定的仓库内参考文档：」逐行 `- {d}`，每段后带一个空行；两者都空时
    `inputs` 是空串。模板里写作 `{inputs}请完成三件事：`（占位符后不另起行，空串时不留空段落）。
  - 现有单测 `the_intake_prompt_lists_attachments_and_doc_refs_when_present`（约 1816 行）用的附件值是裸文件名 `promo.csv`。
- `crates/automed/src/scheduler.rs`
  - 组装 `PromptSpec` 处（约 2583–2599 行）对所有会话类型都传 `attachments: &task.attachments`、`doc_refs: &task.doc_refs`，
    所以 Bug 会话的 spec 里已经有数据，只是没渲染。
  - `write_task_inputs`（约 2878–2900 行；任务卡写的是 2822–2833，已漂移）：首次时把每个附件按 `file_name()` 复制到
    `docs/<slug>/attachments/<文件名>` 并提交。
- 附件的存法：命令行 `autome task new … --attach <文件>` 把路径 `canonicalize` 成**绝对路径**后传给 `task.create`
  （`crates/autome-cli/src/main.rs` 约 410–423 行），`dispatch.rs`（约 1395–1426 行）原样存进任务记录。
  也就是说 `task.attachments` 里是源文件的绝对路径，而任务目录里的副本只保留文件名。
- 三个 Bug 模板 `protocol/prompts/{locate,fix,fix-review}.md` 目前的占位符只有 `{budget_line}`、`{doc_dir}`、`{slug}`，
  `locate.md` 另有 `{request}`。
- 协议版本：`protocol/CHANGELOG.md` 最新一节是 `## protocol/v2`（B-1 … B-4）。当前版本取 CHANGELOG 里编号最大的
  `## protocol/vN` 标题（`crates/automed/src/protocol/mod.rs::current`）；同文件单测
  `the_version_in_force_is_the_newest_changelog_heading_over_the_seed` 断言了 `protocol/v2`，递增后要跟着改。
- 测试现状：
  - `crates/automed/tests/prompt_rendering.rs` 的 `spec()` 固定传空的 `attachments` / `doc_refs`；金样在
    `crates/automed/tests/golden/prompts/`，Bug 相关的有 `locate.md`、`locate-card.md`、`fix.md`、`fix-review.md`。
  - `crates/automed/tests/end_to_end.rs`：Bug 任务用 `task.create`（`"kind":"bug"`）创建，会话 prompt 落在
    `.autome/fake/<n>.prompt`，用 `w.prompt(n)` 读；已有用例断言三个会话分别含「定位轮」「修复轮」「复核轮」（约 4030–4056 行）。
- `plugins/autome/evals/` 是对话追问的 eval（`claude plugin eval`，真实运行收费、不进门禁），与内核 prompt 无关；
  本任务不应触及 `plugins/`。

## 8. 待设计轮回答的问题

- Q1 **附件路径怎么渲染才是对的。** `task.attachments` 存的是源文件绝对路径，而整理轮的渲染是
  `{doc_dir}/attachments/{a}`——`a` 为绝对路径时拼出来的不是任务目录里那份副本的路径（副本只保留文件名）。
  任务卡要求「附件相对路径列表」且 e2e 要断言「prompt 文件含附件路径」。设计轮需先实测确认这一点，再定：
  共用的渲染是否取文件名；如果要改，整理轮的渲染结果会随之变化，这与「不改需求 Loop 的 prompt」如何界定
  （模板文本不变 vs. 渲染出的路径变正确），需要写清楚并在必要时列为请用户表态的事项。
- Q2 `inputs` 渲染抽成共用函数的位置与签名，`{inputs}` 在三个模板里放在哪一行、段落措辞是否沿用整理轮的两句标题
  （Bug 会话读者是定位 / 修复 / 复核轮），以及空串时如何保证不留空行。
- Q3 修复轮与复核轮的 prompt 是否与定位轮列出同样的两段（任务卡说三个步骤都加）。
- Q4 工作区项目（多仓库）下 `doc_dir` 是「从会话工作目录看到的路径」，附件路径随之变化；`--doc` 的路径是用户原样给的
  仓库内路径——两者在工作区项目里是否都指得到文件。
- Q5 CHANGELOG 条目的 `kind`（`behavioral` 需要带 eval 用例且「改前红、改后绿」；`clarify` 不需要）、`evidence`
  取哪两个任务、`predicted_impact` 用哪个指标；新版本号（按现状应为 `protocol/v3`）。
- Q6 协议静态门（`protocol eval`）对模板新增占位符有无检查项需要同步（例如已知占位符清单）。

## 9. 项目背景摘要（生成时）

**仓库。** 工作目录是单个仓库 `autome-v2`（本任务的 worktree），不是多仓库工作区，状态块不写 `repos:`。
Autome 2.0：Rust 内核 + Node 网关 / Web 面板 + Electron 薄壳，用自己的 Loop 开发自己。

**技术栈与布局。**

| 路径 | 是什么 |
|---|---|
| `crates/autome-domain` | 纯域模型（状态机、会话类型、Bug 文档解析、协议文件与哈希），无 I/O |
| `crates/automed` | 内核：调度（`scheduler.rs`）、会话启动与 prompt 渲染（`launcher.rs`）、JSON-RPC（`dispatch.rs`）、账本（`store.rs`）、协议 seed 与 eval（`src/protocol/`）；测试在 `tests/`（`end_to_end.rs`、`prompt_rendering.rs`、`real_cli.rs`、`golden/`） |
| `crates/autome-cli` | `autome` 命令行（`task new … --attach … --doc …`） |
| `protocol/` | 协议唯一来源：`loop-protocol.md`、`session-protocol.md`、`bug-protocol.md`、`prompts/*.md`、`brief-map.toml`、`evals/`、`CHANGELOG.md`、`README.md`；经 `include_str!` 编译进 `automed` |
| `apps/web` | React Router 8 面板 + Node 网关（Node ≥ 22.22） |
| `apps/desktop` | Electron 薄壳（`node --test`） |
| `plugins/autome`、`plugins/loop` | 宿主插件 |
| `scripts/` | `ci-local.sh`（本地 CI）、`gen-eval-seed.py`（生成 `protocol/evals/`）、打包与安装脚本 |
| `docs/<slug>/` | 各任务的文档目录（任务卡、任务文件、设计文档、固定的协议副本等） |

Rust 工具链 1.93.1（`rust-toolchain.toml`），edition 2024，Cargo workspace 三个成员。

**测试与门禁命令。**

- `cargo test --workspace`（验收标准点名）
- `cargo test -p automed --lib launcher`（launcher 单测）
- `cargo test -p automed --test prompt_rendering`（金样；`AUTOME_UPDATE_GOLDEN=1` 接受）
- `cargo test -p automed --test end_to_end`
- `cargo run -p automed -- protocol eval protocol/`（协议静态门；`--behaviour` 跑行为用例，收费，不进门禁）
- `sh scripts/ci-local.sh`——项目门禁（`.autome/config.toml` 的 `[gate]`，超时 30 分钟）：`cargo fmt --all -- --check`、
  `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets`、桌面 `npm test`、契约守卫、
  Web 的 lint / typecheck / test / build。每个实现轮之后、合并前、合并后由宿主运行。
- `sh plugins/autome/tests/run.sh evals`（插件 eval 的静态检查）

**权威规则文件。**

- 仓库根 `AGENTS.md`（见第 6 节 C2、C3）。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `protocol/README.md`：协议怎么改、版本怎么定（第 6 节 C4–C6 摘自它）。
- 参考文档 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §3.1、§4.2。
