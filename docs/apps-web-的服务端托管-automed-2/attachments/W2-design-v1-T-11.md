# Web 内核网关：apps/web 托管 automed、读写白名单、3 秒 tick 与本机安全检查

status: 实现中
design-round: 1/15
implementation-round: 1/25
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01：在允许回环监听的环境重跑验收命令，确认 Web 65 个用例全过后标待审

任务文件：`docs/apps-web-的服务端托管-automed/apps-web-的服务端托管-automed-task.md`。
本任务的规则见 `docs/apps-web-的服务端托管-automed/protocol/loop-protocol.md` 与
`docs/apps-web-的服务端托管-automed/protocol/session-protocol.md`。

## 背景与现状

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 把 Electron 渲染进程换成浏览器面板。
W1 已在 `apps/web` 建好 React Router 8 + express 5 的自定义服务器（`server/main.ts` 的
`startServer({port, mode})`、`server/http.ts`、`server/config.ts` 的 `HOST = '127.0.0.1'`）。
本任务 W2 在同一个进程里接上内核：托管 `automed` 子进程、读写白名单、3 秒 tick、§5.5 本机安全。

设计轮实测的现状（2026-11-10，本 worktree，基线 `df63c20`）：

- `npm test --prefix apps/web`：5 个文件、**38** 个用例全过（ci-config、config、node-version、server、sqlite-probe）。
- `npm test --prefix apps/desktop`：**104** 个用例，103 过、1 跳过。其中迁移来源五个文件分别是
  framing 4、core-restart 5、ipc-gate 10、write-gate 28、sidecar 17，合计 64，全过。
- 本机：Node v26.5.0；`target/debug/automed` 存在；`~/.local/bin/automed` 不存在；本会话能监听 127.0.0.1。
- 迁移来源都是 CommonJS：`apps/desktop/src/{framing,sidecar,core-restart,ipc-gate,write-gate}.js`。
  桌面端的 tick 循环和写操作分派写在 `apps/desktop/main.js` 里（`callCore`、`registerWriteChannel`、
  `startSidecar`、`restartCore`、`startTicking`）。这几段不是独立模块，要按行为重写成 TS 模块。
- 桌面端门禁的拒绝原因是英文（`request must be a JSON object`、`params exceed the 4096-byte limit` 等），
  sidecar 也有英文错误（`already pending`、`exited before replying`）。约束 8 要求用户可见文字是中文，
  所以迁移时要改成中文，对应断言也随之改动（见方案 §6）。
- 桌面端写门禁**允许** `task.create.attachments` 带任意路径（原用例 11 断言 `['/tmp/a.pdf']` 通过）。
  §5.5 要求「附件只能是上传后服务端生成的路径」，所以 Web 这一条必须收紧。
- `dispatch.rs` 的 `READ_METHODS` 有 17 项。桌面读白名单是其中 13 项的子集，不含
  `project.probe`、`config.validate`、`events.since`、`env.detect`。
- `scheduler.tick` 拿驱动锁用 `try_acquire`（非阻塞，`driver_lock.rs`），拿不到就回
  `skipped: true`，这时 `latest_seq` 照样返回。锁是 `<db>.driver.lock` 上的 `flock`。
- Vite 8.3.7 的 middleware 模式在 `createServer` 期间同步执行 `httpServer.on('upgrade', hmrServerWsListener)`
  （`node_modules/vite/dist/node/chunks/node.js` 约 24563 行）。也就是说 `await createViteServer()`
  返回时，HMR 的 upgrade 监听器已经挂在 W1 的 http server 上了。

## 目标与非目标

目标（R 编号见任务文件 §2）：

- R-01：把 framing、sidecar、core-restart、读写白名单迁到 `apps/web/server/`（ESM + TS），原测试迁到
  `apps/web/test/`（vitest）。每条原用例的断言意图都保留，改动逐条列在方案 §6。
- R-02：automed 按 `AUTOMED_BIN` → 发布包内 → `~/.local/bin` → cargo 构建目录的顺序查找。
- R-03：内核崩溃后按原退避策略重启，连续快速失败就停下，等 `core.restart`。
- R-04：§5.5 的四条检查各有测试。
- R-05：tick 每 3 秒一次。返回 `skipped` 时不记日志、不算错误。

非目标（任务文件 §3.2）：`events.since` 拉取、WebSocket 消息协议、通知、`web.sqlite3`、`server.json`、
页面与组件、附件上传的实现（W6）、日志跟随（W8）、目录补全与 `project.probe` 流程（W9）、`autome web`
与发布包组装（W10）、Electron 改造与删除 desktop 旧实现（W11）、迁移 `login-path.js`。
本任务不改 `apps/web/package.json`、`ci.yml`、`ci-local.sh`，**不新增依赖**，所以没有争议项。

## 方案

### 1. 模块布局（全部在 `apps/web/server/`）

