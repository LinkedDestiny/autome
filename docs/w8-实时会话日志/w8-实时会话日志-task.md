# 任务：W8 实时会话日志——服务端 logtail 按订阅跟随 `.log` 并经 /ws 推增量，前端 session-log 组件可展开查看运行中与历史会话

本任务的规则见 `docs/w8-实时会话日志/protocol/loop-protocol.md` 与
`docs/w8-实时会话日志/protocol/session-protocol.md`。

- 设计文档：`docs/w8-实时会话日志/w8-实时会话日志.md`
- 任务卡（第一输入，用户所写）：`docs/w8-实时会话日志/w8-实时会话日志-card.md`
- 任务文件生成日期：2026-11-13；基线提交 `72ed493`（分支 `autome/w8-实时会话日志`，与 main 的分叉点 `ce6ec5f`；
  生成时 main 已前进到 `47a6580`，多出 T-49 通知器补发的合并）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支）
- 证据、简报、运行记录一律写在 `docs/w8-实时会话日志/` 下

## 1. 需求原文

> W8 实时会话日志：服务端按订阅跟随会话日志文件并经 WebSocket 推送增量，任务时间线中可展开查看正在运行或历史会话的日志

这句话是计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 **W8 实时会话日志** 的一句话需求。
任务卡正文把它展开成了具体行为、排除项与验收标准；**两者不一致时以任务卡正文为准**，任务卡与计划 §5.2
不一致时同样以任务卡为准（见 R-04 下的说明）。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-04 取自任务卡「要做什么」，A-01..A-05 取自任务卡「验收标准」，原意不增不减。编号供评审、审计追溯。

- **R-01 服务端 `apps/web/server/logtail*`。** 客户端发 `{ type: "log.subscribe", session_id }` /
  `log.unsubscribe`；服务端用内核 `session.log` 拿到路径——**只接受内核返回的路径，不接受客户端给的路径**——
  从文件末尾（或指定偏移）开始跟随，按增量推 `{ type: "log.chunk", session_id, offset, text }`；
  会话结束后推最后一段并 `log.end`，自动停止跟随。
- **R-02 限制。** 每个连接最多同时订阅 4 个日志；单条消息不超过 64KB（大段按 64KB 切）；
  文件被轮转或删除时推 `log.end { reason }`。
- **R-03 前端 `apps/web/app/components/session-log*`。** 任务时间线（W7）每一轮可展开查看该会话的日志：
  正在运行的实时滚动，历史会话一次加载；可以暂停自动滚动、跳到最新；大日志只保留最近 N KB 在 DOM 里。
- **R-04 协议。** 协议补进 `apps/web/server/eventbus-protocol.ts` 并有测试；计划 §5.2 的断线续传语义对日志订阅
  同样成立（重连带 offset）。
  - **命名以任务卡为准。** 计划 §5.2 写的是 `{ type: "log", session_id, offset, chunk }`、`subscribe-log`、
    `unsubscribe-log`；任务卡写的是 `log.subscribe` / `log.unsubscribe` / `log.chunk`（字段 `text`）/ `log.end`。
    本任务实现任务卡这一组，设计文档里写明与计划 §5.2 的差异，不实现两套名字。

验收标准（任务卡原文）：

- **A-01** 日志路径只能来自内核 `session.log`：客户端传路径的请求被拒绝，有测试。
- **A-02** 第 5 个订阅被拒绝并带中文原因；>64KB 的增量被切块且顺序正确，有测试。
- **A-03** 集成测试：用假 CLI 起一个会话，浏览器端在会话写日志后一个 tick 周期内收到增量；会话结束后收到
  `log.end`，服务端不再持有文件句柄。
- **A-04** 前端：暂停滚动时新内容不抢焦点，「跳到最新」恢复；历史会话展开能看到完整内容。
- **A-05** `npm --prefix apps/web test`、lint、typecheck、build 全绿；`sh scripts/ci-local.sh` 全绿。

**任务卡没有定死、留给设计轮回答并写进设计的问题**（只列问题，不预设答案）：

1. `offset` 与「64KB」的单位（字节还是字符）；切块落在多字节字符中间时怎么办。内核自己截尾时是先对齐字符边界
   再对齐行边界（`dispatch.rs` `session_log`）。
2. 「从文件末尾（或指定偏移）开始」：`log.subscribe` 是否带 `offset` 字段、缺省值是什么；重连时客户端带哪个 offset；
   offset 超过文件长度（文件被截短）时算不算轮转。
