# 面板外壳与项目导航（W5）：animal-island token、项目栏计数、首页总览、⌘K、中文标签表与 getLoadContext

status: 实现中
design-round: 1/15
implementation-round: 4/25
current-milestone: M-04
current-milestone-reopens: 0
convergence-mode: normal
next-action: 实现 M-04 项目栏计数与首页总览，事件驱动实时更新

任务文件：`docs/w5-外壳与项目导航-迁移-animal/w5-外壳与项目导航-迁移-animal-task.md`。
本任务的规则见 `docs/w5-外壳与项目导航-迁移-animal/protocol/loop-protocol.md` 与
`docs/w5-外壳与项目导航-迁移-animal/protocol/session-protocol.md`。

## 背景与现状

本节是设计轮 #1 在基线 `a3455a3` 上实测的结果。命令输出原文在 `.autome/output/w5-plan-*.txt`。

### apps/web 现状（与任务文件 §5 一致的部分不再重复）

- `app/root.tsx` 共 10 行：`App` 调用 `useLiveEvents()`，渲染 `LiveBanner`、`NotificationBell` 和 `Outlet`。它没有 loader，没有 `links`，也没有 `ErrorBoundary`。
- `app/routes.ts` 只有 `index("routes/home.tsx")`。`routes/home.tsx` 是一行占位「面板骨架已就绪」。
- `app/lib/live.tsx` 里的 `LiveBanner` 用内联样式，并带 `top: 0`。`test/live-banner.test.ts` 断言了 `top:0`。
- `app/components/bell.tsx` 全部用内联样式：外层是 `position: 'fixed'`，颜色有 `#fff`、`#d1d5db`、`#1f2937`、`#b91c1c`、`#2563eb` 等。`test/bell-view.test.ts` 断言了 `position:fixed`、`max-height:60vh`、`overflow-y:auto`、`font-weight:600`。另外它用 `createRoutesStub` 渲染没有 loader 的 `App`（「root 同时渲染铃铛与子路由」）。
- `test/live-routes.test.ts` 的守卫扫描 `app/root.tsx` 和 `app/routes/**`：凡是导出 `loader` 的模块，都必须导出 `shouldRevalidate = liveShouldRevalidate(...)`。根 loader 加上以后，这条守卫同样管到 `root.tsx`。
- `test/server.test.ts` 在 dev 和 prod 两种模式下都断言首页 HTML 含「面板骨架已就绪」。prod 用例自己会先跑 `npm run build`，并断言 `/not-found` 返回 404。
- `test/security.test.ts:133` 调用 `createHttpServer({ mode: 'development' })` 时没有传 gateway，也不监听端口，只检查 upgrade 守卫。
- vitest 环境里没有 jsdom，`devDependencies` 也没有 testing-library。组件测试只能用 `react-dom/server` 的 `renderToString` 加 `createRoutesStub`/`MemoryRouter`，交互逻辑要写成纯函数来测（范围不允许新增依赖）。
- **基线用例数**：`npx vitest list` 列出 33 个文件、**287** 个 Web 用例（`.autome/output/w5-plan-vitest-list.txt`）。desktop 是 `node --test`，共 **104** 个用例。本 worktree 没有 `target/debug/automed`，所以本轮跑出 9 个失败、1 个 skipped，都是 `ENOENT` 或超时。宿主上先 `cargo build` 再跑，才是 desktop 的真实基线，W5 不改 desktop。

### React Router 8.5.1 的 `getLoadContext`（R-06 要求逐字引用）

`apps/web/package-lock.json` 锁定 `react-router` 与 `@react-router/express` 均为 `8.5.1`。下面的原文摘自主检出的 `node_modules`，版本相同。

`@react-router/express/dist/index.d.ts`：

```ts
type GetLoadContextFunction = (req: express.Request, res: express.Response) => MaybePromise<RouterContextProvider>;
...
declare function createRequestHandler({
  build,
  getLoadContext,
  mode
}: {
  build: ServerBuild | (() => Promise<ServerBuild>);
  getLoadContext?: GetLoadContextFunction;
  mode?: string;
}): RequestHandler;
```

`@react-router/express/dist/index.js:21`：
`await sendRemixResponse(res, await handleRequest(createRemixRequest(req, res), await getLoadContext?.(req, res)));`

`react-router/dist/development/lib/server-runtime/server.js:51-56`：

```js
if (initialContext && !(initialContext instanceof RouterContextProvider)) {
    let error = /* @__PURE__ */ new Error("Invalid `context` value provided to `handleRequest`. You must return an instance of `RouterContextProvider` from your `getLoadContext` function.");
...
loadContext = initialContext || new RouterContextProvider();
```

`react-router/dist/development/index-react-server.d.ts`：

```ts
declare function createContext<T>(defaultValue?: T): RouterContext<T>;
...
declare class RouterContextProvider {
  constructor(init?: Map<RouterContext, unknown>);
  get<T>(context: RouterContext<T>): T;
  set<C extends RouterContext>(context: C, value: C extends RouterContext<infer T> ? T : never): void;
}
type DefaultContext = Readonly<RouterContextProvider>;
```

`RouterContextProvider.get` 的实现（`index-react-server.js:187-191`）以 `RouterContext` 对象本身为键：先查 `this.#map.has(context)`，没有就用 `defaultValue`，两者都没有就 `throw new Error("No value found for context")`。

在整个 `react-router/dist` 里 grep `AppLoadContext`，结果为空。

**结论：**

1. RR 8.5 没有 `AppLoadContext` 可以做模块扩充，`getLoadContext` 必须返回 `RouterContextProvider` 实例，否则请求直接报错。R-06 只能走 `createContext` + `RouterContextProvider` 这条路。
2. 不传 `getLoadContext` 时，loader 拿到的是一个空的 `RouterContextProvider`。
3. **关键陷阱：上下文键靠对象身份匹配。** `server/http.ts` 由 Node 原生加载，而页面模块在开发模式下由 `vite.ssrLoadModule` 加载，在生产模式下被打进 `build/server/index.js`。如果两边各自 `import` 一个声明了 `createContext()` 的模块，会得到**两个不同的键**：服务端 `set` 进去的值，loader `get` 不到。方案必须让两份模块实例共用一个键（见「方案 · R-06」），而且 M-01 要分别在开发和生产两种 SSR 下实测这一点。

### 内核数据与计数口径（只读核对）

- `crates/autome-domain/src/task.rs`：`enum Node` 有 22 个变体。`Node::BUG: [Node; 7]` 和 `Node::ALL: [Node; 15]` 与任务文件一致。`Node::awaits_user` 的 `matches!` 只含 `AwaitDesignApproval | AwaitMerge | AwaitInfo | NotABug`。`TaskState` 经 `TaskStateRepr`（`#[serde(tag = "state", rename_all = "snake_case")]`）序列化，共 8 个外部取值：`draft`、`queued`（`Requeued` 也序列化成 `queued`，外加 `resume`）、`active{node}`、`paused{resume}`、`stopped{at}`、`failed{at, reason}`、`done`、`cancelled`。
- `crates/autome-domain/src/bug_loop.rs`：Bug 任务复核通过以后进入**共享的合并节点** `Node::Rebase` → `Node::MergeGate` → …，与需求 Loop 是同一组节点。所以 Bug 阶段表的「合并」段必须收进这些节点，不能只覆盖 `Node::BUG`。
- `crates/automed/src/dispatch.rs`：
  - `dashboard_get` 跳过终态任务。`task.state.awaits_user()`（等人节点加 `Failed`）放进 `waiting`，`task.state.occupies_slot()`（`Active` 且不在等人节点）放进 `running`。**`stopped`、`paused`、`draft`、`queued` 两组都不进。**
  - `project_list` 每个项目返回 `{project, counts: task_counts(&tasks), slots_in_use}`。
  - `project_get` 返回 `{project, tasks: [task_json], counts, config, violations, rules}`，同时会读全局和项目配置、扫描 skills、列规则文件，每次调用都要读文件系统。
