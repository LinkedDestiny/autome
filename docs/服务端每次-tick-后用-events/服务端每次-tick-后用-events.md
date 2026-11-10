# Web 事件总线（W3）：tick 后 events.since 拉取、/ws 推送与 useLiveEvents 按事件重验路由

status: 实现中
design-round: 1/15
implementation-round: 9/20
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 为当前 worktree 提供锁定的 ws 8.22.2 与 @types/ws 8.18.2 下载能力或完整缓存，安装后原样重跑 sh scripts/ci-local.sh；通过后再继续 M-01 验收

任务文件：`docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events-task.md`。
本任务的规则见 `docs/服务端每次-tick-后用-events/protocol/loop-protocol.md` 与
`docs/服务端每次-tick-后用-events/protocol/session-protocol.md`。

## 背景与现状

以下都是设计轮在基线 `e53e3a6`（任务基线 `30e5d51` 之上只多了文档提交）上读源码或实测得到的。

**内核侧（只读，不改 `crates/`）**

- `events.since`（`crates/automed/src/dispatch.rs:2325-2342`，`store.rs:610-617`）：参数 `after_seq`（缺省 0）、
  `limit`（缺省 200，上限 1000）；返回 `{events:[{seq,kind,subject,payload}], latest_seq}`，`WHERE seq > after`
  升序。没有时间戳字段。在 `READ_METHODS`（`dispatch.rs:2388-2406`）里，不占驱动锁。`after_seq` 传一个很大的数
  （命令行用 `u64::MAX`，`autome-cli/src/main.rs:700`）可以只读 `latest_seq`。
- `subject` 就是 `events.subject_id`：任务类事件是任务 id（`T-1`），项目类是项目 id（`prj-…`）。
  全部 kind（从 `append_event` 调用点核对）：
  - subject = 任务 id：`task.created` `task.updated`（payload `{state, trigger}`）`task.retro_started`
    `task.decided` `task.archived` `task.restored` `task.snapshot` `task.head` `task.lessons` `task.metrics`
    `task.protocol_failure` `task.resume_input`、`session.*`（started/retry/rerouted/deferred/outcome_held/
    outcome_held_applied/usage_missing/manual_retro）、`runtime.cooldown` `milestone.contradicted`
    `integrity_warning` `guard.failed` `guard.warning` `gate.started` `gate.skipped` `gate.reused` `gate.result`
    `merge.auto` `merge.rebased` `merge.repo`；
  - subject = 项目 id：`project.added` `project.removed` `project.onboarding` `project.alert` `rule.*`、
    onboarding 会话的 `session.started`；
  - 其它：`config.updated`（`global`）、`config.changed`（角色或项目）、`project.alert_dismissed`（告警 id）、
    `env.probing`（`environment`）。
- 账本是 `$AUTOME_HOME/state/automed.sqlite3`，WAL + 5 秒 busy timeout。`autome` 命令行**不连** automed，
  直接链接库、自己开 SQLite；写命令拿 `<db>.driver.lock`（最多等 10 秒），`scheduler.tick` 只 try-lock，
  拿不到回 `skipped:true`。
- sidecar 的 Event 帧（`automed/src/main.rs:114-125`）只针对本进程发出的命令；`scheduler.tick` 不发任何帧
  （`dispatch.rs:2167-2182`）。帧的 `event_type` 与账本 `kind` 不总相同（账本 `project.onboarding` 在帧里是
  `project.updated`，`dispatch.rs:936`）。这两点都说明推送只能以 `events.since` 为准。
- **命令行如何改任务状态又不拉起真实 Agent（实测，输出在 `.autome/output/w3-cli-exp*.txt`）**：`autome task new`
  会在命令行进程里顺带 tick 并尝试启动会话，运行时二进制取 `AUTOMED_CLAUDE_BINARY`/`AUTOMED_CODEX_BINARY`，
  否则查 PATH（`launcher.rs:888-899`）。在 `PATH=/usr/bin:/bin`（没有 claude/codex）下：
  `project add <仓库> --skip-onboarding` → `task new "…" -p <仓库>` 得到 `task.created`、`task.updated`(active)、
  `task.updated`(failed，原因 PATH 中找不到 claude)，没有任何进程被拉起；之后 `scheduler.tick` 是空操作；
  `autome cancel T-1` 追加 `task.updated`（`{state:"cancelled", trigger:"cancel"}`，subject `T-1`）与 `task.metrics`。
  注意：在 zsh 里、cwd 含非 ASCII 字符时命令行会因 `_` 环境变量 panic（`launcher.rs:839`）；从 Node 用构造好的
  env 启动子进程没有 `_`，不受影响。
- `cargo test --workspace --all-targets` 会构建 `target/debug/autome`（`crates/autome-cli/tests/cli.rs` 用
  `env!("CARGO_BIN_EXE_autome")`），所以 `scripts/ci-local.sh` 的 Web test 之前已有 `autome`；`ci.yml` 的 `web`
  job 只有 `cargo build -p automed`，拿不到 `autome`。

**Web 侧（`apps/web`）**

- `server/main.ts`：`new CoreSupervisor({home, binaryPath, log})`（没传 `onStatus`）、`createTicker({...})`（没传
  `onTick`），返回 `{server, gateway, registerUpgrade, close}`。
