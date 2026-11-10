# 任务：Web 事件总线（W3）——tick 后用 events.since 拉取全部来源的事件，经 /ws 推给浏览器，useLiveEvents 按事件重新验证路由

本任务的规则见 `docs/服务端每次-tick-后用-events/protocol/loop-protocol.md` 与
`docs/服务端每次-tick-后用-events/protocol/session-protocol.md`。

- 设计文档：`docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md`
- 任务文件生成日期：2026-11-10；基线提交 `30e5d51`（main 为 `274a64b`）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支 `autome/服务端每次-tick-后用-events`）
- 证据、简报、运行记录一律写在 `docs/服务端每次-tick-后用-events/` 下

## 1. 需求原文

> 服务端每次 tick 后用 events.since 拉取所有来源的新事件，经 WebSocket 推给浏览器，前端 useLiveEvents 按事件让对应页面重新加载（顺带把 apps/web/test/write-gate.test.ts 里的原始 NUL 字节改成 '\u0000' 转义，让 git 不再把它当二进制）

括号前的部分是计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 中的 **W3 事件总线与 WebSocket**，
一字不差。括号里是本任务额外附带的一处小修，与 W3 无关，单独成一条要求（R-06）。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-04 原样取自计划 W3 的「验收」；R-05 是计划 §5.1 / §5.2 对 W3 的约束；R-06 是需求原文括号里的修改。
编号供评审、审计追溯使用。

- **R-01 消息格式和计划 §5.2 一致，支持断线续传。** 本任务负责的消息（计划 §5.2 原文）：
  - 服务端 → 浏览器：
    ```jsonc
    { "type": "hello", "seq": 1234, "core": "connected" }
    { "type": "event", "seq": 1235, "kind": "task.updated", "subject": "T-3", "payload": {} }
    { "type": "core", "status": "offline" | "connected", "reason": "…" }
    ```
  - 浏览器 → 服务端：
    ```jsonc
    { "type": "resume", "after": 1234 }        // 断线续传；缺口超过 1000 条只回 { "type": "reload" }
    ```
  - 同一节里的 `notification`（W4）、`log` / `subscribe-log` / `unsubscribe-log`（W8）**不归本任务实现**，
    但消息信封与分发方式不得让它们以后接不进来。设计要说明这两类消息以后从哪里挂进来。
- **R-02 跨进程集成测试。** 命令行在另一个进程里改变任务状态，WebSocket 客户端在一个 tick 周期内
  （默认 3 秒）收到对应事件。「另一个进程」指 `autome` 命令行（`crates/autome-cli`，二进制名 `autome`），
  不是本服务托管的那个 `automed`。
- **R-03 内核离线和恢复都会推 `core` 消息。** 内核状态来自 W2 的 `CoreSupervisor`（`onStatus` 回调，
  状态有 `starting | connected | restarting | offline | stopped`），映射到 §5.2 的 `offline | connected`
  的规则由设计给出并测试。恢复后要补齐离线期间的事件（续读 `events.since`，不丢不重）。
- **R-04 只刷新受影响的路由，有测试。** 前端 `useLiveEvents()` 按事件的 `kind` 与 `subject` 只让受影响
  路由的 loader 重新验证（React Router 的 `useRevalidator`），不整页刷新，也不在客户端维护状态副本。
  断线时指数退避重连，页面顶部显示「连接中断，正在重连」（计划 §5.2 末段）。
- **R-05 事件只从 `events.since` 拉（计划 §5.1）。** sidecar 只会收到本进程发出的命令所产生的事件；
  命令行、Agent 在别的进程里改账本，这些事件不会出现在 sidecar 管道上。所以推给浏览器的事件流以
  「每次 tick 之后从游标开始 `events.since`」为唯一来源。sidecar 推过来的 Event 帧最多只用来让本进程
  发起的写操作更快刷新（例如触发一次提前拉取），不得成为第二条事件来源，也不得造成重复推送。
- **R-06 去掉 `apps/web/test/write-gate.test.ts` 里的原始 NUL 字节。** 生成时实测：该文件有**两个**原始
  NUL（0x00）字节，在第 117 行 `params: { task_id: 'T-1<NUL>rest' }` 与第 398 行
  `for (const value of [..., 'a<NUL>b'])` 的字符串字面量里，因此 `git diff --numstat` 对它显示 `-  -`
  （按二进制处理）。把它们改成源码里的 `\u0000` 转义，字符串运行时的值不变。验收至少包括：
  文件里 0x00 字节数为 0；`git diff --numstat <基线> -- apps/web/test/write-gate.test.ts` 显示行数而不是
  `-`；这两个用例仍然存在、仍然通过，`write-gate.test.ts` 的用例数不变。被测代码
  `apps/web/server/gates/write-gate.ts` 已经用 `'\u0000'` 转义，不改。