3. 「历史会话一次加载」走哪条路（同一条订阅从偏移 0 读到尾后 `log.end`，还是别的），以及它与
   「大日志只保留最近 N KB 在 DOM 里」「历史会话展开能看到完整内容」三句话怎么同时成立；N 取多少。
   注意内核 `session.log` 回包里的 `log` 字段只有末尾 256KB（`truncated` 标记），不是完整内容。
4. 会话结束怎么判定（内核 `session.log` 的 `session.running` / `lifecycle`，还是账本事件），「最后一段」怎么保证不丢。
5. `log.end` 的 `reason` 取值集合；拒绝订阅（路径、超限、找不到会话、内核离线）用什么消息回给客户端。
6. A-03 的「一个 tick 周期」按 `server/ticker.ts` 的 3 秒算；跟随用轮询还是文件监听、间隔多少，由设计实测后定。
7. A-03「服务端不再持有文件句柄」用什么可执行的办法断言。
8. 组件拿什么发消息：现有 `app/lib/live-client.ts` 只把收到的消息经 `subscribeLiveMessages` 分发出去，
   没有给组件暴露「往这条连接上发消息」的入口，重连后的重新订阅也要有落点。
9. 「最小的任务页占位」的路由路径与内容（见 §3）。

## 3. 范围

**可以改动的目录与文件：**

- `apps/web/server/logtail*`（新建）——跟随、切块、订阅上限、结束与轮转判定；
- `apps/web/server/eventbus-protocol.ts`——补日志订阅的消息类型与客户端消息校验；
  `apps/web/server/eventbus.ts`、`eventbus-ws.ts`——**只在**现有扩展点（`onClientMessage` / `onDisconnect` /
  `connection.send`）不够用时改，设计里写明缺什么；
- `apps/web/server/main.ts`——只为接线：创建 logtail、挂到事件总线、关闭时停掉并释放句柄；
- `apps/web/app/components/session-log*`（新建）——日志组件及其纯逻辑模块；
- `apps/web/app/lib/live*`——只为让组件能在同一条 `/ws` 连接上发订阅、收 `log.*`、重连后带 offset 重新订阅；
- 一个**最小的任务页占位**：`apps/web/app/routes.ts` 加一条路由 + 一个新路由文件，用来挂 `session-log` 组件。
  `apps/web/app/routes/tasks.$id*` 是 W7 的目录（W7 尚未开始，目前只有 `routes/_index.tsx` 一条路由）：
  占位要小到 W7 可以整份替换，不在里面做时间线、阶段条、操作按钮；
- `apps/web/app/styles/app.css`——只加日志组件与占位页用到的样式，用 `tokens.css` 已有的 token；
- `apps/web/test/`——新测试与 `test/fixtures/` 下的替身；已有测试只在接线引起的必要变化时改，逐条说明理由；
- `apps/web/package.json`、`package-lock.json`——预期**不需要**改（`ws ^8.22.2` 已在依赖里）。确需新增依赖时，
  设计里写明为什么不手写、`npm view <包> engines` 实测值、与 Node ≥ 22.22 的兼容性；
- `.github/workflows/ci.yml`（只 `web` job）、`scripts/ci-local.sh`、`apps/web/test/ci-config.test.ts`——
  **只在**新集成测试需要现有步骤给不了的东西时改，先实测并写明证据；不需要就不改。

**明确不做（出现即越界；前三组是任务卡「不做什么」原文）：**

- 不解析 `.jsonl`，只推 `.log` 文本；
- 不做搜索、下载；不做多任务合并视图；
- 不改 `crates/`；不改 `apps/desktop/`。W7 的时间线本身不在这里做，W8 只提供组件与服务端能力，
  先挂在一个最小的任务页占位上即可；
- 不改内核 `session.log` 的行为或回包（它在 `crates/` 里）；内核给不了的信息，在设计里写成 Backlog 或争议项，
  不绕过去改内核；
- 不改 W2 的安全检查语义：`/ws` 升级继续走 `server/security.ts` 的 `installUpgradeGuard`（Host + Origin），
  不另开第二条 WebSocket 路径，不加令牌、Cookie、登录会话（计划 §5.5）；
- 不新开 HTTP 接口让浏览器按路径读文件；不把 `events.since` 放进读白名单；
- 项目页与任务卡（W6）、任务详情的其余部分（W7）、项目管理与环境（W9）、启动与安装（W10）及之后的任务。

## 4. 硬性约束