- `server/ticker.ts`：每 3 秒；`pending`/`isBusy()` 防叠加；`onTick?.(report)` 在每次成功返回后**同步**调用，
  `skipped:true` 也会调用。`CoreSupervisor`：状态 `starting|connected|restarting|offline|stopped`，
  `onStatus(status)` 每次 `setStatus` 都调；崩溃重启见 `core/restart-policy.ts`（5 秒内退出算快速失败，
  连续 3 次停在 `offline`，否则 1 秒后重启）。
- `server/security.ts` 的 `installUpgradeGuard`：先校验 Host 与 Origin（不合格回 403），再把升级交给**安装时已有**
  的监听器（开发模式下是 Vite HMR）以及 `registerUpgrade(path, …)` 注册的路径处理器；`security.test.ts` 断言
  `upgrade` 监听器只有 1 个。Vite 8.3.7 的 HMR 监听器（`vite/dist/node/chunks/node.js:24557`）只接
  `sec-websocket-protocol` 为 `vite-hmr`/`vite-ping` 且路径为 base（`/`）的升级，其余只挂一个空的 error 处理，
  不碰 socket——所以 `/ws` 与 HMR 可以共用同一个守卫。
- `app/`：`root.tsx`（Layout + `<Outlet/>`）、`routes.ts` 只有 `routes/home.tsx`（占位，没有 loader）。
  依赖里没有 jsdom、testing-library，也没有任何 WebSocket 服务端库。
- React Router 8.5.1：`ShouldRevalidateFunctionArgs` 不带「为什么重验」；实测（`.autome/output/w3-probe/rr.mjs`）
  `createMemoryRouter` 在没有 DOM 的 Node 里能跑，`router.revalidate()` 在调用当下**同步**调用各路由的
  `shouldRevalidate`，返回 false 的路由 loader 不跑。
- 基线测试：Web 16 个文件 175 个用例全过、0 跳过；desktop 104 个用例、103 过、1 跳过（基线就有，不归本任务）。
  `write-gate.test.ts` 35 个用例；两处原始 NUL 在第 117 行（用例「a null byte is refused anywhere in a string」）
  与第 398 行（用例「looksLikeAPath catches the shapes that matter and allows an ordinary name」）。

## 目标与非目标

**目标**（对应任务要求）

1. R-01：`/ws` 上的 `hello`/`event`/`core`/`reload` 与 `resume` 按计划 §5.2 实现，断线续传不丢不重；
   信封与分发方式给 W4 的 `notification`、W8 的 `log`/`subscribe-log`/`unsubscribe-log` 留好挂点。
2. R-02：真实 `automed` + 另一进程的 `autome` 命令行改任务状态，WebSocket 客户端在一个 tick 周期内收到事件。
3. R-03：内核离线与恢复都推 `core`；恢复后从游标续读，补齐离线期间的事件。
4. R-04：`useLiveEvents()` 按 `kind`/`subject` 只让受影响路由的 loader 重验；断线指数退避重连并在顶部提示。
5. R-05：推给浏览器的事件只来自「tick 之后从游标 `events.since`」。
6. R-06：`write-gate.test.ts` 两处原始 NUL 改成 `\u0000` 转义。