- `crates/automed/src/store.rs:2047` `task_counts` 的键：`draft`、`queued`（含 `Requeued`）、`awaiting_user`（`Active` 且在等人节点）、`running`（其余 `Active`）、`paused`、`stopped`、`failed`、`done`、`cancelled`。
- 任务状态变化会产生 `task.updated` 事件（`dispatch.rs`、`scheduler.rs` 多处）。账本里还有 `task.created`、`task.started`、`project.added`、`project.removed`、`bug.*`、`merge.*` 等事件。
- `apps/web/server/core/supervisor.ts`：`call` 在内核不是 `starting`/`connected` 时立即抛出 `内核未连接：…`，在线时最多等 `CALL_TIMEOUT_MS = 15000`。gateway 把它映射成 503 或 504。
- `Trigger::Stop`（`task.rs:860`）只接受 `Active`（非等人节点）或 `Paused`。真实内核测试要把任务推进到 `active`，才能用 `autome stop` 让它进入「已停止」。

### 设计来源

- `apps/desktop/renderer/style.css` 第 34–48 行是 Layer 1 的 `:root`，含全部 `--animal-*` 与 `--tile-*` token，第 63 行起是 `[data-theme="dark"]` 暗色调色板。字体在 `renderer/fonts/nunito/Nunito-Variable.ttf`（276932 字节），旁边是 `OFL.txt`（4385 字节）。desktop 的 `@font-face` 写成 `format('truetype-variations')`，字重范围 `200 1000`。
- `apps/desktop/renderer/lib/labels.js`：`NODE_LABELS` 只有 15 个需求节点。`GATE_STONE` 把 `gate` 归到 `implement`、`merge_gate` 归到 `rebase`、`main_check` 归到 `merging`。约定是遇到未知值回退为原始值。
- 计划 W6 的目录是 `routes/projects.$id*`（项目页，「新建任务卡」也在项目页上），W7 是 `routes/tasks.$id*`。

## 目标与非目标

### 目标（对应 R-01..R-08）

1. 交付一个按 animal-island token 着色的外壳：左边是项目栏，顶部是顶栏（⌘K 入口、「新建任务卡」入口、W4 铃铛），下面是横幅行，其余是内容区。外壳在 1512×944 下页面本身不出现纵向滚动（R-07、R-08）。
2. 项目栏和首页总览的数据都来自根 loader 这一个来源，归列只用一个函数，事件到达后由 `liveShouldRevalidate` 触发重验，实现实时更新（R-01、R-02）。
3. 交付 `app/lib/labels.ts`：需求、Bug 两张阶段表，加上本任务用到的节点名、状态名、类型名、列名。另有测试解析 `task.rs` 做交叉核对（R-04）。
4. 交付键盘可达的 ⌘K/Ctrl+K 跳转面板（R-03）。
5. 内核离线时显示横幅，提供可复用的写判定，本任务里的写入口据此置灰（R-05）。
6. 开发、生产两处 `createRequestHandler` 都加上 `getLoadContext`，loader 和 action 以类型安全的方式拿到只暴露 `read`/`write` 的网关（R-06）。

### 非目标

- 任务文件 §3「明确不做」里列的全部内容，包括四列看板、完整任务卡、新建任务卡表单、任务详情页、写操作、通知角标、主题切换。
- **不迁移暗色调色板，也不跟随 `prefers-color-scheme`。** 理由：本任务没有主题切换界面；如果跟随系统主题，W5–W9 的所有页面都会多出一套没人做过视觉验收的暗色外观。暗色 token 等主题切换一起做（Backlog B-02）。
- **本任务不把面板库 `store` 交给 `getLoadContext`。** 理由：角标在 W6。现在交出去，要么改 `createHttpServer` 的签名，要么在 `main.ts` 接线，交出的对象却没有消费者，也没有测试。方案在 `WebServices` 类型里给 `store`/`notifications` 留了注明的位置，W6 补一个字段、加一行 `set` 就能接上。
- 不改内核口径，不新增 npm 依赖，不碰 `crates/`、`apps/desktop/`。

## 方案

### 总体数据流

```text
http.ts ──getLoadContext──▶ RouterContextProvider{ servicesContext → { gateway: {read, write} | null } }
                                   │
root.tsx loader ── loadShell(gateway) ──▶ gateway.read project.list ×1 + project.get ×N（并行）
                                   │            └── 每个任务过一次 columnOf(state) / stageOf(kind, state)
                                   ▼
              ShellData { kernel, projects[{id,name,counts|null,error?}], tasks[精简字段] }
                 ├── 项目栏（计数）            ── root.tsx 组件
                 ├── 首页总览（等你处理 / 进行中卡片）── routes/_index.tsx 经 useRouteLoaderData('root')
                 └── ⌘K 候选                      ── command-palette.tsx 经 useRouteLoaderData('root')
useLiveEvents ─ 事件批次 ─▶ root.shouldRevalidate = liveShouldRevalidate(byKind('task.','project.','bug.','merge.'))
             └ connection/core ─▶ liveBanner(...) 横幅 + writeGate(...) 写判定（React context）
```

前端不保存状态副本：三处显示都从根 loader 的结果派生（硬性约束 3）。首页**不设自己的 loader**，live-routes 守卫因此不需要覆盖它。根 loader 的重验声明有自己的测试（M-04）。

### R-06 getLoadContext

- 新文件 `app/load-context.ts`。它属于范围允许的「类型扩充文件」，只放 R-06 的上下文类型和键：
  - `type KernelGateway = Pick<ReturnType<typeof createGateway>, 'read' | 'write'>`（`import type`，会被擦除）。loader 拿到的类型里**没有 `core`**，`gateway.core.call` 过不了 typecheck，从类型上堵住绕过白名单的路（硬性约束 5）。
  - `type WebServices = { gateway: KernelGateway | null }`，注释里写明 W6 在这里加 `store`/`notifications`。
  - `servicesContext`：用 `Symbol.for('autome.web.services')` 在 `globalThis` 上登记，第一次加载时调用 `createContext<WebServices>()` 并保存，之后再加载的模块实例都取同一个对象。这样 Node 原生加载的 `http.ts`、Vite SSR 模块图和 `build/server/index.js` 三处共用一个键，解决「背景」里结论 3 的问题。**不设默认值**：一旦接线漏了，`get` 会抛出 `No value found for context`，不会悄悄变成「离线」。
  - `createLoadContext(services): GetLoadContextFunction`：每次请求新建一个 `RouterContextProvider`，并 `set(servicesContext, services)`。
- `server/http.ts` 只做两处改动：开发和生产的 `createRequestHandler({ build, mode, getLoadContext: createLoadContext({ gateway: gateway ?? null }) })`。已有路由、安全中间件和 `/api/*` 都不动。
- 没有拿到 gateway 时（`security.test.ts` 的用法），`gateway` 为 `null`。根 loader 返回 `kernel: { ok: false, error: '内核未连接：服务未接入网关' }`，页面按离线显示，不抛错。这个用例只构造服务器、不发请求，所以不受影响。
- 读取一律走 `gateway.read({ method, params })`，读白名单照常生效。根 loader 只用 `project.list` 和 `project.get`，两者都在 `READ_METHODS` 里。

### R-01 计数口径与唯一的归列函数

新文件 `app/components/task-columns.ts` 导出 `columnOf(state) → 'todo' | 'running' | 'needs_you' | 'done' | null`，以及 `countByColumn(tasks)`。列名的中文在 `labels.ts` 的 `COLUMN_LABELS` 里：待开始、进行中、等你处理、已完成。

| 内核 `state`（task_json） | 列 | 依据 |
|---|---|---|
| `draft` | 待开始 | §3.2 原文 |
| `queued`（含带 `resume` 的重新排队） | 待开始 | §3.2「排队中的任务」 |
| `active`，节点在 `AWAITING_NODES` 内 | 等你处理 | §3.2「等你批准设计、等你合并、Bug 无法复现等你补充」；`not_a_bug` 同样是 `Node::awaits_user` |
| `active`，其他节点（含 `gate`、`merge_gate`、`main_check`，以及重试等待、限流冷却） | 进行中 | §3.2「正在跑会话或内核步骤」 |
| `failed` | 等你处理 | §3.2「失败」 |
| `stopped` | 等你处理 | §3.2「已停止」 |
| `paused` | 等你处理 | 本设计的决定，理由见下 |
| `done` | 已完成 | 「近 14 天」是 W6 看板的展示规则，W5 不显示已完成计数 |
| `cancelled`、未知取值 | 不入列 | 已取消不需要任何人处理；未知值不猜 |

`AWAITING_NODES` 写在 `task-columns.ts` 里，测试会解析 `task.rs` 中 `Node::awaits_user` 的 `matches!` 得到同一集合，两者必须相等。

