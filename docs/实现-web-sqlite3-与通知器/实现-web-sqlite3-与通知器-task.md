# 任务：Web 通知（W4）——web.sqlite3 面板库、事件→通知的规则引擎与实时推送、顶栏铃铛已读、后台标签页系统通知

本任务的规则见 `docs/实现-web-sqlite3-与通知器/protocol/loop-protocol.md` 与
`docs/实现-web-sqlite3-与通知器/protocol/session-protocol.md`。

- 设计文档：`docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器.md`
- 任务文件生成日期：2026-11-11；基线提交 `f4abb56`（任务分支 `autome/实现-web-sqlite3-与通知器`，
  从 main 拉出；W1、W2、W3、K0、K0b、K1、K2 均已合并进基线）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支）
- 证据、简报、运行记录一律写在 `docs/实现-web-sqlite3-与通知器/` 下

## 1. 需求原文

> 实现 web.sqlite3 与通知器：按规则把内核事件变成通知并实时推送，顶栏铃铛支持已读，标签页在后台时发系统通知

这句话是计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 中 **W4 通知** 的「一句话需求」，一字不差。
计划给 W4 的「目录」与「验收」见下面 §2、§3；它们和计划 §5.3（面板库）、§5.4（通知）一起构成本任务的范围。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-04 原样取自计划 W4 的「验收」；R-05..R-07 是需求原文与计划 §5.2 / §5.4 对 W4 的约束。编号供评审、审计追溯使用。

- **R-01 表结构和计划 §5.3 一致，带版本迁移。** 文件位置 `<AUTOME_HOME>/state/web.sqlite3`（默认 `~/.autome/state/web.sqlite3`，
  与内核账本 `automed.sqlite3` 同目录、互不读写）。表（计划 §5.3 原文）：
  - `schema_version`
  - `event_cursor(name, seq)`
  - `notifications(id, kind, project_id, task_id, title, body, event_seq, created_at, read_at)`，在 `(kind, task_id, event_seq)` 上加唯一约束
  - `preferences(key, value)`

  迁移按 `schema_version` 逐级执行、可重复启动（第二次启动不重复建表、不丢数据）；版本比代码新时的处理由设计给出。
  数据库用 Node 内置 `node:sqlite`（W1 已在系统 Node 26.5.0、Homebrew Node 22.23.1、Electron 43.2.0 内置 Node 24.18.0
  上验证可用且 WAL 生效，结论「采用 `node:sqlite`，不引入 `better-sqlite3`」）。Node 22 线上 `node:sqlite` 会打印
  `ExperimentalWarning: SQLite is an experimental feature`，W1 把「是否在入口过滤」留给了 W4，由设计决定并写明理由。
  **注意**：SQLite 唯一约束里 `NULL` 互不相等，`project.alert` 这类可能没有 `task_id` 的通知若存 `NULL`，
  `(kind, task_id, event_seq)` 唯一约束挡不住重复——设计必须说明怎么保证这类通知同样不重复，并有测试。
- **R-02 规则和计划 §5.4 一致，表驱动，每条规则一个测试。** 计划 §5.4 原文：

  | 内核事件 | 通知 | 级别 |
  |---|---|---|
  | 进入 `await_design_approval` | 「T-3 设计已定稿，等你批准」 | 需要你 |
  | 进入 `await_merge` | 「T-3 可以合并了」 | 需要你 |
  | Bug 进入「等你补充信息」 | 「T-7 无法复现，需要更多信息」 | 需要你 |
  | 进入 `failed` | 「T-3 停在失败上：<原因>」 | 需要你 |
  | `project.alert` | 「main 门禁失败」 | 需要你 |
  | 进入 `done` | 「T-3 已合并」 | 知会 |

  规则是一张数据表（匹配条件 → 通知 `kind`、标题模板、级别），通知器按表逐条匹配，不是一串手写 `if`。
  只做这六条；表外的事件（例如 Bug 的 `not_a_bug`、`stopped`、`cancelled`）不产生通知——若设计认为应当加，
  写进 `## 争议项` 交用户决定，不在本任务单方加入。
  「进入」的含义（同一任务连续两条状态相同的 `task.updated` 是否算两次进入）、`<原因>` 如何从 `FailureReason`
  渲染成中文一句话、`project.alert` 的标题如何随告警 `kind` 变化（目前账本里只有 `main_gate_failed`），由设计给出并测试。