**非目标**：通知与 `web.sqlite3`/`event_cursor`（W4）；业务路由与页面、内核离线横幅与写按钮置灰（W5–W7）；
`getLoadContext`（W5）；日志跟随（W8）；`server.json`/`autome web`（W10）；Electron（W11）；Playwright（W12）；
任何令牌/Cookie；改 W2 的 Host/Origin 语义；碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`。

## 方案

### 1. 文件与职责

| 文件 | 内容 | 是否需要监听端口 |
|---|---|---|
| `server/eventbus-protocol.ts` | 消息类型、服务端消息编码、浏览器消息解析、内核状态映射（纯函数） | 否 |
| `server/eventbus.ts` | `createEventBus({call, log})`：游标、分页拉取、连接表、续传、`core` 推送、扩展点 | 否（连接是抽象的 `{send, close}`） |
| `server/eventbus-ws.ts` | 用 `ws` 的 `WebSocketServer({noServer:true, maxPayload:64KB})` 把升级接成总线连接 | 是 |
| `server/main.ts` | 只接线（见 §5） | — |
| `app/lib/live-client.ts` | `createLiveClient(...)`：浏览器端连接状态机、`lastSeq`、`resume`、退避（纯逻辑，WebSocket 构造器与定时器注入） | 否 |
| `app/lib/live-routes.ts` | `liveShouldRevalidate(matcher)`、常用匹配器、`revalidateForEvents`/`revalidateAll` | 否 |
| `app/lib/live.tsx` | `useLiveEvents()` 钩子与 `<LiveBanner/>` | 否 |
| `app/root.tsx` | 只挂 `useLiveEvents()` 与 `<LiveBanner/>` | — |

消息类型只在 `eventbus-protocol.ts` 定义一次，`app/lib/live-*` 用 `import type` 引用（类型导入在构建时擦除，
不会把服务端代码打进浏览器包）。

### 2. WebSocket 服务端依赖：`ws`

新增 `dependencies.ws: ^8.22.2` 与 `devDependencies["@types/ws"]: ^8.18.2`。实测：`npm view ws version engines`
→ `8.22.2`、`{ node: '>=10.0.0' }`，与 `engines.node >=22.22.0` 兼容；`ws` 没有运行时依赖，只有两个可选 peer
（`bufferutil`、`utf-8-validate`，不装）；`npm view @types/ws version` → `8.18.2`。不手写的理由：服务端要正确处理
RFC 6455 的握手、客户端掩码、分片、控制帧、关闭握手和 UTF-8 校验，手写这些的测试量比本任务的业务本身还大；
Vite 8 自己的 HMR 就用的同一份 `ws` 代码（打包在 vite 内部、不导出），选它不引入新的行为差异。测试里的
WebSocket 客户端也用 `ws`（它能设置 `origin` 选项，过 W2 的 Origin 校验）；浏览器用原生 `WebSocket`。

### 3. 服务端事件总线

**游标与拉取。** 总线在内存里保存 `cursor`（已广播的最大 `seq`）。

- 初始化：内核第一次变成 `connected` 时调 `events.since({after_seq: Number.MAX_SAFE_INTEGER})` 取 `latest_seq`
  作为游标，不推历史。初始化之前 `hello.seq` 为 0。
- 服务重启后游标从当时的 `latest_seq` 开始。理由：游标不落盘（`event_cursor` 是 W4 的表）；重启前连着的浏览器会带着
  自己的 `lastSeq` 发 `resume`，续传直接从内核账本读（账本 append-only、持久），所以重启期间的事件仍然补得回来，
  缺口超过 1000 条才降级为 `reload`。
- `pull()`：单飞。正在拉时再被调用只置一个「再拉一次」标记，本次结束后补拉一次；任何时刻最多一个 `events.since`
  请求在途。每次循环 `events.since({after_seq: cursor, limit: 1000})`，按序广播这一页、推进游标，直到本页为空或
  最后一条 `seq >= latest_seq`。失败时写中文日志（`[事件] 拉取失败：…`），游标不动，下次从原处续拉。
- 若 `latest_seq < cursor`（账本被换掉）：游标重置为 `latest_seq`；对每个连接，取消尚未完成的续传（丢弃排队区、
  `state = live`）、`sentSeq = latest_seq`，然后发 `{type:"reload"}`。此后该连接上只会出现 `seq > latest_seq` 的事件，
  即使这个数字小于换账本之前的 `sentSeq`——`sentSeq` 与 `cursor` 一起切到新序列，不留旧值。
- **`reload` 的统一语义（服务端承诺 + 客户端动作）**：服务端发出 `reload` 的同时保证该连接上之后的 `event` 都大于
  它此刻的 `sentSeq` 基线（上面一条与情形表里的两种 `reload` 都按此设置）；客户端收到 `reload` 即**放弃检查点**
  （`lastSeq` 置空，见 §4）并整体重验，下一条 `event` 重建检查点。信封仍是任务给定的 `{type:"reload"}`，不加字段。
- 触发：`onTick` 里 `void bus.pull()`（不 await，ticker 的 `pending` 照常复位，慢拉取不拖住下一次 tick）；
  内核状态变成 `connected` 时立即 `pull()` 一次。`scheduler.tick` 的回复里也有 `latest_seq`，本任务不依赖它。

**R-05：sidecar Event 帧不用。** `CoreSupervisor` 构造 `AutomedSidecar` 时没有传 `onEvent`，而 `server/core/`
不在本任务可改范围内；帧的 `event_type` 又与账本 `kind` 不一致。所以本任务完全不接 Event 帧，事件只有
`events.since` 一条来源。以后若要让本进程的写操作更快刷新，只需在写成功后调用 `bus.pull()`——拉取是单飞且按游标
去重的，不会产生第二条来源或重复推送。

**连接。** 每个连接有 `sentSeq`（已发给它的最大事件 `seq`）、`state`（`live` | `resuming`）和一个排队区。

- 建立后立即发 `{type:"hello", seq: cursor, core}`，`sentSeq = cursor`，`state = live`。
- 广播一个事件时：`live` 的连接只发 `seq > sentSeq` 的事件并更新 `sentSeq`；`resuming` 的连接先放进排队区。
  所以同一连接上的 `event` 永远严格递增、不重复。

**续传 `{type:"resume", after}`**（`after` 必须是非负整数，否则忽略）的情形表：

| 情形 | 处理 |
|---|---|
| `after == sentSeq` | 什么都不发 |
| `after > sentSeq` 且 `after <= latest_seq` | 客户端在别处已收过，`sentSeq = after`，不发 |
| `after > latest_seq`（换了账本；含服务重启后账本也被换掉的重连） | 发 `reload`，`sentSeq = latest_seq`、`state = live`；客户端据 `reload` 清掉旧检查点，所以之后 `seq = latest_seq + 1…` 的事件（可能远小于旧 `lastSeq`）会被接收 |
| hello 之后已经给它发过事件（`sentSeq > hello.seq`），又收到 `after < sentSeq` | 发 `reload`（补发会破坏递增；客户端整体重验即可；`sentSeq` 不变，之后的 live 事件照常 `seq > sentSeq`） |
| `latest_seq - after > 1000` | 只发 `reload`，`sentSeq = latest_seq` |
| `latest_seq - after <= 1000` | `state = resuming`，从内核分页读 `(after, latest_seq]` 逐条发出（`seq > after`），`sentSeq` 推到最后一条；然后把排队区里 `seq > sentSeq` 的事件按序发出，回到 `live` |
| 处理时内核不可用（`call` 抛错） | 保持 `resuming`、记住 `after`，下一次成功 `pull()` 之后重做 |

「缺口」按 `latest_seq - after` 计；恰好 1000 条走补发，1001 条走 `reload`。`reload` 的消息是 `{type:"reload"}`。

**R-03 内核状态 → `core` 消息。** `onStatus` 接到 `bus.setCoreStatus(status)`，映射：

| `CoreStatus.state` | `core.status` | `reason` |
|---|---|---|
| `connected` | `connected` | 不带 |
| `starting` | `offline` | `内核正在启动` |
| `restarting` | `offline` | `内核已退出，正在重启：<status.reason>` |
| `offline` | `offline` | `status.reason`，没有则 `内核已离线` |
| `stopped` | `offline` | `服务正在关闭` |

只在 `(status, reason)` 与上一次推送不同时广播 `{type:"core", status, reason?}`；`hello.core` 取当前映射值。
变成 `connected` 时立即 `pull()`，游标留在内存，离线期间别的进程写进账本的事件从游标续读补齐、不丢不重。

**W4 / W8 的挂点。** 所有消息是带 `type` 字段的 JSON 对象。总线额外提供：

- `bus.subscribe(listener)`：进程内消费者按 `seq` 顺序拿到每个已广播事件（W4 的通知器从这里接，它自己的持久游标
  仍是 W4 的事）；
- `bus.broadcast(message)` / `connection.send(message)`：发非事件消息（W4 的 `notification`、W8 的 `log`），
  不参与 `seq` 过滤；
- `bus.onClientMessage(type, handler)` 与 `bus.onDisconnect(handler)`：W8 在这里注册 `subscribe-log` /
  `unsubscribe-log` 并在断开时清理订阅。未注册的 `type`、非法 JSON 一律忽略，不断开连接。

**关闭。** `bus.close()`：停止拉取，用 1001「服务正在关闭」关闭所有连接（升级后的 socket 不归 `closeAllConnections`
管，不关会拖住进程退出）。

### 4. 浏览器端

**`createLiveClient`**：连 `ws(s)://<当前 host>/ws`（浏览器自动带本页 Origin，过 W2 守卫）。

