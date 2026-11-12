# 任务：面板外壳与项目导航（W5）——animal-island 设计 token、左侧项目列表计数、首页总览、⌘K 跳转、中文标签表与 getLoadContext

本任务的规则见 `docs/w5-外壳与项目导航-迁移-animal/protocol/loop-protocol.md` 与
`docs/w5-外壳与项目导航-迁移-animal/protocol/session-protocol.md`。

- 设计文档：`docs/w5-外壳与项目导航-迁移-animal/w5-外壳与项目导航-迁移-animal.md`
- 任务文件生成日期：2026-11-12；基线提交 `a3455a3`（任务分支 `autome/w5-外壳与项目导航-迁移-animal`，
  从 main 拉出；K0、K0b、K1、K2、C1、W1、W2、W3、W4 均已合并进基线）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支）
- 证据、简报、运行记录一律写在 `docs/w5-外壳与项目导航-迁移-animal/` 下；超过 5 行的命令输出与临时产物写进 `.autome/output/`

## 1. 需求原文

> W5 外壳与项目导航：迁移 animal-island 设计 token，左侧项目列表（带进行中与等你处理计数）、首页总览、⌘K 跳转与中文标签表，并给 server/http.ts 加 getLoadContext 把 gateway 交给 loader/action

这句话对应计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 的 **W5 外壳与项目导航**。计划给 W5 的「目录」与「验收」
见下面 §2、§3。它们和计划 §2（UI 硬规则）、§3.2（看板列定义）、§4.3（阶段表）一起，构成本任务的范围。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-06 取自计划 W5 的「验收」1–6，文字没有改动；R-07、R-08 是需求原文里的「迁移 animal-island 设计 token」，以及计划 §2 的 UI 硬规则。
编号供评审、审计追溯。

- **R-01 左侧是项目列表，每个项目显示进行中和等你处理的数量，实时更新。**
  - 计数口径以计划 §3.2 为准（原文）：
    | 列 | 包含 |
    |---|---|
    | 待开始 | 新增的 `Draft` 状态：卡片已创建，还没开始跑；以及排队中的任务 |
    | 进行中 | 正在跑会话或内核步骤（含门禁、重试等待、限流冷却） |
    | 等你处理 | 等你批准设计、等你合并、Bug 无法复现等你补充、失败、已停止 |
    | 已完成 | 近 14 天合并完成的；更早的折叠到「查看全部」 |
  - **已知口径差异（设计必须处理并写明）**：内核现有的两个计数口径都和 §3.2 不一致。
    1. `project.list` 的 `counts` 来自 `crates/automed/src/store.rs` 的 `task_counts`，键为 `draft`、`queued`、`awaiting_user`
       （只含 `Active` 且节点在等人：`await_design_approval`/`await_merge`/`await_info`/`not_a_bug`）、`running`、`paused`、
       `stopped`、`failed`、`done`、`cancelled`。
    2. `dashboard.get` 的 waiting 用的是 `TaskState::awaits_user()`，包含等人节点与 `Failed`，但**不含 `Stopped`**。

    「等你处理」按 §3.2 应为等人节点 + `failed` + `stopped`。`paused` 在 §3.2 里没有归到任何一列，设计决定它算在哪一列
    （或哪一列都不算），并写明理由；拿不准就写进 `## 争议项`。本任务**不得改内核**来统一口径，只能在 Web 侧按任务状态计算，
    或者组合已有字段。另外，列表、首页、⌘K 三处必须使用同一个计数/归列函数，并且有测试。
  - 「实时」走 W3 的 `/ws` 事件与 `liveShouldRevalidate`：命令行或别的进程改变任务状态之后，计数在不刷新页面的情况下更新。
- **R-02 首页总览：所有项目里「等你处理」和「进行中」的任务卡，点击进入任务。**
  任务详情页属于 W7（`/tasks/$id` 之类的路由目前不存在）。本任务只给出链接，目标路由不存在时不报错；链接格式由设计定，
  要让 W6、W7 不改本任务的代码就能接上（可参照 W4 `app/components/bell-targets.ts` 的做法）。
  卡片遵守 UI 硬规则（卡片优先、低密度），信息量由设计决定。阶段名必须来自 R-04 的标签表，不在组件里另写一份中文。