- **R-03 服务重启后不重复、不漏发（游标测试）。** 通知器的进度存在 `event_cursor` 表里（`name` 由设计定）。
  服务停止期间别的进程（`autome` 命令行、`autome run`）写进账本的事件，重启后都要补成通知，且已生成过的不再生成。
  至少覆盖：停服前已处理的事件重启后不再产生通知；停服期间产生的事件重启后补齐；处理到一半崩溃（通知已写、游标未推进，
  或反过来）后重启不重不漏；账本被换掉（`latest_seq` 小于持久游标）时的处理由设计给出。
  首次启动（库里还没有游标）从哪里开始——追溯历史还是从当前 `latest_seq` 开始——由设计决定并写明理由。
- **R-04 已读、全部已读，未读数实时更新。** 单条已读、全部已读写 `read_at`；未读数在**所有已打开的标签页**里实时更新
  （一个标签页里点已读，另一个标签页的铃铛计数随之变化，不刷新页面）。
- **R-05 实时推送沿用计划 §5.2 的消息格式。** 服务端 → 浏览器（计划 §5.2 原文）：
  ```jsonc
  { "type": "notification", "id": "ntf-…", "kind": "await_merge", "task_id": "T-3", "title": "…" }
  ```
  经 W3 的事件总线 `/ws` 推送（见 §5 背景里的 `bus.broadcast` 挂点），不另开 WebSocket 或轮询。已读变化在标签页之间
  如何同步（新增消息类型、还是重新拉取）由设计给出；新增的消息类型不得改变 §5.2 已定义消息的形状。
- **R-06 顶栏铃铛。** 显示未读数，展开后列出通知（标题、时间、已读状态），能单条已读、全部已读，点通知跳到对应任务
  （任务详情页是 W7，本任务只需给出链接，目标路由不存在时不报错）。铃铛的数据来自服务端（loader 或服务端接口），
  不在客户端维护一份通知副本之外的「真相」（计划 §5.2：前端不维护状态副本）。
- **R-07 标签页在后台时发系统通知。** 级别为「需要你」的通知，在 `document.visibilityState !== 'visible'` 时用浏览器
  `Notification` API 发系统通知；「知会」级只进铃铛。请求通知权限的时机（需要用户手势）、权限被拒时的表现、
  多个标签页同时在后台时避免同一通知弹多次，由设计给出并测试。Electron 薄壳里的原生通知是 W11 的事（preload `notify()`），
  本任务只需让发系统通知这一步可替换（例如一个接口），不实现 Electron 侧。

## 3. 范围

**可以改动的目录与文件：**

- `apps/web/server/store*`、`apps/web/server/notifier*`、`apps/web/app/components/bell*`（计划 W4「目录」）——新代码放这里；
- `apps/web/server/main.ts`——只为接线：打开面板库、创建通知器、把它挂到 W3 总线（`bus.subscribe` 或总线拉取之后）、
  关闭时关库；
- `apps/web/server/http.ts`——**只在**设计选择用服务端接口（而不是 React Router 的 loader/action）给铃铛读数据、标记已读时，
  新增通知相关的接口；写接口必须经过 W2 已有的 `securityMiddleware`（Host + Origin），不改已有路由与安全语义。
  注意 `getLoadContext` 是 W5 的事，本任务不加；
- `apps/web/server/start.js`——**只在**设计决定过滤 `node:sqlite` 的 ExperimentalWarning 时改，只改这一处；
- `apps/web/app/root.tsx`——只为把铃铛挂到页面顶部（W5 之后会搬进正式的外壳，设计说明挂法如何便于搬迁）；
- `apps/web/app/routes.ts` 与 `apps/web/app/routes/` 下以 `notifications` 开头的资源路由——**只在**设计选择用
  React Router 的 loader/action 承载铃铛数据与已读操作时新增；
- `apps/web/test/`——新测试与 `test/fixtures/` 下的替身；已有测试只在接线引起的必要变化时改，并逐条说明理由。

**明确不做（出现即越界）：**

- 任务卡上的「等你处理」角标（计划 §5.4 提到，但属于 W6 的卡片）；本任务的数据要能支撑它（例如按任务查未读的「需要你」通知），
  设计说明以后从哪里取，但不实现卡片；
