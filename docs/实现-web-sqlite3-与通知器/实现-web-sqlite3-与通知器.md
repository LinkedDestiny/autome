# Web 通知（W4）：web.sqlite3 面板库、事件→通知规则与实时推送、顶栏铃铛已读与后台系统通知

status: 实现中
design-round: 2/15
implementation-round: 8/35
current-milestone: M-07
current-milestone-reopens: 0
convergence-mode: normal
next-action: 独立审计 M-07 补齐失败重试与负向对照，并在可监听环境复验全量检查

任务文件：`docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-task.md`。
本任务的规则见 `docs/实现-web-sqlite3-与通知器/protocol/loop-protocol.md` 与
`docs/实现-web-sqlite3-与通知器/protocol/session-protocol.md`。

## 背景与现状

基线 `f21065b`（任务分支，含 W1–W3、K0–K2）。下面是与本任务有关的现状，均为本轮读源码或实测所得。

- **W3 总线**（`apps/web/server/eventbus.ts`）：游标在内存里，首次连上内核时从 `latest_seq` 开始，不推历史；`pull()` 单飞，
  `onTick` 里 `void bus.pull()`。挂点：`subscribe`、`broadcast(ExtensionMessage)`、`onClientMessage`、`onDisconnect`。
  `bus.pull()` 自己吞掉错误，返回的 Promise 不会 reject。
- **浏览器端**：`app/lib/live-client.ts` 的 `switch (message.type)` 只处理 `hello / event / reload / core`，其它类型直接忽略，
  也没有把扩展消息交给调用方的回调。`app/root.tsx` 用 `app/lib/live.tsx` 的 `useLiveEvents()` 建每个标签页唯一的一条 `/ws`；
  `createLiveClient` 自己用 `socket.onmessage` 属性收消息，消息处理只在 `!stopped && current === socket` 时进行。
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
Bug 的「不是 Bug」节点另跑了一本账本（脚本 `.autome/output/w4-not-a-bug/run.sh`，复用上面脚本的前半段，只建一个 Bug 任务、假 CLI 两轮都写
`status: 不是 Bug`，约 2 秒，不入库），同样是 `events.since` 原始输出经 `jq -c` 的结果，序号与任务 id 属于那本账本：

```text
{"kind":"task.updated","payload":{"state":{"node":"locate","state":"active"},"trigger":"slot_available"},"seq":5,"subject":"T-1"}
{"kind":"task.updated","payload":{"state":{"node":"not_a_bug","state":"active"},"trigger":"bug_session_ended"},"seq":9,"subject":"T-1"}
```

同一账本 `task.get` 的 `task` 为 `{"id":"T-1","kind":"bug","title":"not bug","state":{"node":"not_a_bug","state":"active"}}`（`jq` 取子集）。

`task.get`（参数 `task_id`）实测顶层键：`brief, decisions, documents, gate, next_role, pending_decisions, project, queue_position,
retry, sessions, so_far, status_block, status_error, status_error_detail, task, worktree`；其中 `project` 原文为
`{"default_branch":"main","id":"prj-f5a8e540132845849a830aa90fa0667a","name":"repo"}`，`task` 带 `id, kind, slug, title, state` 等。

## 目标与非目标

目标：覆盖任务 R-01..R-07。按规则把内核账本事件变成通知、落进 `<AUTOME_HOME>/state/web.sqlite3`，经 W3 的 `/ws` 推送；
顶栏铃铛显示未读数与列表，支持单条已读、全部已读，各标签页实时同步；「需要你」级通知在标签页后台时发系统通知。

非目标（任务 §3「明确不做」原样适用）：任务卡角标（只提供查询，见方案 §2）、外壳与 W5 各项、`getLoadContext`、W6–W12 的页面与
Electron 侧、偏好设置界面、改 W3 总线语义或 W2 安全语义、令牌与登录、新增 npm 依赖、改 `crates/` 等目录。