| 文件 | 职责 | 来源 |
|---|---|---|
| `core/framing.ts` | `encodeFrame`、`FrameDecoder`（4 字节大端长度前缀） | framing.js |
| `core/sidecar.ts` | `AutomedSidecar`、`killIfRunning`：spawn，stdout 只走协议帧、stderr 只走诊断，按 `request_id` 关联回复；新增 `requestUntilReply()`（见 §4） | sidecar.js + 新增 |
| `core/binary.ts` | `resolveAutomedBinary()`（R-02） | sidecar.js `defaultBinaryPath` 重写 |
| `core/restart-policy.ts` | `nextRestart`、`QUICK_FAILURE_MS=5000`、`MAX_QUICK_FAILURES=3`、`RESTART_DELAY_MS=1000` | core-restart.js 原样 |
| `core/supervisor.ts` | `CoreSupervisor`：启动、对账 tick、崩溃重启、手动重启、状态、`call()`、`stop()` | main.js 相应段落 |
| `ticker.ts` | `createTicker`：3 秒 tick、不叠加、`skipped` 安静 | main.js `startTicking` |
| `gates/read-gate.ts` | 读白名单与形参校验 | ipc-gate.js |
| `gates/write-gate.ts` | 写白名单与形参校验（与读门禁互不 import） | write-gate.js |
| `home.ts` | `resolveAutomeHome(env)`：与 `config_io.rs::default_global_dir` 一致，有 `AUTOME_HOME` 用它，否则 `$HOME/.autome` | 新 |
| `security.ts` | Host/Origin 检查、Cookie 解析、令牌（生成、落盘、兑换、轮换）、express 中间件、upgrade 守卫 | 新 |
| `gateway.ts` | 浏览器请求到内核的路由：门禁 → 转发或本地处理；`/api/read`、`/api/write` | 新 |
| `main.ts`、`http.ts` | `startServer` 接入以上模块；`close()` 依次停 ticker、内核、HTTP | 改 |

所有用户可见文字（拒绝原因、离线原因、启动日志）用中文，方法名、字段名照写原文。

### 2. automed 查找（R-02）

`resolveAutomedBinary({ env, home, webRoot, exists })` 返回 `{ path, source }`。参数全部可以注入，测试不碰真实
`$HOME`，也不改 `process.env`。

| 顺序 | source | 位置 | 条件 |
|---|---|---|---|
| 1 | `env` | `env.AUTOMED_BIN` | 非空就用，不检查是否存在（与 desktop 一致） |
| 2 | `bundle` | `<webRoot>/../automed` | 存在才用 |
| 3 | `local-bin` | `<home>/.local/bin/automed` | 存在才用 |
| 4 | `cargo` | `<repo>/target/<AUTOMED_CARGO_PROFILE 或 debug>/automed` | 兜底，不存在也返回，让启动失败时说出这个路径 |

`webRoot` 是 `apps/web` 目录，即 `resolve(import.meta.dirname, '../..')`，按 `server/core/` 的深度算。
发布包的约定：W10 把构建好的 `apps/web` 作为包内的 `web/` 目录，`automed` 与 `autome` 放在 `web/` 旁边，
所以「同一发布包里的」就是 `<webRoot>/../automed`。在开发仓库里这个位置是 `apps/automed`，不存在，会被跳过。
测试会锁住这个相对位置，W10 照它摆放。只支持 macOS，不处理 `.exe`。

### 3. 内核守护（R-03）

`CoreSupervisor({ binaryPath, home, dbPath?, log, now, onStatus })`：

- 状态取值：`starting`（已 spawn，对账 tick 未返回）、`connected`、`restarting`（等 1 秒延迟）、`offline`（放弃重启，
  带 `reason` 和 `attempts`）、`stopped`。每次状态变化调用 `onStatus`，W3 拿它推 `core` 消息。
- 子进程环境：`{...process.env, AUTOME_HOME: home}`，内核账本因此落在 `<home>/state/automed.sqlite3`。
- 启动后立即发一次 `scheduler.tick` 作对账（与 desktop 启动时那次一致），回复后转为 `connected`。对账走与周期 tick 同一条
  `supervisor.tick()` 路径（§4）：本地超时不等于内核已返回，超时后状态保持 `starting`、不再发第二条 tick，
  真实回复到达才转 `connected`，进程退出则走重启策略。
- 退出时：`ranForMs = now() - startedAt`，交给 `nextRestart`。`restart` 为真就转 `restarting`，`delayMs` 后重新
  spawn；为假就转 `offline`，`reason = sidecar.failureReason()`，不再自动重启。
- 二进制不存在时，spawn 发出 `error`，被 sidecar 收敛到 `_finish`，按一次快速失败计，进程本身不崩溃。
- `restart()`（即 `core.restart`）：内核在 `starting`/`connected` 时返回 `{already_running: true}`。否则把计数
  清零，启动，并用一次 tick 验证；失败（含验证 tick 超时）就抛出 `failureReason()` 或超时原因，成功返回 `{restarted: true}`。
  验证超时时进程仍在、状态为 `starting`，再次 `core.restart` 返回 `{already_running: true}`，不会叠加第二个子进程或第二条 tick。