- **R-03 ⌘K 能跳到任意项目、任务，以及「新建任务卡」。**
  键盘可达，macOS 上用 ⌘K，其它平台上用 Ctrl+K；可以输入过滤，用方向键和回车选择，Esc 关闭。「新建任务卡」的页面是 W6，本任务只给入口和目标链接，
  规则同 R-02。候选数据从哪里来（`project.list`、`dashboard.get`、按需 `project.get`）、已完成或已取消的任务是否列入，
  都由设计决定并写明理由。
- **R-04 `labels.ts` 包含需求、Bug 两张阶段表（计划 §4.3），有测试与内核节点列表交叉核对。**
  文件位置：`apps/web/app/lib/labels.ts`。计划 §4.3 原文：
  | 类型 | 阶段 |
  |---|---|
  | 需求 | 整理 · 设计 · 批准设计 · 实现 · 审计 · 合并 · 完成。实现阶段另显示里程碑进度 `已完成 3 / 5` |
  | Bug | 定位 · 修复 · 复核 · 合并 · 完成。修复阶段另显示 `修复第 2 / 3 轮` |

  - 内核节点的唯一来源是 `crates/autome-domain/src/task.rs` 的 `enum Node`（serde `snake_case`）。`Node::ALL` 有 15 个需求节点：
    `intake, design, review, adjudicate, await_design_approval, implement, gate, audit, retro, rebase, merge_gate,
    await_merge, merging, main_check, cleanup`；`Node::BUG` 有 7 个 Bug 节点：`locate, repro_check, fix, fix_gate, fix_review,
    await_info, not_a_bug`。任务状态见同文件 `TaskState`，序列化形状为 `{"state": "draft"|"queued"|"active"|"paused"|"stopped"|
    "failed"|"done"|"cancelled", "node"|"resume"|"at": …}`。
  - 「交叉核对」的意思是：测试**读取并解析** `task.rs` 源码（只读），断言每个内核节点在对应类型的阶段表里都有归属，
    表里也没有内核不存在的节点。内核新增节点而标签表没跟上时，测试必须变红。先例：`apps/web/test/read-gate.test.ts`
    解析 `crates/automed/src/dispatch.rs` 的 `READ_METHODS`。
  - 节点映射到阶段的规则是：`gate`、`review` 等节点归入哪个阶段，`rebase`、`merge_gate`、`main_check` 等归入「合并」还是别的阶段，`await_info`、
    `not_a_bug` 怎么显示。这套规则由设计给出。可参考 `apps/desktop/renderer/lib/labels.js` 的 `NODE_LABELS`、`FLOW`、`GATE_STONE`
    （那是需求 Loop 的旧版细粒度节点表，不含 Bug 节点）。未知节点按 desktop 的约定回退为原始值，不能猜。
  - 「中文标签表」除了阶段表，还应包含本任务页面实际用到的节点名、状态名、类型名（需求、Bug）。页面里不得散写中文状态名。
    W6、W7 会复用这张表；本任务只收录本任务用到的条目，加上计划 §4.3 的两张阶段表。
- **R-05 内核离线时显示横幅，写操作按钮置灰。**
  内核状态来自 W3 的 `core` 消息（`useLiveEvents()` 已经返回 `core`）。它和现有的「连接中断，正在重连」（WebSocket 断开）是两件事，
  两者的显示怎样并存或合并由设计决定。本任务页面上的写操作入口（如果有的话，例如「新建任务卡」入口）在内核离线时置灰。
  另外要提供一个可复用的「是否允许写」判定（hook 或 context），W6、W7 的按钮会用到它。