用户表态（2026-11-11 驳回第 1 版定稿时给出，优先于任务文件的对应条款）：
- 规则表增加第 7 行「进入 `not_a_bug`」（任务 R-02 要求加规则须由用户决定，此即用户决定），见 §3。
- 范围放宽到 `app/lib/live-client.ts`：给 `createLiveClient` 加消息回调，铃铛经它接收推送，`root.tsx` 保持 `useLiveEvents()`，见 §6。
  回调要到达铃铛，必须由创建连接的 `useLiveEvents()` 把它传进去，所以 `app/lib/live.tsx` 随之改一行（传入 `onMessage`），除此之外不动 `app/lib/`。
- 系统通知只按本标签页 `document.hidden` 与权限判断，靠 `tag` 由系统合并重复，不做跨标签页协调，见 §7。

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
| `app/lib/live-client.ts` | `createLiveClient` 加可选 `onMessage(type, message)`；新增 `subscribeLiveMessages(listener)` / `publishLiveMessage(type, message)`（§6） | 否 |
| `app/lib/live.tsx` | 只改一行：`useLiveEvents()` 建连接时传 `onMessage: publishLiveMessage` | — |
| `app/components/bell-client.ts` | 铃铛的拉取控制器（不依赖 React、不自己建连接）：`start()`、`handleMessage(type, message)`、单飞拉取、已读 POST | 否 |
| `app/components/bell-targets.ts` | 通知目标路由表与 `resolveTarget(item, targets)`（§7 跳转） | 否 |
| `app/components/bell-system.ts` | 系统通知接口 `SystemNotifier`、浏览器实现、是否弹出的判定 `shouldShow` | 否 |
| `app/components/bell.tsx` | `NotificationBell` 组件：挂载时 `subscribeLiveMessages` 把消息交给控制器，卸载时退订 | 否 |
| `app/root.tsx` | 保持 `useLiveEvents()` 不变，只在 `LiveBanner` 之后加 `<NotificationBell />` | — |

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

`notifier-rules.ts` 导出 `NOTIFICATION_RULES`（7 行数据）与 `matchEvent(event)`；通知器按表逐行匹配，第一条匹配的规则生效。
前 6 行是计划 §5.4 原文；第 7 行 `not_a_bug` 是用户 2026-11-11 的决定（见「目标与非目标」）。

| 事件 `kind` | 匹配 `payload.state` 子集 | 通知 `kind` | 级别 | 标题模板 | 补齐 |
|---|---|---|---|---|---|
| `task.updated` | `{"state":"active","node":"await_design_approval"}` | `await_design_approval` | `need_you` | `{task} 设计已定稿，等你批准` | 任务 |
| `task.updated` | `{"state":"active","node":"await_merge"}` | `await_merge` | `need_you` | `{task} 可以合并了` | 任务 |
| `task.updated` | `{"state":"active","node":"await_info"}` | `await_info` | `need_you` | `{task} 无法复现，需要更多信息` | 任务 |
| `task.updated` | `{"state":"failed"}` | `failed` | `need_you` | `{task} 停在失败上：{reason}` | 任务 |
| `project.alert` | —（按 `payload.kind` 取告警标题） | `project_alert` | `need_you` | `{alert}` | 项目 |
| `task.updated` | `{"state":"done"}` | `done` | `fyi` | `{task} 已合并` | 任务 |
| `task.updated` | `{"state":"active","node":"not_a_bug"}` | `not_a_bug` | `need_you` | `{task} 被判定不是 Bug，等你决定` | 任务 |

- 表外一律不产生通知：其它 `active` 节点（如 `locate`）、`draft`、`queued`、`paused`、`stopped`、`cancelled`，以及
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
   「[通知] 账本已更换（最新序号 X 小于游标 Y），从 X 继续」，本轮结束。重置时同时清空进程内的「每任务最近状态」记忆与补齐缓存：新账本会复用 T-1 之类的任务编号，旧账本留下的状态会抑制新账本的首次进入、旧缓存会把旧项目信息补给新任务（评审 N2-01，实现注意事项；`notifier.test.ts` 的换账本用例顺带断言）。新账本在游标之前的事件不补（无法判断哪些已通知过）。
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