- 收 `hello`：若 `lastSeq` 非空，发 `{type:"resume", after:lastSeq}`；若 `lastSeq` 为空且这不是首次连接（已经连上过，
  检查点被 `reload` 清掉后断线），则 `lastSeq = hello.seq` 并 `onReload`（断线期间的事件无从补起，整体重验）；首次连接
  `lastSeq = hello.seq`；退避计数归零；状态 `open`。
- 收 `event`：`lastSeq` 非空且 `seq <= lastSeq` 丢弃；否则（`lastSeq` 为空时接受任意 `seq`）`lastSeq = seq` 并交给
  `onEvents`（同一轮宏任务内的事件合并成一批）。同一连接上服务端保证严格递增，所以去重只需要防「重连后重复」。
- 收 `reload` → **`lastSeq = null`（放弃检查点）**，再 `onReload`；收 `core` → `onCore`（W5 的离线横幅从
  `useLiveEvents()` 的返回值取）。账本被换掉（在线发现，或重连时 `resume.after > latest_seq`）两条路径都落在这条规则上：
  服务端把 `sentSeq` 切到新序列并发 `reload`，客户端清检查点，之后新序列里较小的 `seq` 正常接收并触发重验。
- 断开：状态 `reconnecting`，按 `reconnectDelay(n) = min(500·2ⁿ, 10000)` 毫秒（500、1000、2000、4000、8000、
  10000…）重连；`stop()` 后不再重连。首次连接中的状态是 `connecting`，不显示提示。

**按事件只重验受影响路由（R-04）。** React Router 的 `shouldRevalidate` 拿不到「为什么重验」，所以由
`live-routes.ts` 持有一个**待处理事件批**（只是事件，不是数据副本）：

- `revalidateForEvents(events, revalidate)`：把事件并入待处理批、调用 `revalidate()`，等它 settle 且期间没有更新的
  一次调用时清空。重验进行中又来一批时，新的 `revalidate()` 会中断上一次，此时待处理批是两批的并集，前一批选中的
  路由不会丢。`revalidateAll(revalidate)` 用于 `reload`。
- `liveShouldRevalidate(matcher)` 生成路由的 `shouldRevalidate`：有待处理批、且是纯重验（`currentUrl.href ===
  nextUrl.href`——**按值比较**；没有 `formMethod`）时，返回「批里是否有事件满足 `matcher(event, currentParams)`」（`reload` 时恒真）；其它情况
  （导航、表单提交、没有待处理批）原样返回 `defaultShouldRevalidate`，不改变路由的正常行为。
- 匹配器：`byKind(...prefixes)`（如 `byKind('project.', 'rule.')`）、`bySubjectParam(prefixes, param)`（如
  `bySubjectParam(['task.', 'session.', 'gate.', 'merge.'], 'taskId')`：kind 匹配且 `subject === params.taskId`）。
