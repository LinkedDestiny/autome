# 任务：通知器首次启动补发当前「等你处理」的任务，已通知状态落库

本任务的规则见 `docs/通知器首次启动补发当前等你处理的任务/protocol/loop-protocol.md` 与
`docs/通知器首次启动补发当前等你处理的任务/protocol/session-protocol.md`。

- 设计文档：`docs/通知器首次启动补发当前等你处理的任务/通知器首次启动补发当前等你处理的任务.md`
- 任务卡（用户写的，第一输入）：`docs/通知器首次启动补发当前等你处理的任务/通知器首次启动补发当前等你处理的任务-card.md`
- 任务整理日期：2026-11-12
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）

## 1. 需求原文

> 通知器首次启动补发当前等你处理的任务，已通知状态落库，重启不重复、离开再进入重新通知

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§4 照任务卡整理。

## 2. 目标与背景

**背景（任务卡）。** W4（T-16，「实现 web.sqlite3 与通知器」）的设计明确选了两件事：

- 首次启动（面板库里没有 `notifier` 游标）从 `latest_seq` 起步，不补历史；
- 每个任务「最近一条状态」只存在进程内的 `Map` 里，不持久化（当时 B-03 被用户裁定忽略）。

T-38 的定位轮核实后判「不是 Bug」——现状符合既有设计。**本任务是有意改变这两条设计**：

- 装好面板的第一天，已经停在 `await_merge` / `await_design_approval` / `await_info` / `not_a_bug` / 失败 上的任务
  必须出现在铃铛里；
- 面板重启不应把同一次等待再通知一遍。

**目标（任务卡「要做什么」四条）。**

1. **首次启动补发。** 无游标时，先经 `dashboard.get`（或 `project.get` 遍历）拿到当前所有处于「等你处理」状态的任务，
   按通知规则各生成一条通知（事件序号用当时的 `latest_seq`，来源标 `backfill`），再把游标设为 `latest_seq`。
   只补当前等待中的，不补历史。
2. **「最近已通知状态」落库。** 存进 `web.sqlite3`（新表，或在通知行上记 `(kind, task_id, state_key)`），`commit` 时按它去重；
   **任务离开等待状态再进入时必须重新通知**（`state_key` 要包含进入的序号或上一次离开的序号，由设计轮定）。
3. **协调既有测试。** `notifier.test.ts` 的「首次启动已有事件时通知数为 0」与 `notifier-real.test.ts` 的「新面板库不补历史」
   **改写**为新契约（补当前等待中的、不补历史事件），不是删除；新增「重启后同状态新序号不重复」「离开再进入重新通知」两条。
4. **迁移。** 已有 `web.sqlite3` 升级时不重发历史。

## 3. 验收标准

以下逐字取自任务卡「验收标准」。设计文档的里程碑必须逐条覆盖；用户在设计定稿时会对照它们批准或驳回。

1. 真实账本用例：账本里有一个 `await_merge` 任务、空面板库启动 → 恰 1 条通知；
2. （同一用例续）重启面板并追加一条不改状态的 `task.updated` → 通知数不变；
3. （同一用例续）任务 `reject` 后再次到 `await_design_approval` → 新增 1 条；
4. `npm test --prefix apps/web` 全绿，用例数只增不减。

补充说明（从任务卡与调研得出，不是新范围）：

- 「真实账本」指用真实的 `automed` / `autome` 二进制产出的账本，写法参照 `apps/web/test/notifier-real.test.ts`
  （`CoreSupervisor` + `fixtures/w4-ledger.sh`，模型换成假 CLI）。只用 `notifier.test.ts` 那种 `vi.fn` 替身的 `call`
  不能代替验收 1–3；替身用例是补充。替身返回的形状必须与真实 `dashboard.get` / `events.since` 的形状一致（见 §6）。
- 验收 2 的「追加一条不改状态的 `task.updated`」：设计须说明在真实账本里用什么操作产生这样一条事件；若真实内核
  确实产生不出来，如实写明并说明用什么等价手段验证（不要悄悄退回成纯替身用例）。
- 验收 3 的 `reject` 指用户驳回设计（任务离开 `await_design_approval`，重新设计后再回到 `await_design_approval`）。
  任务卡前半句的任务在 `await_merge`；设计须把这条用例的任务与步骤写清（同一任务还是另一任务、从哪个等待状态驳回）。