**浏览器端怎么收到推送（只有一条 `/ws`，满足 R-05）**：沿用 `root.tsx` 里 `useLiveEvents()` 建的那一条连接，给它加一个消息出口。
- `createLiveClient` 新增可选 `onMessage?: (type: string, message: { type: string } & Record<string, unknown>) => void`。每条解析成功、
  是对象且 `type` 为字符串的消息，在内置处理（`hello / event / reload / core` 的现有分支，行为一字不改）之后交给 `onMessage`；扩展类型
  （`notification`、`notification-read` 及以后的新类型）只交给它。旧 socket 上的迟到消息、`stop()` 之后的消息、非 JSON 不交（沿用现有的
  `stopped || current !== socket` 判断）。内置类型也交出，是为了让订阅者从 `hello` 得知首次连上或重连。
- 同文件新增一个进程内的订阅表：`subscribeLiveMessages(listener)` 返回退订函数，`publishLiveMessage(type, message)` 依次调用全部订阅者，
  单个订阅者抛错被捕获，不影响其它订阅者，也不影响 `createLiveClient` 的内置处理。
- `useLiveEvents()` 传入 `onMessage: publishLiveMessage`（`live.tsx` 唯一的改动）。`NotificationBell` 在 `useEffect` 里 `subscribeLiveMessages`
  把消息交给控制器，卸载时退订；服务端渲染不执行 `useEffect`，不会订阅。
- 结果：**每个标签页始终只有一条 `/ws`**（`root.tsx` 的 `useLiveEvents()`），不新增服务端通道，不轮询；重连与退避仍由 `createLiveClient`
  负责，控制器不管。订阅表里没有订阅者时 `publishLiveMessage` 什么都不做，其它页面不受影响。

控制器 `start()` 时拉取一次（铃铛挂载可能晚于首个 `hello`）；之后只关心三类消息：`hello`（重连后重新拉取，补上断线期间的变化）、
`notification`、`notification-read`（都触发重新拉取），其余（`event`、`core`、`reload`）忽略。

**不维护副本**：铃铛的未读数和列表一律来自 `GET /api/notifications`，收到消息只触发重新拉取，不在客户端自增自减。拉取单飞：在途时再来消息
只记一次「再拉」。`notification` 触发的拉取结束后，不论成败，控制器都把「这条消息 + 刷新后的列表」交给 `onNotification`（§7 的系统通知从列表里取该条的 `body` 与 `project_id`）；拉取失败时列表为 `null`，铃铛照常显示「通知加载失败：…」，系统通知走 §7 的降级。理由：消息是服务端已提交游标后唯一一次推送，不会重放，不能让补齐正文的 GET 失败连带吞掉提醒（评审 D2-P01）。单条已读 / 全部已读发 POST 后重新拉取；其它标签页靠 `notification-read` 广播重新拉取，于是一个标签页点已读，所有标签页
的计数都会变。

### 7. 铃铛界面与系统通知（R-06、R-07）

- **挂法**：`NotificationBell` 自带数据订阅，接受可选 `systemNotifier`（见下文）。`root.tsx` 只加一行 `<NotificationBell />`；容器 `position: fixed`、右上角，
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
- **何时弹**：收到 `notification` 消息、刷新结束后（§6，成败不论），`shouldShow({ message, permission, hidden })` 只看三件事，全部满足就 `show`：
  `levelOf(message.kind) === 'need_you'`（「知会」级只进铃铛，R-07）；`permission() === 'granted'`；本标签页 `document.hidden === true`。
  不看别的标签页，不做跨标签页认领，不用 Web Locks、`localStorage`，也没有新鲜期或保留期。
  - `show` 的参数：`id`、`title` 取消息自带的；`body` 取列表里同 id 条目的 `body`（列表为 `null`、找不到或为空时不带正文）；`href` 为
    `resolveTarget(列表条目 ?? { task_id: message.task_id, project_id: null })`；刷新失败（列表为 `null`）与找不到条目走同一降级，只靠消息自带的 `id`、`title`、`task_id`。
  - **多个后台标签页**：每个后台标签页都会 `show` 同一条，但 `tag` 都是通知 id，由系统按 `tag` 把它们合并成一条（同 tag 的新通知替换旧的，
    `renotify` 保持缺省 `false`，替换时不再次提醒）。不同通知 id 不同，互不合并。
  - 已知取舍（用户 2026-11-11 选定）：一个标签页在前台、另一个在后台时，后台那个仍会弹；标签页被冻结后迟到处理时，可能在已读之后才弹。两者都只多一条横幅，不漏发。
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
- **多标签页去重依赖系统按 `tag` 合并**：`Notification` 的 `tag` 合并是标准行为，但各浏览器 / 系统通知中心的表现（是否闪一下、是否再响）
  本机无法在实现轮验证；单测只能断言每个标签页传的 `tag` 都等于通知 id，真实合并效果由 H-03 人工验收。