**`paused` 归入「等你处理」的理由。** 暂停在当前会话结束后生效（`TaskState::Paused` 的注释说明了这一点）。之后任务不会自己继续，只有人点「恢复」或「停止」才会往下走。这和 §3.2 明确列入的「已停止」性质相同：两者都停在一个节点上，都在等一次人的操作（`Trigger::Stop` 本来也接受从 `Paused` 进入）。把它放进「进行中」，会让一个没在跑的任务显示为在跑；放进「待开始」，则与「还没开始跑」的定义冲突。备选方案是「哪一列都不算」，那样暂停的任务会从总览里消失，用户可能忘了它，所以不采用。

**为什么不直接用内核现成的口径。** `dashboard.get` 不含 `stopped`，`project.list` 的 `counts` 只有计数、没有任务，而首页卡片和 ⌘K 都需要任务本身。如果计数用 `counts` 组合、卡片用 `dashboard.get`，就成了两套口径：项目栏显示「等你处理 1」，首页却一张卡片都没有（已停止的任务）。所以三处都由同一份任务列表经 `columnOf` 计算。与内核口径的对应关系（`等你处理 = awaiting_user + failed + stopped + paused`，`进行中 = running`）作为**测试里的核对式**存在：M-03 用逐字采集的真实 `project.get` 输出核对，M-04 用真实内核在线核对。生产代码里不另写第二个函数。

### 根 loader 的数据：`loadShell`

新文件 `app/components/shell-data.ts` 导出 `loadShell(gateway: KernelGateway | null): Promise<ShellData>`。它不依赖路由，可以直接用假 gateway 测。

1. 先 `read({method:'project.list', params:{}})`。失败或 gateway 为 `null` 时返回 `{ kernel: {ok:false, error}, projects: [], tasks: [] }`，`error` 原样取自 gateway 的 `body.error`，例如「内核未连接：…」或「内核响应超时：…」。
2. 对每个项目**并行**发 `read({method:'project.get', params:{project_id}})`。某个项目失败时，只把它标成 `counts: null, error`，项目栏显示「—」，悬停可看到原因；其他项目照常显示。这能覆盖「列表与读取之间项目被移除」这类竞态。
3. 每个任务只保留 `{id, kind, title, project_id, state, fix_round?}`，不把 `request` 正文、`metrics` 这类字段带进根数据。项目只保留 `{id, name: display_name}` 加 `counts: {running, needs_you}`。

**根 loader 的重验范围**是 `liveShouldRevalidate(byKind('task.', 'project.', 'bug.', 'merge.'))`。`task.*` 涵盖 `task.updated`/`created`/`started` 等，`project.*` 涵盖项目增删，`bug.*`、`merge.*` 是保险。`config.*`、`session.*`、`gate.*` 不改变任务状态，不触发重验。

**代价与取舍。** 每次重验要发 1 + N 次读请求，每次 `project.get` 都要扫一遍配置和 skills。本机面板的项目数是个位数到十几个，事件按 tick 批量到达，这个代价可以接受。在内核侧加一个一次返回所有任务的读方法要改 `crates/`，记入 Backlog B-01。

### R-04 标签表 `app/lib/labels.ts`

- `KIND_LABELS`：`feature: '需求'`，`bug: 'Bug'`（专有名词保留原文）。
- `STATE_LABELS`（8 个）：`draft: '未开始'`、`queued: '排队中'`、`active: '进行中'`、`paused: '已暂停'`、`stopped: '已停止'`、`failed: '失败'`、`done: '已完成'`、`cancelled: '已取消'`。
- `COLUMN_LABELS`：`todo: '待开始'`、`running: '进行中'`、`needs_you: '等你处理'`、`done: '已完成'`。
- `NODE_LABELS`（22 个）：需求节点沿用 desktop 的写法，比如 `intake: '任务整理'`、`gate: '门禁'`、`merge_gate: '合并前门禁'`、`main_check: '验证 main'`、`rebase: 'rebase'`。Bug 节点新增 `locate: '定位'`、`repro_check: '复现检查'`、`fix: '修复'`、`fix_gate: '修复门禁'`、`fix_review: '复核'`、`await_info: '等你补充信息'`、`not_a_bug: '判定不是 Bug'`。
- `AWAIT_PHRASES`：卡片状态行用，例如 `await_design_approval: '等你批准设计'`、`await_merge: '等你合并'`、`await_info: '无法复现，等你补充'`、`not_a_bug: '判定不是 Bug，等你确认'`。
- 阶段表与节点归属如下。终态 `done` 对应「完成」，它不是节点，是合成段：

| 类型 | 阶段 | 归入的内核节点 |
|---|---|---|
| 需求 | 整理 | `intake` |
| 需求 | 设计 | `design`、`review`、`adjudicate` |
| 需求 | 批准设计 | `await_design_approval` |
| 需求 | 实现 | `implement`、`gate` |
| 需求 | 审计 | `audit`、`retro` |
| 需求 | 合并 | `rebase`、`merge_gate`、`await_merge`、`merging`、`main_check`、`cleanup` |
| 需求 | 完成 | （`state: done`） |
| Bug | 定位 | `locate`、`repro_check`、`await_info`、`not_a_bug` |
| Bug | 修复 | `fix`、`fix_gate` |
| Bug | 复核 | `fix_review` |
| Bug | 合并 | `rebase`、`merge_gate`、`await_merge`、`merging`、`main_check`、`cleanup` |
| Bug | 完成 | （`state: done`） |

  映射规则：门禁节点归到它检查的那一步（`gate` → 实现，`fix_gate` → 修复），这与 desktop `GATE_STONE` 的思路一致，只是粒度更粗。`retro` 在实现循环结束后、rebase 之前回顾整轮工作，归入「审计」。`main_check` 和 `cleanup` 发生在合并之后，但此时任务仍是 `active`，归入「合并」，直到 `state: done` 才算「完成」。`repro_check` 在修复之前运行（`bug_loop.rs` 中 `start_fix(Node::ReproCheck, …)`），`await_info`、`not_a_bug` 都是定位得出的结论，三者都归入「定位」。
- `stageOf(kind, state)` 返回 `{ label, index, total }`。节点取自 `active.node`、`paused.resume`、`stopped.at`、`failed.at` 或 `queued.resume`。`draft`/`queued` 没有节点时返回 `null`（未开始）。`done` 返回「完成」。遇到未知节点时，`label` 回退为原始节点字符串，`index` 为 -1。
- 本任务**不显示**「已完成 3 / 5」和「修复第 2 / 3 轮」。理由是 R-02 允许 W5 的卡片取简版信息量，完整卡片归 W6/W7（Backlog B-03）。这不是数据拿不到：`task_json` 已有 `budget_n`，Bug 任务另有 `fix_round`，`project.get` 也返回解析后的 `config`；需求里程碑进度的数据来源则尚未核实，由 W6/W7 设计时确认。
- 页面代码只按内核给的值查这张表，不自己推断进度（硬性约束 4）。M-03 的守卫测试会扫描 `app/routes`、`app/components` 下的 `.tsx`，检查里面没有散写的状态名、列名、阶段名中文。

### 跳转目标（R-02、R-03 的链接格式）

新文件 `app/components/nav-links.ts` 提供以下纯函数，ID 一律用 `encodeURIComponent` 编码：

- `taskHref(id)` → `/tasks/:taskId`（W7 的 `routes/tasks.$id*`）；
- `projectHref(id)` → `/projects/:projectId`（W6 的 `routes/projects.$id*`）；
- `newTaskHref(projectId)` → `/projects/:projectId/tasks/new`（落在 W6 的 `projects.$id*` 下）。

链接**一律直接渲染**。目标路由还不存在时，靠 `root.tsx` 新增的 `ErrorBoundary` 兜底：404 显示卡片「这个页面还没开放」和「回到总览」链接，其他错误显示「页面出错了：<消息>」。这样既不会抛出未捕获的错误，HTTP 状态也仍然是 404（`server.test.ts` 的「prod 未知路径返回 404」保持不变）。W6、W7 只要加上路由就能接上，**不需要改本任务的代码**。

这里有意不照搬 W4 `bell-targets.ts` 的 `available` 开关：那个做法要求后续任务回来改开关。铃铛自己的 `TARGETS` 不在本任务里改。

### R-05 离线横幅与写判定（放在 `app/lib/live.tsx`）

