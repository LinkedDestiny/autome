# Web 通知（W4）：web.sqlite3 面板库、事件→通知规则与实时推送、顶栏铃铛已读与后台系统通知

status: 实现中
design-round: 1/15
implementation-round: 0/0
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01：新建 apps/web/server/store.ts 与 server/store-warnings.ts，在 start.js 接上告警过滤，跑「M-01 验收契约」

任务文件：`docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-task.md`。
本任务的规则见 `docs/实现-web-sqlite3-与通知器/protocol/loop-protocol.md` 与
`docs/实现-web-sqlite3-与通知器/protocol/session-protocol.md`。

## 背景与现状

基线 `f21065b`（任务分支，含 W1–W3、K0–K2）。下面是与本任务有关的现状，均为本轮读源码或实测所得。

- **W3 总线**（`apps/web/server/eventbus.ts`）：游标在内存里，首次连上内核时从 `latest_seq` 开始，不推历史；`pull()` 单飞，
  `onTick` 里 `void bus.pull()`。挂点：`subscribe`、`broadcast(ExtensionMessage)`、`onClientMessage`、`onDisconnect`。
  `bus.pull()` 自己吞掉错误，返回的 Promise 不会 reject。
- **浏览器端**：`app/lib/live-client.ts` 的 `switch (message.type)` 只处理 `hello / event / reload / core`，其它类型直接忽略，
  也没有把扩展消息交给调用方的回调；`app/lib/` 不在本任务可改范围内。
- **HTTP**（`server/http.ts`）：`securityMiddleware` 在最前，GET/HEAD 只查 Host，其它方法再查 Origin；`/api` 下有
  `express.json` 与 JSON 错误处理，只注册了 `POST /api/read`、`POST /api/write`。React Router 的请求处理没有 `getLoadContext`
  （W5 的事），开发模式下路由模块经 `vite.ssrLoadModule` 加载，与服务端模块不是同一份实例——loader 拿不到服务端创建的库对象。
- **内核错误形状**（`server/core/supervisor.ts`）：内核回 RPC 错误时抛 `Error` 并带 `code`；未连接抛「内核未连接：…」、
  超时带 `timeout: true`，两者都没有 `code`。
- **`node:sqlite`**：本机实测，Node 22.23.1 导入时打印
  `(node:57980) ExperimentalWarning: SQLite is an experimental feature and might change at any time`，Node 26.5.0 不打印；
  两版 `INSERT OR IGNORE` 遇唯一约束冲突都返回 `changes: 0`。在 Node 22 上先替换 `process.emitWarning`、再 `import('node:sqlite')`，
  这条告警不再出现，其它 `ExperimentalWarning` 照常打印（实测）。
- **基线用例数**（本轮实跑）：`npm test --prefix apps/web` 22 个文件 225 个用例全过；`npm test --prefix apps/desktop` 104 个用例，
  103 过、1 个跳过（基线自带，与本任务无关）。

### 实测事件原文

用真实 `target/debug/autome` 与 `target/debug/automed`，临时 `AUTOME_HOME` 与 `HOME`，`AUTOMED_CLAUDE_BINARY` /
`AUTOMED_CODEX_BINARY` 指向 `crates/automed/tests/end_to_end.rs` 同款假 CLI，跑出一本 156 条事件的账本（脚本
`.autome/output/w4-events/run.sh`，可从零重跑，约 14 秒；工作区内保留，不入库）。下面是
`autome rpc events.since '{"after_seq":0,"limit":1000}'` 原始输出经 `jq -c` 压成单行的结果，键名、键序、取值均未改动：

```text
{"kind":"task.created","payload":{"project_id":"prj-f5a8e540132845849a830aa90fa0667a"},"seq":3,"subject":"T-1"}
{"kind":"task.updated","payload":{"state":{"node":"await_design_approval","state":"active"},"trigger":"session_ended"},"seq":21,"subject":"T-1"}
{"kind":"task.updated","payload":{"state":{"node":"await_merge","state":"active"},"trigger":"gate_done"},"seq":45,"subject":"T-1"}
{"kind":"task.updated","payload":{"state":{"state":"done"},"trigger":"core_step_done"},"seq":53,"subject":"T-1"}
{"kind":"project.alert","payload":{"alert_id":"alr-e3bc2b65c341451ea98f1fa54c6b612b","kind":"main_gate_failed","task_id":"T-2"},"seq":102,"subject":"prj-f5a8e540132845849a830aa90fa0667a"}
{"kind":"task.updated","payload":{"state":{"at":"intake","reason":{"detail":"缺少状态字段 `status`","kind":"protocol"},"state":"failed"},"trigger":"session_ended"},"seq":109,"subject":"T-3"}
{"kind":"task.protocol_failure","payload":{"detail":"缺少状态字段 `status`","node":"intake"},"seq":110,"subject":"T-3"}
{"kind":"task.updated","payload":{"state":{"at":"intake","reason":{"detail":"会话退出码 3","kind":"session_crashed"},"state":"failed"},"trigger":"session_ended"},"seq":116,"subject":"T-4"}
{"kind":"task.updated","payload":{"state":{"node":"await_info","state":"active"},"trigger":"bug_session_ended"},"seq":124,"subject":"T-5"}
{"kind":"task.updated","payload":{"state":{"node":"locate","state":"active"},"trigger":"reply"},"seq":145,"subject":"T-5"}
{"kind":"task.updated","payload":{"state":{"node":"await_info","state":"active"},"trigger":"bug_session_ended"},"seq":150,"subject":"T-5"}
```