## 3. 范围

**可以改动的目录与文件：**

- `apps/web/server/eventbus*`、`apps/web/app/lib/live*`（计划 W3「目录」）——新代码放这里；
- `apps/web/server/main.ts`——只为接线：把事件总线挂到 ticker 的 `onTick`、把 `CoreSupervisor` 的
  `onStatus` 接到总线、用 W2 的 `registerUpgrade('/ws', …)` 注册 WebSocket 升级、关闭时停掉总线；
- `apps/web/app/root.tsx`——只为挂 `useLiveEvents()` 与「连接中断，正在重连」提示；
- `apps/web/test/`——新测试与 `test/fixtures/` 下的替身；`write-gate.test.ts` 只做 R-06 的转义改动；
  其它已有测试只在接线引起的必要变化时改，并逐条说明理由；
- `apps/web/package.json`、`apps/web/package-lock.json`——**只在**设计论证需要一个 WebSocket 服务端依赖
  时改（Node 有全局 `WebSocket` 客户端，但没有内置服务端；`apps/web` 目前没有 `ws` 一类依赖）。
  新增依赖须在设计里写明：为什么不手写、`npm view <包> engines` 与版本实测值、与 Node ≥ 22.22 的兼容性；
- `.github/workflows/ci.yml`（只改 `web` job）与 `scripts/ci-local.sh`、`apps/web/test/ci-config.test.ts`——
  **只在** R-02 的集成测试需要 `autome` 二进制、而现有 Web 测试步骤之前拿不到它时改（例如 `web` job
  目前只有 `cargo build -p automed`）。设计要先实测，写明证据与理由；不需要就不改。

**明确不做（出现即越界）：**

- 通知、`web.sqlite3`、`event_cursor` 表、通知铃铛（W4）。本任务的游标放内存，服务重启后从哪里开始
  由设计决定并写明理由；
- 页面与组件：项目列表、首页总览、项目页、任务详情（W5–W7）。本任务不新建业务路由，只提供
  「事件 → 受影响路由」的映射机制，以及在现有路由上可测的重新验证；映射如何供 W5 起的路由扩展，由设计说明；
- `getLoadContext`（W5，W2 的 Backlog B-03）；
- 会话日志跟随与 `log` / `subscribe-log` 消息（W8）；
- `server.json`、`autome web` 子命令（W10）；Electron 改造（W11）；Playwright 端到端（W12）；
- 改 W2 的安全检查语义：`/ws` 升级必须经过 W2 已有的 `installUpgradeGuard`（Host + Origin），不得另开旁路；
- 任何令牌、Cookie、登录会话（计划 §5.5：鉴权只做 Host 与 Origin）；
- 除 R-06 那两处以外，`write-gate.test.ts` 的任何改动；仓库里其它文件的 NUL 字节。

## 4. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`。** 计划 D1：内核只在 K1、K2 里改，Web 任务不碰
   `crates/`。只能**读**或**运行**它们（构建并运行 `target/debug/automed`、`target/debug/autome`）。
   每个里程碑的 `npm test --prefix apps/desktop` 结果不得退化。
2. **事件来源唯一（R-05）**：只从 `events.since` 拉。`events.since` 的实测行为（`crates/automed/src/dispatch.rs`
   `events_since`）：参数 `after_seq`（缺省 0）、`limit`（缺省 **200**，上限 **1000**）；返回
   `{ events: [{seq, kind, subject, payload}], latest_seq }`，按 `seq` 升序。一次拉不完（`events` 的最后一个
   `seq` 仍小于 `latest_seq`）时要继续拉，不能只拉一页。`events.since` 在 `READ_METHODS` 里，不占驱动锁。
3. **不叠加、不阻塞 tick**：W2 的 ticker 保证 tick 不叠加；拉取事件不得让下一次 tick 叠加，也不得让一次慢的
   拉取拖住推送顺序（同一连接上的 `event` 消息按 `seq` 严格递增、不重复）。
4. **`/ws` 走 W2 的升级守卫**（`server/security.ts` 的 `installUpgradeGuard`，`registerUpgrade` 已由
   `createHttpServer` 返回）：Host 必须是 `127.0.0.1:<port>` 或 `localhost:<port>`，Origin 必须是本服务自己的地址。
   开发模式下 Vite HMR 也挂在同一个 `server` 的 upgrade 上，两者都要能用，有测试。
5. **测试不碰真实 `~/.autome`，也不启动真实的 Agent。** 任何启动内核、服务器或命令行的测试都用 `mkdtemp`
   出来的临时 `AUTOME_HOME`（或显式 `home`），不改 `process.env`。R-02 的集成测试里，tick 不得拉起真实的
   `claude` / `codex` 会话——如何构造一个能被命令行改变状态、又不会启动真实会话的任务，由设计实测后给出。