- 纯函数 `liveBanner({ connection, core, kernelError }) → { tone, text } | null`。同一时刻只显示一条，优先级从高到低：
  1. `connection === 'reconnecting'`：显示「连接中断，正在重连」。此时内核状态未知，所以不叠加内核横幅。
  2. `core?.status === 'offline'`：显示「内核离线：<reason>，写操作暂不可用」。
  3. 还没收到 `core` 消息，但根 loader 报了 `kernel.ok === false`：显示「内核离线：<loader 的 error>，写操作暂不可用」。这一条覆盖 SSR 首屏，以及没有 WebSocket 的情况。
  4. 其他情况不显示。
- 纯函数 `writeGate({ connection, core }) → { allowed, reason }`：只有 `connection === 'open' && core?.status === 'connected'` 时才允许写。其他情况分别给出中文理由：「正在连接服务」「连接中断，正在重连」「内核离线：<reason>」。还没收到 `hello`（`core` 未知）时**不允许写**，宁可多置灰一会儿，也不放过一次必然失败的写。
- **恢复后的根数据重验。** 连接状态只是展示，不会让根数据自己刷新；内核离线时打开的页面，根 loader 已经返回 `kernel.ok === false`、空项目和空任务，内核恢复后如果账本里没有新事件，项目栏、首页和 ⌘K 就会一直停在离线结果上。所以 `live.tsx` 新增纯函数 `needsKernelReload({ core, kernelOk }) → boolean`：`core?.status === 'connected' && kernelOk === false` 时为真。同文件的 hook `useKernelRecovery({ core, kernelOk })` 在这个值由假变真时调用一次 `revalidateAll(() => revalidator.revalidate())`；`revalidateAll` 会让根 `shouldRevalidate` 无条件通过。这条路径同时覆盖两种时序：先离线、之后收到 `core(connected)`；以及 SSR 时内核离线、WebSocket 首个 `hello` 就带 `core=connected`。如果重验后 loader 仍然返回 `ok === false`，两个输入没有变化，hook 不会再次触发，不会形成重试循环；内核下一次离线再恢复时会重新触发。`App` 用根 loader 的 `kernel.ok` 和 `useLiveEvents()` 的 `core` 调用这个 hook。`live-client.ts`、`live-routes.ts`、服务端协议都不动。
- `WriteGateProvider` 与 `useWriteGate()`（React context）：`App` 用 `writeGate(useLiveEvents())` 的结果包住整个外壳。W6、W7 的按钮直接调用 `useWriteGate()`。
- `LiveBanner` 的签名改成 `LiveBanner({ connection, core, kernelError })`，不再用内联样式，改用 `.live-banner` 类和 token。重连时用 `role="status"`，内核离线时用 `role="alert"`。

本任务里受写判定约束的入口有两个：顶栏的「新建任务卡」按钮，以及 ⌘K 里的「新建任务卡」候选。不允许写时，二者都带 `disabled`/`aria-disabled` 和 `title={reason}`。`live-client.ts`、`live-routes.ts` 的语义不变。

### R-07 设计 token、字体与样式

- `app/styles/tokens.css`：文件头注明「token 取自 animal-island-ui（`docs/design-system/css-variables.md`），以 CC BY-NC 4.0 授权；经 `apps/desktop/renderer/style.css` Layer 1 迁移」。内容是 `@font-face` 和 Layer 1 的 `:root` 声明，**逐条照抄**，测试会与 desktop 源文件比对。不迁移 `[data-theme="dark"]`。
- 字体复制到 `app/styles/fonts/nunito/Nunito-Variable.ttf`，`OFL.txt` 一并复制。复制时不改 desktop 的原文件。`@font-face` 用相对路径 `url('./fonts/nunito/Nunito-Variable.ttf')`，由 Vite 打包进 `build/client/assets/`。不走 CDN，CSS 里不能出现外部 URL。
- `app/styles/app.css`：开头 `@import './tokens.css'`，然后是全局基础样式与本任务组件的样式（外壳、项目栏、顶栏、横幅、卡片、⌘K、铃铛）。规则如下：
  - 颜色只能用 `var(--animal-*)`/`var(--tile-*)`，色值字面量只允许出现在 `tokens.css`。不用 `#000`/`black`。
  - 焦点环一律是 `:focus-visible { outline: 3px solid var(--animal-focus-yellow) }`。
  - `border-radius` 只能是 `var(--animal-radius*)` 或 `50%`。
  - 3D 实色阴影 `0 4px 0 var(--animal-shadow-btn)` 只用于 `.btn-primary`。
  - 不用 emoji 当图标。图标用内联 SVG，比如铃铛、加号、放大镜。
- `root.tsx` 导出 `links = () => [{ rel: 'stylesheet', href: appCss }]`，`appCss` 来自 `import appCss from './styles/app.css?url'`，tsconfig 已经包含 `vite/client` 类型。
- W4 铃铛只改**呈现**：去掉所有内联样式，换成 `.bell*` 类；外层从 `position: fixed` 改为在顶栏右侧排布（`position: relative`），列表仍是 `position: absolute`，`max-height: 60vh` 和 `overflow-y: auto` 移进 CSS。`createBellClient`、`subscribeLiveMessages`、已读、系统通知相关代码一行不动。

**需要随之修改的已有测试**（用例数不变，逐条理由如下）：

| 测试 | 改什么 | 理由 |
|---|---|---|
| `bell-view.test.ts`「徽标 0 隐藏…」 | `not.toContain('font-weight:600')` 改为检查徽标类 `bell-badge` 是否出现 | 粗体从内联样式移进 CSS，原断言会变成永远成立，失去鉴别力 |
| `bell-view.test.ts`「固定定位与内部 60vh 滚动」 | 改为「顶栏内定位与内部 60vh 滚动」：断言列表元素带 `bell-panel` 类，并且 `app.css` 的 `.bell-panel` 规则含 `max-height: 60vh` 和 `overflow-y: auto` | R-07 要求去掉内联样式；任务范围要求铃铛收进顶栏，不再 `fixed` |
| `bell-view.test.ts`「root 同时渲染铃铛与子路由」 | `createRoutesStub` 的根路由加 `id: 'root'`、`loader`，并通过 `hydrationData` 给出 `ShellData` | 根组件开始读 loader 数据；断言内容（铃铛、子路由、`useLiveEvents()` 只出现一次）不变 |
| `live-banner.test.ts` 两条 | `top:0` 改为断言 `class="live-banner"`；新签名下补传 `core`/`kernelError` 参数 | 横幅从 sticky 内联样式改为外壳网格中单独的一行，样式移进 CSS |
| `server.test.ts`「dev 首页 SSR…」「prod 首页…」 | `面板骨架已就绪` 改为首页标题 `总览` | 占位页 `home.tsx` 按范围删除。断言不依赖内核此时是否在线：离线时总览照样渲染 |

### R-08 一屏布局

- `.shell`：`display: grid; grid-template-columns: 248px 1fr; grid-template-rows: auto auto 1fr; height: 100dvh; overflow: hidden`。三行依次是顶栏、横幅行（无横幅时高度为 0）、内容。项目栏占左列，从第 1 行跨到第 3 行。
- 允许内部滚动的区域只有这些：项目栏的列表 `.project-list`、首页两列的卡片区 `.overview-column-body`、⌘K 的结果列表 `.palette-list`、铃铛列表 `.bell-panel`。每个都设 `overflow-y: auto; min-height: 0`。`.shell-main` 设 `min-height: 0; overflow: hidden`，`body` 设 `margin: 0`。
- 会话能自己跑的检查是 M-02 的静态 CSS 测试：解析 `app.css`，断言上述声明存在，并且 `body`/`html` 上没有 `overflow: auto/scroll`。真实浏览器在 1512×944 下的一屏验收写进「人工验收清单」，Playwright 版本是 W12 的事。

### 外壳与页面的呈现（卡片优先、低密度）