- `stop()`：标记 stopping，清掉待执行的重启定时器，再调 `sidecar.stop()`（关 stdin，2 秒后 SIGKILL）。
  主动停止不算崩溃。
- `call(method, params)`：只在 `starting`/`connected` 时可用，否则抛 `内核未连接：<reason>`。命令格式与 desktop
  `callCore` 相同（`request_id: web-req-N`、`command_id: web-cmd-N`、`expected_revision: null`、
  `protocol_version: 1`）。内核回 `outcome.status === 'error'` 时抛错，带上 `code` 和内核原话。

### 4. tick（R-05、约束 4）

`createTicker({ call, isConnected, isBusy, intervalMs = 3000, log, onTick })`。

**「上一次 tick 结束」的定义**：内核真的回了 Reply，或承载它的子进程已经退出。本地等待超时**不算**结束——
`AutomedSidecar.request()` 超时只是删掉 pending 项并 reject，命令已经写进 stdio 管道，内核仍会处理（`main.rs` 的 stdio
主循环同步处理每条命令，后到的帧只能排队，取消不了）。若超时就放行下一拍，t=3s 的 tick A 处理超过 5 秒、t=9s 又发 tick B，
就在管道里排了第二条，违反「不叠加」。因此：

- `AutomedSidecar` 增加 `requestUntilReply(command)`：不设本地超时，Promise 在真实 Reply 到达时 resolve，进程退出时
  reject；并增加 `isOutstanding(requestId)`：已发出、Reply 未到、进程未退出期间为真。普通 `request()` 的行为与迁移
  用例保持不变（超时仍 reject，迟到回复仍被忽略）。
- `supervisor.tick({ timeoutMs = 5000 })` 用 `requestUntilReply` 发送，自己与超时计时器赛跑：超时则向调用方 reject
  「tick 超时」，但在途状态保持，由 `supervisor.isTickBusy()` 暴露（对账 tick 与周期 tick 共用）。真实 Reply 到达，
  或进程退出（含 `stop()`、崩溃、重启），在途状态才清除；迟到的对账 Reply 照样把 `starting` 转为 `connected`。
- ticker 的 `isBusy` 取 `supervisor.isTickBusy()`，同时自己也保留一个本地在途标志，两者任一为真都不发。

| 情形 | 行为 |
|---|---|
| 到点但 `isConnected()` 为假 | 不发 |
| 到点但上一次 tick 还没结束（含已本地超时、Reply 尚未到、进程仍在） | 不发，也不排队（不叠加）；Reply 到达或进程退出后的下一拍恢复 |
| 回复 `skipped: true` | 不记日志、不算错误；`onTick(report)` 照常调用，W3 靠它在别人驱动时也能拉事件 |
| 回复 `errors` 非空 | 每条记一行 `[调度] <原文>` |
| 调用失败（超时、内核退出） | 记一行 `[调度] tick 失败：<原因>`；超时的下一拍按上一行规则仍不发（Reply 未到），内核退出后随重启恢复 |
| `stop()` 之后 | 不再发 |

网关不碰驱动锁，轮流推进完全交给内核的 `try_acquire`。

### 5. 读写白名单与网关路由（R-01、R-04 第 3/4 条、约束 3）

两份门禁各自独立：互不 import，也都不 import `apps/desktop`。各自用测试解析 `dispatch.rs`，与 `READ_METHODS`
交叉核对。

- **读门禁**：方法表照搬 desktop 的 13 项。`project.probe`、`events.since`、`env.detect`、`config.validate`
  只供服务端内部调用，浏览器不能读。新增一条规则：参数里的字符串值不能长得像路径（复用 `looksLikeAPath`），
  因为读方法的参数只有各种 id 和 `component`。
- **写门禁**：op 表、`ROLES`、各种长度上限照搬（`MAX_PROSE_LENGTH 8000`、`MAX_DOCUMENT_LENGTH 256K`、
  标识符 512、`MAX_PAYLOAD_JSON_LENGTH 32K`、数组 256 项、每项 1024）。导出 `COMPONENTS = ['git','claude','codex','iterm2']`。
  两处收紧，都是因为浏览器不能给出任意路径，且内核对这两个字段的值不做路径校验就存下并拼进任务整理提示词
  （`dispatch.rs` 的 `doc_refs` 原样转交 `create`，`launcher.rs` 把它们作为「用户指定的仓库内参考文档」写进提示词）：
  1. `task.create.attachments` 的每一项必须是上传编号（小写 UUID），不能是路径。
  2. `task.create.doc_refs` W2 只接受空数组或缺省；非空一律拒绝「网页端暂不支持指定参考文档」。参考文档的 Web 形态
     （服务端可解析的引用标识）由后续任务设计，届时再放开；W2 不让浏览器提供的路径进入任务。
  固定文件名枚举不属于任意路径，保持原样：`project.onboarding.save` 只收 `AGENTS.md` 或 `docs/agent-project-profile.md`。