1. **路径只来自内核。** logtail 打开的每一个文件路径都必须是本次（或本订阅此前）内核 `session.log` 回包里的路径；
   客户端消息里出现路径（任何字段名）即拒绝，不得拿来拼接、比较后放行或回退使用。`session_id` 也按不可信输入处理
   （现有 `server/gates/read-gate.ts` 对读请求的规则是「params 不能包含路径」：含 `/`、`\`、`..`、`~` 开头、NUL 即拒）。
   计划 §5.5 原文：「浏览器不能给出任意路径……日志路径只能来自内核。」
2. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`、`.autome/`。** 只能读或运行它们
   （构建并运行 `target/debug/automed`、`target/debug/autome`）。
3. **同一条连接。** 日志订阅走 W3 的 `/ws` 与 `eventbus-protocol.ts` 的信封，不影响已有的 `hello` / `event` /
   `resume` / `reload` / `core` / 通知消息的行为与测试。
4. **资源有界。** 每连接最多 4 个订阅；连接断开、`log.unsubscribe`、`log.end`、服务关闭（`startServer().close()`）
   之后不留文件句柄、定时器或监听器；单条下行消息不超过 64KB。注意 `eventbus-ws.ts` 里
   `WebSocketServer({ maxPayload: 64 * 1024 })` 限的是**上行**，下行的 64KB 要 logtail 自己保证。
5. **测试不碰真实 `~/.autome`，不启动真实 Agent。** 启动内核、服务器或命令行的测试用 `mkdtemp` 的临时
   `AUTOME_HOME` / `HOME`；会话用假 CLI（`AUTOMED_CLAUDE_BINARY` / `AUTOMED_CODEX_BINARY` 指向替身脚本，
   现成写法见 `apps/web/test/gateway-task-lifecycle.test.ts`、`notifier-live.test.ts`、
   `test/fixtures/{w4-ledger.sh,w5-states.sh,notifier-failure.ts}`，源头是 `crates/automed/tests/end_to_end.rs` 的 `FAKE_CLI`）。
   替身必须对齐**真实接口**（真实内核写出的 `.log`、真实的 `session.log` 回包形状），不是对齐调用方的想象：
   本项目四次「测试全绿、真机坏」都出在替身跟调用方对齐上。
6. **服务端 TypeScript 走 Node 原生类型剥离**（ESM、`.ts` 后缀导入、`erasableSyntaxOnly`，无 tsx），与 W1–W5 一致；
   测试用 vitest（`fileParallelism: false`，超时 60 秒）；`npm run lint`（`--max-warnings 0`）、`npm run typecheck`、
   `npm run build` 必须无错误。
7. **用户可见文字全中文**，专有名词、方法名、字段名保留原文（2026-09-15 定下的 UI 硬规则）。拒绝原因、
   `log.end` 的说明、按钮与空态文字、服务端日志都算。
8. **前端不维护业务状态副本。** 任务、会话等数据只来自 loader；日志文本是流式内容，由组件自己持有并按 R-03 限量。
9. **界面沿用现有外壳与 token**（`app/root.tsx`、`components/shell.tsx`、`styles/tokens.css`），卡片优先、低密度；
   不引入新的 UI 库。
10. **`.gitignore` 里有 `*.log`。** 测试夹具如果是 `.log` 文件，`git add` 不会带上它——夹具在测试里现写，
    或换不被忽略的名字，并在证据里用 `git status --porcelain` 核对。
11. **宿主门禁只在合并前跑一次**（`.autome/config.toml`：`[gate] command = "sh scripts/ci-local.sh"`、
    `run = "before_merge"`、超时 30 分钟）。实现轮、审计轮只跑里程碑验收与直接相关的测试；A-05 的
    `sh scripts/ci-local.sh` 全绿由合并前门禁与最后的审计给出，不要每轮都跑全量。
12. 不切分支、不推送、不操作 `.worktree/` 下的其它目录；超过 5 行的命令输出与临时验证产物写进
    `.autome/output/`（已 gitignore），不进任务目录。

## 5. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（任务整理 → 设计 → 评审/裁决 → 实现/审计 → 合并）驱动 Agent 会话开发软件的
本地工具，仅支持 macOS，用自己的 Loop 开发自己。计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 把
Electron 渲染进程换成浏览器面板：`apps/web` 的 Node 服务托管 Rust 内核 `automed`，对浏览器提供 HTTP + WebSocket。
W1（骨架）、W2（内核网关）、W3（事件总线）、W4（通知）、W5（外壳与导航）已合并；W6（项目页）、W7（任务详情）
尚未进 main。计划 §7.3 安排 W6 与 W8 并行——`app/routes.ts`、`server/main.ts`、`app/load-context.ts` 是两边都可能
碰的文件，接线改动越小，合并前 rebase 越省事。