- **项目栏**：顶部是品牌「Autome」，下面是「总览」链接，再下面是「项目」小标题和项目列表。每个项目一张小卡片：显示名，加两枚计数胶囊「进行中 n」（主色）和「等你处理 n」（警示黄），计数为 0 时用弱化色。`aria-label` 写成「<项目名>：进行中 n，等你处理 n」。整张卡片链接到 `projectHref`。没有项目时显示「还没有项目。可以用命令行 `autome project add <路径>` 添加」（W9 才有界面入口）。
- **顶栏**：左侧是「搜索或跳转」按钮，带快捷键提示（mac 显示 `⌘K`，其他平台显示 `Ctrl K`）。服务端渲染先输出 `Ctrl K`，挂载后再按平台修正。右侧是「新建任务卡」主按钮（`.btn-primary`，受写判定约束），最右是铃铛。点「新建任务卡」会打开 ⌘K，并把查询预填为「新建」。
- **首页 `routes/_index.tsx`**：标题「总览」，下面是「等你处理（n）」「进行中（n）」两列并排，每列内部滚动。卡片内容有：类型标签（需求 / Bug）、任务 ID、标题（最多两行）、项目名、阶段名（`stageOf`），以及状态行。状态行写法：等人节点用 `AWAIT_PHRASES`；`failed`/`stopped`/`paused` 写成 `STATE_LABELS · NODE_LABELS[节点]`；在跑的写成 `NODE_LABELS[节点]`。整张卡片链接到 `taskHref`。排序是先等你处理、再进行中，各列内部按任务 ID 的数字部分倒序（新的在上）。空列分别显示「没有需要你处理的任务」「没有正在进行的任务」。内核离线时两列都换成「内核离线，暂时拿不到任务」。

### R-03 ⌘K 跳转面板

- 纯逻辑在 `app/components/palette-model.ts`：
  - `isPaletteShortcut(event, platform)`：mac（`navigator.platform` 或 `userAgentData.platform` 匹配 `/Mac|iPhone|iPad/`）上要求 `metaKey && !ctrlKey`，其他平台要求 `ctrlKey && !metaKey`。另外要求 `key.toLowerCase() === 'k'`，不带 Shift/Alt，并且不在 `isComposing`（输入法组合）中。
  - `attachPaletteShortcut(target: EventTarget, platform, onToggle) → detach`：命中快捷键时调用 `preventDefault()` 再 `onToggle()`。它只依赖 `EventTarget`，所以能在 Node 里测。
  - `buildCandidates(shell, gate)` 生成三类候选：每个项目一条（链接 `projectHref`）；**每个任务一条，包括已完成和已取消的**；每个项目一条「在「<项目名>」新建任务卡」（链接 `newTaskHref`，`!gate.allowed` 时为禁用项）。任务候选的副标题是「项目名 · 状态名」。
  - `filterCandidates(list, query)`：查询去掉首尾空白后转小写，在 ID、标题、项目名和「新建任务卡」字样里做子串匹配（候选只来自根 loader 的白名单字段，所以**不按 slug 搜索**；R-03 也没有要求）。空查询时显示所有项目、等你处理和进行中的任务、新建项。结果最多 50 条，超出时提示「还有 n 条，继续输入可缩小范围」。
  - 排序：先是任务（等你处理 → 进行中 → 待开始 → 已完成 → 已取消，同组内 ID 倒序），然后是项目，最后是新建项。
  - `paletteReducer(state, action)`：处理 `open(query?)`、`close`、`input`、`down`、`up`、`enter`。`down`/`up` 循环移动并跳过禁用项；`input` 后高亮回到第一个可用项；`enter` 在可用项上产生 `{ navigate: href }`，并关闭面板；`close` 由 Esc 触发。
- 视图在 `app/components/command-palette.tsx`：`role="dialog"`、`aria-modal="true"`、`aria-label="跳转"`。输入框带 `role="combobox"`、`aria-expanded`、`aria-controls`，以及指向高亮项的 `aria-activedescendant`。结果放在 `role="listbox"` 里，每条是 `role="option"`，禁用项加 `aria-disabled="true"`。回车用 `useNavigate()` 跳转。打开时把焦点放到输入框，关闭时还给打开前的元素。点遮罩会关闭面板。
- 候选的来源是根 loader（`project.list` + `project.get`）。打开面板时不额外发请求，候选也随事件重验保持最新。理由：R-03 要求「任意任务」，而 `dashboard.get` 不含草稿、排队、暂停、停止和终态任务；如果打开时按需再拉 `project.get`，就和根 loader 重复取数。把已完成、已取消的任务也列进来，是为了满足「任意任务」；它们排在后面，不影响常用项的选取。

### 文件清单

- 新增：`app/load-context.ts`、`app/styles/{tokens.css,app.css,fonts/nunito/Nunito-Variable.ttf,fonts/nunito/OFL.txt}`、`app/lib/labels.ts`、`app/components/{task-columns.ts,shell-data.ts,nav-links.ts,shell.tsx,project-list.tsx,overview-card.tsx,palette-model.ts,command-palette.tsx}`、`app/routes/_index.tsx`。
- 修改：`app/root.tsx`、`app/routes.ts`（只改 index 的指向）、`app/lib/live.tsx`、`app/components/bell.tsx`、`server/http.ts`（只加 `getLoadContext`）。
- 删除：`app/routes/home.tsx`。
- `server/main.ts` **不改**（本任务不交出 store）。
- 新增测试文件，按里程碑列出：`load-context`、`styles`、`live-status`、`shell`、`labels`、`task-columns`、`shell-data`、`overview`、`shell-live`、`palette`。另有 `test/fixtures/w5-project-get.json`，是从真实内核逐字采集的 `project.get` 输出。

## 风险与验证安排

| 风险 | 影响 | 验证与对策 | 何时 |
|---|---|---|---|
| 上下文键的身份在 Node 原生、Vite SSR、生产 build 三处模块实例之间不一致 | loader 拿不到 gateway，R-06 整体失效 | M-01 不监听端口，分别在 Vite `middlewareMode`（`ssrLoadModule`）和 `react-router build` 产物上实测 SSR；负向对照是去掉 `Symbol.for` 登记后必须变红 | M-01，最先做 |
| `RouterContextProvider` 的 `instanceof` 检查在双包或多份实例下失败 | 每个请求都报「Invalid `context` value」 | 同上，两种模式的 SSR 都要拿到 200 和数据才算过 | M-01 |
| Vite `server.watch: null` 在 Vite 8 下的行为与预期不符，或撞上 watcher 的 EMFILE | dev SSR 测试不稳定 | 先用 `watch: null`，不行就退到 `watch: { ignored: ['**/*'] }`；把实测结果写进证据 | M-01 |
| 实现轮环境不允许监听 127.0.0.1（W1–W4 都遇到过） | `server.test`、`gateway-http`、`shell-live` 第 3 条在实现轮跑不了 | 计数、标签、交叉核对、⌘K、SSR 取值都写成不监听端口的测试；需要监听的只集中在 `shell-live.test.ts` 第 3 条和已有文件；验收命令分段报告各自的退出码，被拦住的用例交给审计轮复验 | 全程 |
| 每次重验发 1 + N 次 `project.get` 太慢 | 事件密集时项目栏刷新滞后 | M-04 的真实内核测试记录单次 `loadShell` 的耗时，写进证据；内核侧的根治记入 Backlog B-01 | M-04 |
| 内核挂起时 SSR 最长被 `CALL_TIMEOUT_MS`（15 秒）阻塞 | 首屏慢 | 沿用 W2 的超时语义，不在 W5 改；超时时 `kernel.error` 原样显示「内核响应超时」 | 已知取舍 |
| 内核以后新增节点或状态 | 标签和归列静默出错 | M-03 的交叉核对和篡改自检（`task.rs` 一新增变体，测试就变红） | M-03 |
| 真实内核用例在 `ci-local` 全量负载下超时（W4 L-05） | 宿主门禁抖动 | 证据里写出在全量负载下的实测耗时，超时上限设为实测值的 2 倍以上 | M-04 |
| 夹具让任务进入 `failed` 时被重试拖住（W4 L-02） | 夹具不收敛 | 夹具显式写 `retry attempts=0`、`backoff_seconds=0` | M-04 |

**验证方式的通则：**

- 每个里程碑的验收命令分段执行，**不要用 `&&` 串起来**，每段单独报退出码。顺序是：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、本里程碑点名的测试文件、`npm test --prefix apps/web`（全量）、`npm run build --prefix apps/web`、`npm test --prefix apps/desktop`。
- 真实内核用例的前置命令是 `cargo build -p automed -p autome-cli`。本机有二进制时不允许跳过，`skipped` 必须是 0。
- 负向对照逐条实跑并记下退出码。全部还原后，在仓库根执行 `git status --porcelain`，输出必须为空。
- 报告 Web 和 desktop 的**用例总数**，以及相对上一里程碑基线的差值。Web 基线是 287。desktop 基线是 104，W5 不改 desktop，所以应保持 104。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | getLoadContext 端到端：共享上下文键、两种模式 SSR 取到网关 | 0 | load-context |
| M-02 | 已完成 | animal-island token、字体与外壳布局，铃铛入顶栏，离线横幅与写判定 | 0 | shell-style |
| M-03 | 已完成 | labels.ts 阶段表与唯一归列函数，交叉核对 task.rs | 0 | labels |
| M-04 | 开放 | 项目栏计数与首页总览，事件驱动实时更新 | 0 | overview |
| M-05 | 开放 | ⌘K 跳转面板与新建任务卡入口 | 0 | palette |