6. **服务端 TypeScript 走 Node 原生类型剥离**（ESM，`.ts` 后缀导入，无 tsx），与 W1、W2 一致；测试用 vitest；
   `npm run lint`（`--max-warnings 0`）与 `npm run typecheck` 必须无错误；`npm run build` 必须通过。
7. **用户可见文字全中文**，专有名词、方法名、字段名保留原文（2026-09-15 定下的 UI 硬规则）。
   重连提示、离线原因、服务端日志都算用户可见文字。
8. **前端不维护状态副本**：数据只来自 loader，事件只用来决定让哪些路由重新验证（计划 §5.2）。
9. **宿主门禁会跑 ci-local。** `.autome/config.toml` 配置了 `[gate] command = "sh scripts/ci-local.sh"`
   （超时 30 分钟），每个实现轮之后、合并前、合并后在任务 worktree 里跑。新 worktree 首次运行要装
   `apps/web/node_modules`；Web 步骤不得依赖交互或浏览器；整套 ci-local 仍需在超时内跑完。
10. 不修改 `.autome/` 下任何内容；不切分支、不推送、不操作 `.worktree/` 下的其它目录；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`（已 gitignore），不进任务目录。

## 5. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（任务整理 → 设计 → 评审/裁决 → 实现/审计 → 合并）驱动 Agent 会话
开发软件的本地工具，仅支持 macOS。本仓库用自己的 Loop 开发自己；计划
`docs/plans/2026-11-10-web-dashboard-via-loop.md` 要把 Electron 渲染进程换成浏览器面板：`apps/web` 里的
Node 服务托管 Rust 内核 `automed`，对浏览器提供 HTTP + WebSocket。W1（骨架）、W2（内核网关）已合并到 main；
本任务是 W3。

**技术栈与布局：**

- Rust workspace（toolchain 1.93.1，edition 2024），成员：
  - `crates/autome-domain` —— 纯类型与状态转换；
  - `crates/automed` —— SQLite 账本（`events` 表 append-only，带 `seq`、`kind`、`subject_id`、`payload`）、
    Git、会话启动、调度、宿主门禁、协议；framed JSON-RPC over stdio（4 字节大端长度前缀）。
    `src/dispatch.rs`：`READ_METHODS`（17 项，含 `events.since`）、`events_since`；非读方法在分派前要拿驱动锁，
    `scheduler.tick` 拿不到锁时回 `skipped: true`。账本里出现过的事件 kind 举例：`task.created`、`task.updated`、
    `session.started`、`gate.result`、`merge.rebased`、`merge.auto`、`project.added`、`project.updated`、
    `project.removed`、`config.changed`、`milestone.contradicted`——完整清单设计轮自己从源码核对；
  - `crates/autome-cli` —— `autome` 命令行（`target/debug/autome` 已存在）。状态在 `$AUTOME_HOME`（默认
    `~/.autome`），与面板共用；有 `approve / reject / pause / resume / stop / cancel / rerun / extend`、
    `project add <路径> --skip-onboarding`、`task new`、`rpc <方法> '<JSON>'` 等子命令。
- `apps/web` —— React Router 8 框架模式 + express 5 自定义服务器，TypeScript ~6.0，Vite 8，vitest 5，ESLint 10，
  `engines.node >= 22.22.0`。依赖里**没有**任何 WebSocket 服务端库。现状：
  - `server/main.ts`：`startServer({port, mode, home, binaryPath})` 创建 `CoreSupervisor`、`createGateway`、
    `createHttpServer`，监听后 `core.start()`，再 `createTicker({call, isConnected, isBusy, log})`（**没有**传
    `onTick`，也没有给 `CoreSupervisor` 传 `onStatus`）；返回 `{server, gateway, registerUpgrade, close}`；
  - `server/ticker.ts`：每 3 秒一次，`pending`/`isBusy` 防叠加，`onTick?(report)` 在每次成功 tick 之后调用；
  - `server/core/supervisor.ts`：`CoreSupervisor`（`call(method, params)` 15 秒超时、`tick()`、`restart()`、
    `onStatus(status)`）；`server/core/sidecar.ts`：`onEvent` 收 sidecar 的 Event 帧；
  - `server/http.ts`：`/healthz`、`POST /api/read`、`POST /api/write`、开发模式 Vite middleware + HMR
    （`hmr: { server }`）、生产模式静态资源；最后 `installUpgradeGuard(server, port)` 返回 `registerUpgrade(path, handler)`；
  - `server/security.ts`：`acceptsHost` / `acceptsOrigin` / `securityMiddleware` / `installUpgradeGuard`；
  - `server/gateway.ts`、`server/gates/{read-gate,write-gate}.ts`：读写白名单；
  - `app/`：`root.tsx`（Layout + Outlet）、`routes.ts`（只有 `index("routes/home.tsx")`）、`routes/home.tsx`（占位）；
  - `test/`：16 个测试文件（ci-config、config、core-binary、core-framing、core-restart、core-sidecar、
    core-supervisor、gateway-http、gateway、node-version、read-gate、security、server、sqlite-probe、ticker、
    write-gate），`fixtures/` 下有 `fake-core.mjs`、`automed-fail-fast.sh` 等替身。本 worktree 尚未安装 `node_modules`。
- `apps/desktop` —— Electron 外壳（CommonJS，`npm test` = `node --test`）。参考：它的渲染进程收到 sidecar 事件就
  整体刷新（`renderer/app.js` 的 `onEvent(() => queueRefresh())`），**不**拉 `events.since`，所以看不到其它进程
  的变化——这正是计划 §5.1 要 W3 解决的问题。
- `protocol/` —— Loop 协议与 prompt 模板（编进二进制）；`plugins/` —— 1.x 插件。
- `scripts/` —— `ci-local.sh`（本地跑 CI，门禁用它：rust、desktop、contract、web 四段；web 段是
  lint、typecheck、test、build，test 之前没有单独的 cargo build 步骤；前面 rust 段的 `cargo test --workspace`
  是否顺带留下可用的 `target/debug/autome`，设计轮自己实测）、`package.sh`、`package-cli.sh`、`install-cli.sh`。

**本机（生成时）：** Node v26.5.0；`target/debug/automed` 与 `target/debug/autome` 都存在；`~/.local/bin/autome`
存在，`~/.local/bin/automed` 不存在。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/web`、
  `npm run build --prefix apps/web`（首次先 `cd apps/web && npm ci`）