- **网关路由**（`gateway.write`，门禁通过之后）：

| op | 处理 |
|---|---|
| `core.restart` | `supervisor.restart()`，不转发给内核 |
| `project.pick`、`project.confirm`、`open.path`、`open.terminal`、`env.install`、`env.login` | desktop 在 Main 里做的事，Web 由后续任务（W9 等）通过 `gateway.registerLocalOp(op, handler)` 接上。W2 没有注册时回 501「网页端还不支持「<op>」」，内核不会收到 |
| `task.create` | 用 `uploads.resolve(id)` 把每个附件编号换成服务端路径。解析不出，或者路径不在 `<home>/web/uploads/` 下，就拒绝「附件 <id> 不存在或已过期，请重新上传」。W2 的默认 resolver 一律返回 null，W6 换成真实实现 |
| 其余 op | 原样转发，op 名就是内核方法名 |

门禁之后，转发给内核的 `task.create` 参数里只可能有已解析的服务端附件路径，不会有 `doc_refs`。

- **服务端内部调用**用 `gateway.core.call(method, params)`（即 `supervisor.call`），不经过浏览器门禁。W3 的
  `events.since`、W9 的 `project.probe` 走这条路。
- 「日志路径只能来自内核」在网关层的落法：没有任何浏览器 op 能带日志路径；`open.path` 的 `log` 只认
  `session_id`；`session.log` 只收 `session_id`。用一条源码核对测试锁住：解析 `dispatch.rs` 分派表里取
  `str_param(p, "path")` 的方法（现在是 `project.probe`、`project.add`），断言它们不在 Web 的读写白名单里。

### 6. 原测试迁移表（R-01 要求列出每一处改动）

没列出的原用例都原样迁移，断言意图不变（使用 vitest 注册用例，保留 `node:assert` 原断言）。

| 原用例 | 去向与改动 | 理由 |
|---|---|---|
| framing「oversized frame」`/exceeds max/` | 改为 `/超过上限/` | 约束 8，消息中文化 |
| ipc-gate #8 oversized `/limit/` | 改为 `/上限/` | 同上 |
| ipc-gate #9「only the packaged origin is trusted」 | 移到 `security.test.ts`，改测 Origin 检查（两个可信源，以及 `autome://app`、`null`、其它端口、https 等不可信源） | Web 没有 `autome://app`。信任锚换成 Origin + Host + 令牌，意图（只有自家源能调用）不变 |
| ipc-gate #10「component names」 | 移到 `write-gate.test.ts`，对照对象从 `renderer/lib/labels.js` 的 `COMPONENT_ORDER` 换成写门禁的 `COMPONENTS`，`environment.rs` 的解析与 derive 检查照搬 | apps/web 没有 labels。Web 实际发给内核的组件名只在 `env.install/env.login` 的校验表里 |
| write-gate #10 `/null byte/` | `/空字符/` | 约束 8 |
| write-gate #11 第一条断言 | `attachments: ['/tmp/a.pdf']` 从「通过」改为「拒绝」，另用上传编号作为通过例；其余三条断言不变 | §5.5：附件只能是服务端生成的路径 |
| write-gate #11 补充 | 新增断言：`doc_refs` 为 `['/tmp/outside.md']`、`['../../outside.md']`、`['~/.ssh/config']` 各被拒绝，空数组与缺省仍通过 | §5.5：浏览器不能给出任意路径；见 D1-P01 |
| write-gate #17 `/path/` | `/路径/` | 约束 8 |
| write-gate #18 `/unknown open kind/` | `/未知的打开类型/` | 约束 8 |
| write-gate #23 `/limit/` | `/上限/`。payload 仍是 200×1000 的字符串，先撞到总长上限 | 约束 8 |
| sidecar #1「packaged location wins」 | 换成 `core-binary.test.ts` 的四级查找用例 | Web 没有 `process.resourcesPath`。意图（发布包优先于开发构建、只在存在时；覆盖变量优先于一切）由 R-02 用例完整承接 |
| sidecar #2「no test starts a real core against ~/.autome」 | 扫描范围从本文件扩到 `apps/web/test/*.ts`，凡是 `new AutomedSidecar(`、`new CoreSupervisor(`、`startServer(` 及 spawn `npm run dev/start` 的地方，附近必须出现 `AUTOME_HOME`/`home:` 或不可启动的 `binaryPath` | 约束 6。W2 之后启动服务器就会启动内核 |
| sidecar #3「binary is built」 | 缺二进制时：设了 `CI` 就让真实内核用例 `skipIf` 跳过，并打印一行中文原因；没设 `CI`（本机、ci-local、门禁）仍然失败，并提示 `cargo build -p automed` | 风险 1：CI 的 web job 不构建内核，又不改 ci.yml |
| sidecar「already pending」「exited before replying」 | 改为中文消息对应的正则 | 约束 8 |