## 验收说明

里程碑共 5 个，不超过上限 6 个（硬性约束 7）。依赖关系是：M-01 → M-02 → M-04 → M-05；M-03 独立于 M-01 和 M-02，但必须在 M-04 之前完成。为了让编号与执行顺序一致，按编号依次推进。

### M-01 getLoadContext 端到端

最新证据：docs/w5-外壳与项目导航-迁移-animal/evidence/M-01-r1-impl.md · 定向通过，全量环境受阻待复验 · 实现 #1

- 覆盖：R-06。前置：无。
- 结果：新增 `app/load-context.ts`。`server/http.ts` 两处都加上 `getLoadContext`。`root.tsx` 加上根 loader，暂时只调用 `project.list`，作为 `loadShell` 的最小形态，M-04 再补全；同时导出 `shouldRevalidate = liveShouldRevalidate(byKind('task.','project.','bug.','merge.'))`。根组件先用一个不加样式的 `<nav aria-label="项目">` 列出项目名，M-02 再加样式。按「需要随之修改的已有测试」表，改 `bell-view`「root 同时渲染…」一条。
- 新测试 `test/load-context.test.ts`，共 9 条：
  1. `createLoadContext` 每次请求都新建一个 `RouterContextProvider`，能取回传入的 services；gateway 为 `null` 时取回 `null`。
  2. 用不同查询串把 `app/load-context.ts` 加载两次，得到的是同一个 `servicesContext` 对象。
  3. 根 loader 只通过 `gateway.read({method:'project.list', params:{}})` 取数。假 gateway 只有 `read`/`write`，调用记录里恰好有 1 次读请求。
  4. gateway 为 `null` 时，根 loader 返回 `kernel.ok === false`，文案含「内核未连接」，不抛错。
  5. `gateway.read` 返回 503、504、409 时，根 loader 把 `body.error` 原样放进 `kernel.error`。
  6. 开发模式：Vite 用 `middlewareMode`，不监听端口，关掉 HMR 和 watcher；用 `ssrLoadModule('virtual:react-router/server-build')` 得到 build，交给 `react-router` 的 `createRequestHandler`，再把 `createLoadContext({gateway: 假网关})()` 作为上下文传入。请求 `GET /` 得到 200，HTML 含假项目名「演示项目甲」。
  7. 生产模式：先 `npm run build`，再 `import('build/server/index.js')`，按第 6 条同样的方式请求，结果同第 6 条。
  8. 用 TypeScript AST 检查 `server/http.ts`：两处 `createRequestHandler` 的参数对象都有 `getLoadContext` 属性。
  9. 用 `createMemoryRouter` 装上根模块真实导出的 `shouldRevalidate` 和一个计数用的 loader：`task.updated`、`project.added`、`bug.fix_round_started`、`merge.rebased` 都会触发重验；`config.changed`、`session.started` 不会。

### M-01 验收契约

- 验收命令：`npm test --prefix apps/web -- test/load-context.test.ts test/bell-view.test.ts test/live-routes.test.ts`（之后分段跑 `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/web`、`npm run build --prefix apps/web`，各自报退出码）
- 预期：点名的 3 个文件共 24 个用例通过（load-context 9 个、bell-view 8 个、live-routes 7 个）；Web 静态枚举 296 个用例（基线 287 + 9），运行时参数化展开后 310 个（运行基线 301 + 9），`skipped` 为 0；lint、typecheck、build 的退出码都是 0。
- 负向对照：把 `servicesContext` 改成模块顶层直接 `createContext<WebServices>()`、不经 `Symbol.for` 登记，第 2、6、7 条变红（第 6、7 条的 HTML 不含「演示项目甲」，或返回 500「No value found for context」）；删掉生产分支 `createRequestHandler` 里的 `getLoadContext`，第 8 条变红；把根 loader 改成 `gateway.core.call('project.list')`，`npm run typecheck` 失败（`KernelGateway` 上没有 `core`），第 3 条也变红。

### M-02 token、字体、外壳布局、铃铛入顶栏、离线横幅与写判定

最新证据：docs/w5-外壳与项目导航-迁移-animal/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 覆盖：R-07、R-08、R-05（横幅与写判定）。前置：M-01。
- 结果：新增 `app/styles/` 下的 `tokens.css`、`app.css`、字体和许可文件。新增 `app/components/shell.tsx`，包括外壳网格、项目栏框架（暂时只有项目名）、顶栏（暂时只有一个禁用的「新建任务卡」按钮和铃铛）、横幅行。`root.tsx` 加上 `links`、`ErrorBoundary`、`WriteGateProvider`。`live.tsx` 加入 `liveBanner`、`writeGate`、`WriteGateProvider`、`useWriteGate`、`needsKernelReload`、`useKernelRecovery`，`LiveBanner` 改成新签名。`bell.tsx` 改用类名。按表修改 `bell-view` 的两条和 `live-banner` 的两条。
- 新测试 `test/styles.test.ts`，共 10 条：
  1. `tokens.css` 的 `:root` 声明与 desktop `style.css` Layer 1 的 `:root` 逐条相同（解析成名值对后比较）。
  2. 字体文件的 sha256 与 desktop 源文件一致；`OFL.txt` 内容一致；`@font-face` 的 `url()` 指向一个存在的相对路径；`app/styles/*.css` 里没有 `http:`、`https:`、`//` 开头的外部 URL。
  3. 两个样式文件的文件头都注明 animal-island-ui 出处和 CC BY-NC 4.0。
  4. 色值字面量（`#hex`、`rgb(`、`hsl(`、`black`）只出现在 `tokens.css`；`app.css` 和 `app/**/*.tsx` 里都没有。
  5. 所有 `:focus-visible` 规则的 outline 颜色都是 `var(--animal-focus-yellow)`。
  6. 所有 `border-radius` 的取值都是 `var(--animal-radius*)` 或 `50%`。
  7. 引用 `--animal-shadow-btn` 的声明只出现在选择器含 `btn-primary` 的规则里。
  8. `app/` 下的源码不含 Unicode `Extended_Pictographic` 字符，也就是不拿 emoji 当图标。
  9. `app/` 下的 `.tsx` 文件里没有 `style={`，即没有内联样式。
  10. 一屏布局：`.shell` 有 `height: 100dvh` 和 `overflow: hidden`；四个内部滚动区都有 `overflow-y: auto` 和 `min-height: 0`；`body` 有 `margin: 0`；`html`/`body` 上没有 `overflow: auto/scroll`。
- 新测试 `test/live-status.test.ts`，共 6 条：
  1. 横幅优先级：重连 > 内核离线（带 reason）> loader 报告的内核错误 > 不显示；每种情况只出现一条。
  2. 横幅的角色和类名：重连时是 `role="status"`，内核离线时是 `role="alert"`，两者都带 `class="live-banner"`，正文出自上面约定的中文文案。
  3. `writeGate`：`open` + `connected` 时允许写；`connecting`、`reconnecting`、内核 `offline` 时不允许，并各自给出约定的中文理由。
  4. `connection` 为 `open` 但还没收到 `core` 时不允许写。
  5. 在 `WriteGateProvider` 下用 SSR 渲染一个调用 `useWriteGate()` 的按钮：不允许写时按钮带 `disabled`，`title` 是对应理由。
  6. 恢复后的根数据重验：`needsKernelReload` 在 `core` 为 `connected` 且 `kernelOk === false` 时为真，`core` 未知、`offline` 或 `kernelOk === true` 时为假；用 `createMemoryRouter` 装一个计数 loader（首次返回 `ok:false`、之后返回 `ok:true`）和真实的 `useKernelRecovery`，在**没有任何账本事件**的情况下，把 `core` 从 `offline` 改为 `connected`，loader 恰好被重新调用一次、数据变成 `ok:true`；`core` 一上来就是 `connected` 且 `ok:false`（首个 `hello` 的时序）时同样重验一次；重验后仍是 `ok:false` 不会再次重验。