要点：`events.since` 顶层为 `{"events":[…],"latest_seq":156}`；`TaskStateRepr` 是嵌套在 `payload.state` 里的带 `state` 标签对象；
`project.alert` 的 `subject` 是项目 id，`payload.task_id` 是触发它的那次合并的任务（T-2 随后仍走到 `done`，seq 105）；
`task.updated` 不带项目 id，只有 `task.created` 的 payload 带。47 条 `task.updated` 里没有同一任务连续两条 `state` 相同的情况；
T-5 经 `reply` 回到 `locate` 再进 `await_info`，是两次真实进入。`pause` / `resume` 对等批准中的任务被内核拒绝，不产生事件。
`task.get`（参数 `task_id`）实测顶层键：`brief, decisions, documents, gate, next_role, pending_decisions, project, queue_position,
retry, sessions, so_far, status_block, status_error, status_error_detail, task, worktree`；其中 `project` 原文为
`{"default_branch":"main","id":"prj-f5a8e540132845849a830aa90fa0667a","name":"repo"}`，`task` 带 `id, kind, slug, title, state` 等。

## 目标与非目标

目标：覆盖任务 R-01..R-07。按规则把内核账本事件变成通知、落进 `<AUTOME_HOME>/state/web.sqlite3`，经 W3 的 `/ws` 推送；
顶栏铃铛显示未读数与列表，支持单条已读、全部已读，各标签页实时同步；「需要你」级通知在标签页后台时发系统通知。

非目标（任务 §3「明确不做」原样适用）：任务卡角标（只提供查询，见方案 §2）、外壳与 W5 各项、`getLoadContext`、W6–W12 的页面与
Electron 侧、偏好设置界面、改 W3 总线语义或 W2 安全语义、令牌与登录、新增 npm 依赖、改 `crates/` 等目录。

## 方案

### 1. 文件与分工

| 文件 | 内容 | 是否需要监听端口 |
|---|---|---|
| `server/store.ts` | 打开面板库、迁移、游标/通知/偏好的读写 | 否 |
| `server/store-warnings.ts` | 过滤 `node:sqlite` 的那一条 ExperimentalWarning | 否 |
| `server/notifier-rules.ts` | 规则表、失败原因与告警的中文渲染、`levelOf(kind)`；纯模块，不导入 `node:*`，浏览器端也导入它 | 否 |
| `server/notifier.ts` | 通知器：持久游标、补拉、补齐信息、落库、推送 | 否 |
| `server/notifier-api.ts` | 铃铛接口的处理函数（列表、已读）与 `attachNotifier`（tick 与内核状态的挂法） | 否 |
| `server/main.ts`、`server/http.ts`、`server/start.js` | 只接线（见 §5、§6、§1 末） | — |
| `app/components/bell-client.ts` | 铃铛的拉取控制器（不依赖 React、不自己建连接）：`handleMessage(raw)`、单飞拉取、已读 POST；`tapSocket(socket, controller)` | 否 |
| `app/components/bell-live.tsx` | `useLiveWithBell()`：与 `useLiveEvents()` 同样用 `createLiveClient` 建**唯一一条** `/ws`，只是 `createSocket` 里给 socket 挂一个旁听监听，把消息同时交给铃铛控制器；提供 `BellProvider` 上下文 | 否 |
| `app/components/bell-targets.ts` | 通知目标路由表与 `resolveTarget(item, targets)`（§7 跳转） | 否 |
| `app/components/bell-system.ts` | 系统通知接口 `SystemNotifier`、浏览器实现、可见性锁、认领、是否弹出的判定 | 否 |
| `app/components/bell.tsx` | `NotificationBell` 组件 | 否 |
| `app/root.tsx` | 用 `useLiveWithBell()` 取代 `useLiveEvents()`（连接数不变，仍是一条），在 `LiveBanner` 之后挂 `<NotificationBell />` | — |

`start.js` 的改动：版本检查通过后、导入 `main.ts` 之前，`(await import('./store-warnings.ts')).filterSqliteWarning(process)`。
过滤条件同时满足才丢弃：告警类型是 `ExperimentalWarning`，且文字以 `SQLite is an experimental feature` 开头；其它告警原样交给
原 `process.emitWarning`。理由：任务约束 8 把服务端日志算作用户可见文字，这条英文告警每次在 Node 22 启动都出现，对用户没有可操作
信息；只过滤这一条，不用 `--no-warnings` 这类一刀切的做法。

### 2. 面板库（R-01）

路径 `<home>/state/web.sqlite3`（`home` 即 `startServer` 的 `home`，缺省 `resolveAutomeHome(process.env)`），目录不存在时建。
打开后 `PRAGMA journal_mode=WAL`、`PRAGMA busy_timeout=5000`。第 1 版结构：