- 外壳、左侧项目列表、首页总览、⌘K、离线横幅、写按钮置灰、`labels.ts`、设计 token 迁移（W5）；
- `getLoadContext`（W5，W2 的 Backlog B-03）；
- 项目页、任务详情页（W6、W7）；会话日志（W8）；`server.json`、`~/.autome/web/` 目录与 `autome web`（W10）；
- Electron 原生通知与 preload（W11）；Playwright 端到端（W12）；
- `preferences` 表只建表、提供最小的读写函数即可，不做任何偏好设置界面；
- 改 W3 事件总线的游标、续传与 `event` 推送语义；改 W2 的安全检查语义；任何令牌、Cookie、登录会话（计划 §5.5）；
- 新增 npm 依赖（`node:sqlite` 是内置模块）。设计认为确实需要时，作为争议项交用户决定。

## 4. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`、`scripts/`、`.github/`。** 计划 D1：Web 任务不碰 `crates/`。
   只能**读**或**运行**它们（构建并运行 `target/debug/automed`、`target/debug/autome`）。
   每个里程碑的 `npm test --prefix apps/desktop` 结果不得退化。
2. **事件来源唯一。** 通知只由内核账本的事件（`events.since`）产生，不读 sidecar 的 Event 帧，不轮询 `task.get`
   去比对状态。补齐信息（例如通知需要的 `project_id`、`<原因>` 文字）可以调用读白名单里的方法（`task.get` 等），
   失败时的处理（重试、跳过、游标停住）由设计给出——**游标只能在对应事件已经确定处理完之后推进**。
   W3 总线的游标在内存里、首次连上内核时从 `latest_seq` 开始、不推历史，所以只靠 `bus.subscribe` 拿不到停服期间的事件；
   通知器如何用自己的持久游标从 `events.since` 补拉、又不和总线的拉取互相阻塞或叠加，由设计给出。
3. **不阻塞 tick 与事件推送。** 通知器的处理（含 SQLite 写入、补拉、对 `task.get` 的调用）不得让 ticker 叠加，也不得
   拖慢 W3 对 `event` 消息的推送；同一时刻通知器自身最多一个补拉在途。
4. **写入与推送的顺序。** 通知先落库（唯一约束去重成功）再推 `notification` 消息；被唯一约束挡下的重复不推送。
5. **测试不碰真实 `~/.autome`，也不启动真实的 Agent。** 任何打开面板库、启动内核或服务器的测试都用 `mkdtemp` 出来的
   临时 `AUTOME_HOME`（或显式 `home`），不改 `process.env`。需要真实内核事件的用例，tick 不得拉起真实的 `claude` / `codex`
   会话；如何构造出 `await_merge`、`done`、`failed`、`await_info` 等状态的事件，由设计实测后给出（纯函数规则测试可以用
   手写事件，但手写事件的形状必须与下面第 6 条的实测原文一致）。
6. **事件载荷以实测为准，逐字粘贴。** 下面是从源码读出的形状，**不是实测**：`task.updated` 的 `subject` 是任务 id，
   `payload` 形如 `{"state": <TaskStateRepr>, "trigger": "<触发名>"}`，`TaskStateRepr` 是带 `state` 标签的嵌套对象
   （`{"state":"active","node":"await_merge"}`、`{"state":"done"}`、
   `{"state":"failed","at":"<节点>","reason":{"kind":"<snake_case>", …}}`）；Bug 的「等你补充信息」是
   `active` + `node: "await_info"`；`project.alert` 的 `subject` 是项目 id，`payload` 是
   `{"alert_id", "kind": "main_gate_failed", "task_id"}`。设计轮必须用真实 `autome` / `automed` 跑出这些事件，
   把 `events.since` 的原始 JSON **逐字粘贴**进设计文档（W3 的教训：载荷被改写成简写导致测试谓词永远不匹配）。
7. **服务端 TypeScript 走 Node 原生类型剥离**（ESM，`.ts` 后缀导入，无 tsx），与 W1–W3 一致；测试用 vitest；
   `npm run lint`（`--max-warnings 0`）与 `npm run typecheck` 必须无错误；`npm run build` 必须通过。
8. **用户可见文字全中文**，专有名词、方法名、字段名保留原文（2026-09-15 定下的 UI 硬规则）。通知标题、系统通知文字、
   铃铛界面、服务端日志都算用户可见文字。
9. **一屏规则。** 1512×944 下页面本身不出现纵向滚动；铃铛展开的列表可以在内部滚动。
10. **宿主门禁会跑 ci-local。** `.autome/config.toml` 配置了 `[gate] command = "sh scripts/ci-local.sh"`（超时 30 分钟），
    每个实现轮之后、合并前、合并后在任务 worktree 里跑。Web 步骤不得依赖交互或浏览器。