- **R-06 `server/http.ts` 的 `createRequestHandler` 加上 `getLoadContext`，把 W2 的 `gateway` 交给各页面的 loader 与 action（W2 的 Backlog B-03）。**
  - 开发模式、生产模式两处 `createRequestHandler` 都要加上；loader、action 拿到的类型要靠 TypeScript 检查（`AppLoadContext` 的模块扩充，或者
    React Router 8 的 `RouterContextProvider`/`createContext`）。用哪一种，取决于 RR 8.5 的实际 API：设计必须先在本机
    `node_modules` 里核对 `@react-router/express` 8.x 的 `getLoadContext` 签名，以及中间件或上下文的默认行为，并逐字引用类型定义原文，
    不能凭 v7 的文档推断。
  - loader 走 gateway 时，必须经过读白名单（`gateway.read({method, params})`），不能绕过白名单直接 `core.call`。
  - W4 的 Backlog B-02（以及计划 W6 验收 6）预期 W6 会经同一个 `getLoadContext` 拿到面板库（`store.unreadByTask`）。本任务只承诺交出 `gateway`；
    context 的形状要给 W6 留出加 `store`/`notifications` 的位置。现在是否顺手一起交出，由设计决定并写明理由；即使交出，
    本任务也不实现角标。
  - `createHttpServer` 没拿到 gateway 时（现有测试有这种用法），页面的 loader 怎么表现（例如按「内核离线」显示），由设计给出，
    且不能让现有测试变红。
- **R-07 迁移 animal-island 设计 token。** 来源是 `apps/desktop/renderer/style.css` 的 Layer 1 `:root` token（`--animal-*`、`--tile-*`，
  共约 23 行声明），字体 `apps/desktop/renderer/fonts/nunito/Nunito-Variable.ttf`（附 `OFL.txt`）。
  - 落点：`apps/web/app/styles/`。字体随包本地提供，不走 CDN（面板是本机服务，可能离线；以后 W11 的 Electron 薄壳也会用到）。
    字体文件放到哪里、怎么引用，由设计决定，许可文件要一并带上。
  - 只迁移 token，以及本任务组件用到的样式，**不整份搬运** desktop 的 869 行 style.css（那份样式是给旧屏幕写的，W11 会删掉）。
  - desktop 的暗色调色板（`[data-theme="dark"]`）是否一起迁移、是否跟随 `prefers-color-scheme`，由设计决定。主题切换界面不在本任务范围内。
  - 现有内联样式（`LiveBanner`、W4 `bell.tsx` 的 `#fff`、`#d1d5db` 等）改为使用 token。改动时 W4 铃铛的现有测试要保持通过；
    若断言依赖了内联样式，逐条说明理由。
  - animal-island-ui 设计规则的要点（来自其 `docs/design-system/design-rules.md`，desktop 迁移时遵守过）：不用纯黑 `#000`，焦点环不用蓝色
    （用 `--animal-focus-yellow`），圆角至少 12px，3D 阴影只给主按钮，不用 emoji 当图标。上游以 CC BY-NC 4.0 授权，
    样式文件头部要注明出处。
- **R-08 UI 硬规则（计划 §2，原文）。** 全中文，专有名词保留原文；卡片优先、低密度；1512×944 下**页面本身不出现纵向滚动**，
  看板列、时间线、日志面板可以在内部滚动；视觉沿用 animal-island 风格。项目很多时，左侧项目列表在内部滚动。
  怎样用会话自己能跑的命令检查「不出现页面滚动」由设计给出；做不到的部分写进 `## 人工验收清单`（逐页的 Playwright 一屏验收是 W12 的事）。

## 3. 范围

**可以改动的目录与文件（计划 W5「目录」，外加接线必需的最小改动）：**

- `apps/web/app/root.tsx`：外壳布局（左侧项目栏、顶栏、内容区）、根 loader，以及把 W4 铃铛从 `position: fixed` 收进顶栏；
- `apps/web/app/components/`：外壳、项目列表、任务卡（首页总览用的简版）、⌘K 面板、离线横幅等新组件；可以为挂进外壳、改用 token
  而改动 W4 的 `bell*.ts(x)`，但不得改变铃铛的数据流与已读语义；
- `apps/web/app/styles/`：token、字体、全局样式；
- `apps/web/app/lib/labels.ts`：新建；
- `apps/web/app/routes/_index*`：首页总览。`apps/web/app/routes.ts` **只把** `index` 从 `routes/home.tsx` 改指 `routes/_index*`，
  并删除占位的 `routes/home.tsx`；