**技术栈与布局：**

- Rust workspace（toolchain 1.93.1，edition 2024）：
  - `crates/autome-domain`——纯类型与状态转换。`session.rs`：`SessionPaths::dir(task)` =
    `.autome/output/sessions/<task-id>`，`SessionPaths::log` = `<dir>/<session-id>.log`；`Session::is_running()`
    即 `lifecycle == Running`；
  - `crates/automed`——SQLite 账本、Git、会话启动、调度、门禁、协议；framed JSON-RPC over stdio。
    - `src/dispatch.rs` `session_log`（约 2260 行）：参数 `session_id`；找不到回 `NotFound`「找不到会话 …」；
      回包 `{ session, log, truncated, path }`——`session` 是 `session_json`（含 `id`、`kind`、`label`、`runtime`、
      `round`、`started_at`、`ended_at`、`lifecycle`、`running`、`log_path` 等），`log` 是文件末尾至多 256KB
      （对齐字符与行边界），`path` 即 `session.log_path`。`session.log` 在 `READ_METHODS`（17 项）里，不占驱动锁；
    - `src/launcher.rs` `launch`：`log_path = spec.repo.join(SessionPaths::log(task, session))`，存成字符串；
      `spec.repo` 在 worktree 任务里指向哪个目录、路径是否恒为绝对路径，设计轮自己实测；
    - `.log` 是渲染过的可读文本，`.jsonl` 是同名的原始流（本任务不读）；
  - `crates/autome-cli`——`autome` 命令行。`src/main.rs` `logs`（约 989 行）是 `autome logs <任务> [--session id] [-f]`
    的跟随实现，可照搬读法：每 500ms 整读文件、输出 `shown` 之后的字节；会话不再 `is_running()` 且文件长度
    不超过已输出长度时结束——先判生命周期再读，保证结束前最后一次写入被带上（同文件 `logs_json` 的注释说明了顺序）。
- `apps/web`——React Router 8 框架模式 + express 5 自定义服务器；TypeScript ~6.0、Vite 8、vitest 5、ESLint 10、
  React 19、`ws ^8.22.2`；`engines.node >= 22.22.0`。现状：
  - `server/main.ts`：`startServer({port, mode, home, binaryPath})` 依次建 `openWebStore`、`createEventBus`、
    `CoreSupervisor`、`createNotifier`、`attachNotifier`、`createNotificationApi`、`createGateway`、`createHttpServer`，
    监听后 `registerUpgrade('/ws', createWsUpgrade(bus))`、`core.start()`、`createTicker`；`close()` 里按序停掉；
  - `server/eventbus.ts`（168 行）：`createEventBus({call, log})` 返回 `pull`、`setCoreStatus`、`connect(transport)`、
    `receive(transport, text)`、`broadcast`、`onClientMessage(type, handler)`、`onDisconnect(handler)`、`subscribe(listener)`、
    `close`。非 `resume` 的客户端消息按 `type` 交给 `onClientMessage` 注册的处理器，处理器拿到
    `(message, connection)`，`connection.send(message)` 只发给这一个连接——日志订阅从这里挂进来；
  - `server/eventbus-protocol.ts`（26 行）：`LedgerEvent`、`CoreMessage`、`ServerMessage`、`ExtensionMessage`
    （`{ type: string; [key]: unknown }`）、`encodeMessage`、`mapCoreStatus`、`parseClientMessage`（目前只校验 `resume`）；
  - `server/eventbus-ws.ts`（15 行）：`WebSocketServer({ noServer: true, maxPayload: 64 * 1024 })`，文本帧交给 `bus.receive`；
  - `server/core/supervisor.ts`：`CoreSupervisor.call(method, params)`（服务端直接调内核，不经读白名单）、`onStatus`；
    `server/ticker.ts`：每 3 秒一次 `scheduler.tick`；`server/gateway.ts` + `gates/{read-gate,write-gate}.ts`：
    浏览器经 `POST /api/read|write` 调内核的白名单（`session.log` 在读白名单里）；
  - `server/http.ts`：`/healthz`、`/api/*`、开发模式 Vite middleware + HMR、生产模式静态资源、
    `getLoadContext: createLoadContext({ gateway })`；`server/security.ts`：Host / Origin 检查与 `installUpgradeGuard`；
  - `app/lib/live-client.ts`：`createLiveClient`（重连退避 `reconnectDelay`、`hello` 后发 `resume`、`event` 批处理）、
    `subscribeLiveMessages` / `publishLiveMessage`（把每条下行消息按 `type` 分发给组件）；`app/lib/live.tsx`：
    `useLiveEvents()`、横幅、`WriteGateProvider`；`app/lib/live-routes.ts`：事件 → 路由重验；
  - `app/load-context.ts`：`servicesContext`（`{ gateway }`，键用 `Symbol.for` 以跨 Node / Vite SSR / 生产包共享）；
  - `app/routes.ts` 只有 `index("routes/_index.tsx")`；`app/components/` 有 bell、shell、project-list、overview-card、
    command-palette 等；`app/styles/{tokens.css,app.css}`；
  - `test/`：45 个 `*.test.ts` 加一个 `web-typecheck-regression.mjs`，`fixtures/` 有 `fake-core.mjs`、`automed-pidfile.sh`、`w4-ledger.sh`、
    `w5-states.sh`、`notifier-failure.ts` 等。跨进程集成测试的写法见 `eventbus-live.test.ts`（起 `server/start.js`、
    用 `ws` 客户端带 `origin` 连 `/ws`、`autome` 命令行在另一进程改账本）。