- **改动 `app/lib/`**：只加可选回调与订阅表，内置四类消息的处理不变；W3 已有的 `live-client.test.ts` 不改、照常全过，作为回归保护。
- **`not_a_bug` 的事件形状**：实测账本只有一本跑出过它（见「实测事件原文」第二段），M-02 的 fixture 也要造出它（`w4-ledger.sh` 加一个
  「不是 Bug」的 Bug 任务），真实账本用例按 kind 计数时包含它。
- 每个里程碑报 Web 与 desktop 的用例总数和与基线的差，`skipped` 必须为 0（desktop 基线自带的 1 个跳过除外，报告时单列）；
  负向对照逐条实跑并写退出码，全部还原后 `git status --porcelain` 为空。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 面板库 web.sqlite3：结构、迁移、去重、游标与已读读写，过滤 SQLite 告警 | 0 | store |
| M-02 | 已完成 | 规则表与通知器：持久游标补拉、补齐信息、崩溃与换账本，真实账本验证 | 0 | notifier |
| M-03 | 已完成 | 接线与推送：通知器挂到 tick 之后，铃铛接口，notification 与 notification-read 广播 | 0 | api |
| M-04 | 已完成 | 顶栏铃铛：未读数、列表、单条与全部已读、跨标签页同步、挂到 root | 0 | bell |
| M-05 | 已完成 | 后台标签页系统通知：可替换接口、权限按钮、按通知 id 作 tag 合并重复 | 0 | system-notify |
| M-06 | 已完成 | B-01：createLiveClient 消息回调独立验收（实现随 M-04 接线） | 0 | live-client |
| M-07 | 待审 | B-04：同一事件补齐失败后重试生成通知的独立回归验收（实现随 M-02） | 0 | notifier |

## 验收说明

所有验收命令在仓库根执行，每条单独运行、单独记退出码，不用 `&&` 串起来。下文「全量四件」指：
`npm test --prefix apps/web`、`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/desktop`。

| 里程碑 | 覆盖 | 前置 | 新增 / 改动文件 |
|---|---|---|---|
| M-01 | R-01 | 无 | `server/store.ts`、`server/store-warnings.ts`、`server/start.js`、`test/store.test.ts`、`test/store-warnings.test.ts` |
| M-02 | R-02、R-03 | M-01 | `server/notifier-rules.ts`、`server/notifier.ts`、`test/notifier-rules.test.ts`、`test/notifier.test.ts`、`test/notifier-real.test.ts`、`test/fixtures/w4-ledger.sh` |
| M-03 | R-03、R-04（服务端）、R-05 | M-02 | `server/notifier-api.ts`、`server/main.ts`、`server/http.ts`、`test/notifier-api.test.ts`、`test/notifier-live.test.ts` |
| M-04 | R-04（浏览器端）、R-05（单连接）、R-06 | M-03 | `app/lib/live-client.ts`、`app/lib/live.tsx`（一行）、`app/components/bell-client.ts`、`app/components/bell-targets.ts`、`app/components/bell.tsx`、`app/root.tsx`、`test/bell-client.test.ts`、`test/bell-view.test.ts` |
| M-05 | R-07 | M-04 | `app/components/bell-system.ts`、`app/components/bell.tsx`、`test/bell-system.test.ts`、`test/bell-view.test.ts` |