- `apps/web/app/lib/live.tsx`：**只在**设计决定把离线横幅、写判定放到这里时改，不改 `live-client.ts` 与 `live-routes.ts` 的语义；
- `apps/web/server/http.ts`：**只加** `getLoadContext`（R-06），不改已有路由、安全中间件与 `/api/*` 语义；
- `apps/web/server/main.ts`：**只在** `getLoadContext` 需要额外的对象（例如 R-06 提到的 store）时改接线，一处即可；
- `apps/web/test/`：新测试与 `test/fixtures/` 下的替身；已有测试只在接线引起必要变化时改，并逐条说明理由；
- 类型扩充文件（例如 `apps/web/app/env.d.ts` 或 RR 约定的位置）：只用于 R-06 的 context 类型。

**明确不做（出现即越界）：**

- 项目页的四列看板、任务卡完整版、新建任务卡表单、附件上传（W6）；
- 任务详情页、阶段条的完整呈现、各种写操作（批准、合并、开始……）（W7）；
- 会话日志（W8）；添加/移除项目、Onboarding、环境页（W9）；`autome web`、`server.json`（W10）；Electron（W11）；Playwright e2e（W12）；
- 「等你处理」通知角标（W6，数据来自 W4 的 `store.unreadByTask`）；
- 主题切换、偏好设置界面、多角色或路由图（计划 D6）；
- 改 W2 的白名单与安全检查语义、W3 的事件总线语义、W4 的通知规则；任何令牌、Cookie、登录会话（计划 §5.5）；
- 新增 npm 依赖。⌘K、图标、样式都用现有依赖手写（React 19、React Router 8）。设计认为确实需要新依赖时，作为争议项交给用户决定。

## 4. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`、`scripts/`、`.github/`、`.autome/`。** 这些只能**读**或**运行**，
   比如解析 `task.rs` 做交叉核对、从 desktop 复制 token 与字体（复制到 apps/web，不改原文件）、构建并运行 `target/debug/automed`。计划 D1 规定 Web 任务不碰 `crates/`。
2. 只在当前 worktree 内工作，不切分支、不推送、不操作 `.worktree/` 下的其他目录，也不启动下一个会话（`AGENTS.md`）。
3. **前端不维护状态副本**（计划 §5.2）：项目列表、计数、首页卡片全部来自 loader，实时更新靠 `useLiveEvents` 加 `liveShouldRevalidate`
   让 loader 重新验证。`apps/web/test/live-routes.test.ts` 会检查 `app/routes/` 下每个带 `loader` 的路由模块都导出了
   `shouldRevalidate = liveShouldRevalidate(...)`。`root.tsx` 不在这个扫描范围内，但它的 loader 同样要声明自己对哪些事件重验，并有测试。
4. **阶段、状态的中文只在 `labels.ts` 里写一份**。页面代码按内核给的节点或状态查表，不自己推断进度（计划 §4.3：「由内核的节点映射而来，前端不自己推断」）。
5. loader、action 访问内核只经 `gateway.read`、`gateway.write`，不得绕过白名单。
6. TypeScript 严格类型，`npm run lint`（`--max-warnings 0`）、`typecheck`、`test`、`build` 全部通过；`skipped` 必须为 0。
7. 计划 §7.4 止损：设计文档的里程碑**不超过 6 个**。每次修订设计后都要重新核对这个上限（W4 L-04）。

## 5. 项目背景摘要（生成时）

**仓库：** autome-v2，单仓库。Autome = Rust 内核 + `autome` 命令行 + Web 面板（进行中）+ Electron 外壳（将改成薄壳）。
它用自己的 Loop 开发自己，`.autome/config.toml` 配置 `[loop] parallel = 3`、`[gate] command = "sh scripts/ci-local.sh"`（30 分钟）、
`[retry] attempts = 2`。

