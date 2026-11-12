# 任务：「不是 Bug」改建成需求——`autome task convert` 与 `task.convert` RPC（W7 按钮前置）

本任务的规则见 `docs/不是-bug-改建成需求-autome/protocol/loop-protocol.md` 与
`docs/不是-bug-改建成需求-autome/protocol/session-protocol.md`。

- 设计文档：`docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome.md`
- 任务卡正文（用户写的，第一输入）：`docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome-card.md`
- 任务整理日期：2026-11-12；基线提交 `42c90a5`（「T-40 固定协议 protocol/v2」，本任务即 T-40，冻结在 `protocol/v2`）
- 来源：用户的任务卡正文、一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`（下称「计划」）§4.2「不是 Bug」一段与 §8 风险表第二行。
- 任务卡正文与一句话需求不一致时，以任务卡正文为准。

## 1. 需求原文

> 「不是 Bug」改建成需求：autome task convert 与 task.convert RPC（W7 按钮前置）

## 2. 任务卡正文（原样转录，本任务的规格）

> # 「不是 Bug」改建成需求任务：`autome task convert`
>
> ## 背景
>
> 计划 §4.2：定位轮写出「不是 Bug」时任务停下等你决定——取消它，或者用同一份正文改建成需求任务。现在状态机会停在
> `NotABug`（`bug_loop.rs:227`），CLI 提示让用户自己 `task new --brief <卡片>`（`render.rs:112`），但没有任何命令或
> RPC 能导出卡片并改建；e2e `bug_loop_not_a_bug_…` 是在测试里直接读 store 再调 `task.create`。W7 任务详情页的
> 「改为需求」按钮也以此为前提。
>
> ## 要做什么
>
> 1. 内核新增写方法 `task.convert {task_id, kind: "feature", start?: bool}`：只接受停在 `not_a_bug` 的 Bug 任务
>    （其它状态拒绝并说明）；复制 brief、附件、`doc_refs` 新建一个需求任务（默认存为待开始，`start: true` 则直接
>    排队），原任务取消并写事件 `task.converted {from, to}`，新任务写 `task.created {converted_from}`。新任务的
>    卡片正文沿用原文，标题沿用原标题。
> 2. CLI：`autome task convert <任务> [--start]`，支持 `--json`（输出新任务）；`render.rs` 里 `not_a_bug` 的提示
>    改成指向这条命令。
> 3. 网页网关 `write-gate.ts` 放行 `task.convert`（与 T-37 协调：若 T-37 已合并则在其基础上加）。
> 4. e2e：改建后新任务 `kind=feature`、brief 相同、附件文件存在、原任务 `cancelled`；非 `not_a_bug` 状态被拒。
>
> ## 不做什么
>
> 不做 W7 的按钮；不改协议 prompt。
>
> ## 验收标准
>
> 见「要做什么」4，另：`autome --help` 列出 `task convert`；`cargo test --workspace`、`npm test --prefix apps/web`
> 全绿。
>
> ## 参考
>
> `crates/automed/src/dispatch.rs`（`task.create`、`task.cancel`）、`crates/autome-cli/src/{main.rs,render.rs}`、
> `crates/automed/tests/end_to_end.rs:4466`、计划 §4.2、§8。

## 3. 目标与背景

**目标。** 让「定位轮判定不是 Bug」之后的第二条出路——「用同一份正文改建成需求任务」——成为一步操作：
一个内核写方法 `task.convert`、一条命令 `autome task convert`、网页网关放行该方法。W7 的「改为需求」按钮
（不在本任务内）之后只需调用这个方法。

**计划里的依据（原文）。**

- §4.2：「写成『不是 Bug』时（行为符合设计，要改的是需求），任务同样停下等你决定：取消它，或者用同一份正文改建成
  需求任务。」
- §8 风险表：「Bug 本身就是需求（要改行为，而不是修错）｜定位轮写 `status: 不是 Bug`，任务停下等你决定（§4.2）；
  W7 提供『改为需求』按钮，用同一份正文新建一个需求任务」。

**现状。** 状态机已能停在 `not_a_bug`；之后用户只能手工 `autome cancel` 加 `autome task new --kind feature --brief <卡片>`，
而卡片正文存在内核的库里，没有命令把它导出来——提示里的 `<卡片>` 用户无从填写。

## 4. 验收标准

设计文档的里程碑必须覆盖以下各条（A-01…A-04 来自任务卡「要做什么」，A-05…A-07 来自「验收标准」）：

- **A-01 内核 `task.convert`。** 参数 `{task_id, kind: "feature", start?: bool}`。
  - 只接受 `kind=bug` 且停在 `active/not_a_bug` 的任务；其它状态（以及非 Bug 任务）拒绝，错误信息说明原因。
  - 新建一个需求任务：`kind=feature`；卡片正文（brief）与原任务逐字节相同；附件、`doc_refs` 复制自原任务；
    标题沿用原标题。
  - 默认新任务存为待开始（`draft`）；`start: true` 时直接排队（`queued`，有空位即启动，与 `task.create` 不带
    `draft` 的行为一致）。
  - 原任务被取消（`cancelled`），并写事件 `task.converted {from, to}`；新任务写事件 `task.created {converted_from}`。
  - 是写方法：不进 `READ_METHODS`。
- **A-02 CLI。** `autome task convert <任务> [--start]`；`--json` 输出新任务；`<任务>` 与其它命令一样可写 id 或
  slug 前缀。`render.rs` 里 `not_a_bug` 的「下一步」提示改成指向 `autome task convert`。
- **A-03 网页网关。** `apps/web/server/gates/write-gate.ts` 的 `ALLOWED_WRITE_OPS` 放行 `task.convert`，并有相应的
  参数校验与正反例测试（`apps/web/test/write-gate.test.ts`）。
- **A-04 e2e。** 改建后：新任务 `kind=feature`、brief 相同、附件文件存在、原任务 `cancelled`；对非 `not_a_bug`
  状态的任务调用被拒。
- **A-05** `autome --help` 列出 `task convert`。
- **A-06** `cargo test --workspace` 全绿。
- **A-07** `npm test --prefix apps/web` 全绿。

## 5. 范围

**在范围内（预计改动的位置）。**

- `crates/automed/src/dispatch.rs`：派发表新增 `task.convert` 及其实现与单测。
- `crates/automed/tests/end_to_end.rs`：A-04 的端到端用例（现有
  `bug_loop_not_a_bug_waits_for_a_decision_and_can_be_cancelled_or_rebuilt_as_a_feature` 是手工读 store 再
  `task.create` 的写法，可改写或并列新增）。
- `crates/autome-cli/src/main.rs`（`task` 子命令、`USAGE`、选项白名单）、`crates/autome-cli/src/render.rs`
  （`not_a_bug` 提示）、`crates/autome-cli/tests/cli.rs`。
- `apps/web/server/gates/write-gate.ts`、`apps/web/test/write-gate.test.ts`。
- 若设计认为需要：`crates/automed/src/store.rs`、`crates/autome-domain`（仅当确有必要，见 §8 待定问题）、
  提到 `not_a_bug` 出路的用户文档（如 `README.md`）。

**不在范围内。**

- W7 的「改为需求」按钮与任何 `apps/web/app` 页面改动。
- 协议 prompt：不改仓库根 `protocol/` 下的任何文件，也不改本任务目录里的协议副本。
- `kind` 取 `"feature"` 以外的改建方向（需求改 Bug 等）；从 `not_a_bug` 以外的状态改建。
- 桌面端 `apps/desktop`（含它的白名单副本 `apps/desktop/src/{ipc-gate,write-gate}.js`，W11 将删除）。
- T-37 的工作内容（`task.start`、`task.reply` 的放行，`task.create` 的 `kind`/`brief`/`draft`，读白名单补齐）。

## 6. 硬性约束

1. **不扩大范围。** 只做 §4 列出的行为；§5「不在范围内」的各项一律不做。
2. **不改协议 prompt。** `protocol/` 是 `include_str!` 编进 `automed` 的协议源，改它会动金样与协议版本。
3. **`task.convert` 是写方法。** 不得加入 `READ_METHODS`；桌面端测试会解析 `dispatch.rs` 里的
   `pub const READ_METHODS` 做交叉检查，该常量不得改名或搬家。
4. **改建应当要么全做、要么不做。** 不允许出现「新任务已建、原任务没取消」或反过来的半截状态对用户可见；
   失败时的行为由设计给出（见 §8）。
5. **与 T-37 协调。** T-37（分支 `autome/网关放行任务卡的创建与开始-write`，改同一个 `write-gate.ts` 及其测试）
   在本任务基线 `42c90a5` 时**尚未合并**。本任务对 `write-gate.ts` 的改动应尽量小且局部（白名单加一项、
   `validateOpSpecific` 加一个分支、测试追加用例），以便任何一方先合并后另一方 rebase 时冲突最小；若实现时
   T-37 已进入本分支的基线，则在其基础上加。不得顺手实现 T-37 的内容。
6. **现有行为不退化。** `task.create`、`task.cancel`、`not_a_bug` 下的 `task.reply`（确认是 Bug）、
   `pause/stop/extend_budget` 在 `not_a_bug` 下被拒等现有行为与用例保持不变；各套件用例数只增不减。
7. **门禁。** 每个实现轮后宿主跑 `sh scripts/ci-local.sh`（fmt、clippy `-D warnings`、`cargo test --workspace
   --all-targets`、桌面端测试、Web lint/typecheck/test/build），必须全绿。
8. **仓库规则（`AGENTS.md`）。** 不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下其它
   目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入
   `.autome/output/`，不进任务文档目录。

## 7. 调研时观察到的事实（2026-11-12，基线 `42c90a5`；供设计参考，不是设计结论）

**内核（`crates/automed/src/dispatch.rs`）。**

- 派发表在约 387–433 行；`task.create` → `task_create`（约 1327 行）做参数校验后调 `create(ctx, project_id,
  request, attachments, doc_refs, (kind, brief, draft))`（约 1431 行）。
- `create`：slug 由 `slugify(request)` 加去重得出；`title` 取 `first_line(request)`；状态 `Draft` 或 `Queued`；
  `ctx.store.create_task(&task, brief)` 落库；事件 `task.created` 的载荷目前是 `{project_id}`；非 draft 时
  立即 `scheduler::tick`，返回 `{task, started, queue_position}`。
- `task_create` 对附件逐个检查 `Path::is_file()`，不存在即拒（「附件不存在：…」）；brief 上限 65536 字节、不能为空。
- `task.cancel` → `task_cancel`（约 1878 行）：先 `stop_running_session`，再 `task_trigger(.., Trigger::Cancel)`；
  取消的实际清理在 `scheduler.rs` 的 `cancel_task`（约 3596 行，`Action::RunCancel`）：把任务文档目录归档、
  清 worktree 与分支。现有 e2e 断言取消后 `state == "cancelled"` 且 `is_archived()`。
- 写命令在整个执行期间持有驱动锁（约 225–240 行）；`READ_METHODS`（17 项）在约 2507 行。
- `task.get` 的回答里 `brief` 是与 `task` 并列的顶层字段；列表与看板不带 brief。

**附件怎么存。** `TaskRecord.attachments` 是**源文件的绝对路径**列表（CLI 用 `canonicalize`；网页端由网关把上传
编号解析成 `~/.autome/web/uploads/` 下的路径，见 `apps/web/server/gateway.ts` 约 46–54 行）。任务第一次开工时
调度器把它们拷进 worktree 的 `docs/<slug>/attachments/`（`scheduler.rs` 约 2886 行），之后 prompt 引用的是
worktree 内的副本。`doc_refs` 是仓库内相对路径的字符串列表。

**状态机（`crates/autome-domain`）。** `Node::NotABug`（`task.rs`，串化为 `not_a_bug`）；定位轮写「不是 Bug」时
`bug_loop.rs:227` 转到 `active(NotABug)`；`not_a_bug` 下接受 `Cancel` 与 `Reply`，拒绝 `Pause/Stop/ExtendBudget`
（现有 e2e 断言）。

**CLI（`crates/autome-cli`）。** `main.rs` 的 `task()`（约 402 行）现有子命令 `new`、`start`、`ls`；未知子命令报
「task 没有子命令」；每条命令允许的选项在 `run()` 约 130–145 行的白名单里；帮助文本是常量 `USAGE`；
`c.resolve_task` 把 id 或 slug 前缀解析成任务 id。`render.rs:112` 的 `not_a_bug` 提示现为
`autome cancel {id}    或    autome task new --kind feature --brief <卡片>；确认是 Bug：autome reply {id} "理由"`。
CLI 集成测试 `tests/cli.rs` 约 553 行已有把任务跑到 `not_a_bug` 的用例。

**网页网关（`apps/web/server/gates/write-gate.ts`）。** `ALLOWED_WRITE_OPS` 是冻结的允许清单，当前含
`task.create/approve/reject/merge/pause/resume/stop/cancel/decide/extend_budget/rerun_from/retro/archive/restore`，
**没有** `task.start`、`task.reply`（T-37 的内容）。`validateShape` 对所有 op 通用：非散文字符串上限 512、
只允许字符串/数字/布尔、数组仅限 `ARRAY_FIELDS` 里登记的字段；op 专属校验在 `validateOpSpecific`。
`apps/web/server/notifier-rules.ts:13` 已有 `not_a_bug` 的通知规则（本任务不需要改）。

**现有 e2e（`crates/automed/tests/end_to_end.rs:4466`）。** `World`、`bug_step`、`bug_doc`、`create_bug`、
`w.settle()`、`w.node()`、`w.state()`、`ok()` 等夹具可直接复用；该用例把任务跑到 `not_a_bug` 后手工读
`store.task_brief` 再 `task.create {kind:"feature", brief, draft:true}`，最后 `task.cancel`。现有用例创建的 Bug
任务不带附件。

**T-37。** 卡片要求：`write-gate.ts` 放行 `task.start`、`task.reply`，`task.create` 识别 `kind/draft/brief` 并放宽
载荷上限；`read-gate.ts` 补齐读方法。其分支在基线时停在裁决阶段，未合并。

## 8. 待定问题（设计必须给出答案）

1. **新任务的 `request` 与 slug。** 卡片要求「标题沿用原标题」。`create` 现在从 `request` 派生 `title` 与 slug；
   新任务的 `request` 取原任务的 `request` 还是 `title`？slug 与原任务同源时去重后会带后缀——原任务取消后其 slug
   是否仍占位、归档目录是否会与新任务的文档目录冲突？
2. **附件的来源路径。** 复制的是记录里的源路径。源文件已被用户删除或移动时怎么办（拒绝改建并说明、跳过、
   还是从原任务 worktree 里的副本取）？A-04 要求「附件文件存在」——断言的是哪里的文件（源路径、还是新任务开工后
   worktree 里的副本），须在设计里写明并让 e2e 带一个真实附件。
3. **顺序与原子性。** 先建新任务还是先取消原任务；中途失败如何回滚或补偿（硬性约束 4）；两条事件的写入顺序；
   `start: true` 时 `scheduler::tick` 在取消之前还是之后（取消会腾出并行名额）。
4. **`kind` 参数。** 必填还是可缺省为 `"feature"`；传其它值时的错误信息。
5. **事件载荷的确切形状。** `task.converted {from, to}` 挂在哪个任务 id 下；`task.created` 现有载荷
   `{project_id}` 与新增的 `converted_from` 如何并存；网页端事件总线/通知器是否需要认识新事件（预期不需要改，
   设计须确认现有消费者遇到未知事件类型不会出错）。
6. **返回值形状。** RPC 返回什么（至少要有新任务，建议与 `task.create` 的 `{task, started, queue_position}` 对齐，
   并带上原任务 id）；CLI `--json` 输出「新任务」具体指哪一层；非 `--json` 时打印什么、下一步提示是什么。
7. **原任务的取消方式。** 是否完全复用 `task_cancel` 的路径（停会话、归档文档、清 worktree）；`not_a_bug` 下
   是否可能有正在跑的会话或门禁。
8. **网关的参数校验。** `task.convert` 在 `validateOpSpecific` 里校验哪些字段（`task_id` 字符串、`kind` 只能是
   `feature`、`start` 布尔）；与 T-37 的改动如何避免冲突（硬性约束 5）。
9. **拒绝的情形清单。** 非 Bug 任务、Bug 任务但不在 `not_a_bug`（各状态：draft、queued、运行中、await_info、
   failed、cancelled、completed、已归档）、任务不存在、项目已移除——各自的错误类别与文案。
10. **文档。** `README.md` 等用户文档是否需要补一句 `task convert`；CLI 帮助里的措辞。

## 9. 项目背景摘要（2026-11-12 整理时）

**仓库。** 工作目录下只有一个仓库 `autome-v2`（工作目录即本任务的 worktree），因此设计文档状态块不写 `repos:`。
Autome 2.0：命令行加本机 Web 面板（另有待退役的 Electron 桌面外壳），把任务卡在专用 worktree 里经由 Loop 推进成
合并提交。需求任务走完整 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 → rebase →
合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）；Bug 任务走简化 Loop（定位 → 复现检查 → 修复 → 门禁 → 复核 →
合并）。**内核独占节点转换**：会话只写文档，下一个节点由 `automed` 读状态块决定。本仓库正用自己的 Loop 开发自己
（计划 §7），多个任务并行，各占一个 worktree。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`；依赖 serde、serde_json、rusqlite、toml 等）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O：角色、配置、状态块解析、任务状态机（`task.rs`、`bug_loop.rs`、
    `bug_doc.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git、会话启动器与 prompt（`launcher.rs`）、
    调度器（`scheduler.rs`）、宿主门禁（`gate.rs`）、JSON-RPC 派发（`dispatch.rs`）；测试在 `tests/`
    （`end_to_end.rs`、`prompt_rendering.rs`、`real_cli.rs`、`golden/`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`、调同一个派发器，自身只做参数解析（`args.rs`）、
    客户端（`client.rs`）与输出（`render.rs`）；集成测试 `tests/cli.rs` 跑真实二进制。