- 验收 4 的「只增不减」：整理时 `apps/web/test/*.test.ts` 顶层 `test(` 约 286 条（`grep` 粗计，实现轮以 vitest 报告为准）。
  任务卡点名的两条是改写，不是删除；其余既有用例若必须改断言，设计要逐条列出理由。
- 任务卡要求的四条测试变化都要能在测试名或断言上认出来：两条改写（首次启动、新面板库）、两条新增
  （「重启后同状态新序号不重复」「离开再进入重新通知」），外加迁移（已有库升级不重发）的用例。

## 4. 范围

在范围内（全部在 `apps/web/`）：

- `apps/web/server/notifier.ts`：首次启动的补发流程；去重改为读写持久化的已通知状态；
- `apps/web/server/store.ts`：`MIGRATIONS` 新增一级迁移（新表或新列）、`commit` 的去重、`resetLedger` 的配套清理、
  供通知器读写已通知状态的方法；
- `apps/web/server/notifier-rules.ts`：仅当补发需要「按任务当前状态匹配规则」的入口时才动（现有 `matchEvent` 只吃事件）；
- `apps/web/test/notifier*.test.ts`、`apps/web/test/store.test.ts`、`apps/web/test/fixtures/`：改写与新增用例；
- 文档：`docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器.md` 是已归档任务的设计文档，**不改**；
  本任务对那两条设计的改变写在本任务自己的设计文档里。

不在范围内（任务卡「不做什么」，不要做）：

- **不改内核**：`crates/` 下三个 crate 一行不动——不新增 JSON-RPC 方法、不改 `dashboard.get` / `project.get` /
  `task.get` / `events.since` 的参数与返回；
- **不改铃铛界面**：`apps/web/app/` 下的铃铛组件、`bell-*.test.ts` 不动；
- **不改系统通知**：浏览器端系统通知的触发条件（`levelOf`、`notifier-api.ts` 里的推送逻辑）不动；
- 不补历史事件、不为 `done`（`fyi` 级）补发；
- 不动 `apps/desktop/`、`protocol/`、`plugins/`、`.autome/`、`scripts/`；
- 不顺手重构通知器或面板库中与本需求无关的代码。

## 5. 硬性约束

1. **只补当前等待中的任务，不补历史。** 补发的集合是首次启动那一刻处于「等你处理」的任务：
   `active` 且节点为 `await_design_approval` / `await_merge` / `await_info` / `not_a_bug`，或 `failed`。
   已合并、已取消、正在跑的任务不产生通知；一个任务历史上进过多少次等待都只补当前这一次。
2. **补发通知的形状。** 每个等待任务恰一条；`kind`、标题、`body`、`project_id` 与同一状态走事件路径时生成的一致
   （复用 `NOTIFICATION_RULES` / `renderTitle` / `renderReason`，不另写一套文案）；`event_seq` 用当时的 `latest_seq`；
   来源标 `backfill`。现有 `notifications` 表没有「来源」列，设计须定它记在哪里，并保证不改变铃铛接口
   （`/api/notifications` 的返回、WebSocket `notification` 消息恰五键 `type/id/kind/task_id/title`）对界面可见的形状——
   若要多返回字段，须论证铃铛界面与其测试不需要改。
3. **补发与游标原子落库。** 现有 `store.commit` 把「一条通知 + 游标」放在一个事务里。首次启动要写多条通知再写游标：
   设计须保证中途崩溃或 `close()` 后重启，结果仍是每个等待任务恰一条、且游标之后的事件不丢不重
   （例如全部放进一个事务，或给出等价的可重入方案）。
4. **快照与序号之间的竞态要写明。** `dashboard.get` 与 `events.since` 是两次调用，不是原子的：两次调用之间任务可能进入或
   离开等待状态。设计须给出调用顺序与去重规则，使任何交错下都不漏发、不重复（同一次等待最多一条）。
5. **补发是否向在线连接推送**（`publish`）由设计定并写明理由；无论哪种，都不能因此触发范围外的界面或系统通知改动。
6. **已通知状态持久化，重启后仍然有效。** 判断「是不是同一次等待」不能再依赖进程内 `Map`。重启后收到同一任务、
   状态未变、序号更大的 `task.updated`，不得产生新通知（任务卡：「重启后同状态新序号不重复」）。