```sql
CREATE TABLE schema_version (version INTEGER NOT NULL);
CREATE TABLE event_cursor (name TEXT PRIMARY KEY, seq INTEGER NOT NULL);
CREATE TABLE notifications (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, project_id TEXT, task_id TEXT,
  title TEXT NOT NULL, body TEXT, event_seq INTEGER, created_at TEXT NOT NULL, read_at TEXT);
CREATE UNIQUE INDEX notifications_dedupe ON notifications (kind, ifnull(task_id, ''), event_seq);
CREATE INDEX notifications_unread ON notifications (read_at, task_id);
CREATE TABLE preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

- **NULL 不重复**：唯一约束建成表达式索引，`task_id` 为 NULL 时按空串参与比较，`project.alert` 一类没有任务的通知同样被挡。
  `task_id` 列保留 NULL 的本义（「没有任务」），不拿空串冒充。
- **迁移**：`MIGRATIONS` 是按版本排列的 SQL 数组，`migrate(db, migrations)` 导出供测试注入。打开时读 `schema_version`
  （表不存在或无行视为 0）；每一级在自己的 `BEGIN IMMEDIATE … COMMIT` 里执行该级 SQL 并写新版本号，失败整级回滚、版本停在上一级、
  抛「面板库迁移到第 N 版失败：…」。已是最新版时什么都不做，所以重复启动不建表、不丢数据。
- **库比代码新**（版本号大于 `MIGRATIONS.length`）：不改库，抛「面板库版本 X 比当前程序支持的 Y 新，请升级 Autome，或移走 <路径> 后重启」，
  `startServer` 随之失败、进程以中文信息退出。理由：降级运行的旧代码若继续写新结构的库，可能写出新代码读不懂的数据；面板只是展示与
  控制面，启动失败并说清原因比悄悄关掉通知更容易被发现。
- **写通知与推进游标同一事务**：`commit({ cursor, notification? })` 在一个事务里 `INSERT OR IGNORE` 通知、`UPSERT` 游标，返回
  实际插入的行（被唯一约束挡下返回 `undefined`）。通知 id 为 `ntf-` + `crypto.randomUUID()`；`created_at` 为写入时刻 ISO 字符串
  （账本事件不带时间）。
- **账本更换**：`resetLedger(name, latest)` 在一个事务里把游标设为 `latest`、把所有通知的 `event_seq` 置 NULL。旧账本的序号在新账本里
  没有意义，置空后不会和新账本同序号的事件撞上唯一约束（NULL 的 `event_seq` 互不相等）。
- 其它：`list({limit})`（按 rowid 新到旧，缺省 50、上限 200）、`unreadCount()`、`markRead(ids)` / `markAllRead()`（只改 `read_at`
  为空的行，返回实际改动的 id）、`getPreference` / `setPreference`、`close()`。
- **给 W6 角标的数据**：`unreadByTask(kinds)` 返回 `[{task_id, count}]`，W6 传入 `levelOf` 为「需要你」的那几个 kind 调用即可；
  本任务只实现并测试这个查询，不做卡片。

### 3. 规则（R-02）

`notifier-rules.ts` 导出 `NOTIFICATION_RULES`（6 行数据）与 `matchEvent(event)`；通知器按表逐行匹配，第一条匹配的规则生效。

| 事件 `kind` | 匹配 `payload.state` 子集 | 通知 `kind` | 级别 | 标题模板 | 补齐 |
|---|---|---|---|---|---|
| `task.updated` | `{"state":"active","node":"await_design_approval"}` | `await_design_approval` | `need_you` | `{task} 设计已定稿，等你批准` | 任务 |
| `task.updated` | `{"state":"active","node":"await_merge"}` | `await_merge` | `need_you` | `{task} 可以合并了` | 任务 |
| `task.updated` | `{"state":"active","node":"await_info"}` | `await_info` | `need_you` | `{task} 无法复现，需要更多信息` | 任务 |
| `task.updated` | `{"state":"failed"}` | `failed` | `need_you` | `{task} 停在失败上：{reason}` | 任务 |
| `project.alert` | —（按 `payload.kind` 取告警标题） | `project_alert` | `need_you` | `{alert}` | 项目 |
| `task.updated` | `{"state":"done"}` | `done` | `fyi` | `{task} 已合并` | 任务 |

- 表外一律不产生通知：其它 `active` 节点（含 Bug 的 `not_a_bug`）、`draft`、`queued`、`paused`、`stopped`、`cancelled`，以及
  `task.created`、`task.protocol_failure`、`project.alert_dismissed` 等其它事件。
- **「进入」**：通知器在进程内记住每个任务最近一条 `task.updated` 的 `state`（不论是否命中规则）；新事件的 `state` 与上一条深相等时不算进入、
  不产生通知，中间隔了别的状态再回来算新的进入（T-5 的 seq 124 / 145 / 150）。记忆不持久：重启后每个任务的第一条事件按进入处理。记忆只在这条事件**确定处理完之后**更新（不命中规则的事件立即更新；命中的在 `commit` 成功或按「带 `code` 的拒绝」
  生成通知之后更新）；补齐因无 `code` 的错误停下时不更新，下次重试同一条仍按进入处理，不会被当成重复跳过。
  实测账本没有连续相同状态，这条规则防的是将来内核重复记事件；重启时恰好跨过一对重复事件的最坏结果是多一条通知，不会漏发。
- **`{reason}`**：按 `FailureReason.kind` 查表——`design_budget`「设计轮次用完（上限 {limit}）」、`impl_budget`「实现轮次用完（上限 {limit}）」、
  `fix_budget`「修复轮次用完（上限 {limit}）」、`infeasible`「设计判定不可实现」、`protocol`「协议失败（{detail}）」、
  `session_crashed`「会话异常退出（{detail}）」、`rebase_conflict`「rebase 冲突（{detail}）」、`config`「配置问题（{detail}）」、
  `core_step`「内核步骤 {node} 失败（{detail}）」，未知 kind「未知原因（{kind}）」。`detail` 把连续空白压成一个空格、超过 60 字截断加「…」。
  例：seq 116 →「T-4 停在失败上：会话异常退出（会话退出码 3）」。
- **`{alert}`**：按告警 `payload.kind` 查表，`main_gate_failed` →「{branch} 门禁失败」，`branch` 取项目的 `default_branch`，
  拿不到时为 `main`（即计划原文「main 门禁失败」）；未知告警 kind →「项目告警：{kind}」。
- **补齐**：「任务」类调 `task.get {"task_id"}`，取 `project.id` 作 `project_id`、`task.title` 作 `body`；「项目」类调
  `project.get {"project_id": subject}`，取 `default_branch` 渲染标题，`body` 为「{项目名}：{task_id} 合并后验证未通过」（项目名取
  `display_name`，为空时取 `path` 最后一段）。告警的 `task_id` 存 `payload.task_id`（缺省 NULL），`project_id` 存 `subject`。
  补齐结果按任务 id / 项目 id 在进程内缓存。
- `levelOf(kind)`：由规则表派生，浏览器端用它判断系统通知（§7）。

### 4. 通知器（R-03）

`createNotifier({ store, call, publish, log, pageSize = 200 })` → `{ pull(): Promise<void>, close() }`，游标名 `notifier`。

1. **单飞**：同 W3 的写法，`pull()` 在途时只置 `again`，结束后再跑一轮；同一时刻最多一个补拉在途。
2. **首次启动**（库里没有 `notifier` 游标）：调 `events.since {"after_seq": Number.MAX_SAFE_INTEGER}` 取 `latest_seq`，写成游标，
   不追溯历史。理由：面板第一次打开时账本里可能已有几百条旧事件（早已合并的任务），补成通知只会淹没铃铛；当时正等你处理的任务由
   W5/W6 的「等你处理」列展示。之后的每次启动都从持久游标续拉，停服期间的事件全部补齐。
3. **拉取**：`events.since {"after_seq": cursor, "limit": pageSize}`，按 `seq` 升序逐条处理，直到 `cursor >= latest_seq` 或本页为空。
4. **账本更换**：`latest_seq < cursor` 时调 `store.resetLedger('notifier', latest_seq)`，日志一行
   「[通知] 账本已更换（最新序号 X 小于游标 Y），从 X 继续」，本轮结束。新账本在游标之前的事件不补（无法判断哪些已通知过）。
5. **逐条处理**：不命中规则的事件只推进内存里的「待写游标」；命中时先补齐信息——
   - 补齐抛错且没有 `code`（内核未连接、超时）：本轮停下，把「待写游标」写到这条事件之前，日志一行「[通知] 补齐 T-3 的信息失败，稍后重试：…」，
     下次 `pull` 从这条重来。游标只在事件确定处理完之后推进（任务约束 2）。
   - 补齐抛错且带 `code`（内核明确拒绝，例如任务已不存在）：照样生成通知，`project_id`、`body` 为 NULL，不让一条坏数据卡住后面所有事件。
   - 成功：`store.commit({ cursor: {name:'notifier', seq}, notification })`；返回插入的行时才 `publish`。
   每页结束把「待写游标」落库一次（不命中的事件不逐条写库）。
6. **推送消息**严格为计划 §5.2 的形状：`{"type":"notification","id":"ntf-…","kind":"await_merge","task_id":"T-3","title":"…"}`，
   不加字段；没有任务的告警 `task_id` 为 `null`。
7. **崩溃语义**：通知与游标同事务，「通知已写、游标未推进」只能来自事务外的人为状态（测试直接构造）；重启后重做这条事件，被唯一约束挡下、
   不推送。「游标已推进、通知未写」在同事务下不会出现；测试注入游标写失败，验证通知随事务回滚，重启后补上且只一条。
8. `close()` 之后 `pull()` 立即返回，不再调 `call`、不再写库。

### 5. 接线（任务约束 2–4）

`main.ts`：`startServer` 开头 `openWebStore(home)`（失败直接抛出，见 §2），建 `notifier`（`call` 走 `core.call`，`publish` 走
`bus.broadcast`）。`attachNotifier({ bus, notifier })`（在 `notifier-api.ts`）返回两样东西：
- `onTick()`：`void bus.pull().then(() => notifier.pull())`——同步返回，先让 W3 推完 `event`，再跑通知器；通知器单飞，慢了也不叠加，
  也不占住 ticker（ticker 只等 `core.tick()`）。
- `onStatus(status)`：先 `bus.setCoreStatus(status)`，`connected` 时再 `void notifier.pull()`，内核一连上就补拉停服期间的事件。

关闭顺序：`ticker.stop()` → `notifier.close()` → `bus.close()` → `core.stop()` → `store.close()` → 关 HTTP。

### 6. 铃铛接口与跨标签页同步（R-04、R-05、R-06）

数据走服务端接口而不是 React Router 的 loader/action：没有 `getLoadContext`（W5）时，loader 拿不到服务端创建的库对象（§背景）。
`createHttpServer({ mode, gateway, notifications? })` 新增可选参数，`notifications` 存在时在 `/api/read`、`/api/write` 之后注册，复用同一
JSON 解析与错误处理，不改已有路由；写接口同样经过最前面的 `securityMiddleware`。

- `GET /api/notifications?limit=N` → `200 {"ok":true,"payload":{"unread":N,"items":[{id,kind,level,project_id,task_id,title,body,created_at,read_at}]}}`；
  `limit` 缺省 50、1..200，否则 `400 {"ok":false,"error":"已拒绝：limit 必须是 1 到 200 的整数"}`。
- `POST /api/notifications/read`，请求体二选一：`{"ids":["ntf-…"]}`（1..200 个、每个是不超过 64 字的字符串）或 `{"all":true}`；
  → `200 {"ok":true,"payload":{"unread":N,"ids":[实际改动的 id]}}`；其它形状 `400`「已拒绝：…」且不写库。
- 改动非空时广播新增的扩展消息 `{"type":"notification-read","ids":[…]}`（先落库再广播；`ids` 为空不广播）。它是新类型，不改变
  §5.2 已有消息的形状。

**浏览器端怎么收到推送（只有一条 `/ws`，满足 R-05）**：`app/lib/live-client.ts` 会丢掉扩展消息，`app/lib/` 又不在本任务可改范围内，但 `createLiveClient`
的 `createSocket` 是注入点，`app/root.tsx` 在范围内。做法：`app/components/bell-live.tsx` 的 `useLiveWithBell()` 取代 `root.tsx` 里的 `useLiveEvents()`——
同样用 `createLiveClient` 建连接（`onConnection / onCore / onEvents / onReload` 的接法与 `app/lib/live.tsx` 一致，W3 的续传与重验行为不变），
区别只有 `createSocket: address => tapSocket(new window.WebSocket(address), controller)`：`tapSocket` 用 `addEventListener('message', …)` 给同一个 socket
加一个旁听监听（`createLiveClient` 自己用 `onmessage` 属性，两者互不覆盖），把每条原始消息交给铃铛控制器的 `handleMessage`。重连时 `createLiveClient`
新建 socket，会再经过 `createSocket` 被再次旁听。所以**每个标签页始终只有一条 `/ws`**，不新增服务端通道，不轮询。
控制器只关心四类消息：`hello`（首次连上或重连后重新拉取，补上断线期间的变化）、`notification`、`notification-read`（都触发重新拉取），其余（`event`、
`core`、`reload`）与非 JSON 忽略；重连与退避由 `createLiveClient` 负责，控制器不管。`useLiveWithBell()` 与 `useLiveEvents()` 的函数体重复约 15 行，这是
为了不改 `app/lib/` 付出的代价，W5 搬外壳时删掉重复（Backlog B-01）。

**不维护副本**：铃铛的未读数和列表一律来自 `GET /api/notifications`，收到消息只触发重新拉取，不在客户端自增自减。拉取单飞：在途时再来消息
只记一次「再拉」。`notification` 触发的拉取完成后，控制器把「这条消息 + 刷新后的列表」交给 `onNotification`（§7 的系统通知据此判断条目是否新鲜、是否已读）；拉取失败则不交。单条已读 / 全部已读发 POST 后重新拉取；其它标签页靠 `notification-read` 广播重新拉取，于是一个标签页点已读，所有标签页
的计数都会变。

### 7. 铃铛界面与系统通知（R-06、R-07）

- **挂法**：`NotificationBell` 无 props，自带数据与连接。`root.tsx` 只加一行 `<NotificationBell />`；容器 `position: fixed`、右上角，
  不占文档流，不引起页面纵向滚动。W5 搬进外壳时把这一行挪进顶栏即可。
- **按钮**：中文 `aria-label`「通知，N 条未读」；徽标 0 不显示，1..99 显示原数，≥100 显示「99+」。
- **展开面板**：`max-height: 60vh; overflow-y: auto`，内部滚动；每项显示标题、`body`（有则显示）、时间（本地 `MM-DD HH:mm`）、未读圆点；
  顶部「全部已读」（无未读时禁用）；空列表「还没有通知」；拉取失败「通知加载失败：…」。
- **每条的操作**：未读项有「标为已读」按钮（单条已读，不跳转）；点击标题链接同样先发单条已读。
- **跳转与目标不可用（R-06）**：`bell-targets.ts` 里一张表 `TARGETS = [{ pattern: '/tasks/:taskId', available: false }, { pattern: '/projects/:projectId', available: false }]`，
  `resolveTarget(item, targets = TARGETS)` 返回 `{ href: string | null, note?: string }`：有 `task_id` 取 `/tasks/<task_id>`，否则有 `project_id` 取
  `/projects/<project_id>`，两者都没有 `href` 为 `null`；对应表项 `available` 为 `false`（W6/W7 的页面还没有）时 `href` 同样为 `null`，且 `note` 为「详情页尚未开放」。
  `href` 非空才渲染 React Router `<Link>`；为 `null` 时标题是纯文本、下方显示淡色的 `note`（若有）。**因此 W6/W7 之前，铃铛与系统通知都不会把用户带进 404**，
  也不抛错。`available` 由守卫用例与 `app/routes.ts` 绑定：用例读 `routes.ts` 文本，断言 `tasks` / `projects` 路由出现与否和对应表项的 `available` 一致——
  W6/W7 加路由时这条用例会变红，提醒同时把表项改为 `true`。铃铛列表、系统通知的 `href` 都经 `resolveTarget` 得出，各入口行为一致。
- **系统通知接口**：`SystemNotifier = { permission(): 'default'|'granted'|'denied'|'unsupported'; request(): Promise<…>; show({id,title,body,href: string | null}): void }`。
  `browserSystemNotifier(window)` 用 `new Notification(title, { body, tag: id })`，点击时 `window.focus()`，`href` 非空才跳转（`resolveTarget` 为 `null` 时只回到面板，不进 404）。
  `NotificationBell` 接受可选的 `systemNotifier`，缺省用浏览器实现；W11 传入调 preload `notify()` 的实现即可替换。
- **何时弹**：不依赖任何固定等待时间或消息到达的先后。收到 `notification` 消息并拿到刷新后的列表（§6）后，依次满足才继续：
  `levelOf(kind) === 'need_you'`；权限为 `granted`；列表里找得到这条 id 且 `read_at` 为空；`Date.now() - created_at ≤ 60 s`（新鲜期。浏览器与服务端在同一台机器上，
  时钟一致；陈旧的通知只在铃铛里，不再弹横幅）。然后进入「认领」：
  - **可见性锁**：每个标签页在可见期间持有一把共享 Web Lock `autome-visible`（`document.visibilityState === 'visible'` 时请求，转为不可见或卸载时释放；标签页关闭 /
    崩溃由浏览器自动释放）。这样「有没有标签页在前台」是实时状态，不取决于哪个标签页先处理消息、处理得多慢。
  - **认领**：`navigator.locks.request('autome-notify', …)`（独占）临界区内读写 `localStorage['autome.notified']`，值为 `{ [id]: 认领时刻毫秒 }`。若 `id` 已在其中→放弃；
    否则写入 `id`，并顺手清掉**早于 10 分钟**的条目（按时间清理，**不按条数**），然后在临界区内用 `navigator.locks.query()` 看是否有别的标签页持有 `autome-visible`，
    且本标签页自己也不可见：都满足才在出临界区后 `show`。**谁先认领谁决定，决定对所有标签页生效**：有标签页在前台时，先到的后台标签页只认领不弹，之后任何标签页（含前台那个
    迟到的）再处理同一条都因已认领而放弃。
  - 为什么保留期（10 分钟）大于新鲜期（60 秒）：被清掉的记录对应的通知一定已过新鲜期，迟到的标签页即使找不到认领记录也不会弹——所以多条消息涌入（>100 条）、标签页被冻结后很久才处理，
    都不会造成重复。
  - `navigator.locks` 或 `localStorage` 不可用（抛错）时没有可行的跨页协调，只看本标签页自己的 `visibilityState`，不可见就直接 `show`，靠同一 `tag` 让系统合并同 id 的横幅。
- **权限**：只在用户点击面板里的「开启系统通知」按钮时调 `request()`（需要用户手势）；挂载、收到通知时都不请求。`denied` 时面板显示
  「系统通知已被浏览器拒绝，铃铛照常提醒」；`unsupported`、`granted` 不显示提示。

## 风险与验证安排

- **实现轮不能监听 127.0.0.1**（W1–W3 都撞上）：规则、渲染、迁移、游标、去重、接口处理函数、铃铛控制器、系统通知判定都写成不监听的单测；
  需要监听的只有 M-03 的 `notifier-live.test.ts`（3 个用例）。被 `listen EPERM` 阻断时，按「M-03 验收契约」逐条单独运行并写出各自退出码，
  交审计轮复验。M-02 的真实内核用例用 `CoreSupervisor`（stdio，不监听）加真实 `autome` 命令行。
- **事件形状**：规则测试的手写事件一律复制「实测事件原文」里的行；M-02 另有真实账本用例，用 `test/fixtures/w4-ledger.sh`（由实现轮依
  `.autome/output/w4-events/run.sh` 改写：临时 `AUTOME_HOME`/`HOME`、假 CLI、每个任务一个步骤目录、`run --until-idle`），
  不启动真实 Agent。该目录若已不在，按本节与「实测事件原文」的描述重写。
- **账本更换的检测盲区**：只有新账本的 `latest_seq` 小于持久游标时才能发现；新账本已长过旧游标再启动，会从旧游标续拉，漏掉其前的事件。
  与 W3 同一限制，记为已知风险，不在本任务处理。
- **重复事件**：「进入」的记忆不持久，跨重启最多多一条通知（§3），不漏。
- **补齐失败卡住游标**：内核离线时通知器停在那一条，内核恢复后 `onStatus(connected)` 立即重试；内核明确拒绝的不卡（§4.5）。
- **跨标签页协调依赖 Web Locks**：`navigator.locks` 的 `request`（独占 / 共享）与 `query()` 是标准 API（Chrome 69+、Firefox 96+、Safari 15.4+），但本机没有浏览器可在实现轮
  验证；单测用实现了 `request` / `query` 语义的假 locks，真实浏览器里的行为由 H-03 人工验收。
- **重复代码**：`useLiveWithBell()` 与 `useLiveEvents()` 有约 15 行重复，W5 删除（B-01）。
- 每个里程碑报 Web 与 desktop 的用例总数和与基线的差，`skipped` 必须为 0（desktop 基线自带的 1 个跳过除外，报告时单列）；
  负向对照逐条实跑并写退出码，全部还原后 `git status --porcelain` 为空。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 面板库 web.sqlite3：结构、迁移、去重、游标与已读读写，过滤 SQLite 告警 | 0 | store |
| M-02 | 开放 | 规则表与通知器：持久游标补拉、补齐信息、崩溃与换账本，真实账本验证 | 0 | notifier |
| M-03 | 开放 | 接线与推送：通知器挂到 tick 之后，铃铛接口，notification 与 notification-read 广播 | 0 | api |
| M-04 | 开放 | 顶栏铃铛：未读数、列表、单条与全部已读、跨标签页同步、挂到 root | 0 | bell |
| M-05 | 开放 | 后台标签页系统通知：可替换接口、权限按钮、多标签页只弹一次 | 0 | system-notify |

## 验收说明

所有验收命令在仓库根执行，每条单独运行、单独记退出码，不用 `&&` 串起来。下文「全量四件」指：
`npm test --prefix apps/web`、`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/desktop`。

| 里程碑 | 覆盖 | 前置 | 新增 / 改动文件 |
|---|---|---|---|
| M-01 | R-01 | 无 | `server/store.ts`、`server/store-warnings.ts`、`server/start.js`、`test/store.test.ts`、`test/store-warnings.test.ts` |
| M-02 | R-02、R-03 | M-01 | `server/notifier-rules.ts`、`server/notifier.ts`、`test/notifier-rules.test.ts`、`test/notifier.test.ts`、`test/notifier-real.test.ts`、`test/fixtures/w4-ledger.sh` |
| M-03 | R-03、R-04（服务端）、R-05 | M-02 | `server/notifier-api.ts`、`server/main.ts`、`server/http.ts`、`test/notifier-api.test.ts`、`test/notifier-live.test.ts` |
| M-04 | R-04（浏览器端）、R-05（单连接）、R-06 | M-03 | `app/components/bell-client.ts`、`app/components/bell-live.tsx`、`app/components/bell-targets.ts`、`app/components/bell.tsx`、`app/root.tsx`、`test/bell-client.test.ts`、`test/bell-view.test.ts` |
| M-05 | R-07 | M-04 | `app/components/bell-system.ts`、`app/components/bell.tsx`、`test/bell-system.test.ts`、`test/bell-view.test.ts` |

### M-01 验收契约

- 验收命令：`npm test --prefix apps/web -- test/store.test.ts test/store-warnings.test.ts`；然后全量四件。
- 预期：两文件共 17 个用例全过（`store.test.ts` 14 个：首次建库四表列名与 §5.3 一致且 WAL、版本 1；重开不重建不丢数据；注入第 2 级只跑缺的那级；
  某级失败整级回滚且版本停在上一级；库比代码新时抛中文错误且库不变；同键去重；`task_id` 为 NULL 时同 kind 同 `event_seq` 去重；
  `commit` 注入游标写失败时通知随之回滚；列表新到旧与 `limit`、未读数；单条已读只改该条且不覆盖已有 `read_at`；全部已读返回实际改动 id；
  `resetLedger` 后旧通知 `event_seq` 为 NULL、新账本同键通知可写入；偏好读写；`unreadByTask`。`store-warnings.test.ts` 3 个：子进程装过滤后
  SQLite 告警不出现、其它 ExperimentalWarning 仍出现；不装过滤时同一条告警出现；`start.js` 在导入 `main.ts` 之前装过滤）。
  Web 全量 22+2=24 个文件、225+17=242 个用例，0 跳过；desktop 104 个、103 过 1 跳过（同基线）；lint、typecheck 无错误。
- 负向对照：唯一约束写成普通的 `UNIQUE (kind, task_id, event_seq)` 时，「`task_id` 为 NULL 时去重」用例失败；去掉「库比代码新」的检查时，
  该用例失败；`commit` 拆成两个事务时，「注入游标写失败」用例失败。

### M-02 验收契约

- 验收命令：`npm test --prefix apps/web -- test/notifier-rules.test.ts test/notifier.test.ts test/notifier-real.test.ts`；然后全量四件。
- 预期：三文件共 24 个用例全过。`notifier-rules.test.ts` 11 个：6 条规则各 1 个（输入为「实测事件原文」seq 21、45、124、116、102、53 的原行，
  断言 kind、级别、标题，如「T-4 停在失败上：会话异常退出（会话退出码 3）」）；表外事件不产生通知（列举 §3 的全部表外状态与事件）；9 种
  `FailureReason` 加未知 kind 的渲染（含空白压缩与截断）；告警标题随 kind 与 `default_branch` 变化；规则表恰 6 行、kind 不重复、`levelOf` 与
  §5.4 一致；「进入」（seq 124/145/150 两次进入，连续相同只算一次）。`notifier.test.ts` 11 个（临时库 + 假 `call`）：首次启动从
  `latest_seq` 开始；停服前已处理的重启后不再生成；停服期间的重启后按序补齐且各推一次；「通知已写、游标未推进」重启不重不推；注入游标写失败
  重启后补上且只一条；换账本重置游标、旧 `event_seq` 置空、日志一行、之后正常；补齐抛无 `code` 错误时游标停在该事件前、下次补上；补齐抛带
  `code` 错误时照样生成且 `project_id` 为 NULL；超过一页分页拉完且单飞（并发 `pull` 只有一个 `events.since` 在途、期间再 `pull` 合并为一次）；
  被唯一约束挡下不推送、推送消息恰为 5 个键；`close` 后不调 `call`。`notifier-real.test.ts` 2 个（真实 `automed` 经 `CoreSupervisor`、真实
  `autome` 跑 `w4-ledger.sh`）：先 `pull` 建游标再跑 fixture，再 `pull` 后通知按 kind 计数为 await_design_approval 3、await_merge 2、done 2、
  failed 3、await_info 2、project_alert 1，且 `project_id` 均为该项目 id；关库后 fixture 再造一条失败事件，重开后只多出这一条。
  Web 全量 27 个文件、266 个用例，0 跳过；desktop 同基线。
- 负向对照：先推进游标、后补齐（补齐失败也推进）时，「补齐抛无 `code` 错误」用例失败；首次启动从 0 开始时，「首次启动」用例与真实账本
  第 1 个用例失败（计数变多）；规则表把 `done` 的级别写成 `need_you` 时，`done` 规则用例与 `levelOf` 用例失败。

### M-03 验收契约

- 验收命令：`npm test --prefix apps/web -- test/notifier-api.test.ts`；`npm test --prefix apps/web -- test/notifier-live.test.ts`
  （需要监听，被 `listen EPERM` 阻断时照实记录交审计）；然后全量四件。
- 预期：`notifier-api.test.ts` 7 个用例：列表形状与 `level`、`limit` 缺省 / 上限 / 非法 400；单条已读写库并广播 `notification-read`；全部已读同上；
  无改动时 200 且不广播；非法请求体（`ids` 非字符串数组、为空、超 200、`ids` 与 `all` 同给）400 且不写库；`attachNotifier.onTick` 同步返回、
  `bus.pull` 完成后才调 `notifier.pull`、`notifier.pull` 挂起时后续 tick 的 `bus.pull` 照常完成；`onStatus(connected)` 触发一次 `notifier.pull`。
  `notifier-live.test.ts` 3 个用例（`startServer`、临时 home、真实内核、假 CLI 造出 `session_crashed` 失败）：`<home>/state/web.sqlite3` 已建，
  两条 `/ws` 连接都收到恰为 5 个键的 `notification`（`kind: "failed"`、标题「T-1 停在失败上：会话异常退出（会话退出码 3）」）；连接 A 带
  Origin POST 单条已读后连接 B 收到 `notification-read`、`GET` 未读数为 0，不带 Origin 的 POST 返回 403 且库不变；停服后在服务外再造一条失败，
  重启后列表恰 2 条、第一条未重复。Web 全量 29 个文件、276 个用例，0 跳过；desktop 同基线。
- 负向对照：`onTick` 里 `bus.pull()` 与 `notifier.pull()` 并发启动时，`attachNotifier` 顺序用例失败；已读后不广播时，单条已读用例与 live 第 2 个
  用例失败；POST 路由注册在 `securityMiddleware` 之前时，live 第 2 个用例的 403 断言失败。

### M-04 验收契约

- 验收命令：`npm test --prefix apps/web -- test/bell-client.test.ts test/bell-view.test.ts`；然后全量四件；再 `npm run build --prefix apps/web`。
- 预期：`bell-client.test.ts` 6 个用例（假 socket、假 fetch）：`hello` 才触发拉取，`notification` 与 `notification-read` 触发重新拉取，拉取完成后把
  「消息 + 刷新后的列表」交给 `onNotification`（拉取失败不交）；拉取单飞（在途时来两条消息只补拉一次）；`markRead` / `markAll` 的 POST 体与失败时的中文错误且计数不变；
  `tapSocket` 只用 `addEventListener` 旁听、返回同一个 socket，`createLiveClient` 经它建连接时每次连接只创建一个 socket（计数 1）、其 `onmessage` 不被覆盖
  （`event` 仍交给 `onEvents`），断线重连新建的 socket 再次被旁听、重连 `hello` 后重新拉取、`stop` 后不再重连；控制器忽略 `event` / `core` / `reload` 与非 JSON。
  `bell-view.test.ts` 7 个用例（`renderToString`）：徽标 0 / 1 / 99 / 100→「99+」与中文 `aria-label`；展开列表的标题、正文、时间、未读圆点与「标为已读」按钮，无未读时「全部已读」禁用；
  目标：默认表（W6/W7 前 `available: false`）下任务通知与项目告警都不渲染链接、显示「详情页尚未开放」，注入 `available: true` 的表后链到 `/tasks/T-3`、`/projects/<id>`，
  无任务无项目的不出链接；空列表与加载失败文案；容器 `position:fixed`、面板 `max-height` 与 `overflow-y:auto`；用 `createRoutesStub` 渲染 root 的 `App`，HTML 里同时有
  铃铛按钮与子路由内容，且 `root.tsx` 源码里不再直接调用 `useLiveEvents(`（只剩 `useLiveWithBell`，保证只有一条连接）；守卫用例：读 `app/routes.ts` 文本，
  `tasks` / `projects` 路由出现与否与 `TARGETS` 的 `available` 一致。Web 全量 31 个文件、289 个用例，0 跳过；desktop 同基线；build 通过。
- 负向对照：收到 `notification` 时在客户端自增未读数而不重新拉取时，`bell-client` 第 2 个与重连用例失败；`tapSocket` 改成 `socket.onmessage = …` 覆盖时，`tapSocket` 用例失败；
  徽标阈值写成 `> 100` 时 100→「99+」断言失败；`TARGETS` 的 `available` 改成 `true`（而 `routes.ts` 没有对应路由）时，守卫用例与默认表的无链接断言失败。

### M-05 验收契约

- 验收命令：`npm test --prefix apps/web -- test/bell-system.test.ts test/bell-view.test.ts`；然后全量四件；再 `npm run build --prefix apps/web`。
- 预期：`bell-system.test.ts` 9 个用例（假 `SystemNotifier`、实现了独占 / 共享 `request` 与 `query` 的假 locks、假 storage、假 `Date.now`，多个标签页共用一份 locks 与 storage）：
  (1)「需要你」+ 后台 + 已授权 + 新鲜未读 → 无需等待、`show` 一次且 `tag` 为通知 id；(2) 知会级、已读的条目、超过 60 秒的陈旧条目、列表里找不到的 id 都不弹；
  (3) 另一标签页可见（持有 `autome-visible`）时后台标签页不弹但认领，随后另一后台标签页不弹，可见标签页自己迟到处理也不弹（覆盖「后台先到」「前台先到」两种顺序，
  含处理时差 500 ms 以上）；可见性跟踪随 `visibilitychange` 与卸载释放锁；(4) 两个后台标签页同一通知只弹一次；(5) n1 弹出后再认领 150 条其它 id，迟到标签页处理 n1 仍不弹
  （记录按 10 分钟清理而非按条数）；时间推进超过 10 分钟后旧记录被清掉，且此时 n1 已过新鲜期仍不弹；(6) `default` / `denied` / `unsupported` 不弹也不请求权限，`request` 只在调用
  `request()` 时发生；(7) 没有 `navigator.locks`（或 `localStorage` 抛错）时只看本标签页可见性、直接弹；(8) 点击系统通知 `focus`，`href` 为 `/tasks/T-3` 时跳转，
  `href` 为 `null` 时只 `focus` 不跳转；(9) `browserSystemNotifier` 传给 `Notification` 的参数，以及 `NotificationBell` 使用传入的 `systemNotifier` 而不碰全局 `Notification`。
  `bell-view.test.ts` 在 M-04 的 7 个之上加 1 个：权限 `default` 显示「开启系统通知」按钮、`denied` 显示拒绝提示、`granted` / `unsupported` 不显示。两文件共 17 个。
  Web 全量 32 个文件、299 个用例，0 跳过；desktop 同基线；build 通过。
- 负向对照：去掉跨标签页认领时，用例 (4) 失败；挂载时就调 `request()` 时，用例 (6) 失败；不判断级别时，用例 (2) 失败；去掉对 `autome-visible` 的检查时，用例 (3) 失败；
  把认领记录改回只留最近 100 条（按条数淘汰）并去掉新鲜期判断时，用例 (5) 失败。

## 人工验收清单

- H-01 开两个标签页打开面板，在 A 的铃铛里点一条「已读」，B 的未读数不刷新页面即减少；在 A 点「全部已读」，B 的徽标消失。
- H-02 点铃铛里的「开启系统通知」，浏览器弹出授权框；允许后切到别的标签页，用 `autome` 让一个任务进入等你批准，系统弹出「T-x 设计已定稿，等你批准」横幅；点横幅回到面板标签页，W7 路由上线前停留在当前页（不进 404），上线后跳到 `/tasks/T-x`。铃铛里该条标题是纯文本并显示「详情页尚未开放」。
- H-03 两个面板标签页都放在后台时造出一条「需要你」通知，系统横幅只出现一次；有一个面板标签页在前台（另一个在后台）时不弹横幅；这是 Web Locks（`autome-visible` / `autome-notify`）在真实浏览器里的唯一验证，失败即退回 M-05。
- H-04 在授权框里选「拒绝」，铃铛面板显示「系统通知已被浏览器拒绝，铃铛照常提醒」，铃铛计数与列表照常工作。
- H-05 1512×944 窗口下页面本身无纵向滚动条；铃铛列表条目多于一屏时只在面板内部滚动。
- H-06 用 Node 22（`/opt/homebrew/opt/node@22/bin/node apps/web/server/start.js`）启动面板，终端里不出现 `SQLite is an experimental feature` 告警。
- H-07 开着面板的标签页在开发者工具 Network 的 WS 过滤下只看到一条 `/ws` 连接。

## Backlog

- B-01 `app/components/bell-live.tsx` 的 `useLiveWithBell()` 与 `app/lib/live.tsx` 的 `useLiveEvents()` 有约 15 行重复，因为 `app/lib/` 不在本任务范围；W5 搬外壳时给 `createLiveClient` 加扩展消息回调，删掉 `tapSocket` 与重复的 hook，`root.tsx` 改回 `useLiveEvents()`。
- B-03 评审 N-01：「进入」的记忆不持久（§3），seq 不同但状态相同的两条事件若恰好跨重启，会比不中断多一条通知。当前设计接受这个最坏结果（不漏发）；若 W6 之后觉得吵，再把最近状态存进 `event_cursor` 的另一个名字下。
- B-04 评审 N-02：实现时「最近状态」的记忆只在事件确定处理完之后更新（§3 已写明）；`notifier.test.ts` 的「补齐抛无 `code` 错误」用例里补一步：重试同一事件仍生成通知。
- B-02 W6 的「等你处理」角标从 `store.unreadByTask(<levelOf 为 need_you 的 kind>)` 取数，由 W5 的 `getLoadContext` 把库对象交给 loader。

## 争议项