11. 不修改 `.autome/` 下任何内容；不切分支、不推送、不操作 `.worktree/` 下的其它目录；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`（已 gitignore），不进任务目录。

## 5. 项目背景摘要（生成时）

**仓库：** autome-v2（单仓库）。Autome 2.0：本地 Agent Loop 编排器，Rust 内核 + Node Web 面板 + Electron 外壳。

- `crates/autome-domain` —— 纯域模型：任务状态机、节点（`Node::as_str()`：`await_design_approval`、`await_merge`、
  `await_info`、`not_a_bug` 等）、`TaskState`（序列化为 `TaskStateRepr`，`#[serde(tag = "state", rename_all = "snake_case")]`：
  `draft | queued | active{node} | paused{resume} | stopped{at} | failed{at, reason} | done | cancelled`）、
  `FailureReason`（`#[serde(tag = "kind")]`：`design_budget`、`impl_budget`、`fix_budget`、`infeasible`、`protocol`、
  `session_crashed`、`rebase_conflict`、`config`、`core_step` 等，多数带 `detail`）、`TaskKind`（`feature | bug`）。
- `crates/automed` —— SQLite 账本 `<AUTOME_HOME>/state/automed.sqlite3`（`events` 表 append-only，带 `seq`、`kind`、
  `subject_id`、`payload`）、Git、会话、调度、门禁、协议；framed JSON-RPC over stdio。
  - `src/dispatch.rs`：`READ_METHODS`（17 项，含 `task.get`、`project.get`、`dashboard.get`、`events.since`）；
    `events.since` 参数 `after_seq`（缺省 0）、`limit`（缺省 200，上限 1000），返回 `{events:[{seq,kind,subject,payload}], latest_seq}`。
  - 状态转换在 `src/scheduler.rs` 里统一 `append_event("task.updated", task_id, {"state", "trigger"})`；
    启动失败也记 `task.updated`（`trigger: "action_failed"`）；`project.alert` 由合并后验证 main 失败时记
    （`scheduler.rs` 约 2711 行）；`project.alert.dismiss` 记 `project.alert_dismissed`。
- `crates/autome-cli` —— `autome` 命令行（`target/debug/autome` 已存在）。状态在 `$AUTOME_HOME`（默认 `~/.autome`），
  与面板共用；有 `task new [--kind bug --brief - --draft]`、`task start`、`reply`、`approve / reject / pause / resume /
  stop / cancel / rerun / extend`、`project add <路径> --skip-onboarding`、`rpc <方法> '<JSON>'` 等子命令。
- `apps/web` —— React Router 8 框架模式 + express 5 自定义服务器，TypeScript ~6.0，Vite 8，vitest 5，ESLint 10，
  依赖 `ws` 8.22.2，`engines.node >= 22.22.0`。现状：
  - `server/main.ts`：`startServer({port, mode, home, binaryPath})` 依次创建 `createEventBus`、`CoreSupervisor`
    （`onStatus → bus.setCoreStatus`）、`createGateway`、`createHttpServer`，监听后 `registerUpgrade('/ws', createWsUpgrade(bus))`、
    `core.start()`、`createTicker({…, onTick: () => { void bus.pull(); }})`；返回 `{server, gateway, registerUpgrade, bus, close}`。
  - `server/home.ts`：`resolveAutomeHome(env)` = `AUTOME_HOME ?? $HOME/.autome`；`server/config.ts`：`HOST`、`resolvePort`（默认 7420）。
  - `server/eventbus.ts`（W3）：内存游标、单飞 `pull()`、续传；给 W4/W8 的挂点：`bus.subscribe(listener)`（进程内按 `seq`
    顺序拿到每个**已广播**事件）、`bus.broadcast(message)`（发非事件消息给所有连接，不参与 `seq` 过滤）、
    `bus.onClientMessage(type, handler)`、`bus.onDisconnect(handler)`。`server/eventbus-protocol.ts`：消息类型、
    `encodeMessage`、`ExtensionMessage = {type: string, …}`。`server/eventbus-ws.ts`：`/ws` 升级处理。
  - `server/http.ts`：`/healthz`、`POST /api/read`、`POST /api/write`（经读写白名单）、开发模式 Vite middleware、
    生产模式静态资源与 `createRequestHandler`；`securityMiddleware(port)` 在最前；返回 `registerUpgrade`。
  - `server/security.ts`、`server/gateway.ts`、`server/gates/{read-gate,write-gate}.ts`、`server/ticker.ts`、`server/core/*`。
  - `scripts/probe-sqlite.ts` + `scripts/sqlite-check.mjs`：W1 的 `node:sqlite` 双运行时验证（`npm run probe:sqlite`）。
  - `app/`：`root.tsx`（Layout + `useLiveEvents()` + `LiveBanner` + `Outlet`）、`routes.ts`（只有 `index("routes/home.tsx")`）、
    `routes/home.tsx`（占位）、`lib/live.tsx`、`lib/live-client.ts`、`lib/live-routes.ts`（W3：事件 → 受影响路由重验）。
    还没有 `app/components/`。
  - `test/`：22 个测试文件 + `fixtures/`（`fake-core.mjs` 等替身）。本 worktree 尚未安装 `node_modules`。