### M-01 验收契约

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-01-r1-audit.md · 通过 · 审计 #1

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

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 验收命令：`npm test --prefix apps/web -- test/notifier-rules.test.ts test/notifier.test.ts test/notifier-real.test.ts`；然后全量四件。
- 预期：三文件共 25 个用例全过。`notifier-rules.test.ts` 12 个：7 条规则各 1 个（输入为「实测事件原文」seq 21、45、124、116、102、53 的原行，
  以及第二段 seq 9 的 `not_a_bug` 原行；断言 kind、级别、标题，如「T-4 停在失败上：会话异常退出（会话退出码 3）」「T-1 被判定不是 Bug，等你决定」）；
  表外事件不产生通知（列举 §3 的全部表外状态与事件，含第二段 seq 5 的 `locate`）；9 种 `FailureReason` 加未知 kind 的渲染（含空白压缩与截断）；
  告警标题随 kind 与 `default_branch` 变化；规则表恰 7 行、kind 不重复、`levelOf` 与 §3 表一致（`done` 为 `fyi`，其余 6 个为 `need_you`）；
  「进入」（seq 124/145/150 两次进入，连续相同只算一次）。`notifier.test.ts` 11 个（临时库 + 假 `call`）：首次启动从
  `latest_seq` 开始；停服前已处理的重启后不再生成；停服期间的重启后按序补齐且各推一次；「通知已写、游标未推进」重启不重不推；注入游标写失败
  重启后补上且只一条；换账本重置游标、旧 `event_seq` 置空、日志一行、之后正常；补齐抛无 `code` 错误时游标停在该事件前，下次 `pull`（补齐恢复正常）
  重试同一事件仍生成通知且只一条（「最近状态」记忆没有被失败的那次提前更新，原 Backlog B-04）；补齐抛带
  `code` 错误时照样生成且 `project_id` 为 NULL；超过一页分页拉完且单飞（并发 `pull` 只有一个 `events.since` 在途、期间再 `pull` 合并为一次）；
  被唯一约束挡下不推送、推送消息恰为 5 个键；`close` 后不调 `call`。`notifier-real.test.ts` 2 个（真实 `automed` 经 `CoreSupervisor`、真实
  `autome` 跑 `w4-ledger.sh`，fixture 比 `.autome/output/w4-events/run.sh` 多一个「不是 Bug」的 Bug 任务）：先 `pull` 建游标再跑 fixture，再 `pull`
  后通知按 kind 计数为 await_design_approval 3、await_merge 2、done 2、failed 3、await_info 2、project_alert 1、not_a_bug 1，且 `project_id`
  均为该项目 id；关库后 fixture 再造一条失败事件，重开后只多出这一条。
  Web 全量 27 个文件、267 个用例，0 跳过；desktop 同基线。
- 负向对照：先推进游标、后补齐（补齐失败也推进）时，「补齐抛无 `code` 错误」用例失败；补齐之前就更新「最近状态」记忆时，同一用例的重试步骤失败
  （重试被当成重复、不生成通知）；首次启动从 0 开始时，「首次启动」用例与真实账本第 1 个用例失败（计数变多）；规则表把 `done` 的级别写成
  `need_you` 时，`done` 规则用例与 `levelOf` 用例失败；漏掉 `not_a_bug` 一行时，该规则用例、「恰 7 行」用例与真实账本计数失败。

### M-03 验收契约

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-03-r4-audit.md · 已完成（审计通过） · 审计 #3

- 验收命令：`npm test --prefix apps/web -- test/notifier-api.test.ts`；`npm test --prefix apps/web -- test/notifier-live.test.ts`
  （需要监听，被 `listen EPERM` 阻断时照实记录交审计）；然后全量四件。