- Desktop：`npm test --prefix apps/desktop`
- 内核与命令行：`cargo build -p automed -p autome-cli`；`cargo fmt --all -- --check`、
  `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets`
- 全部：`sh scripts/ci-local.sh`（门禁用的就是它）
- CI：`.github/workflows/ci.yml`，`web` job 步骤为 Install、Lint、Typecheck、`cargo build -p automed`、Test、Build，
  并由 `apps/web/test/ci-config.test.ts` 逐字守卫；仓库无 remote，ci.yml 不会被执行，ci-local 是唯一真正跑起来的
  那份，两者要保持一致。
- R-06 的检查：`git diff --numstat 30e5d51 -- apps/web/test/write-gate.test.ts`；数 0x00 字节，例如
  `python3 -I -c "print(open('apps/web/test/write-gate.test.ts','rb').read().count(b'\x00'))"`。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，无具体规则）；仓库内无 `docs/agent-project-profile.md`；
- `docs/development/requirements.md`、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2 决策 D1–D6、§5 架构、§5.1 事件来源、§5.2 WebSocket
  协议、§5.5 本机安全、§6 W3（以及 W4、W5、W8、W10、W12 的边界，用于判断什么不归本任务）；
- W2 的设计文档 `docs/apps-web-的服务端托管-automed-2/apps-web-的服务端托管-automed-2.md`（网关、tick、升级守卫的
  已实现设计；其 Backlog B-03 归 W5，不归本任务）。

**前车之鉴（W1、W2 的复盘教训，供设计参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`，常伴随 watcher EMFILE），审计轮可以。W1 六个实现轮、
  W2 五个实现轮都撞上。消息编解码、续传判定、事件→路由映射、退避计算尽量写成纯函数或用假的 socket 测；
  需要监听的用例（R-02 集成测试、`/ws` 升级）集中在少数文件。实现轮被环境阻断时，验收契约的每一段分别给出
  单独运行的退出码（不要用 `&&` 串成一条），并写明被拦住的用例数和不依赖监听的那部分结果，交审计轮复验。
- 实现轮不得改动任何「M-xx 验收契约」小节的文字；实际用例数与预期不符时，只在实现证据里写差值和每个用例的来源。
- 负向对照（临时注入）全部还原之后，在仓库根执行 `git status --porcelain`，输出必须为空，并写进证据。
- 宿主门禁在基线上可能本来就不全绿。与本任务改动无关、按范围约束不修的门禁失败，在设计文档 `## Backlog` 写一条：
  失败用例全名、断言原文、机制。不要在本任务里修 `crates/` 或 `apps/desktop/`。
- 报验收结果时报 Web 与 desktop 的**用例总数**及与上一轮基线的差，`skipped` 必须为 0（本机有二进制时真实内核
  用例不允许跳过）。
- 新增依赖时附 `npm view <包> engines` 的实测值；与 Node 下限冲突时作为争议项交用户选择，不由裁决轮单方决定。