真实内核用例用的二进制是 `process.env.AUTOMED_BIN || <repo>/target/debug/automed`，用 `binaryPath` 显式传入，
不改 `process.env`。每个用例各自有 `mkdtemp` 出来的临时 `AUTOME_HOME`。

### 7. 本机安全（R-04 第 1/2 条）

请求按下表依次检查，任何一步失败都回 403 和中文原因（JSON `{ok:false, error}`；页面请求回纯文本）。
`<port>` 取监听后 `server.address().port`，所以端口 0 也能用。

| 请求 | Host | `?t=` 兑换 | Origin | 会话 Cookie |
|---|---|---|---|---|
| GET/HEAD | 必须 | 带 `t` 时兑换 | — | — |
| 其它方法（POST 等，含 RR action 与 `/api/*`） | 必须 | — | 必须 | 必须 |
| WebSocket upgrade | 必须 | — | 必须 | 必须 |

- **Host**：小写后必须正好等于 `127.0.0.1:<port>` 或 `localhost:<port>`。缺失、缺端口、端口不对、IPv6、
  `localhost.evil.com:<port>` 都拒绝。`/healthz` 也要检查。
- **Origin**：必须正好等于 `http://127.0.0.1:<port>` 或 `http://localhost:<port>`。缺失、`null`、https、
  带路径都拒绝。
- **令牌**：服务启动时生成 32 字节随机数（hex），写到 `<home>/web/token`。目录权限 0700，文件 0600，
  先写临时文件再 rename。`GET` 带 `?t=` 时用 `timingSafeEqual` 比较（先比长度）：
  - 对上了：生成随机会话 id 放进内存集合，`Set-Cookie: autome_session_<port>=<id>; HttpOnly; SameSite=Strict; Path=/`，
    **立即轮换**令牌并重写文件，然后 303 跳到去掉 `t` 的同一地址（其它参数保留）。
  - 没对上：403「访问令牌无效或已经用过，请重新运行 autome web 打开面板」。

  这样 `?t=` 只能用一次，而文件里始终有一枚可用的新令牌，W10 的「重复执行 `autome web` 时打开已有服务」
  读文件就行。Cookie 名带端口，因为 Cookie 不区分端口，开发和生产两个实例会互相覆盖。会话只存在内存里，
  服务重启后要重新用带令牌的地址打开（见 Backlog B-04）。
- **开发时拿令牌**：`npm run dev` 启动时在 `已启动：…` 那行之后多打一行
  `打开面板：http://127.0.0.1:<port>/?t=<token>`（只在开发模式打）。生产模式由 W10 读令牌文件。
- **upgrade 守卫**：`createHttpServer` 在挂完 Vite 之后，取出 server 上已有的全部 `upgrade` 监听器
  （开发模式下就是 Vite HMR 那个），移除它们，换成唯一一个守卫监听器。守卫检查不通过时，往 socket 写
  `HTTP/1.1 403 Forbidden` 加中文原因，然后销毁。通过时交给取出来的那些监听器和 `registerUpgrade(path, handler)`
  注册的处理器（W3 在这里接 `/ws`）。都没有时回 404 并销毁。守卫必须是唯一的 upgrade 监听器，测试断言
  `listenerCount('upgrade') === 1`。
- 中间件顺序：Host → 令牌兑换 → 非安全方法的 Origin + Cookie → `/healthz` → `/api/*`
  （`express.json({limit: '1mb'})`，JSON 坏了回 400 中文）→ Vite 或静态资源 → RR。

### 8. HTTP 接口与 `startServer`

- `POST /api/read`，请求体 `{method, params}`；`POST /api/write`，请求体 `{op, params}`。
  - 成功：200 `{ok:true, payload}`。
  - 门禁拒绝：400 `{ok:false, error:'已拒绝：<中文原因>'}`。
  - 本地 op 还没接上：501。
  - 内核不在线：503 `{ok:false, error:'内核未连接：<原因>'}`。
  - 内核报错：409 `{ok:false, error:<内核原话>, code}`。
- `startServer({ port, mode, home?, binaryPath? })`：`home` 默认取 `resolveAutomeHome(process.env)`，
  `binaryPath` 默认取 `resolveAutomedBinary().path`。listen 成功之后才启动内核和 ticker，所以端口被占时
  不会留下子进程。返回 `{ server, gateway, close }`，`close()` 依次停 ticker、内核、HTTP。