- 预期：`notifier-api.test.ts` 7 个用例：列表形状与 `level`、`limit` 缺省 / 上限 / 非法 400；单条已读写库并广播 `notification-read`；全部已读同上；
  无改动时 200 且不广播；非法请求体（`ids` 非字符串数组、为空、超 200、`ids` 与 `all` 同给）400 且不写库；`attachNotifier.onTick` 同步返回、
  `bus.pull` 完成后才调 `notifier.pull`、`notifier.pull` 挂起时后续 tick 的 `bus.pull` 照常完成；`onStatus(connected)` 触发一次 `notifier.pull`。
  `notifier-live.test.ts` 3 个用例（`startServer`、临时 home、真实内核、假 CLI 造出 `session_crashed` 失败）：`<home>/state/web.sqlite3` 已建，
  两条 `/ws` 连接都收到恰为 5 个键的 `notification`（`kind: "failed"`、标题「T-1 停在失败上：会话异常退出（会话退出码 3）」）；连接 A 带
  Origin POST 单条已读后连接 B 收到 `notification-read`、`GET` 未读数为 0，不带 Origin 的 POST 返回 403 且库不变；停服后在服务外再造一条失败，
  重启后列表恰 2 条、第一条未重复。`notifier-live-fixture.test.ts` 另有 1 项真实 CLI 回归：与 live 共用禁用重试的夹具，首次退出 3 后没有 `session.retry`，且只有一条终态 `failed`；临时项目显式设置 `[retry] attempts = 0`、`backoff_seconds = 0`。Web 全量 30 个文件、278 个用例，0 跳过；desktop 同基线。
- 负向对照：`onTick` 里 `bus.pull()` 与 `notifier.pull()` 并发启动时，`attachNotifier` 顺序用例失败；已读后不广播时，单条已读用例与 live 第 2 个
  用例失败；POST 路由注册在 `securityMiddleware` 之前时，live 第 2 个用例的 403 断言失败。

### M-04 验收契约

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-04-r5-audit.md · 通过 · 审计 #4

- 验收命令：`npm test --prefix apps/web -- test/bell-client.test.ts test/bell-view.test.ts`；然后全量四件；再 `npm run build --prefix apps/web`。
- 预期：`bell-client.test.ts` 6 个用例（假 socket、假 fetch，不监听端口）：(1) `start()` 拉取一次，`hello`、`notification`、`notification-read` 各触发重新拉取，
  `notification` 的拉取结束后把「消息 + 刷新后的列表」交给 `onNotification`（拉取失败时仍交，列表为 `null`，铃铛显示加载失败文案），`event` / `core` / `reload` 不触发拉取；(2) 拉取单飞（在途时来两条消息只补拉一次）；
  (3) `markRead` / `markAll` 的 POST 体与失败时的中文错误且计数不变；(4) `createLiveClient` 的 `onMessage`：`hello` / `event` / `core` / `reload` 照旧走内置回调
  （`event` 仍交给 `onEvents`、`reload` 仍触发 `onReload`）且之后也交给 `onMessage`，`notification` / `notification-read` 只交给 `onMessage`、不触发任何内置回调，
  非 JSON、没有字符串 `type` 的、旧 socket 上的迟到消息、`stop()` 之后的消息都不交；不传 `onMessage` 时行为与现有一致；(5) 订阅表：两个订阅者都收到，
  退订后不再收到，一个订阅者抛错时另一个照常收到、`createLiveClient` 的 `onEvents` 照常被调；(6) 串起来：`createLiveClient({ onMessage: publishLiveMessage })` + 订阅了
  控制器的订阅表，一次连接只创建一个 socket（计数 1），收到 `notification` 后控制器重新拉取，断线重连后的 `hello` 再拉取一次，`stop` 后不再重连也不再拉取。
  `bell-view.test.ts` 7 个用例（`renderToString`）：徽标 0 / 1 / 99 / 100→「99+」与中文 `aria-label`；展开列表的标题、正文、时间、未读圆点与「标为已读」按钮，无未读时「全部已读」禁用；
  目标：默认表（W6/W7 前 `available: false`）下任务通知与项目告警都不渲染链接、显示「详情页尚未开放」，注入 `available: true` 的表后链到 `/tasks/T-3`、`/projects/<id>`，
  无任务无项目的不出链接；空列表与加载失败文案；容器 `position:fixed`、面板 `max-height` 与 `overflow-y:auto`；用 `createRoutesStub` 渲染 root 的 `App`，HTML 里同时有
  铃铛按钮与子路由内容，且源码守卫保证只有一条连接：`root.tsx` 仍调用 `useLiveEvents()`，`app/components/bell*` 的源码里不出现 `createLiveClient(` 与 `WebSocket`；守卫用例：读 `app/routes.ts` 文本，
  `tasks` / `projects` 路由出现与否与 `TARGETS` 的 `available` 一致。已有的 `live-client.test.ts` 不改、照常全过。
  Web 全量 32 个文件、291 个用例，0 跳过；desktop 同基线；build 通过。