- `apps/web` — React Router 8 + 自定义 Node 服务器的面板（TypeScript，vitest）；`server/gates/{read,write}-gate.ts`
  是网页到内核的两份允许清单，`server/gateway.ts` 转发，`test/` 下是 vitest 用例。
- `apps/desktop` — Electron 外壳（`node --test`），自带一份白名单副本；本任务不动。
- `protocol/` — Loop 协议源（`include_str!` 编译进 `automed`）；本任务不动。
- `plugins/` — Claude 插件（`autome` 技能）；`scripts/` — 本地 CI 与打包脚本。
- `docs/development/` 规格权威；`docs/adr/` ADR；`docs/plans/` 按日期的决策记录；`docs/<slug>/` 各任务的文档目录。

**测试命令。**

```sh
cargo test --workspace                              # domain、core、命令行、端到端（验收 A-06）
cargo test -p automed --test end_to_end             # 只跑端到端；可再跟用例名过滤
cargo test -p autome-cli --test cli                 # 命令行集成测试（真实二进制）
cargo test -p automed dispatch                      # 派发器单测
cargo clippy --all-targets -- -D warnings           # 警告即错误
cargo fmt --all -- --check
npm test --prefix apps/web                          # vitest（验收 A-07）；首次需 (cd apps/web && npm ci)
npm run lint --prefix apps/web && npm run typecheck --prefix apps/web
sh scripts/ci-local.sh                              # 全量本地 CI，即宿主门禁跑的那条
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本，只把模型换成替身。Web 的部分用例要先有
`cargo build -p automed -p autome-cli` 的产物。不要跑 `protocol eval --behaviour`（要花钱）。输出长的命令重定向到
`.autome/output/`，只读头尾。

**权威规则文件。**

- 仓库根 `AGENTS.md`：见硬性约束 8。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`（只读）：并行 3；`[gate] command = "sh scripts/ci-local.sh"`，超时 30 分钟；崩溃重试 2 次，
  退避 60 秒。
- `.github/workflows/ci.yml` 与 `scripts/ci-local.sh`：门禁的确切步骤。
- 计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§3 任务卡、§4.2 Bug Loop、§5.5 本机安全、§8 风险。