- 新测试 `test/shell.test.ts`，共 4 条：
  1. 用 `createRoutesStub` 给根路由 `hydrationData` 后渲染：有 `nav aria-label="项目"` 的项目栏；顶栏里有铃铛按钮 `aria-label="通知，0 条未读"`；`main` 里是子路由的内容。
  2. 根模块的 `links()` 包含一个指向 `app.css` 的样式表。
  3. 访问 `/tasks/T-1` 这类未知路径时，渲染「这个页面还没开放」和指向 `/` 的「回到总览」，不抛错。
  4. 在 SSR 首屏（`connection` 为 `connecting`）下，顶栏的「新建任务卡」按钮是禁用的，`title` 为「正在连接服务」。

### M-02 验收契约

- 验收命令：`npm test --prefix apps/web -- test/styles.test.ts test/live-status.test.ts test/shell.test.ts test/bell-view.test.ts test/live-banner.test.ts`（之后分段跑 lint、typecheck、Web 全量测试、build、desktop 测试，各自报退出码）
- 预期：点名的 5 个文件共 30 个用例通过（10 + 6 + 4 + 8 + 2）；Web 静态枚举 316 个用例（296 + 20），运行时参数化展开后 330 个（上一轮运行基线 310 + 20），`skipped` 为 0；desktop 104 个用例，数量不变；build 后的 `apps/web/build/client/assets/` 里有一个名字含 `Nunito` 的 `.ttf` 文件，用 `ls apps/web/build/client/assets | grep -c 'Nunito.*\.ttf'` 检查，输出 ≥ 1。
- 负向对照：在 `bell.tsx` 按钮上恢复 `style={{ background: '#fff' }}`，第 4、9 条变红；把 `.shell` 的 `overflow: hidden` 改成 `overflow: auto`，或者删掉 `.project-list` 的 `min-height: 0`，第 10 条变红；`writeGate` 在 `core` 为 `undefined` 时返回 `allowed: true`，`live-status` 第 3、4 条变红（`shell` 第 4 条验证 connecting 首屏，保持禁用）；`needsKernelReload` 恒返回 `false`（或 `App` 不调用 `useKernelRecovery`），`live-status` 第 6 条变红；删掉 `tokens.css` 中的 `--tile-peach` 声明，第 1 条变红。

### M-03 labels.ts 与唯一归列函数

最新证据：docs/w5-外壳与项目导航-迁移-animal/evidence/M-03-r4-audit.md · 通过 · 审计 #4

- 覆盖：R-04，以及 R-01 的口径部分。前置：无，但必须在 M-04 之前完成。
- 结果：新增 `app/lib/labels.ts`，内容见「方案 · R-04」；新增 `app/components/task-columns.ts`，内容见「方案 · R-01」。新增 `test/fixtures/w5-project-get.json`：用真实内核和命令行造出至少包含 `draft`、`queued`、`active`（普通节点和一个等人节点）、`stopped`、`failed` 的项目，逐字保存 `autome rpc project.get` 的原始输出，采集命令写进证据。
- 新测试 `test/labels.test.ts`，共 7 条：
  1. 解析 `task.rs`：`enum Node` 的变体集合等于 `Node::ALL` 与 `Node::BUG` 的并集，共 22 个；按 serde 规则转成 snake_case。
  2. 需求阶段表有 7 段，顺序与计划 §4.3 一致；表里的节点恰好覆盖 `Node::ALL`，每个节点只出现一次。
  3. Bug 阶段表有 5 段，顺序一致；表里的节点等于 `Node::BUG` 加上「合并」段的共享节点；`bug_loop.rs` 中引用的每个非 Bug 节点都在「合并」段里；「合并」段的节点都属于 `Node::ALL`。
  4. `NODE_LABELS` 的键恰好是 22 个内核节点；遇到未知节点，`nodeLabel` 和 `stageOf` 都回退为原始字符串。
  5. 解析 `enum TaskStateRepr`：变体集合等于 `STATE_LABELS` 的键（8 个）；`enum TaskKind` 的变体集合等于 `KIND_LABELS` 的键。
  6. 篡改自检：在 `task.rs` 源码字符串里给 enum 和 `ALL` 各插入一个 `NewNode`，检查函数要报告缺失；从需求表删掉 `gate`，要报告缺失；在表里加一个 `ghost`，要报告多余。
  7. `app/routes/**` 和 `app/components/**` 下的 `.tsx` 不含 `STATE_LABELS`、`COLUMN_LABELS` 或任一阶段名的中文字面量。
- 新测试 `test/task-columns.test.ts`，共 6 条：
  1. 表驱动：以下 14 种状态 JSON 逐一映射到约定的列——`draft`、`queued`、带 `resume` 的 `queued`、`active` 的 `implement`/`gate`/`await_design_approval`/`await_merge`/`await_info`/`not_a_bug`、`paused`、`stopped`、`failed`、`done`、`cancelled`。
  2. `AWAITING_NODES` 等于从 `task.rs` 的 `Node::awaits_user` 解析出来的集合。
  3. `paused` 归入等你处理；`cancelled` 和未知 `state` 都返回 `null`。
  4. `countByColumn` 与逐个调用 `columnOf` 的结果一致；输入空列表时各列都是 0。
  5. 用夹具 `w5-project-get.json` 核对：`countByColumn(tasks)` 的等你处理等于夹具里 `counts.awaiting_user + failed + stopped + paused`，进行中等于 `counts.running`，缺失的键按 0 计。
  6. `stageOf`：`draft` 和不带 `resume` 的 `queued` 返回 `null`；`implement` 和 `gate` 返回「实现」；`await_merge` 返回「合并」；`done` 返回「完成」；Bug 的 `fix_review` 返回「复核」，`await_info` 返回「定位」；未知节点返回原始值。

### M-03 验收契约

- 验收命令：`npm test --prefix apps/web -- test/labels.test.ts test/task-columns.test.ts`（之后分段跑 lint、typecheck、Web 全量测试、build，各自报退出码）
- 预期：2 个文件共 13 个用例通过（7 + 6）；Web 全量 329 个用例（316 + 13），`skipped` 为 0。
- 负向对照：从需求「实现」段删掉 `gate`，`labels` 第 2 条变红；把 `retro` 同时写进「审计」和「合并」，第 2 条变红（违反「恰好一次」）；`columnOf` 把 `stopped` 归入进行中，`task-columns` 第 1、5 条变红；`AWAITING_NODES` 漏掉 `not_a_bug`，第 1、2 条变红。

### M-04 项目栏计数与首页总览，实时更新

- 覆盖：R-01、R-02。前置：M-01、M-02、M-03。
- 结果：新增 `shell-data.ts`，`loadShell` 按完整形态实现，根 loader 改为调用它。新增 `project-list.tsx`（项目栏，带计数）、`overview-card.tsx`、`nav-links.ts`、`routes/_index.tsx`。`routes.ts` 的 index 改指向 `routes/_index.tsx`，删除 `routes/home.tsx`。按表修改 `server.test.ts` 的两条。
- 新测试 `test/shell-data.test.ts`，共 6 条：
  1. `loadShell` 调用 `project.list` 一次，并为每个项目各调用 `project.get` 一次，全部经 `read`。
  2. 每个项目的 `{running, needs_you}` 与首页卡片集合都由 `columnOf` 得出：对同一份假数据，计数等于 `countByColumn` 的结果，卡片恰好是 `needs_you` 加 `running` 的任务，排序符合约定。
  3. 某个项目的 `project.get` 失败时，该项目 `counts` 为 `null` 并带 `error`，其他项目正常。
  4. `project.list` 失败时，返回 `kernel.ok === false`，`projects` 和 `tasks` 都为空。
  5. 输出的任务对象只含白名单字段 `id, kind, title, project_id, state, fix_round?`，不含 `request` 和 `metrics`。
  6. 根 loader 端到端：在 `RouterContextProvider` 里放入假 gateway，调用根模块导出的 `loader`，结果与直接调用 `loadShell` 一致。
- 新测试 `test/overview.test.ts`，共 5 条：
  1. 项目栏：每个项目显示名称、「进行中 n」「等你处理 n」，以及约定的 `aria-label`；链接是 `/projects/<编码后的 id>`；`counts` 为 `null` 的项目显示「—」；没有项目时显示空状态文案。
  2. 首页有「等你处理（n）」「进行中（n）」两列；卡片链接是 `/tasks/<编码后的 id>`；卡片上的阶段名等于 `stageOf(...).label`。
  3. 卡片内容包括类型（需求 / Bug）、ID、标题、项目名和状态行；等人节点的状态行来自 `AWAIT_PHRASES`，`stopped`/`failed`/`paused` 的状态行是「状态 · 节点」；空列显示各自的文案。
  4. 内核离线时（`kernel.ok === false`），首页两列显示「内核离线，暂时拿不到任务」，不抛错。
  5. `nav-links`：三种目标的格式正确，`a/b ?中` 被编码成 `a%2Fb%20%3F%E4%B8%AD`。