- `crates/autome-domain`：纯域，包括任务状态机 `task.rs`（`Node`、`TaskState`、`TaskKind`）、`bug_loop.rs`、状态块解析等。
- `crates/automed`：内核进程，framed JSON-RPC over stdio。`src/dispatch.rs` 的 `READ_METHODS`（17 个）包括 `project.list`、
  `project.get`、`task.get`、`dashboard.get`、`events.since` 等。要点：
  - `project.list` → `{projects: [{project, counts, slots_in_use}]}`，其中 `project` 含 `id`、`display_name`、`path` 等；
  - `project.get({project_id})` → `{project, tasks: [task_json], counts, config, violations, rules}`；
  - `dashboard.get` → 跨项目的 waiting 与 running 两组 `{project_id, project_name, task, pending_decisions, session, retry}`，以及 `environment`；
  - `task_json`：`id, kind, slug, title, request, state, budget_n, branch, created_at, completed_at, merge_commit, archived,
    protocol_ref, metrics, doc_dir`，Bug 额外带 `fix_round`。

  引用 RPC 返回值时，以实测时逐字粘贴的原始 JSON 为准（W3 L-03）；上面这些只是摘要。
- `crates/autome-cli`：`autome` 命令行，与面板共用 `$AUTOME_HOME`（默认 `~/.autome`）。可用 `task new [--kind bug --brief - --draft]`、
  `task start`、`approve`、`reject`、`stop`、`project add <路径> --skip-onboarding`、`rpc <方法> '<JSON>'` 等在测试里驱动状态变化。
- `apps/web`：React Router 8.5 框架模式（`ssr: true`）+ express 5 自定义服务器，React 19，TypeScript ~6.0，Vite 8，vitest 5，
  ESLint 10，`ws` 8，`engines.node >= 22.22.0`。服务端 TypeScript 走 Node 原生类型剥离（不用 tsx）。现状：
  - `server/main.ts`：`startServer` 依次打开 `openWebStore`，创建 `createEventBus`、`CoreSupervisor`、`createNotifier`、
    `createNotificationApi`、`createGateway`、`createHttpServer({mode, gateway, notifications})`，然后监听、`registerUpgrade('/ws', …)`、启动 ticker；
  - `server/http.ts`：`securityMiddleware` 放在最前；提供 `/healthz`、`POST /api/read`、`POST /api/write`、`GET /api/notifications`、
    `POST /api/notifications/read`；开发模式用 Vite middleware 加 `createRequestHandler({build: () => vite.ssrLoadModule(...), mode})`，
    生产模式用静态资源加 `createRequestHandler({build, mode})`，**两处都还没有 `getLoadContext`**；
  - `server/gateway.ts`：`createGateway({core, home, uploads})` → `{core, registerLocalOp, read(request), write(request)}`，返回
    `{status, body: {ok, payload?|error?, code?}}`；
  - `server/eventbus*.ts`（W3）、`server/notifier*.ts`、`server/store*.ts`（W4）、`server/security.ts`、`server/gates/*`、
    `server/core/*`、`server/ticker.ts`；
  - `app/root.tsx`：`Layout`（`<html lang="zh-CN">`）加 `App`（`useLiveEvents()` → `LiveBanner` + `NotificationBell` + `Outlet`）；
  - `app/routes.ts`：只有 `index("routes/home.tsx")`；`app/routes/home.tsx` 是占位页「面板骨架已就绪」；
  - `app/lib/live.tsx`（`useLiveEvents` 返回 `{connection, core}`，`LiveBanner` 只处理重连）、`live-client.ts`、
    `live-routes.ts`（`liveShouldRevalidate`、`byKind`、`bySubjectParam`）；
  - `app/components/bell*.ts(x)`（W4 铃铛，内联样式，`position: fixed` 右上角）；
  - 还没有 `app/styles/` 和 `app/lib/labels.ts`；
  - `test/`：33 个测试文件加 `fixtures/`（`fake-core.mjs`、`w4-ledger.sh` 等替身）。
- `apps/desktop`：旧 Electron 渲染进程，W11 前不动，本任务只读。迁移来源：`renderer/style.css`（token）、`renderer/fonts/nunito/`、
  `renderer/lib/labels.js`（节点中文名与流程石子）、`renderer/screens/{dashboard,projects}.js`（旧总览与项目列表的交互可作参考）。
- `docs/design/autome-2.0-desktop-ui.html`：2026-09-15 定稿的交互设计稿，animal-island 风格的视觉与文案参考。
- `protocol/`、`plugins/`、`scripts/`（`ci-local.sh` 等）：本任务不改。