- 负向对照：收到 `notification` 时在客户端自增未读数而不重新拉取时，用例 (1) 与 (6) 失败；`onMessage` 只在 `switch` 的 `default` 分支调用（内置类型不交出）时，
  用例 (4) 与 (6) 的重连后再拉取失败；`publishLiveMessage` 不捕获订阅者异常时，用例 (5) 失败；铃铛自己 `createLiveClient` 建第二条连接时，`bell-view` 的源码守卫失败；
  徽标阈值写成 `> 100` 时 100→「99+」断言失败；`TARGETS` 的 `available` 改成 `true`（而 `routes.ts` 没有对应路由）时，守卫用例与默认表的无链接断言失败。

### M-05 验收契约

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-05-r6-audit.md · 通过 · 审计 #5

- 验收命令：`npm test --prefix apps/web -- test/bell-system.test.ts test/bell-view.test.ts`；然后全量四件；再 `npm run build --prefix apps/web`。
- 预期：`bell-system.test.ts` 8 个用例（假 `SystemNotifier`、假 `document.hidden`、假 `Notification` 构造函数，不需要真实浏览器）：
  (1)「需要你」+ `document.hidden` 为 `true` + 已授权 → 拿到列表后立即 `show` 一次，参数为消息的 `id` / `title`、列表条目的 `body`、`resolveTarget` 得出的 `href`；
  (2) 知会级（`done`）不弹；(3) `document.hidden` 为 `false` 时不弹（即使「需要你」且已授权）；(4) `default` / `denied` / `unsupported` 不弹也不请求权限，`request` 只在调用 `request()` 时发生；
  (5) 两个后台标签页（两个控制器）处理同一条：各 `show` 一次、传给 `Notification` 的 `tag` 都等于该通知 id，另一条通知的 `tag` 是它自己的 id；
  (6) 列表里找不到该 id、或刷新失败（列表为 `null`）时仍弹，用消息自带的 `title`、不带正文，`href` 由消息的 `task_id` 经 `resolveTarget` 得出；(7) 点击系统通知 `focus`，`href` 为 `/tasks/T-3` 时跳转，
  `href` 为 `null` 时只 `focus` 不跳转；(8) `browserSystemNotifier` 传给 `Notification` 的是 `(title, { body, tag: id })`，`NotificationBell` 使用传入的 `systemNotifier` 而不碰全局 `Notification`，
  且 `bell-system.ts` 源码里不出现 `navigator.locks` 与 `localStorage`（简化后的源码守卫）。
  `bell-view.test.ts` 在 M-04 的 7 个之上加 1 个：权限 `default` 显示「开启系统通知」按钮、`denied` 显示拒绝提示、`granted` / `unsupported` 不显示。两文件共 16 个。
  Web 全量 33 个文件、300 个用例，0 跳过；desktop 同基线；build 通过。