- `apps/desktop`——Electron 外壳（CommonJS，`node --test`），本任务不改。
- `protocol/`——Loop 协议源（编进二进制）；`plugins/`——1.x 插件；`scripts/`——`ci-local.sh`、打包与安装脚本。

**本机（生成时）：** Node v26.5.0。本 worktree **没有** `apps/web/node_modules`，也没有 `target/`——
跑 Web 测试前先 `cd apps/web && npm ci`；需要真实内核的测试默认找 `<worktree>/target/debug/{automed,autome}`
（或环境变量 `AUTOMED_BIN` / `AUTOME_BIN`），先在 worktree 里 `cargo build -p automed -p autome-cli`。
Web 用例总数基线生成时未实测，设计轮装好依赖后自己取，并写明是 `npm test` 运行时报告的数。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm --prefix apps/web test`、
  `npm run build --prefix apps/web`；单文件：`cd apps/web && npx vitest run test/<文件>`
- 内核与命令行（只构建、不修改）：`cargo build -p automed -p autome-cli`
- 全部：`sh scripts/ci-local.sh`（rust：fmt、clippy、test；desktop；contract；web：lint、typecheck、test、build）
- CI：`.github/workflows/ci.yml` 的 `web` job（Install、Lint、Typecheck、`cargo build -p automed -p autome-cli`、Test、Build），
  由 `apps/web/test/ci-config.test.ts` 守卫，须与 ci-local 保持一致。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 `README.md`，无具体规则）；仓库内无 `docs/agent-project-profile.md`；
- `docs/development/requirements.md`、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§5（架构，含 logtail 的位置）、§5.2（WebSocket 协议与
  断线续传）、§5.3（面板库与文件）、§5.5（本机安全）、§6 的 W8（以及 W6、W7 的边界）；
- W3 的设计文档 `docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md`（事件总线、续传、扩展点的已实现设计）；
  W5 的 `docs/w5-外壳与项目导航-迁移-animal/`（外壳、`getLoadContext`、样式 token）。

**前车之鉴（此前 Web 任务的复盘教训，供参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`，常伴 watcher EMFILE），审计轮可以。W1–W5 每个任务都撞上。
  切块、偏移推进、订阅计数、轮转判定、DOM 限量、滚动状态尽量写成纯函数或用假的传输与假的文件接口测；必须监听的用例
  （A-03 集成测试）集中在少数文件。跑 Web 全量之前先跑一个不加载项目源码的 listen 探针；探针报 EPERM 时不要反复跑全量，
  验收契约各段分别给退出码，写明被拦住的用例数，交审计轮复验。
- 设计里的用例数预期写验收命令**运行时**报告的数（vitest 参数化用例会展开）；数字依赖环境时在同一句写明各环境下的值。
- 实现轮不改任何「M-xx 验收契约」小节的文字；负向对照（临时注入）还原后 `git status --porcelain` 必须为空并写进证据。
- 门禁或全量里出现任务范围外（`crates/`、`apps/desktop/`）的失败：不在本任务修，在设计文档 `## Backlog` 写一条，
  含用例全名、文件行号、断言原文与机制。
- `## 里程碑` 一节只放那一张表；逐轮证据写进 `evidence/`，不进设计文档。