7. **离开再进入必须重新通知。** 任务离开等待状态（包括转到另一个等待状态、转到非等待状态）后再次进入同一等待状态，
   必须新增一条。这包括「离开」发生在面板停服期间的情形：重启后按游标续拉，能看到中间那条离开事件，结果必须与不停服一致。
   `state_key` 的构成（进入的序号或上一次离开的序号）由设计轮定，并说明它在补发路径上取什么值——补发时并不知道
   任务进入等待状态的真实序号。
8. **既有保证不退化。** 以下现有行为各有测试守着，改动后必须仍然成立：停服期间的事件按 `seq` 补齐且各推一次；
   通知已写、游标未推进时重启不重不推；游标写失败时通知随事务回滚；补齐信息遇到无 `code` 的错误时保留游标稍后重试、
   带 `code` 的拒绝仍生成通知；并发 `pull()` 合并为一轮；`close()` 后不再调 `call`、在途结果不写库；
   `project.alert` 的去重（`kind` + 空 `task_id` + `event_seq`）。
9. **账本更换。** `latest_seq < cursor` 时现有逻辑调 `resetLedger`（把旧通知的 `event_seq` 置空、游标回到新 `latest_seq`、
   清空进程内状态与补齐缓存）。持久化的已通知状态必须同时清掉——新账本会复用 `T-1` 之类的编号，旧状态不能抑制新账本的通知。
   换账本后是否对新账本做一次补发，由设计定并写明理由。
10. **迁移不重发历史。** 已有 `web.sqlite3`（schema 第 1 版、已有游标和通知行）升级后：不触发首次启动补发（游标已存在）；
    不因为「已通知状态」表是空的，就把升级前已经通知过、此刻仍在等待的任务再通知一遍。设计须说明升级时已通知状态
    如何取得初值（例如由已有通知行或当前快照播种），并给出用旧版库文件验证的用例。迁移走现有 `MIGRATIONS` 数组追加一级，
    不改第 1 级的 SQL；「库比代码新」的中文报错与整级回滚的现有行为不变。
11. **面板只读内核。** 通知器只调读方法；`dashboard.get`、`project.get`、`project.list`、`task.get`、`events.since`
    都已在 `apps/web/server/gates/read-gate.ts` 的白名单里，不需要也不允许扩白名单。
12. **测试替身要与真实接口对齐。** 本项目多次出过「替身跟调用方对齐、真机坏」的缺陷。凡替身返回 `dashboard.get` 等结果的
    用例，形状以 §6 记录的真实返回为准；验收 1–3 必须有跑真实二进制的用例。
13. `npm test --prefix apps/web`、`npm run lint --prefix apps/web`（`--max-warnings 0`）、`npm run typecheck --prefix apps/web`、
    `npm run build --prefix apps/web` 全部通过；宿主门禁 `sh scripts/ci-local.sh` 由 Autome 在实现轮之后自己跑，
    不要为了让门禁变绿去改 `ci-local.sh` 或跳过步骤。
14. **并行任务。** 本仓库 `[loop] parallel = 3`，其它任务也可能改 `apps/web/`。改动保持在必要范围内，不做无关的移动或重排。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

**通知器（`apps/web/server/notifier.ts`，85 行）。**

- `createNotifier({ store, call, publish, log, pageSize = 200 })` → `{ pull(), close() }`，游标名 `notifier`。
  `main.ts:19` 创建；`notifier-api.ts` 的 `attachNotifier` 在每次 tick 与内核连上时调 `pull()`。
- 首次启动（`store.getCursor('notifier') === undefined`，第 22–27 行）：调 `events.since {after_seq: Number.MAX_SAFE_INTEGER}`
  取 `latest_seq`，`save(latest_seq)` 后直接返回，不生成任何通知。
- 进程内 `states = new Map<任务 id, 最近一条 task.updated 的 payload.state>`（第 15 行）：`task.updated` 的 `state` 与上一条
  `isDeepStrictEqual` 时不匹配规则；每处理完一条 `task.updated` 更新。重启即空——重启后每个任务的第一条事件都按「进入」处理，
  所以重启后一条不改状态的 `task.updated`（新 `seq`）现在会多出一条通知。