- **给 W5 起的路由：** 每个导出 `loader`/`clientLoader` 的路由模块都必须导出
  `shouldRevalidate = liveShouldRevalidate(...)`，否则它会被每个事件重验。M-03 加一个守卫用例扫描 `app/root.tsx`
  与 `app/routes/**`，不满足就报出文件名。建议映射：任务详情 `bySubjectParam(任务类前缀, 'taskId')`；项目页
  `byKind('project.', 'task.')`；首页总览 `byKind('task.created', 'task.updated', 'project.')`。
- 为什么不能写 `currentUrl === nextUrl`：React Router 8.5.1 的 `getMatchesToLoad`（`router.js:2065-2066`）对两者分别
  调 `history.createURL(...)`，得到两个不同的 `URL` 对象，同地址时对象比较也是 false，会掉进 `defaultShouldRevalidate`
  分支而绕过匹配器（评审实测：对象比较时无关的项目 loader 也跑；改成 href 比较后项目 0 次、任务 1 次）。
  同一地址的链接点击也会被判为纯重验，但只有存在待处理批（刚由事件触发）时才走匹配器，窗口极短，接受。
- 现有路由（`root`、`home`）都没有 loader，所以重验行为在测试里用 `createMemoryRouter` 搭桩路由、套用同一个
  `liveShouldRevalidate` 验证。

**`useLiveEvents()`**：在 `useEffect` 里用 `window.WebSocket` 建 `createLiveClient`，`onEvents` →
`revalidateForEvents(batch, revalidator.revalidate)`，`onReload` → `revalidateAll`；返回 `{connection, core}`。
SSR 时不连（effect 不跑），初始 `connecting`。`<LiveBanner connection>` 只在 `reconnecting` 时渲染
`<div role="status">连接中断，正在重连</div>`，固定在页面顶部。`root.tsx` 的 `App` 改为渲染 `<LiveBanner/>` + `<Outlet/>`。

### 5. `main.ts` 接线

1. `const bus = createEventBus({ call: (m, p) => core.call(m, p), log: console.error })`；
2. `new CoreSupervisor({..., onStatus: status => bus.setCoreStatus(status)})`；
3. `createTicker({..., onTick: () => { void bus.pull(); }})`；
4. `registerUpgrade('/ws', createWsUpgrade(bus))`；
5. `close()` 顺序：`ticker.stop()` → `bus.close()` → `core.stop()` → 其余照旧；返回值加 `bus`（测试用）。

### 6. CI

R-02 的测试要 `target/debug/autome`。ci-local 已经有（见背景），不改；`ci.yml` 的 `web` job 把
`cargo build -p automed` 改成 `cargo build -p automed -p autome-cli`，`ci-config.test.ts` 里逐字守卫的那一项同步改，
用例数不变。测试用 `process.env.AUTOME_BIN`，否则 `../../target/debug/autome`；找不到二进制时**失败**并给中文提示，
不跳过。

## 风险与验证安排

1. **R-02 跨进程测试不得拉起真实 Agent。** 集成测试（`test/eventbus-live.test.ts`）用 `process.execPath` 启动
   `node server/start.js` 子进程（开发模式），env 是**构造出来的最小集合**而不是 `process.env` 的拷贝：
   `PATH=/usr/bin:/bin`、`HOME` 与 `AUTOME_HOME` 都指向 `mkdtemp` 目录、`AUTOMED_BIN`=包装脚本、
   `AUTOME_WEB_PORT=0`、`NODE_ENV=development`，不带任何 `AUTOMED_*_BINARY`；命令行子进程用同样的 env。
   测试开头断言 `/usr/bin`、`/bin` 下没有 `claude`/`codex`，有就失败。临时仓库用 `/usr/bin/git` 建（带 `-c user.*`）。
   顺序：`project add <仓库> --skip-onboarding` → `task new` → 任务停在 `failed`；起服务、连 `/ws`；
   `autome cancel T-1`，从命令行退出开始计时。
2. **「一个 tick 周期」的判据。** 3000 ms 间隔加这一次 tick 与拉取本身的执行时间，断言 ≤ 3500 ms 收到
   `task.updated`/`T-1`/`payload.state === "cancelled"`，再等 3500 ms 确认这个 `seq` 只出现一次、全程 `seq` 严格递增。
   实际延迟写进证据。
3. **R-03 杀内核。** `test/fixtures/automed-pidfile.sh`：把 `$$` 写进 `$AUTOME_HOME/automed.pid` 再
   `exec "$W3_REAL_AUTOMED"`（真实路径由测试放进子服务 env，sidecar 继承）。测试 `kill` 这个 pid，
   断言收到 `core offline`（中文原因）→ `core connected`，之后命令行的改动照常推到。
4. **实现轮可能不能监听 127.0.0.1（W1、W2 都撞上过）。** 需要监听的用例全部集中在 `eventbus-live.test.ts`；
   编解码、续传情形、状态映射、退避、路由映射全是纯函数或假 socket 测试。被拦时按验收契约分别运行、分别报退出码，
   写明被拦用例数与其余结果，交审计轮复验。