- `apps/desktop` —— Electron 外壳（CommonJS，`npm test` = `node --test`），W11 前不动。
- `protocol/`（Loop 协议与 prompt，编进二进制）、`plugins/`、`scripts/`（`ci-local.sh` 等）。

**本机（生成时）：** Node v26.5.0；`target/debug/automed` 与 `target/debug/autome` 都存在；`~/.local/bin/autome` 存在，
`~/.local/bin/automed` 不存在；`apps/web/node_modules` 不存在。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/web`、
  `npm run build --prefix apps/web`（首次先 `cd apps/web && npm ci`）；`npm run probe:sqlite --prefix apps/web`
- Desktop：`npm test --prefix apps/desktop`
- 内核与命令行：`cargo build -p automed -p autome-cli`；`cargo fmt --all -- --check`、
  `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets`
- 全部：`sh scripts/ci-local.sh`（门禁用的就是它；rust、desktop、contract、web 四段；web 段只在 `apps/web/node_modules`
  不存在时才 `npm ci`）
- CI：`.github/workflows/ci.yml`（由 `apps/web/test/ci-config.test.ts` 逐字守卫；仓库无 remote，ci-local 是唯一真正跑的那份）。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，无具体规则）；仓库内无 `docs/agent-project-profile.md`；
- `docs/development/requirements.md`、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2 决策 D1–D6 与 UI 硬规则、§5 架构、§5.2 WebSocket 协议、
  §5.3 面板库、§5.4 通知、§5.5 本机安全、§6 W4（以及 W5、W6、W7、W10、W11 的边界，用于判断什么不归本任务）；
- 已实现的上游设计：W1 `docs/在-apps-web-建-react/在-apps-web-建-react.md`（§7 `node:sqlite` 结论）、
  W2 `docs/apps-web-的服务端托管-automed-2/apps-web-的服务端托管-automed-2.md`（网关、安全、tick）、
  W3 `docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md`（§3 总线游标与 W4/W8 挂点、§4 浏览器端）。

**前车之鉴（W1–W3 的复盘教训，供设计参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`，常伴随 watcher EMFILE），审计轮与宿主门禁可以。W1、W2、W3
  三个任务都撞上。规则匹配、标题渲染、迁移、游标推进、去重尽量写成纯函数或直接对临时文件库测，不需要监听端口；
  需要监听的用例（经 `/ws` 收到 `notification`、跨标签页已读同步）集中在少数文件。实现轮被环境阻断时，验收契约的每一段
  分别给出单独运行的退出码（不要用 `&&` 串成一条），写明被拦住的用例数和不依赖监听的那部分结果，交审计轮复验。
- 引用实测数据结构（RPC 返回值、事件 payload）时逐字粘贴原始 JSON，不得改写成简写（W3 L-03）。
- 验收契约里每条负向对照（「改坏 X → 点名的测试变红」），实现轮必须对点名的测试实跑该变异并在证据里写出退出码（W3 L-05）；
  实现轮不得改动「M-xx 验收契约」小节的文字，实际与预期不符时只在实现证据里写差值和来源。
- 负向对照全部还原之后，在仓库根执行 `git status --porcelain`，输出必须为空，并写进证据。
- 宿主门禁的同一失败签名连续出现、而本轮没有可改的代码时，在设计文档 `## 争议项` 写明失败签名和需要人在宿主上执行的
  具体命令及工作目录，不要逐轮只写进证据（W3 L-02）。
- 宿主门禁在基线上可能本来就不全绿。与本任务无关、按范围约束不修的失败，在设计文档 `## Backlog` 写一条：失败用例全名、
  断言原文、机制。不要在本任务里修 `crates/`、`apps/desktop/` 或 `scripts/`。
- 报验收结果时报 Web 与 desktop 的**用例总数**及与上一轮基线的差，`skipped` 必须为 0（本机有二进制时真实内核用例不允许跳过）。
- 计划 §7.4 止损：设计文档的里程碑不超过 6 个。