- 新测试 `test/shell-live.test.ts`，共 3 条，使用真实二进制，夹具 `test/fixtures/w5-states.sh`：
  1. 真实内核：用命令行在临时 `AUTOME_HOME` 里 `project add --skip-onboarding`，造出 draft、active（用阻塞住的假 CLI 让任务停在 `active`）、failed（`retry attempts=0`、`backoff_seconds=0`）等任务。然后经 `createGateway` + `CoreSupervisor`（不监听端口）调用 `loadShell`：每个项目的计数等于同一时刻 `project.list` 的 `counts` 按核对式组合的结果。
  2. 跨进程：内核进程保持运行，另起一个进程执行 `autome stop <id>`，把 active 任务停下。之后 `events.since` 里出现一条被根 `shouldRevalidate` 的匹配器命中、`subject` 为该任务的事件；再调用一次 `loadShell`，该项目的「等你处理」加 1，「进行中」减 1。
  3. 监听端到端：`startServer({ mode: 'development' })` 指向同一个 home，`GET /` 的 HTML 含该项目名和「等你处理」计数文本。
- 证据里要写出 `shell-live.test.ts` 在 `sh scripts/ci-local.sh` 全量负载下的实测耗时，以及单次 `loadShell` 的耗时；文件里的超时上限不能低于实测值的 2 倍。

### M-04 验收契约

- 验收命令：`npm test --prefix apps/web -- test/shell-data.test.ts test/overview.test.ts test/shell-live.test.ts test/server.test.ts test/live-routes.test.ts`（前置命令 `cargo build -p automed -p autome-cli`；之后分段跑 lint、typecheck、Web 全量测试、build、desktop 测试，各自报退出码）
- 预期：点名的 5 个文件共 30 个用例通过（shell-data 6、overview 5、shell-live 3、server 9、live-routes 7）；Web 全量 343 个用例（329 + 14），`skipped` 为 0。实现轮如果被 `listen EPERM` 拦住，单独列出被拦的用例名单（`shell-live` 第 3 条和 `server.test` 中需要监听的用例），以及不依赖监听的那部分结果，交给审计轮复验。
- 负向对照：项目栏的「等你处理」改成直接读 `counts.awaiting_user`、不经 `columnOf`，`shell-live` 第 1、2 条和 `shell-data` 第 2 条变红；根 `shouldRevalidate` 去掉 `'task.'`，`shell-live` 第 2 条（匹配器未命中）和 `load-context` 第 9 条变红；`taskHref` 不做编码，`overview` 第 5 条变红。

### M-05 ⌘K 跳转面板与新建任务卡入口

- 覆盖：R-03，以及 R-05 中受写判定约束的入口。前置：M-04。
- 结果：新增 `palette-model.ts` 和 `command-palette.tsx`。顶栏的「搜索或跳转」按钮和「新建任务卡」按钮接到面板。`App` 里挂上 `attachPaletteShortcut(window, …)`。
- 新测试 `test/palette.test.ts`，共 7 条：
  1. 快捷键判定：mac 上 ⌘K 触发，Ctrl+K 不触发；其他平台 Ctrl+K 触发，⌘K 不触发；`isComposing`、带 Shift 或 Alt 时都不触发。
  2. `attachPaletteShortcut`：用 Node 的 `EventTarget` 加上一个带 `key`/`metaKey` 的 `Event` 做测试。命中时调用 `preventDefault` 并切换一次；`detach` 之后不再响应。
  3. 候选：每个项目一条；每个任务一条，包括已完成和已取消的，排在后面；每个项目一条「在「X」新建任务卡」；`href` 都来自 `nav-links`。
  4. 过滤：大小写不敏感，能按 ID、标题、项目名匹配；空查询返回约定的默认列表；超过 50 条时截断，并给出剩余条数的提示。
  5. 键盘：`down`/`up` 循环移动并跳过禁用项；`input` 后高亮回到第一个可用项；`enter` 产生 `navigate` 效果并关闭面板；`close` 关闭面板。
  6. 写判定不允许时，新建候选是禁用的，`enter` 不会产生 `navigate`；顶栏的「新建任务卡」按钮 SSR 后带 `disabled` 和理由。
  7. 视图 SSR：有 `role="dialog"`、`role="combobox"`、`role="listbox"`、`role="option"`；`aria-activedescendant` 指向高亮项的 `id`；禁用项带 `aria-disabled="true"`。

### M-05 验收契约

- 验收命令：`npm test --prefix apps/web -- test/palette.test.ts test/shell.test.ts`（之后分段跑 lint、typecheck、Web 全量测试、build、desktop 测试，各自报退出码；最后在仓库根执行 `git status --porcelain`）
- 预期：2 个文件共 11 个用例通过（7 + 4）；Web 全量 350 个用例（343 + 7），`skipped` 为 0；desktop 104 个用例，数量不变；`git status --porcelain` 的输出为空。
- 负向对照：`isPaletteShortcut` 在 mac 上也接受 `ctrlKey`，第 1 条变红；`down` 不跳过禁用项，第 5、6 条变红；`filterCandidates` 只匹配标题，第 4 条变红（按 ID `T-12` 搜不到）；新建候选不读写判定，第 6 条变红。

## 人工验收清单

- H-01 用 Chrome 打开面板，窗口内容区调到 1512×944，依次查看首页总览和「这个页面还没开放」页：页面本身没有纵向滚动条。用命令行加 30 个以上项目后，只有左侧项目栏在内部滚动。
- H-02 视觉符合 animal-island 风格：字体是 Nunito（DevTools 网络面板里没有对外的字体请求）；配色是暖米色底加青色主色；圆角是圆润的；只有「新建任务卡」主按钮带 3D 实色阴影；页面上没有 emoji 图标。
- H-03 真实键盘：macOS 上按 ⌘K 能打开面板（Windows/Linux 上是 Ctrl+K）。能输入过滤，能用方向键移动，按回车会跳转，按 Esc 关闭后焦点回到原来的位置。用 Tab 能走到所有入口，焦点环是黄色的。
- H-04 页面开着时，在另一个终端执行 `autome stop <任务 id>`（或推进任意任务）。不刷新页面，左侧计数和首页卡片在几秒内变化。
- H-05 结束 `automed` 进程：出现「内核离线：…，写操作暂不可用」横幅，「新建任务卡」按钮和 ⌘K 里的新建项都变成灰色。内核恢复后横幅消失，按钮恢复可用。另外在内核停掉的状态下刷新页面，首页、项目栏显示离线；再启动内核，不刷新、不做任何任务操作，项目栏和首页卡片也应在几秒内回到正常。停掉面板服务时，只显示「连接中断，正在重连」，两条横幅不会同时出现。
- H-06 铃铛在顶栏右侧。展开的通知列表不被外壳遮挡，可以在内部滚动。已读、全部已读、系统通知的行为和 W4 一样。
- H-07 点首页上的任务卡和 ⌘K 里的项目、任务、新建项：在 W6、W7 合并之前，都会到「这个页面还没开放」，再点「回到总览」能回到首页。

## Backlog

- B-01 内核侧加一个只读方法，一次返回所有项目的任务及状态，替代根 loader 的 `project.get` ×N（每次都要扫配置和 skills）。这要改 `crates/`，不属于 Web 任务。
- B-02 暗色调色板（desktop `[data-theme="dark"]`）和跟随系统主题，与主题切换界面一起做。
- B-03 卡片上的「已完成 3 / 5」里程碑进度和「修复第 n / 上限 轮」，归 W6/W7 的完整任务卡。Bug 修复轮的数据已有：`task_json.budget_n`、`fix_round`，上限的解析见 `bug_loop.rs` 与 `project.get` 的 `config.bug.fix_rounds`；需求里程碑进度的数据来源要在 W6/W7 设计时先核实。
- B-04 ⌘K 支持拼音首字母和模糊匹配，并记住最近访问过的条目。
- B-05 外壳一屏无滚动的浏览器自动验收（Playwright，1512×944），归 W12。

## 争议项