- 负向对照：不判断级别时，用例 (2) 失败；判断 `visibilityState === 'visible'` 写反（可见时弹）时，用例 (1) 与 (3) 失败；`tag` 用固定字符串或不传时，用例 (5) 与 (8) 失败；
  挂载时就调 `request()` 时，用例 (4) 失败；列表找不到条目或刷新失败就放弃时，用例 (6) 失败（`bell-client` 用例 (1) 的「拉取失败仍交」断言同样失败）。

### M-06 验收契约

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-06-r7-audit.md · 通过 · 审计 #6

- 用户 2026-11-11 将 B-01 纳入为独立里程碑；实现随其调用方 M-04 一起交付，本里程碑独立核对 §6 的消息回调契约，不重复实现。
- 命令：`npm test --prefix apps/web -- test/live-client.test.ts test/bell-client.test.ts`；然后全量四件。
- 验收：M-04 用例 (4)(5)(6) 全过，原有 live-client 用例不改、无跳过；执行 M-04 的回调只放 default 分支与订阅异常两项负向对照。

### M-07 验收契约

最新证据：docs/实现-web-sqlite3-与通知器/evidence/M-07-r8-impl.md · 专项通过，全量环境阻断，待审 · 实现 #8

- 用户 2026-11-11 将 B-04 纳入为独立里程碑；补步已是 M-02 的强制契约，本里程碑对其作独立回归验收，不重复实现。
- 命令：`npm test --prefix apps/web -- test/notifier.test.ts`；然后全量四件。
- 验收：无 code 的离线和超时均停在事件前；恢复后重试同一事件只生成一条、推一次；提前更新最近状态的负向对照必须失败。

## 人工验收清单

- H-01 开两个标签页打开面板，在 A 的铃铛里点一条「已读」，B 的未读数不刷新页面即减少；在 A 点「全部已读」，B 的徽标消失。
- H-02 点铃铛里的「开启系统通知」，浏览器弹出授权框；允许后切到别的标签页，用 `autome` 让一个任务进入等你批准，系统弹出「T-x 设计已定稿，等你批准」横幅；点横幅回到面板标签页，W7 路由上线前停留在当前页（不进 404），上线后跳到 `/tasks/T-x`。铃铛里该条标题是纯文本并显示「详情页尚未开放」。
- H-03 两个面板标签页都放在后台时造出一条「需要你」通知，系统通知中心里这条只留一条（同 tag 合并）；再造一条不同的通知，出现第二条；面板标签页在前台时它自己不弹。这是 `tag` 合并在真实浏览器里的唯一验证；结果由用户合并前确认，不触发里程碑退回。
- H-04 在授权框里选「拒绝」，铃铛面板显示「系统通知已被浏览器拒绝，铃铛照常提醒」，铃铛计数与列表照常工作。
- H-05 1512×944 窗口下页面本身无纵向滚动条；铃铛列表条目多于一屏时只在面板内部滚动。
- H-06 用 Node 22（`/opt/homebrew/opt/node@22/bin/node apps/web/server/start.js`）启动面板，终端里不出现 `SQLite is an experimental feature` 告警。
- H-07 开着面板的标签页在开发者工具 Network 的 WS 过滤下只看到一条 `/ws` 连接。

## Backlog

- B-01 已纳入独立里程碑 M-06（实现随 M-04，用户 2026-11-11 意见 2）：`createLiveClient` 的消息回调，见 §6、M-04。
- B-03 用户 2026-11-11 裁定忽略、不做；评审 N-01：「进入」的记忆不持久（§3），seq 不同但状态相同的两条事件若恰好跨重启，会比不中断多一条通知。当前设计接受这个最坏结果（不漏发）；若 W6 之后觉得吵，再把最近状态存进 `event_cursor` 的另一个名字下。
- B-04 已纳入独立里程碑 M-07（实现随 M-02，用户 2026-11-11）：重试同一事件仍生成通知的补步写进「M-02 验收契约」的 `notifier.test.ts` 用例。
- B-02 用户 2026-11-11 裁定忽略、不做：W6 的「等你处理」角标从 `store.unreadByTask(<levelOf 为 need_you 的 kind>)` 取数，由 W5 的 `getLoadContext` 把库对象交给 loader。

## 争议项