- 补齐：任务类调 `task.get {task_id}`，取 `project.id`、`task.title`；项目类调 `project.get {project_id}`。结果按进程内 `cache` 缓存。
- 账本更换：`page.latest_seq < cursor` → `store.resetLedger('notifier', latest_seq)`，清 `states` 与 `cache`，记一行日志后返回。

**规则（`apps/web/server/notifier-rules.ts`）。** `NOTIFICATION_RULES` 七条：`await_design_approval`、`await_merge`、`await_info`、
`failed`、`not_a_bug`（均 `task.updated`、`need_you`），`project_alert`（`project.alert`、`need_you`），`done`（`fyi`）。
`matchEvent(event)` 按 `event.kind` + `payload.state.state` + `payload.state.node` 匹配；`renderTitle(rule, event, branch)` 用
`event.subject` 作 `{task}`、`payload.state.reason` 渲染 `{reason}`。两者的入参都是账本事件，补发路径手里是任务快照。

**面板库（`apps/web/server/store.ts`，115 行）。**

- `node:sqlite` 的 `DatabaseSync`，文件 `<AUTOME_HOME>/state/web.sqlite3`，WAL。`MIGRATIONS` 目前只有一级：
  `schema_version(version)`、`event_cursor(name PK, seq)`、
  `notifications(id PK, kind, project_id, task_id, title, body, event_seq, created_at, read_at)`、`preferences(key PK, value)`；
  唯一索引 `notifications_dedupe (kind, ifnull(task_id,''), event_seq)`。
- `migrate()` 按 `schema_version` 逐级在事务里执行，失败整级回滚；库版本比代码新时抛中文错误。
- `commit({cursor, notification?})`：一个 `BEGIN IMMEDIATE` 事务里 `INSERT OR IGNORE` 一条通知并写游标；被唯一索引挡下时返回
  `undefined`（通知器据此不推送）。
- `resetLedger(name, latest)`：`UPDATE notifications SET event_seq=NULL` 并写游标。
- `apps/web/test/store.test.ts`（14 条）里「首次建库四表列名、WAL 与版本 1」断言了表清单、列名与版本号 1；
  新增一级迁移后这条断言必然要跟着改，属于必须列出理由的那一类。

**内核读接口（`crates/automed/src/dispatch.rs`，只读不改）。**

- `dashboard.get`（约第 2190 行）返回 `{waiting, running, alerts, cooldowns, environment}`。`waiting` 是所有项目里
  `task.state.awaits_user()` 的未终结任务，每项 `{project_id, project_name, task, pending_decisions, session, retry}`，
  其中 `task` 是 `task_json`：`{id, kind, slug, title, request, state, budget_n, branch, created_at, completed_at, …}`，
  `state` 与 `task.updated` 事件的 `payload.state` 同形（`{state:"active", node:"await_merge"}` / `{state:"failed", reason:{kind,…}}`）。
- `awaits_user()`（`crates/autome-domain/src/task.rs` 第 125、381 行）= `Active` 且节点为 `AwaitDesignApproval` / `AwaitMerge` /
  `AwaitInfo` / `NotABug`，或 `Failed`——恰好对应规则表里五条任务类 `need_you` 规则。`is_terminal()` 只含 `Done` / `Cancelled`，
  所以失败的任务在 `waiting` 里。
- `dashboard.get` 的返回里**没有 `latest_seq`**；`alerts` 是未消除的项目告警（任务卡只要求补任务，告警不在要求之内）。
- `events.since {after_seq, limit}`（约第 2455 行）返回 `{events:[{seq, kind, subject, payload}], latest_seq}`，`limit` 上限 1000。
- `project.get {project_id}` 返回 `{project, tasks:[task_json…], counts, config, violations, rules}`，会读项目配置并扫描技能目录，
  比 `dashboard.get` 重。

**现有通知器测试（`apps/web/test/`）。**

- `notifier.test.ts`（11 条，替身 `call`）：第 30–36 行「首次启动从 latest_seq 开始且空账本可初始化」断言首次调用是
  `events.since {after_seq: MAX_SAFE_INTEGER}`、通知数为 0——任务卡点名改写的第一条。其 `fixture()` 的替身 `call` 对
  `events.since`、`task.get` 以外的方法一律返回 `project.get` 的形状，新增 `dashboard.get` 调用后这个替身要补上真实形状。
  其余用例都以「预置游标 0」开场，不经过首次启动分支。