5. **框架模式与内存路由的差别。** 单元测试验证的是路由核心的 `shouldRevalidate` 选择；框架模式按同一个导出决定向
   服务端取哪些路由的数据。真浏览器里的效果列入人工验收 H-01。
6. **HMR 与 `/ws` 共存**（硬性约束 4）在 `eventbus-live.test.ts` 用真开发服务验证：从 `/@vite/client` 源码取
   `wsToken`，用 `vite-hmr` 子协议连 `/?token=…` 收到 Vite 的 `connected`，同时 `/ws` 收到 `hello`；
   `security.test.ts` 已有的「只保留一个 upgrade 守卫」继续成立。
7. 每个里程碑都要跑 `npm test --prefix apps/desktop`，基线 104 个用例、103 过、1 跳过，不得退化；
   并确认 `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 为空。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 最小端到端：分页拉取的事件总线、/ws 升级、main 接线、命令行跨进程集成测试 | 0 | eventbus-e2e |
| M-02 | 开放 | 断线续传、reload、core 消息与恢复补拉、W4/W8 挂点 | 0 | eventbus-resume |
| M-03 | 开放 | 浏览器端 live 客户端、按事件重验路由、重连提示与 root 接线 | 0 | live-routes |
| M-04 | 开放 | write-gate.test.ts 的原始 NUL 改为 \u0000 转义 | 0 | nul-escape |

## 里程碑说明

**M-01**（R-01 的 hello/event、R-02、R-05；前置：无）。新增 `ws`/`@types/ws`；`eventbus-protocol.ts` 先实现
`hello`/`event` 编码与内核状态映射；`eventbus.ts` 实现游标初始化、单飞分页 `pull()`、连接与 `sentSeq` 过滤、
`subscribe`、`close()`；`eventbus-ws.ts`；`main.ts` 第 1–5 步接线；`ci.yml` 与 `ci-config.test.ts` 的一处改动；
`test/fixtures/automed-pidfile.sh`。新测试：

- `test/eventbus.test.ts`（8，假 `call` 与假连接）：①首次连接用 `latest_seq` 初始化游标、不推历史；
  ②2500 条分 3 页（`limit` 1000）全部按序广播、游标到最后一条；③拉取中再触发只合并成一次补拉，`call` 从不并发；
  ④拉取失败写中文日志、游标不动、下次从原游标续拉；⑤同一段被拉两次时每个连接只收到一次、严格递增；
  ⑥新连接先收 `hello`（`seq` = 游标、`core` 为映射值），之后只收更大的 `seq`；⑦`subscribe` 按序收到、退订后不再收；
  ⑧`close()` 后不再调用 `call`，所有连接以 1001 关闭。
- `test/eventbus-live.test.ts`（5，监听端口、真 `automed` 与 `autome`）：①合格升级 `/ws` 收到
  `hello`（`core: "connected"`）；②外站 Origin 升级 `/ws` 回 403；③同一开发服务上 Vite HMR（`vite-hmr`）与 `/ws`
  都能连上；④R-02：`autome cancel T-1` 后 ≤ 3500 ms 收到对应 `event`，再等一个周期不重复；⑤（最后执行）向服务
  子进程发 SIGTERM，`/ws` 客户端收到关闭，子进程 5 秒内退出。

最新证据：docs/服务端每次-tick-后用-events/evidence/M-01-r9-impl.md · 锁定依赖下载与缓存均不可用，未取得绿色复验，保持开放 · 实现 #9

**M-02**（R-01 的 resume/reload/core 与挂点、R-03；前置 M-01）。按 §3 情形表实现续传；`core` 推送与去重；
恢复后立即补拉；账本回退；`broadcast`/`onClientMessage`/`onDisconnect`。新测试：

- `test/eventbus-protocol.test.ts`（3，纯函数）：①`hello`/`event`/`core`/`reload` 编码的字段集合与 §5.2 逐字一致；
  ②浏览器消息解析：合法 `resume`、非法 JSON、缺 `type`、`after` 不是非负整数、未知 `type` 的处理；
  ③内核五种状态的映射表与中文原因。
- `test/eventbus.test.ts` 增加 10 个：⑨`after == sentSeq` 不发任何消息；⑩小缺口从内核补发且与补发期间到来的
  live 事件不重不漏、严格递增；⑪缺口恰好 1000 补发、1001 只回 `reload`；⑫`after > latest_seq` 回 `reload`，且之后 `seq = latest_seq + 1`（小于旧 `sentSeq`）的新事件仍送到该连接；
  ⑬hello 后已发过事件再收到更早的 `resume` 回 `reload`；⑭内核不可用时 `resume` 延后到下一次成功拉取后处理；
  ⑮离线一次、恢复一次各推一条 `core`，`(status, reason)` 不变时不重复推；⑯恢复后立即补拉，离线期间新增的事件
  不丢不重；⑰`latest_seq < cursor` 时全体 `reload`、游标与每个连接的 `sentSeq` 一起重置（含取消正在续传的连接），之后账本新增的 `seq`（小于换账本前的 `sentSeq`）被每个连接收到恰好一次；⑱`onClientMessage` 收到对应 `type`、未注册 `type`
  与非法 JSON 被忽略且连接不断，`broadcast` 的非事件消息不影响 `seq` 过滤。
- `test/eventbus-live.test.ts` 增加 2 个（放在 SIGTERM 用例之前）：⑥客户端记下 `seq` 后断开，命令行改状态，
  重连发 `resume` 收到漏掉的事件且只一次；⑦按 pid 文件杀掉 `automed`，所有连接收到 `core offline`（中文原因）
  再收到 `core connected`，之后命令行的改动照常推到。

**M-03**（R-04；前置 M-02）。`app/lib/live-client.ts`、`live-routes.ts`、`live.tsx`；`root.tsx` 挂钩子与提示。
新测试（都不需要 DOM，也不监听端口）：

- `test/live-client.test.ts`（8，假 WebSocket 构造器 + vitest 假定时器）：①首次 `hello` 记 `lastSeq`、不发
  `resume`；②重连后发 `resume{after:lastSeq}`；③`reconnectDelay` 序列 500…10000 封顶、收到 `hello` 归零；
  ④`seq <= lastSeq` 的事件被丢弃；⑤`reload`/`core` 分别回调；⑥状态 `connecting → open → reconnecting → open`，
  只有 `reconnecting` 时 `<LiveBanner/>` 需要显示；⑦`stop()` 后不再重连；⑧`reload` 清检查点：之后 `seq` 小于旧 `lastSeq` 的事件被接受并回调 `onEvents`（覆盖在线换账本与 `resume` 之后收到 `reload` 两条路径）；`reload` 后、尚无新事件时断线重连，`hello` 按 `reload` 处理（`onReload`）并以 `hello.seq` 重建检查点。
- `test/live-routes.test.ts`（6，`createMemoryRouter` 桩路由 + 计数 loader）：①`task.updated`/`T-1` 只重验
  `taskId=T-1` 的任务路由，不重验项目路由与 `T-2`（地址不变的 `router.revalidate()`，所以必须靠 href 值比较识别纯重验）；②`project.*` 只重验项目类路由；③重验进行中又来一批，前后两批
  选中的路由最终都拿到新数据；④导航（参数变化）与表单提交仍按默认行为重验；⑤`revalidateAll` 重验所有声明了
  live 的路由；⑥守卫：对 `test/fixtures/live-routes-bad/` 报出「导出 loader 却没有 liveShouldRevalidate」的文件名，
  对真实的 `app/root.tsx` 与 `app/routes/**` 通过。
- `test/live-banner.test.ts`（2，`react-dom/server` 的 `renderToString` + `createElement`）：①`reconnecting` 渲染
  `role="status"` 与「连接中断，正在重连」；②`connecting`、`open` 不渲染任何内容。

**M-04**（R-06；前置：无，排在最后只是为了不打断主链路）。只把第 117、398 行字符串里的 0x00 换成源码 `\u0000`，
文件其余字节不变。

**R-06 验收判据与 Git 行为的冲突（裁决 D1-P03）。** 任务 R-06 写「`git diff --numstat <基线> -- …` 显示行数而不是
`-`」。实测（Git 2.53.0）：基线 blob 本身含 2 个 NUL，Git 只要比较的任一侧含 NUL 就整体按二进制处理，所以**无论新版本
怎么改**，相对基线 `30e5d51` 的 `--numstat` 都是 `-	-`（加 `--text` 也是）；它只能在「以修改后的提交为新基线」之后的
后续 diff 里显示行数。这不是实现能解决的问题，也不允许用 `.gitattributes` 或改基线去凑数。因此本里程碑的判据改为：
NUL 为 0（Git 不再把新文件当二进制）、`git diff --no-index --numstat /dev/null <文件>` 显示行数、`--text` 补丁恰好两处
替换、运行时字符串不变、35 个用例不变；相对基线的 numstat 如实记录为 `-	-`，不算失败。

### M-01 验收契约

- 验收命令：分别运行、分别记录退出码：`npm run lint --prefix apps/web`；`npm run typecheck --prefix apps/web`；`npm test --prefix apps/web`；`npm run build --prefix apps/web`；`npm test --prefix apps/desktop`；`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins`。
- 预期：Web 18 个测试文件、188 个用例全过、0 跳过（基线 175 + `eventbus.test.ts` 8 + `eventbus-live.test.ts` 5；`ci-config.test.ts` 改一项断言、数目不变）；lint、typecheck、build 退出码 0；desktop 104 个用例、103 过、1 跳过（与基线相同）；最后一条 diff 输出为空。
- 负向对照：`pull()` 只拉一页不看 `latest_seq` 时 `eventbus.test.ts` ② 失败（只收到 1000 条）；`main.ts` 不传 `onTick` 时 `eventbus-live.test.ts` ④ 在 3500 ms 内收不到事件而失败；`/ws` 改用自己的 `server.on('upgrade')` 绕过守卫时 `eventbus-live.test.ts` ② 不再回 403、`security.test.ts`「开发服务只保留一个 upgrade 守卫」的监听器数变成 2 而失败。

### M-02 验收契约

- 验收命令：分别运行、分别记录退出码：`npm run lint --prefix apps/web`；`npm run typecheck --prefix apps/web`；`npm test --prefix apps/web`；`npm run build --prefix apps/web`；`npm test --prefix apps/desktop`；`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins`。
- 预期：Web 19 个测试文件、203 个用例全过、0 跳过（M-01 的 188 + `eventbus-protocol.test.ts` 3 + `eventbus.test.ts` 新增 10 + `eventbus-live.test.ts` 新增 2）；lint、typecheck、build 退出码 0；desktop 104 / 103 过 / 1 跳过；diff 输出为空。
- 负向对照：补发期间不把 live 事件排队而是直接发出时 `eventbus.test.ts` ⑩ 看到 `seq` 倒退而失败；缺口判断写成 `>= 1000` 时 ⑪ 的「恰好 1000 补发」失败；把 `restarting` 映射成 `connected` 时 `eventbus-protocol.test.ts` ③ 与 `eventbus-live.test.ts` ⑦ 失败；恢复时把游标重置为 `latest_seq` 而不是续读时 ⑯ 丢事件而失败；账本回退时只重置 `cursor`、不重置连接的 `sentSeq` 时 ⑫、⑰ 的「回退后小 `seq` 的新事件被收到」失败。

### M-03 验收契约

- 验收命令：分别运行、分别记录退出码：`npm run lint --prefix apps/web`；`npm run typecheck --prefix apps/web`；`npm test --prefix apps/web`；`npm run build --prefix apps/web`；`npm test --prefix apps/desktop`；`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins`。
- 预期：Web 22 个测试文件、219 个用例全过、0 跳过（M-02 的 203 + `live-client.test.ts` 8 + `live-routes.test.ts` 6 + `live-banner.test.ts` 2）；lint、typecheck、build 退出码 0（build 证明 `root.tsx` 的改动与浏览器包里没有服务端代码）；`server.test.ts` 的开发/生产首页 SSR 用例仍通过；desktop 104 / 103 过 / 1 跳过；diff 输出为空。
- 负向对照：`revalidateForEvents` 不设待处理批、直接 `revalidate()` 时 `live-routes.test.ts` ① 里无关路由的 loader 计数增加而失败；`revalidate()` 一调用就清空待处理批时 ③ 前一批路由拿不到新数据而失败；退避不封顶时 `live-client.test.ts` ③ 失败；`liveShouldRevalidate` 里把纯重验判断写成 `currentUrl === nextUrl`（对象比较）时 `live-routes.test.ts` ① 里无关路由的 loader 计数增加而失败；收到 `reload` 不清 `lastSeq` 时 `live-client.test.ts` ⑧ 失败。

### M-04 验收契约

- 验收命令：分别运行、分别记录退出码：`python3 -I -c "print(open('apps/web/test/write-gate.test.ts','rb').read().count(b'\x00'))"`；`git diff --numstat 30e5d51 -- apps/web/test/write-gate.test.ts`；`git diff --text -U0 30e5d51 -- apps/web/test/write-gate.test.ts | grep -cE '^[-+][^-+]'`；`git diff --no-index --numstat /dev/null apps/web/test/write-gate.test.ts`；`cd apps/web && npx vitest run test/write-gate.test.ts`；`cd apps/web && npx vitest run test/write-gate.test.ts -t "a null byte is refused anywhere in a string|looksLikeAPath catches the shapes"`；`npm test --prefix apps/web`；`npm test --prefix apps/desktop`。
- 预期：NUL 计数输出 `0`；相对基线的 numstat 输出 `-	-	apps/web/test/write-gate.test.ts`（**预期如此**，基线一侧含 NUL，Git 仍按二进制，见上面「R-06 验收判据与 Git 行为的冲突」，只记录不算失败）；`--text` 补丁里被改动的行数输出 `4`（两条删除、两条添加，且第 117 行为 `params: { task_id: 'T-1\u0000rest' }`、第 398 行列表末项为 `'a\u0000b'`）；`--no-index --numstat /dev/null` 输出以行数开头（如 `562	0	…`）而不是 `-	-`；`write-gate.test.ts` 35 个用例全过；`-t` 过滤后 2 个通过、33 个跳过（过滤造成的跳过，不计入全量）；Web 全量仍是 22 个文件、219 个用例、0 跳过；desktop 104 / 103 过 / 1 跳过。
- 负向对照：只改第 117 行时 NUL 计数输出 `1`、`--no-index --numstat /dev/null` 仍是 `-	-`、`--text` 补丁行数为 `2`；写成 `'\\u0000'`（字面反斜杠）时字符串里没有 NUL，「a null byte is refused anywhere in a string」的 `result.ok` 为 true 而失败。

## 人工验收清单

- H-01 `npm run dev --prefix apps/web` 打开面板，开发者工具 Network → WS 里 `/ws` 收到 `hello`；另开终端用 `autome` 命令行改一个任务的状态，3 秒左右出现对应的 `event` 帧，页面没有整页刷新。
- H-02 停掉 Web 服务，页面顶部出现「连接中断，正在重连」；重新启动服务后提示自动消失，WS 帧里能看到重连后的 `hello` 与 `resume`。
- H-03 开发模式下修改 `app/routes/home.tsx` 的文字，页面经 HMR 更新，同时 `/ws` 连接不断开。

## Backlog

- B-01 `/ws` 没有心跳。本机连接靠 TCP 关闭发现断线已经够用；以后若有半开连接堆积，再加 `ws` 的 ping/pong 与
  `bufferedAmount` 上限（慢客户端背压）。

## 争议项