**本机（生成时）：** Node v26.5.0；本 worktree 里还没有 `target/` 和 `apps/web/node_modules`（首次需要 `cargo build -p automed -p autome-cli`
和 `cd apps/web && npm ci`）；`~/.local/bin/autome` 存在，`~/.local/bin/automed` 不存在。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/web`、`npm run build --prefix apps/web`
- Desktop（确认没被误改）：`npm test --prefix apps/desktop`
- 内核与命令行：`cargo build -p automed -p autome-cli`；`cargo test --workspace --all-targets`
- 全部：`sh scripts/ci-local.sh`（宿主门禁跑的就是它，分 rust、desktop、contract、web 四段；web 段只在 `apps/web/node_modules` 不存在时才执行 `npm ci`）
- CI：`.github/workflows/ci.yml`，由 `apps/web/test/ci-config.test.ts` 逐字守卫。仓库没有 remote，真正会跑的只有 ci-local。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，没有具体规则）；仓库内没有 `docs/agent-project-profile.md`；
- `docs/development/requirements.md`、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2 决策 D1–D6 与 UI 硬规则、§3.2 看板列、§4.3 阶段表、§5.2 WebSocket 协议、
  §5.5 本机安全、§6 W5（以及 W6、W7、W9、W12 的边界，用于判断哪些不归本任务）；
- 已实现的上游设计：W2 `docs/apps-web-的服务端托管-automed-2/apps-web-的服务端托管-automed-2.md`（网关、白名单、Backlog B-03），
  W3 `docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md`（总线、`core` 消息、路由重验），
  W4 `docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器.md`（铃铛、面板库、Backlog B-02）。

**前车之鉴（W1–W4 的复盘教训，供设计参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`，常伴随 watcher EMFILE），审计轮与宿主门禁可以。W1–W4 四个任务都撞上了。
  计数归列、标签表、交叉核对、⌘K 过滤与键盘导航、`getLoadContext` 的取值，都尽量写成纯函数，或用 `createMemoryRouter`、组件渲染来测，
  不依赖监听端口；需要监听的用例（真实服务端的 loader 经 gateway 取数、跨进程改状态后计数更新）集中放在少数文件里。
  实现轮被环境阻断时，验收契约的每一段分别报出各自的退出码（不要用 `&&` 串成一条），写明被拦住的用例数和不依赖监听那部分的结果，交审计轮复验。
- 驱动真实内核让任务进入 `failed` 的夹具，必须显式写 `retry attempts=0`、`backoff_seconds=0`（W4 L-02）。
- 调用真实 `autome`/`automed` 进程的测试，要在证据里写出 `sh scripts/ci-local.sh` 全程负载下的实测耗时，超时上限不小于实测值的 2 倍（W4 L-05）。
- 引用实测数据结构时逐字粘贴原始 JSON（W3 L-03）。
- 验收契约里每条负向对照（「改坏 X → 点名的测试变红」），实现轮都要实跑，并在证据里写出退出码；实现轮不得改动「M-xx 验收契约」的文字（W3 L-05）。
  负向对照全部还原后，在仓库根执行 `git status --porcelain`，输出必须为空，并写进证据。
- 裁决轮如果为解决评审意见引入设计里原本没有的新机制，要在 `## 争议项` 列出「放宽范围或接受取舍」的简化备选，交给用户选（W4 L-03）。
- 用户纳入的 Backlog 条目，如果实现由别的里程碑交付，就并进那个里程碑的验收契约，不单立里程碑（W4 L-04）。
- 宿主门禁在基线上可能本来就不是全绿（W4 的 B-05、B-06 是 `crates/` 里的时序类失败）。与本任务无关、按范围约束不修的失败，在 `## Backlog`
  写一条，包括失败用例全名、断言原文和机制，不在本任务里修。同一失败签名连续出现、本轮又没有可改的代码时，在 `## 争议项` 写明需要人在宿主上执行的命令与工作目录（W3 L-02）。
- 报验收结果时，要报 Web 与 desktop 的**用例总数**及其相对上一轮基线的差，`skipped` 为 0（本机有二进制时，真实内核用例不允许跳过）。