- `notifier-real.test.ts`（2 条，真实 `automed`）：第一条末尾用一个全新的面板库（`join(root,'fresh')`）跑通知器，断言
  `list()` 为 0 且 `publish` 抛「历史被推送」——任务卡点名改写的第二条。注意主流程的 `beforeAll` 在**空账本**上先 `pull()` 一次，
  此时首次启动补发的集合为空，随后七类计数 14 条的断言依赖这一点。
- `fixtures/w4-ledger.sh`（237 行）：用真实 `autome` / `automed` 加假 CLI 跑出覆盖七类通知的账本；需要先
  `cargo build -p automed -p autome-cli`（`target/debug/` 下的二进制；`ci-local.sh` 里 Rust 测试先于 Web 测试，门禁里已具备）。
- `notifier-live.test.ts`（3 条，整服务 + WebSocket）：`beforeAll` 在空账本上启动并等到游标出现；
  「停服期间新增失败，重启只补一条且保留旧已读」。`notifier-live-fixture.test.ts`、`notifier-api.test.ts`、`notifier-rules.test.ts` 另计。

**参考资料的现状。**

- W4 设计文档 `docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器.md`：§3「进入」的定义与「记忆不持久」（约第 166–169 行）、
  §4 第 2 点「首次启动不追溯历史」的理由（约第 188–190 行）、第 445 行 B-03 的裁定记录。
- 任务卡提到的 T-38 定位文档 `docs/通知器首次启动不补发等你处理的任务/…-bug.md` 与分支
  `autome/通知器首次启动不补发等你处理的任务`：整理时本仓库里**找不到**这个目录，也没有这个分支（`git branch -a` 只有本任务分支，
  `git log --all` 无相关提交）。任务卡「背景」一节已转述其结论，后续各轮以任务卡为准，不必再找这份文档。

## 7. 项目背景摘要（2026-11-12 整理时）

**仓库。** 单个仓库 `autome-v2`。Autome 2.0：一个 macOS 桌面应用、命令行与 Web 面板，把一句话需求在专用 worktree 里经由 Loop
（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 → rebase → 合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）
推进成合并提交；Bug 任务走另一条 Loop。**内核独占节点转换**：会话只写文档与代码，下一个节点由 `automed` 读状态块决定。
本仓库正在用自己的 Loop 开发自己。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1）：`crates/autome-domain`（纯类型与状态机）、`crates/automed`
  （SQLite 账本、Git、调度、JSON-RPC 派发）、`crates/autome-cli`（`autome` 命令）。**本任务只读不改。**
- `apps/web` — 本任务的全部改动所在。React Router 8 + 自定义 Node 服务器（Express 5、`ws`），Node ≥ 22.22，TypeScript 6，
  ESM，直接跑 `.ts`；面板库用内置 `node:sqlite`；测试 vitest 5。
  - `server/`：`main.ts`（组装）、`core/`（托管 `automed` 子进程与 JSON-RPC）、`gates/`（读写白名单）、`eventbus*.ts`（事件推送）、
    `ticker.ts`、`store.ts`（面板库）、`notifier.ts` / `notifier-rules.ts` / `notifier-api.ts`（通知器、规则、HTTP 与推送接线）。
  - `app/`：面板界面（含铃铛）；`test/`：全部测试与 `fixtures/`。
  - 面板只是展示与控制面：读内核账本，自己的状态只有 `web.sqlite3`（游标、通知、偏好）。
- `apps/desktop` — Electron 外壳；`protocol/` — Loop 协议源；`plugins/`；本任务都不动。
- 规格权威：`docs/development/requirements.md`、`docs/development/technical-design.md`；Web 面板的计划在
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`；`docs/adr/`。

**测试命令。**

```sh
cargo build -p automed -p autome-cli     # 真实账本用例需要 target/debug/ 下的二进制
npm test --prefix apps/web               # vitest run（验收 4）
npm run lint --prefix apps/web           # eslint --max-warnings 0
npm run typecheck --prefix apps/web      # react-router typegen && tsc --noEmit
npm run build --prefix apps/web
sh scripts/ci-local.sh                   # 全量本地 CI（Rust + 桌面端 + Web），即本项目的 [gate] command
```

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下的其它目录、不推送；
  完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不要进任务文档目录。
- `.autome/rules/`：只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`，`timeout_minutes = 30`；
  `[retry] attempts = 2`，`backoff_seconds = 60`。