- W1 的 `test/server.test.ts` 必须改，因为接入网关之后，启动服务器就会启动内核：
  - `launch()` 的子进程环境加上临时 `AUTOME_HOME` 和 `AUTOMED_BIN`，不再碰真实 `~/.autome`；
  - 「只监听 IPv4」用例给 `startServer` 传临时 `home`；
  - 新增两条用例：开发启动日志带一次性令牌地址，以及开发模式下不合格的 HMR upgrade 被 403。

## 风险与验证安排

1. **CI 没有 automed**（任务风险 1）：不改 ci.yml。真实内核用例在 `CI` 下跳过并说明原因；本机和门禁下缺二进制
   就直接失败，不会挂起（sidecar 的 `_releasePipes` 照搬，避免 desktop 当年挂到 runner 超时的老问题）。
   要在 CI 上真跑，需要给 web job 加构建步骤，记在 Backlog B-01，由用户决定。
2. **回环监听被沙箱拒绝**（L-01）：Host、Origin、Cookie、令牌兑换、upgrade 守卫、路由都写成纯函数，或用假的
   req/res/socket 测，不监听端口。只有 `gateway-http.test.ts` 和 W1 的 `server.test.ts` 需要监听。实现轮如果遇到
   `listen EPERM`，在证据里写明被拦住的用例数，以及不依赖监听的那部分结果，交审计轮复验。
3. **Vite 的 upgrade 监听器绕过守卫**：已确认 Vite 8.3.7 在 `createServer` 期间同步挂监听器（见背景）。
   用 `listenerCount === 1` 和开发模式 403 用例锁住。假如以后 Vite 改成延迟挂，这条断言会先失败。
4. **基线门禁不全绿**（L-04/L-06）：范围外失败不修，在 Backlog 写一条：用例全名、断言原文、机制。
5. **两份白名单并存**：desktop 那份原样保留，`npm test --prefix apps/desktop` 每个里程碑都要跑，结果必须仍是
   104 个用例、0 失败。
6. **用例计数**：每个里程碑的预期写的是 Web 全量用例总数。本机有二进制时，真实内核用例不允许跳过，
   `skipped` 必须为 0。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 帧、sidecar 与 automed 四级查找 | 0 | |
| M-02 | 开放 | 读写白名单迁移与路径拒绝 | 0 | |
| M-03 | 开放 | 内核守护：崩溃重启、手动重启与 3 秒 tick | 0 | |
| M-04 | 开放 | 本机安全：Host、Origin、一次性令牌与 upgrade 守卫 | 0 | |
| M-05 | 开放 | 网关接入 startServer 与端到端 | 0 | |

## 里程碑说明

### M-01 帧、sidecar 与 automed 四级查找

- 结果：`server/core/{framing,sidecar,binary}.ts` 能在 vitest 里驱动真实 automed 完成往返。这是不确定性最大的一步
  （ESM + 原生类型剥离 + 真实子进程）。
- 覆盖：R-01（framing、sidecar）、R-02、约束 2/6/7/8。前置：无。
- 新增测试：`test/core-framing.test.ts` 4 个、`test/core-binary.test.ts` 6 个（四级顺序各一个、`AUTOMED_BIN`
  为空串视同未设、发布包相对位置与 `source`）、`test/core-sidecar.test.ts` 17 个（原 17 个减去 #1，加 1 个新用例），共 27 个。新用例：用 `test/fixtures/fake-core.mjs`
  （读帧、延迟后回复的最小内核替身）验证 `requestUntilReply`——本地超时之后进程仍在、`isOutstanding` 仍为真；迟到的真实
  Reply 到达后 resolve 且 `isOutstanding` 变假；另一条请求在回复前进程退出则 reject 且 `isOutstanding` 变假；普通 `request()` 超时行为不变。

最新证据：docs/apps-web-的服务端托管-automed/evidence/M-01-r1-impl.md · 核心通过，全量监听受限，保持开放 · 实现 #1

### M-01 验收契约

- 验收命令：`cargo build -p automed && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：lint、typecheck 无输出错误；Web 共 65 个用例全过（基线 38 + 27），skipped 0；desktop 仍是 104 个用例，0 失败。
- 负向对照：把 `FrameDecoder` 的长度读成 `readUInt32LE`，framing 的分片解码用例和 sidecar 的真实往返用例都会失败；把查找顺序里的 `local-bin` 与 `cargo` 对调，core-binary 的第 3 个用例会失败；按到达顺序而不是 `request_id` 关联回复，sidecar「乱序回复各归其主」用例会失败。

### M-02 读写白名单迁移与路径拒绝

- 结果：`server/gates/{read-gate,write-gate}.ts` 两份独立门禁，与 `READ_METHODS`、`role.rs`、`environment.rs`
  交叉核对。附件只收上传编号，`doc_refs` 非空一律拒绝；浏览器不能指定任意文件路径（固定文件名枚举如 `project.onboarding.save` 的两个文档名不算）。
- 覆盖：R-01（两份白名单）、R-04 第 3/4 条（网关层）、约束 3/8。前置：无（纯函数，与 M-01 无依赖，编号在后只是顺序）。
- 新增测试：`test/read-gate.test.ts` 11 个（迁移 8 个 + 参数长得像路径的拒绝 + 中文消息 + `project.probe`/`events.since`
  等内部方法浏览器不可读）；`test/write-gate.test.ts` 35 个（迁移 28 个 + 从 ipc-gate 移来的组件名核对 + 附件必须是
  上传编号 + `doc_refs` 非空被拒（三种路径写法）、空数组通过 + 中文消息 + 解析 `dispatch.rs` 证明取 `path` 的方法不可达 + 两份门禁互不 import、不 import desktop
  + 长度上限钉死成字面值）。共 46 个。

### M-02 验收契约

- 验收命令：`npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 111 个用例全过（65 + 46），skipped 0；desktop 仍是 104 个，0 失败。
- 负向对照：往写白名单里加 `project.add`，「project.add 不可达」和「取 path 的方法不可达」两条用例会失败；放开附件让它收路径，「附件必须是上传编号」和迁移后的 #11 会失败；放开 `doc_refs` 非空，「doc_refs 非空被拒」会失败；读门禁 import 写门禁的任何导出，「互不 import」用例会失败；往读白名单里加一个不在 `READ_METHODS` 里的方法（如 `project.add`），交叉核对用例会失败。

### M-03 内核守护：崩溃重启、手动重启与 3 秒 tick

- 结果：`core/restart-policy.ts`、`core/supervisor.ts`、`ticker.ts` 按方案 §3、§4 的表格工作。
- 覆盖：R-01（core-restart）、R-03、R-05、约束 4。前置：M-01。
- 新增测试：
  - `test/core-restart.test.ts` 5 个（原样迁移）。
  - `test/core-supervisor.test.ts` 9 个：对账 tick 之后才算 connected；真实内核在注入时钟下活过 5 秒后被 SIGKILL，
    1 秒后重启且计数清零；`test/fixtures/automed-fail-fast.sh`（往 stderr 写一行 ERROR 后退出 1）连续 3 次就转
    offline，`reason` 引用 stderr，不再 spawn；offline 之后手动重启成功；手动重启仍失败时抛出内核原因；运行中手动重启
    返回 `already_running`；重启延迟期间 `stop()` 不再 spawn；二进制缺失时按快速失败计，测试进程不崩溃；对账 tick 超过 `timeoutMs` 仍未回复时（`fake-core.mjs` 延迟回复）状态保持
    `starting`、`isTickBusy()` 为真、再次 `restart()` 回 `already_running`，迟到 Reply 到达后转 `connected` 且 `isTickBusy()` 变假。
  - `test/ticker.test.ts` 9 个：用 vitest 假定时器覆盖方案 §4 表中 6 种情形，外加 9000ms 内恰好 3 次，以及「tick 已本地超时但 `isBusy()` 仍为真时，后续拍都不发；`isBusy()` 变假后的下一拍恢复」；另有一个真实内核
    用例：用 `perl` 对 `<home>/state/automed.sqlite3.driver.lock` 持有 `flock`，tick 回 `skipped: true`，且日志函数
    一次都没被调用。
  - 共 23 个。

### M-03 验收契约

- 验收命令：`cargo build -p automed && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 134 个用例全过（111 + 23），skipped 0；desktop 仍是 104 个，0 失败。
- 负向对照：`MAX_QUICK_FAILURES` 改成 4，「连续 3 次转 offline」会因为看到第 4 次 spawn 而失败；去掉在途标志，ticker「上一次未返回时不叠加」在 9 秒内会看到 3 次调用而不是 1 次；本地超时就清掉在途状态，「超时后仍不叠加」和 supervisor 的 `isTickBusy` 用例会失败；`skipped` 时记日志，「skipped 安静」和真实驱动锁用例都会失败；supervisor 不清理重启定时器，「延迟期间 stop」会看到新进程。

### M-04 本机安全：Host、Origin、一次性令牌与 upgrade 守卫

- 结果：`server/security.ts` 按方案 §7 实现。全部用纯函数或假的 req/res/socket 测，不监听端口。
- 覆盖：R-04 第 1/2 条、约束 5/6/8。前置：无（实现上独立；接入 HTTP 在 M-05）。
- 新增测试：`test/security.test.ts` 14 个：
  - Host：接受的写法 1 个，拒绝表 1 个；Origin：接受 1 个，拒绝表 1 个（承接 ipc-gate #9）。
  - 令牌：文件权限 0600、目录 0700；兑换后的 Cookie 属性与 303 去掉 `t`；同一令牌二次使用 403 且文件已轮换、新令牌可用；
    错误令牌 403 且不发 Cookie；长度不同的令牌比较不抛异常。
  - 方法分类：GET 不需要令牌；POST 缺 Cookie、Origin 错各拒绝；Origin 与 Cookie 都对时放行。
  - upgrade 守卫：用假 socket 验证拒绝时写 403 并销毁；通过时转给被取出的监听器。
  - 开发模式下 `createHttpServer` 之后 `listenerCount('upgrade') === 1`（不监听，只建 server）。
  - 畸形 Cookie 头不抛异常；所有拒绝原因都是中文。

### M-04 验收契约

- 验收命令：`npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 148 个用例全过（134 + 14），skipped 0；desktop 仍是 104 个，0 失败。
- 负向对照：Host 检查写成 `startsWith('localhost')`，拒绝表里 `localhost.evil.com:<port>` 那一行会失败；兑换后不轮换令牌，「二次使用 403」会失败；Cookie 漏掉 `SameSite=Strict`，Cookie 属性用例会失败；守卫用 `prependListener` 而没有移除 Vite 的监听器，`listenerCount === 1` 会失败。

### M-05 网关接入 startServer 与端到端

- 结果：`gateway.ts` 与 `/api/read`、`/api/write` 接进 `startServer`。从 HTTP 请求到真实内核整条链路可用，
  `close()` 后内核进程也退出。
- 覆盖：R-03（`core.restart` 经网关）、R-04 全部四条在 HTTP 层再验一遍、R-05（ticker 随服务启停）、约束 2/5/6。
  前置：M-01–M-04。
- 新增测试：
  - `test/gateway.test.ts` 8 个，用假 supervisor，不监听：读转发；读拒绝时内核收不到；写转发；`core.restart` 本地处理不转发；
    6 个本地 op 回 501 且内核收不到；附件编号解析与越界路径拒绝；离线 503；内核报错原话加 `code`。
  - `test/gateway-http.test.ts` 9 个，监听端口，真实内核，临时 home：带令牌的 `config.get`；`config.set_loop` 写了之后读得到；
    缺 Cookie、Origin 错、Host 错各被拒（Host 错用 `http.request` 自定义 Host 头，GET 也拒）；令牌经 HTTP 兑换后二次使用 403；
    不合格的 upgrade 403；合格但没有处理器的 upgrade 404；`close()` 后内核 pid 不存在；二进制缺失时服务照常启动、
    `/api/read` 回 503 且原因含「无法启动内核」；坏 JSON 回 400 中文。
  - `test/server.test.ts` 新增 2 个：开发启动日志带 `?t=` 地址，且与令牌文件内容一致；开发模式下没有 Cookie 的 HMR upgrade 回 403。
  - 共 19 个。

### M-05 验收契约

- 验收命令：`cargo build -p automed && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm run build --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 167 个用例全过（148 + 19），skipped 0；build 成功；desktop 仍是 104 个，0 失败；本轮之后跑测试，真实 `~/.autome/web/token` 不会被创建或修改（证据里给出测试前后 `stat` 的对比）。
- 负向对照：把 `core.restart` 当普通 op 转发，`gateway.test` 的「本地处理」会因内核收到命令而失败；把安全中间件挂在 `/api/*` 之后，「缺 Cookie 被拒」会拿到 200；`close()` 不停 supervisor，「close 后 pid 不存在」会失败；W1 的 `launch()` 不传临时 `AUTOME_HOME`，扫描用例（sidecar #2 的扩展版）会失败。

## 人工验收清单

- H-01 运行 `npm run dev --prefix apps/web`，在浏览器打开日志里「打开面板」那行地址：地址栏里的 `?t=` 消失，开发者工具里看到 `autome_session_<port>` Cookie 是 HttpOnly、SameSite=Strict。
- H-02 同一个开发会话里改一下 `app/routes/home.tsx` 的文字，页面不用手动刷新就更新（HMR 经过守卫后仍然可用）。
- H-03 在别的网站页面（如 https://example.com）的控制台执行 `fetch('http://127.0.0.1:7420/api/write', {method:'POST', body:'{}'})`，请求被拒，内核没有收到任何写操作。

## Backlog

- B-01 CI 的 web job 不构建 automed，真实内核用例在 CI 下会跳过。要真跑，就得在 `.github/workflows/ci.yml` 的 web job 加 `cargo build -p automed` 并同步改 `test/ci-config.test.ts`，由用户决定（本任务不改）。
- B-02 `apps/desktop/src/login-path.js`（登录 shell 的 PATH）没有迁移。W10 用 launchd 自启动时，内核看到的 PATH 会缺 `claude`/`codex`，届时需要处理。
- B-03 W5 起的 loader 要拿到 `gateway`：`startServer` 已经返回它，但还需要在 `http.ts` 的 `createRequestHandler` 上加 `getLoadContext`。W5 的目录不含 `server/`，计划需要给它补上这一处。
- B-04 会话 Cookie 只存在内存里，服务重启后要用新令牌重新打开。如果 W10 觉得不方便，可以改成用持久密钥签名的 Cookie。

## 争议项
