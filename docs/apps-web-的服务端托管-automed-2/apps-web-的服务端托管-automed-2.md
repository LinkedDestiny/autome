# Web 内核网关：apps/web 托管 automed、读写白名单、3 秒 tick 与 Host/Origin 本机检查

status: 实现中
design-round: 1/15
implementation-round: 4/25
current-milestone: M-05
current-milestone-reopens: 0
convergence-mode: normal
next-action: 实现 M-05 网关接入 startServer 与端到端

任务文件：`docs/apps-web-的服务端托管-automed-2/apps-web-的服务端托管-automed-2-task.md`。
本任务的规则见 `docs/apps-web-的服务端托管-automed-2/protocol/loop-protocol.md` 与
`docs/apps-web-的服务端托管-automed-2/protocol/session-protocol.md`。

## 背景与现状

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 把 Electron 渲染进程换成浏览器面板。
W1 已在 `apps/web` 建好 React Router 8 + express 5 的自定义服务器（`server/main.ts` 的
`startServer({port, mode})`、`server/http.ts` 的 `createHttpServer({mode})`、`server/config.ts` 的
`HOST = '127.0.0.1'` 与 `resolvePort`）。本任务 W2 在同一个进程里接上内核：托管 `automed` 子进程、
读写白名单、3 秒 tick、计划 §5.5 的本机安全。

本设计以上一版设计（任务 T-11，附件 `docs/apps-web-的服务端托管-automed-2/attachments/W2-design-v1-T-11.md`）
为基线。用户批准了它除鉴权以外的全部内容。本版有两处改动：一是去掉令牌与 Cookie，鉴权只做
Host 与 Origin 检查（R-07）；二是把附件的 Backlog B-01 纳入（R-06），让 CI 的 web job 构建内核，
真实内核用例在 CI 上实际运行。两处改动在全文各个受影响的位置都已改一致，逐条清单见方案 §10。

设计轮实测的现状（2026-11-10，本 worktree，HEAD `8e769ac`，基线 `7b672c7`）：

- `git log df63c20..HEAD` 里没有任何提交碰过 `apps/`、`crates/automed/src/{dispatch,driver_lock}.rs`、
  `.github/`、`scripts/`。附件在 `df63c20` 上测的基线数字因此仍然成立，下面几条已在本 worktree 重测确认。
- `npm test --prefix apps/web`：5 个文件、**38** 个用例全过（ci-config 6、config、node-version、server、
  sqlite-probe）。本会话能监听 127.0.0.1，W1 `server.test.ts` 的监听用例全部通过。`apps/web/node_modules` 已存在。
- `npm test --prefix apps/desktop`：**104** 个用例，103 个通过，1 个跳过（`electron dom harness`，原因是缺 electron
  二进制，与本任务无关），0 个失败。五个迁移来源文件的用例数：framing 4、core-restart 5、ipc-gate 10、
  write-gate 28、sidecar 17，合计 64，全部通过。
- 本机：Node v26.5.0；`target/debug/automed` 存在；`~/.local/bin/automed` 不存在。
- 迁移来源都是 CommonJS：`apps/desktop/src/{framing,sidecar,core-restart,ipc-gate,write-gate}.js`。
  桌面端的 tick 循环和写操作分派写在 `apps/desktop/main.js` 里（`callCore`、`registerWriteChannel`、
  `startSidecar`、`restartCore`、`startTicking`），不是独立模块，要按行为重写成 TS 模块。
- 桌面端门禁的拒绝原因是英文（如 `request must be a JSON object`、`params exceed the 4096-byte limit`），
  sidecar 也有英文错误（`already pending`、`exited before replying`）。约束 8 要求用户可见文字用中文，
  所以迁移时要改成中文，对应断言也随之改（见方案 §6）。
- 桌面端写门禁**允许** `task.create.attachments` 带任意路径（原 write-gate #11 断言 `['/tmp/a.pdf']` 通过）。
  §5.5 要求「附件只能是上传后服务端生成的路径」，所以 Web 这一条必须收紧。
- `dispatch.rs` 的 `READ_METHODS` 共 17 项。桌面读白名单是其中 13 项组成的子集，不含 `project.probe`、
  `config.validate`、`events.since`、`env.detect` 这四项。
- `scheduler.tick` 用 `try_acquire`（非阻塞，`driver_lock.rs`）拿驱动锁，拿不到就回 `skipped: true`，
  这时 `latest_seq` 照样返回。锁是 `<db>.driver.lock` 上的 `flock`，账本在 `<AUTOME_HOME>/state/automed.sqlite3`
  （`config_io.rs`）。
- **其它写命令会阻塞等锁**：`dispatch.rs` 的 `handle_command` 对 `scheduler.tick` 以外、不在 `READ_METHODS`
  里的方法，用 `driver_lock::acquire(…, DRIVER_LOCK_WAIT)` 最多等 **10 秒**，等不到就回
  「另一个 Autome（桌面端或命令行）正占着状态库，稍后再试」。desktop sidecar 默认的请求超时是 5 秒
  （`DEFAULT_REQUEST_TIMEOUT_MS = 5000`）。所以命令行正在驱动时，desktop 的写请求会先在本地超时，
  而内核稍后仍会执行这个写操作。Web 不能照搬这一点，见方案 §3。
- Vite 8.3.7 的 middleware 模式在 `createServer` 期间同步执行 `wsServer.on("upgrade", hmrServerWsListener)`
  （`node_modules/vite/dist/node/chunks/node.js` 第 24563 行；这里的 `wsServer` 就是 `hmr.server` 传进去的
  W1 http server）。也就是说，`await createViteServer()` 返回时，HMR 的 upgrade 监听器已经挂在 server 上了。
  Vite 自己的 HMR 连接还带一个 Vite 内部的 `webSocketToken` 校验（同文件 `shouldHandle`）：它只在开发模式下存在，
  属于 Vite 的实现细节，不是本服务的鉴权，R-07 不涉及它，本任务也不碰它。
- CI 与本地门禁（R-06 相关）：
  - `.github/workflows/ci.yml` 的 `web` job：`runs-on: macos-14`、`timeout-minutes: 15`、
    `defaults.run.working-directory: apps/web`，`run` 步骤依次是 `npm ci`、`npm run lint`、`npm run typecheck`、
    `npm test`、`npm run build`，没有 Rust 步骤。`desktop` job 没有工具链步骤，也没有 `Swatinem/rust-cache`，
    只有一步 `- name: Build the core the sidecar tests drive / run: cargo build -p automed`
    （靠仓库的 `rust-toolchain.toml` 和 runner 自带的 rustup 拿到工具链）。
  - 在 `apps/web` 目录里执行 `cargo build -p automed` 可以成功（cargo 会向上找到 workspace），
    所以 web job 的默认工作目录不用改。
  - 冷构建实测：`CARGO_TARGET_DIR=<空目录> cargo build -p automed` 在本机（14 核）用时 26 秒。
  - `scripts/ci-local.sh` 的 rust 段先跑 `cargo test --workspace --all-targets`，web 段在它之后。
    `cargo test -p automed --all-targets --no-run --message-format=json` 实测会产出
    `target/debug/automed`（集成测试会连带构建 bin target）。所以 ci-local 跑到 `Web test` 时内核已经构建好了；
    desktop 段同样靠这一点，它在 ci-local 里也没有单独的构建步骤。**结论：`ci-local.sh` 不改。**
  - 仓库没有 remote，ci.yml 实际不会被执行，`apps/web/test/ci-config.test.ts` 是它在本地唯一的守卫。
    其中「CI 按顺序运行四个 Web 检查」**逐字断言**了 `run` 步骤列表。

## 目标与非目标

目标（R 编号见任务文件 §2）：

- R-01：把 framing、sidecar、core-restart 和读写两份白名单迁到 `apps/web/server/`（ESM + TS），原测试迁到
  `apps/web/test/`（vitest）。每条原用例的断言意图都保留，改动逐条列在方案 §6。
- R-02：按 `AUTOMED_BIN` → 发布包内 → `~/.local/bin` → cargo 构建目录的顺序查找 automed（方案 §2）。
- R-03：内核崩溃后按原退避策略重启，连续快速失败就停下，等 `core.restart`（方案 §3）。
- R-04：§5.5 的五条检查各有测试（方案 §5、§7，测试分布见各里程碑说明）。
- R-05：tick 每 3 秒一次，返回 `skipped` 时不记日志、不算错误（方案 §4）。
- R-06：CI 的 `web` job 构建 automed，真实内核用例在 CI 上实际运行，不跳过；`ci-config.test.ts` 同步修改（方案 §9）。
- R-07：鉴权只做 Host 与 Origin 检查。没有令牌、Cookie、`?t=` 兑换和登录会话；带正确 Host 与 Origin、
  不带任何凭据的请求就能通过。反面也有测试锁住（方案 §7「R-07 的反面」）。

非目标（出现即越界）：

- `events.since` 拉取、WebSocket 消息协议（W3）；通知、`web.sqlite3`（W4）；`server.json`（W10）；
- 页面与组件（W5 起）；`getLoadContext`（W5，Backlog B-03）；
- 附件上传的实现（W6；本任务只留 `uploads.resolve` 接口，默认一律返回 null）；
- 日志跟随（W8）；目录补全与 `project.probe` 流程（W9）；`project.pick`、`open.*`、`env.*` 等本地 op 的实现
  （W9 等，本任务回 501）；
- `autome web` 与发布包组装（W10）；迁移 `login-path.js`（W10，Backlog B-02）；
- Electron 改造与删除 desktop 旧实现（W11）；
- 任何令牌、Cookie、登录会话、`?t=` 地址（R-07）。

本任务不改 `apps/web/package.json`、`package-lock.json`，**不新增依赖**；`.github/workflows/ci.yml` 只改 `web` job；
`scripts/ci-local.sh` 不改（理由见背景）；不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`。

## 方案

### 1. 模块布局（全部在 `apps/web/server/`）

| 文件 | 职责 | 来源 |
|---|---|---|
| `core/framing.ts` | `encodeFrame`、`FrameDecoder`（4 字节大端长度前缀） | framing.js |
| `core/sidecar.ts` | `AutomedSidecar`、`killIfRunning`：spawn 子进程，stdout 只走协议帧、stderr 只走诊断，按 `request_id` 关联回复；新增 `requestUntilReply()` 与 `isOutstanding()`（见 §4） | sidecar.js + 新增 |
| `core/binary.ts` | `resolveAutomedBinary()`（R-02） | sidecar.js `defaultBinaryPath` 重写 |
| `core/restart-policy.ts` | `nextRestart`、`QUICK_FAILURE_MS=5000`、`MAX_QUICK_FAILURES=3`、`RESTART_DELAY_MS=1000` | core-restart.js 原样 |
| `core/supervisor.ts` | `CoreSupervisor`：启动、对账 tick、崩溃重启、手动重启、状态、`call()`、`stop()` | main.js 相应段落 |
| `ticker.ts` | `createTicker`：3 秒一次 tick，不叠加，`skipped` 时安静 | main.js `startTicking` |
| `gates/read-gate.ts` | 读白名单与参数校验 | ipc-gate.js |
| `gates/write-gate.ts` | 写白名单与参数校验（与读门禁互不 import） | write-gate.js |
| `home.ts` | `resolveAutomeHome(env)`：与 `config_io.rs::default_global_dir` 一致，有 `AUTOME_HOME` 就用它，否则用 `$HOME/.autome` | 新 |
| `security.ts` | Host 检查、Origin 检查、按请求方法分类的 express 中间件、upgrade 守卫。不含任何令牌或 Cookie 逻辑（R-07） | 新 |
| `gateway.ts` | 把浏览器请求路由到内核：先过门禁，再转发或本地处理；提供 `/api/read`、`/api/write` | 新 |
| `main.ts`、`http.ts` | `startServer` 接入以上模块；`close()` 依次停 ticker、内核、HTTP | 改 |

所有用户可见文字（拒绝原因、离线原因、启动日志）用中文，方法名、字段名照写原文。

### 2. automed 查找（R-02）

`resolveAutomedBinary({ env, home, webRoot, exists })` 返回 `{ path, source }`。所有参数都可以注入，测试不碰真实
`$HOME`，也不改 `process.env`。

| 顺序 | source | 位置 | 条件 |
|---|---|---|---|
| 1 | `env` | `env.AUTOMED_BIN` | 非空就用，不检查文件是否存在（与 desktop 一致）；空串视同未设 |
| 2 | `bundle` | `<webRoot>/../automed` | 存在才用 |
| 3 | `local-bin` | `<home>/.local/bin/automed` | 存在才用 |
| 4 | `cargo` | `<repo>/target/<AUTOMED_CARGO_PROFILE 或 debug>/automed` | 兜底：即使不存在也返回，让启动失败时能说出这个路径 |

`webRoot` 是 `apps/web` 目录，即 `resolve(import.meta.dirname, '../..')`，按 `server/core/` 的目录深度算；
`<repo>` 是 `<webRoot>/../..`。发布包的约定：W10 把构建好的 `apps/web` 作为包内的 `web/` 目录，`automed` 与
`autome` 放在 `web/` 旁边，所以「同一发布包里的」就是 `<webRoot>/../automed`。在开发仓库里这个位置是
`apps/automed`，不存在，会被跳过。测试会锁住这个相对位置，W10 照它摆放。只支持 macOS，不处理 `.exe`。

### 3. 内核守护（R-03）

`CoreSupervisor({ binaryPath, home, dbPath?, log, now, onStatus })`：

- 状态有五种：`starting`（已 spawn，对账 tick 尚未返回）、`connected`、`restarting`（等 1 秒延迟）、
  `offline`（已放弃重启，带 `reason` 和 `attempts`）、`stopped`。每次状态变化都调用 `onStatus`，W3 用它推 `core` 消息。
- 子进程环境是 `{...process.env, AUTOME_HOME: home}`，所以内核账本落在 `<home>/state/automed.sqlite3`。
  这里只是复制一份环境，不修改 `process.env`。
- 启动后立即发一次 `scheduler.tick` 作对账（与 desktop 启动时那次一致），收到回复后转为 `connected`。对账与周期 tick
  走同一条 `supervisor.tick()` 路径（§4）。本地超时不等于内核已经返回：超时后状态保持 `starting`，也不发第二条 tick；
  真实回复到达后才转 `connected`；进程退出则走重启策略。
- 子进程退出时计算 `ranForMs = now() - startedAt`，交给 `nextRestart`。`restart` 为真就转 `restarting`，`delayMs`
  后重新 spawn；为假就转 `offline`，`reason = sidecar.failureReason()`，之后不再自动重启。
- 二进制不存在时，spawn 会发出 `error`，被 sidecar 收敛到 `_finish`，按一次快速失败计，Node 进程本身不会崩溃。
- `restart()`（即 `core.restart`）：内核处于 `starting` 或 `connected` 时返回 `{already_running: true}`。否则先把计数
  清零、启动，再用一次 tick 验证。验证失败（包括验证 tick 超时）就抛出 `failureReason()` 或超时原因；成功则返回
  `{restarted: true}`。验证超时时进程还在、状态是 `starting`，这时再调 `core.restart` 返回 `{already_running: true}`，
  不会多出第二个子进程，也不会多发一条 tick。
- `stop()`：先标记 stopping，清掉待执行的重启定时器，再调 `sidecar.stop()`（关闭 stdin，2 秒后 SIGKILL）。
  主动停止不算崩溃。
- `call(method, params)`：只在 `starting` 或 `connected` 时可用，否则抛出 `内核未连接：<reason>`。命令格式与 desktop
  的 `callCore` 相同（`request_id: web-req-N`、`command_id: web-cmd-N`、`expected_revision: null`、
  `protocol_version: 1`）。内核回 `outcome.status === 'error'` 时抛错，并带上 `code` 和内核原话。
- **`call` 的本地等待上限 `CALL_TIMEOUT_MS = 15000`，必须大于内核写命令等锁的上限 `DRIVER_LOCK_WAIT`（10 秒）。**
  这样命令行正占着驱动锁时，浏览器拿到的是内核那句「另一个 Autome…正占着状态库，稍后再试」（409），而不是一个本地超时、
  内核又在超时之后把写操作做完。这个保证只覆盖单条命令自己的等锁时间，不含 stdio 管道上前序请求的排队时间：多条写命令
  近同时排队时仍可能本地超时，此时按 504「操作可能仍在执行」处理；不引入队列、取消或重试。
  测试解析 `dispatch.rs` 里 `DRIVER_LOCK_WAIT` 的秒数，断言
  `CALL_TIMEOUT_MS` 比它大。万一真的本地超时，抛出带 `timeout` 标记的错误，网关回 504（§8）。

### 4. tick（R-05、约束 4）

`createTicker({ call, isConnected, isBusy, intervalMs = 3000, log, onTick })`。

**「上一次 tick 结束」的定义**：内核真的回了 Reply，或者承载它的子进程已经退出。本地等待超时**不算**结束。
`AutomedSidecar.request()` 超时只是删掉 pending 项并 reject，命令已经写进 stdio 管道，内核仍会处理
（`main.rs` 的 stdio 主循环同步处理每条命令，后到的帧只能排队，没法取消）。如果超时就放行下一拍，会出现这样的情况：
t=3s 发出的 tick A 处理了 5 秒以上，t=9s 又发出 tick B，管道里就排了第二条，违反「不叠加」。因此：

- `AutomedSidecar` 增加 `requestUntilReply(command)`：不设本地超时，真实 Reply 到达时 resolve，进程退出时 reject。
  再增加 `isOutstanding(requestId)`：从发出到 Reply 到达或进程退出之间为真。普通 `request()` 的行为与迁移来的
  用例保持一致（超时仍然 reject，迟到的回复仍然被忽略）。
- `supervisor.tick({ timeoutMs = 5000 })` 用 `requestUntilReply` 发送，再让它和超时计时器赛跑：超时就向调用方
  reject「tick 超时」，但在途状态保留，通过 `supervisor.isTickBusy()` 暴露出来（对账 tick 与周期 tick 共用）。
  只有真实 Reply 到达，或进程退出（包括 `stop()`、崩溃、重启），在途状态才清除。迟到的对账 Reply 照样把
  `starting` 转为 `connected`。
- ticker 的 `isBusy` 取 `supervisor.isTickBusy()`，ticker 自己也保留一个本地在途标志，两者任一为真就不发。

| 情形 | 行为 |
|---|---|
| 到点但 `isConnected()` 为假 | 不发 |
| 到点但上一次 tick 还没结束（包括已经本地超时、Reply 未到、进程还在的情况） | 不发，也不排队（不叠加）；Reply 到达或进程退出后的下一拍恢复 |
| 回复 `skipped: true` | 不记日志、不算错误；照常调用 `onTick(report)`，W3 靠它在别人驱动时也能拉事件 |
| 回复的 `errors` 非空 | 每条记一行 `[调度] <原文>` |
| 调用失败（超时、内核退出） | 记一行 `[调度] tick 失败：<原因>`；超时之后的下一拍仍按上一行规则不发（Reply 未到），内核退出后随重启恢复 |
| `stop()` 之后 | 不再发 |

网关不碰驱动锁，轮流推进完全交给内核的 `try_acquire`。

### 5. 读写白名单与网关路由（R-01、R-04 第 4/5 条、约束 3）

两份门禁各自独立：互不 import，也都不 import `apps/desktop` 下的任何文件。两边各自用测试解析 `dispatch.rs`，
与 `READ_METHODS` 交叉核对。

- **读门禁**：方法表照搬 desktop 的 13 项。`project.probe`、`events.since`、`env.detect`、`config.validate`
  这四项只供服务端内部调用，浏览器不能读。新增一条规则：参数里的字符串值不能长得像路径（复用 `looksLikeAPath`），
  因为读方法的参数只有各种 id 和 `component`。
- **写门禁**：op 表、`ROLES`、各种长度上限照搬（`MAX_PROSE_LENGTH 8000`、`MAX_DOCUMENT_LENGTH 256K`、
  标识符 512、`MAX_PAYLOAD_JSON_LENGTH 32K`、数组 256 项、每项 1024）。导出 `COMPONENTS = ['git','claude','codex','iterm2']`。
  有两处收紧，原因相同：浏览器不能给出任意路径，而内核对这两个字段的值不做路径校验，直接存下并拼进任务整理提示词
  （`dispatch.rs` 把 `doc_refs` 原样转交 `create`，`launcher.rs` 把它们作为「用户指定的仓库内参考文档」写进提示词）。
  1. `task.create.attachments` 的每一项必须是上传编号（小写 UUID），不能是路径。
  2. `task.create.doc_refs` 在 W2 只接受空数组或缺省，非空一律以「网页端暂不支持指定参考文档」拒绝。参考文档在
     Web 上的形态（服务端能解析的引用标识）由后续任务设计，届时再放开；W2 不让浏览器提供的路径进入任务。
  固定文件名的枚举不算任意路径，保持原样：`project.onboarding.save` 只收 `AGENTS.md` 或 `docs/agent-project-profile.md`。
- **网关路由**（`gateway.write`，在门禁通过之后）：

| op | 处理 |
|---|---|
| `core.restart` | 调 `supervisor.restart()`，不转发给内核 |
| `project.pick`、`project.confirm`、`open.path`、`open.terminal`、`env.install`、`env.login` | desktop 在 Main 进程里做这些事。Web 上由后续任务（W9 等）通过 `gateway.registerLocalOp(op, handler)` 接上。W2 没有注册时回 501「网页端还不支持「<op>」」，内核收不到 |
| `task.create` | 用 `uploads.resolve(id)` 把每个附件编号换成服务端路径。解析不出来，或者路径不在 `<home>/web/uploads/` 下，就以「附件 <id> 不存在或已过期，请重新上传」拒绝。W2 的默认 resolver 一律返回 null，W6 换成真实实现 |
| 其余 op | 原样转发，op 名就是内核方法名 |

经过门禁之后，转发给内核的 `task.create` 参数里只可能有已解析的服务端附件路径，不会有 `doc_refs`。

- **服务端内部调用**用 `gateway.core.call(method, params)`（即 `supervisor.call`），不经过浏览器门禁。W3 的
  `events.since`、W9 的 `project.probe` 都走这条路。
- 「添加项目先经服务端 `project.probe`」在 W2 的落法：`project.add` 不在写白名单里，`project.probe` 不在读白名单里，
  浏览器碰不到这两个方法；完整流程由 W9 用本地 op 接上。
- 「日志路径只能来自内核」在网关层的落法：没有任何浏览器 op 能带日志路径；`open.path` 的 `log` 只认 `session_id`；
  `session.log` 只收 `session_id`。用一条源码核对测试锁住：解析 `dispatch.rs` 分派表，找出用 `str_param(p, "path")`
  取参数的方法（现在是 `project.probe`、`project.add`），断言它们都不在 Web 的读写白名单里。

### 6. 原测试迁移表（R-01 要求列出每一处改动）

表里没有列出的原用例都原样迁移，断言意图不变（用 vitest 注册用例，保留 `node:assert` 原断言）。

| 原用例 | 去向与改动 | 理由 |
|---|---|---|
| framing「oversized frame」`/exceeds max/` | 改为 `/超过上限/` | 约束 8，消息中文化 |
| ipc-gate #8 oversized `/limit/` | 改为 `/上限/` | 同上 |
| ipc-gate #9「only the packaged origin is trusted」 | 移到 `security.test.ts`，改为测 Origin 检查：两个可信源可通过，`autome://app`、`null`、其它端口、https、带路径、`localhost.evil.com` 等不可信源被拒 | Web 没有 `autome://app`。信任锚换成 Origin + Host 两项（不再有令牌，R-07），意图不变：只有自家源能调用 |
| ipc-gate #10「component names」 | 移到 `write-gate.test.ts`，对照对象从 `renderer/lib/labels.js` 的 `COMPONENT_ORDER` 换成写门禁的 `COMPONENTS`；`environment.rs` 的解析与 derive 检查照搬 | apps/web 没有 labels。Web 实际发给内核的组件名只出现在 `env.install`、`env.login` 的校验表里 |
| write-gate #10 `/null byte/` | 改为 `/空字符/` | 约束 8 |
| write-gate #11 第一条断言 | `attachments: ['/tmp/a.pdf']` 从「通过」改为「拒绝」，另用上传编号作为通过例；其余三条断言不变 | §5.5：附件只能是服务端生成的路径 |
| write-gate #11 补充 | 新增断言：`doc_refs` 为 `['/tmp/outside.md']`、`['../../outside.md']`、`['~/.ssh/config']` 时各被拒绝，空数组与缺省仍然通过 | §5.5：浏览器不能给出任意路径 |
| write-gate #17 `/path/` | 改为 `/路径/` | 约束 8 |
| write-gate #18 `/unknown open kind/` | 改为 `/未知的打开类型/` | 约束 8 |
| write-gate #23 `/limit/` | 改为 `/上限/`。payload 仍是 200×1000 的字符串，先撞到总长上限 | 约束 8 |
| sidecar #1「packaged location wins」 | 换成 `core-binary.test.ts` 的四级查找用例 | Web 没有 `process.resourcesPath`。原意图（发布包优先于开发构建，且只在存在时使用；覆盖变量优先于一切）由 R-02 的用例完整承接 |
| sidecar #2「no test starts a real core against ~/.autome」 | 扫描范围从本文件扩大到 `apps/web/test/*.ts`。凡出现被扫描的启动写法，附近必须出现 `AUTOME_HOME`、`home:` 或一个不可启动的 `binaryPath`。扫描的写法分两步加入：M-01 是 `new AutomedSidecar(`、`new CoreSupervisor(`；M-05 改 `server.test.ts` 的同时加入 `startServer(` 和 spawn `npm run dev`/`npm start` | 约束 6：W2 之后启动服务器就会启动内核。分两步是因为 W1 的 `server.test.ts` 要到 M-05 才改为传临时 home，提前扫描会让 M-01 无端变红 |
| sidecar #3「binary is built」 | 保留为前置条件断言：缺二进制时**一律失败**（本机、ci-local、门禁、CI 都一样），失败信息是中文，并提示运行 `cargo build -p automed`。不按 `CI` 跳过 | R-06：CI 的 web job 会先构建内核，所以缺二进制就是真错误。另加一条守卫用例，确认测试源码里没有按 `CI` 或其它条件跳过的写法（见 M-01） |
| sidecar「already pending」「exited before replying」 | 改为匹配中文消息的正则 | 约束 8 |
| ci-config「CI 按顺序运行四个 Web 检查」（W1 用例） | 改名为「CI 按顺序运行 Web 检查，测试前先构建内核」，期望列表改为 `npm ci, npm run lint, npm run typecheck, cargo build -p automed, npm test, npm run build` | R-06 |
| server.test.ts `launch()` 与「只监听 IPv4 回环地址」（W1 用例） | `launch()` 给子进程环境加上临时 `AUTOME_HOME` 和 `AUTOMED_BIN`；「只监听 IPv4」给 `startServer` 传临时 `home`。断言不变 | 约束 6：接入网关后，启动服务器就会启动内核 |

真实内核用例用的二进制是 `process.env.AUTOMED_BIN || <repo>/target/debug/automed`，通过 `binaryPath` 显式传入，
不改 `process.env`。每个用例各自用 `mkdtemp` 建一个临时 `AUTOME_HOME`。

### 7. 本机安全（R-04 第 1/2/3 条、R-07）

请求按下表依次检查，任何一步不通过都回 403 和中文原因：`/api/*` 回 JSON `{ok:false, error}`，其它路径回
`text/plain; charset=utf-8`。`<port>` 取监听后 `server.address().port` 的值，所以端口 0 也能用。

| 请求 | Host | Origin |
|---|---|---|
| GET、HEAD | 必须合格 | 不查 |
| 其它方法（POST、PUT、PATCH、DELETE、OPTIONS 等，包括 RR action 与 `/api/*`） | 必须合格 | 必须合格 |
| WebSocket upgrade | 必须合格 | 必须合格 |

- **Host**：转成小写后必须正好等于 `127.0.0.1:<port>` 或 `localhost:<port>`。缺失、缺端口、端口不对、IPv6、
  `localhost.evil.com:<port>` 都拒绝，原因「Host 不是本机地址，已拒绝」。`/healthz` 也要检查。这一条防 DNS rebinding。
- **Origin**：必须正好等于 `http://127.0.0.1:<port>` 或 `http://localhost:<port>`。缺失、`null`、https、
  带路径、其它端口都拒绝，原因「请求来源不是本服务，已拒绝」。GET、HEAD 不查 Origin：跨站网页能发出这类请求，
  但读不到响应（没有 CORS 头），而 GET 不改变状态。
- **鉴权就到这一层为止（R-07）**：
  - 不生成、不落盘、不校验任何访问令牌；不设置、不读取任何 Cookie；没有 `?t=` 兑换、会话集合、登录会话；
    `<home>/web/` 下不写任何鉴权相关的文件（W2 根本不创建这个目录）。
  - 任何响应都不带 `Set-Cookie`，也不带任何 `Access-Control-Allow-*` 头。`OPTIONS` 属于「其它方法」，跨站预检拿到的是 403。
  - Vite 开发服务器默认会对 localhost、127.0.0.1 等本机来源（不限端口）回 CORS 头（本地 Vite 源码 `node.js` 里的默认 `cors` 设置）。
    实现时在 `http.ts` 的 Vite 配置里明确设 `server.cors: false`；核对 `Access-Control-Allow-Origin` 时，请求要带另一个
    localhost 端口的 Origin，不能只看不带 Origin 的响应（`gateway-http.test.ts` 第 5 条照此写）。
  - Host 与 Origin 都合格、不带 Cookie、`Authorization` 或查询参数的请求，就能到达网关。
  - 带了 Cookie 或 `?t=` 的请求，检查结果与不带时完全相同：Cookie 不被读取，`?t=` 不触发跳转。
- **R-07 的反面怎么锁住**：
  1. `security.test.ts`：不带任何凭据、Host 与 Origin 都合格的 POST 放行到下一个中间件，且中间件没有写 `Set-Cookie`；
     带 `?t=…` 的 GET 照常放行，不回 303；带 Cookie 但 Origin 不对的 POST 照样 403。
  2. `security.test.ts` 源码守卫：`apps/web/server/` 下所有 `.ts` 文件都不匹配
     `/set-cookie|\.cookie\(|req\.cookies|timingSafeEqual|web\/token|[?&]t=/i`。实现可以收窄匹配写法以免误报，
     但不得放过这六类。
  3. `gateway-http.test.ts`：真实服务与真实内核下，不带任何凭据的 `config.set_loop` 写操作返回 200，读回来是新值；
     遍历 `GET /healthz`、`GET /`、`POST /api/read`、`POST /api/write` 以及一个 403 响应，全部没有 `Set-Cookie` 和
     `Access-Control-Allow-Origin`（GET 另带一份 Origin 为另一 localhost 端口的请求）；测试结束时临时 home 下的 `web/` 目录不存在。
- **upgrade 守卫**：`createHttpServer` 挂完 Vite 之后，取出 server 上已有的全部 `upgrade` 监听器（开发模式下就是 Vite
  HMR 那一个），移除它们，换成唯一的一个守卫监听器。守卫只检查 Host 与 Origin，不通过时往 socket 写
  `HTTP/1.1 403 Forbidden` 加中文原因，然后销毁 socket。通过时交给取出来的那些监听器，以及用
  `registerUpgrade(path, handler)` 注册的处理器（W3 在这里接 `/ws`）。两者都没有时回 404 并销毁。守卫必须是唯一的
  upgrade 监听器，测试断言 `listenerCount('upgrade') === 1`。
- **中间件顺序**：Host → 非 GET/HEAD 请求的 Origin → `/healthz` → `/api/*`（`express.json({limit: '1mb'})`，
  JSON 解析失败回 400 中文）→ Vite 或静态资源 → RR。
- 启动日志保持 W1 的一行 `autome-web 已启动：http://127.0.0.1:<port>（…模式）`，不再输出任何带令牌的地址；
  `autome web` 打开浏览器时（W10）直接用这个地址。

### 8. HTTP 接口与 `startServer`

- `POST /api/read`，请求体 `{method, params}`；`POST /api/write`，请求体 `{op, params}`。
  - 成功：200 `{ok:true, payload}`。
  - 门禁拒绝：400 `{ok:false, error:'已拒绝：<中文原因>'}`。
  - 本地 op 还没接上：501。
  - 内核不在线：503 `{ok:false, error:'内核未连接：<原因>'}`。
  - 内核报错：409 `{ok:false, error:<内核原话>, code}`。
  - 本地等待超过 `CALL_TIMEOUT_MS`：504 `{ok:false, error:'内核响应超时：<方法>，操作可能仍在执行'}`。
- `startServer({ port, mode, home?, binaryPath? })`：`home` 默认取 `resolveAutomeHome(process.env)`，
  `binaryPath` 默认取 `resolveAutomedBinary().path`。listen 成功之后才启动内核和 ticker，所以端口被占时不会留下
  子进程。返回 `{ server, gateway, close }`，`close()` 依次停 ticker、内核、HTTP。
- W1 的 `test/server.test.ts` 必须改，因为接入网关之后，启动服务器就会启动内核：
  - `launch()` 的子进程环境加上临时 `AUTOME_HOME` 和 `AUTOMED_BIN`，不再碰真实 `~/.autome`；
  - 「只监听 IPv4」用例给 `startServer` 传临时 `home`；
  - 新增两条用例，都以 Host/Origin 为判据：开发模式下，Host 合格但 Origin 是外站的 HMR upgrade
    （`Sec-WebSocket-Protocol: vite-hmr`）回 403；开发模式下，Host 为 `evil.example:<port>` 的首页 GET 回 403
    （DNS rebinding 时 Vite 的页面也读不到）。

### 9. CI 真跑内核用例（R-06）

- `.github/workflows/ci.yml` 的 `web` job 在 `Typecheck` 之后、`Test` 之前加一步：

  ```yaml
        - name: Build the core the gateway tests drive
          run: cargo build -p automed
  ```

  工作目录沿用 job 的 `apps/web`（实测 cargo 能从这里找到 workspace）。不加工具链步骤和 `Swatinem/rust-cache`，
  与 `desktop` job 的做法一致：靠 `rust-toolchain.toml` 和 runner 自带的 rustup。`timeout-minutes` 保持 15：
  冷构建在本机 14 核上要 26 秒，按 `macos-14` runner 的 3 核粗估也在几分钟以内，web job 原有步骤不受影响。
  其它 job 一行不动。
- `test/ci-config.test.ts`：
  - 改一条：「CI 按顺序运行四个 Web 检查」改名为「CI 按顺序运行 Web 检查，测试前先构建内核」，期望的 `run` 列表为
    `npm ci, npm run lint, npm run typecheck, cargo build -p automed, npm test, npm run build`。
  - 新增一条：「本地门禁在 Web test 之前已构建内核」：断言 `ci-local.sh` 里
    `run 'Test' cargo test --workspace --all-targets` 存在，并且位于 `run 'Web test'` 之前。这条把背景里
    「ci-local 不用改」的理由锁成测试：有人把 rust 段挪到 web 段后面或删掉，它就会失败。
  - 其余 5 条原样保留（包括「本地四行独立 run」：ci-local 的 Web 段不变）。
- sidecar #3 不再按 `CI` 跳过（§6），并新增守卫用例：`apps/web/test/*.ts` 里没有 `process.env.CI`、`skipIf(`、
  `.skip(`、`test.todo(`。这样「CI 上实际运行，而不是跳过」不只靠 ci.yml，也靠测试源码本身。

### 10. 相对附件的改动清单

附件中依赖令牌、Cookie 或「不改 ci.yml」的位置，在本文中的处理如下（同类位置已全部查过）：

- §1 模块表 `security.ts`：删去「Cookie 解析、令牌（生成、落盘、兑换、轮换）」。
- §6 迁移表：ipc-gate #9 的理由改为信任锚只剩 Origin + Host；sidecar #3 由「`CI` 下跳过」改为一律失败加守卫用例；
  sidecar #2 的扫描分两步加入（附件里 M-01 就会扫到未改的 `server.test.ts`，属于顺序问题，一并修正）；
  新增 ci-config 与 server.test.ts 两行。
- §7：检查表去掉「`?t=` 兑换」「会话 Cookie」两列；删「令牌」「开发时拿令牌」两段；upgrade 守卫只看 Host + Origin；
  中间件顺序去掉令牌兑换与 Cookie；新增「R-07 的反面怎么锁住」。
- §8：W1 `server.test.ts` 删去「开发启动日志带一次性令牌地址」，「没有 Cookie 的 HMR upgrade 403」改为以 Origin
  与 Host 为判据的两条；新增 504 一行（来自背景里实测到的写命令等锁 10 秒）。
- §3：新增 `CALL_TIMEOUT_MS` 与对应的交叉核对测试（同一来源）。
- §9 整节为新增（R-06）；非目标删去「本任务不改 ci.yml」。
- 里程碑：R-06 并入 M-01；M-04、M-05 的新增测试、用例计数、预期与负向对照中所有令牌、Cookie、`SameSite`、
  「二次使用 403」「`~/.autome/web/token` 不被创建」条目已删除或改写；各里程碑的 Web 用例总数重算。
- 人工验收：H-01（`?t=` 消失、Cookie 属性）删除，编号不复用；新增 H-04。
- Backlog：B-01 纳入本任务（R-06），B-04（Cookie 只在内存）失效，两条都删除；B-02、B-03 原编号保留（计划 W5、W10
  按编号引用它们）。
- 风险：风险 1 改为 CI 构建内核的耗时与不可本地验证；风险 2 去掉 Cookie 与令牌兑换的纯函数测试说明；新增风险 4（写命令等锁）。
- 路径：附件的 `docs/apps-web-的服务端托管-automed/` 一律改为 `docs/apps-web-的服务端托管-automed-2/`。

## 风险与验证安排

1. **CI 构建内核（R-06）**：仓库没有 remote，ci.yml 在本任务内不会真正执行，所以 R-06 的验收以 `ci-config.test.ts`
   为准（步骤存在、顺序正确），再加测试源码里没有条件跳过的守卫。构建耗时的依据是本机冷构建 26 秒；
   web job 的 15 分钟上限留有余量。将来真推到 GitHub 后若超时，再加 `Swatinem/rust-cache`，那属于 CI 调优，
   不改变本设计。本机和门禁上缺二进制时直接失败、不会挂起（sidecar 的 `_releasePipes` 照搬，避免 desktop 当年挂到
   runner 超时的老问题）。
2. **回环监听被沙箱拒绝**：本会话能监听，W1 的监听用例全部通过；但实现轮的环境可能不同。Host、Origin、upgrade 守卫、
   路由都写成纯函数，或者用假的 req/res/socket 测，不监听端口。只有 `gateway-http.test.ts`、W1 的 `server.test.ts`，
   以及 sidecar、supervisor、ticker 里驱动真实内核的少数用例需要监听或子进程。实现轮如果遇到 `listen EPERM`，
   在证据里写明被拦住的用例数，以及不依赖监听的那部分结果，交审计轮复验。
3. **Vite 的 upgrade 监听器绕过守卫**：已确认 Vite 8.3.7 在 `createServer` 期间同步挂上监听器（见背景）。
   用 `listenerCount === 1` 和开发模式下的两条 403 用例锁住。如果以后 Vite 改成延迟挂，这条断言会先失败。
4. **写命令等驱动锁**：内核写命令最多等锁 10 秒，desktop 的 5 秒本地超时会把一次实际成功的写报成失败。Web 用
   15 秒的 `CALL_TIMEOUT_MS`，并用解析 `dispatch.rs` 的测试锁住「大于 `DRIVER_LOCK_WAIT`」。这只覆盖单条命令的等锁上限：
   前序请求排队时仍可能本地超时，那种情形回 504「操作可能仍在执行」，不保证一定得到 409。这期间 stdio 管道上的
   tick 会排在写命令后面，ticker 按「不叠加」规则不会多发。
5. **不做令牌之后的暴露面**：本机任何进程都能调网关。这是计划 §5.5 的明确决定（本机进程本来就能直接用 `autome`
   命令行），不是遗漏；跨站网页由 Host 与 Origin 挡住，各有测试。
6. **基线门禁**：设计轮在本 worktree 跑了一遍 `sh scripts/ci-local.sh`，11 步全部通过，用时 107 秒（不含 package）。
   之后各轮若出现与本任务无关的范围外失败，不修，在 Backlog 里写明：用例全名、断言原文、机制。
7. **两份白名单并存**：desktop 那份原样保留。`npm test --prefix apps/desktop` 每个里程碑都要跑，结果必须仍是
   104 个用例、0 失败（1 个跳过是 electron dom harness，基线就是如此）。
8. **用例计数**：每个里程碑的预期写的是 Web 全量用例总数。本机有二进制时，真实内核用例不允许跳过，Web 的 `skipped`
   必须为 0。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | 帧、sidecar、automed 四级查找与 CI 构建内核 | 0 | |
| M-02 | 已完成 | 读写白名单迁移与路径拒绝 | 0 | |
| M-03 | 已完成 | 内核守护：崩溃重启、手动重启与 3 秒 tick | 0 | |
| M-04 | 已完成 | 本机安全：Host、Origin 与 upgrade 守卫 | 0 | |
| M-05 | 开放 | 网关接入 startServer 与端到端 | 0 | |

## 里程碑说明

要求覆盖：R-01 → M-01、M-02、M-03；R-02 → M-01；R-03 → M-03、M-05；R-04 → M-02（第 4/5 条）、M-04（第 1/2/3 条）、
M-05（五条在 HTTP 层再验一遍）；R-05 → M-03、M-05；R-06 → M-01；R-07 → M-04、M-05。

### M-01 帧、sidecar、automed 四级查找与 CI 构建内核

最新证据：docs/apps-web-的服务端托管-automed-2/evidence/M-01-r1-audit.md · 通过 · 审计 #1

- 结果：`server/core/{framing,sidecar,binary}.ts` 能在 vitest 里驱动真实 automed 完成往返；CI 的 web job 在测试前构建
  automed，真实内核用例不再有任何跳过路径。这是不确定性最大的一步（ESM + 原生类型剥离 + 真实子进程）。
- 覆盖：R-01（framing、sidecar）、R-02、R-06、约束 2/6/7/8。前置：无。
- 改动：`.github/workflows/ci.yml` 的 `web` job 按方案 §9 加一步；`test/ci-config.test.ts` 改一条、加一条。
- 新增测试（共 29 个）：
  - `test/core-framing.test.ts` 4 个（原样迁移，消息中文化）。
  - `test/core-binary.test.ts` 6 个：四级顺序各 1 个、`AUTOMED_BIN` 为空串视同未设、发布包相对位置与 `source`。
  - `test/core-sidecar.test.ts` 18 个：原 17 个去掉 #1，加 1 个 `requestUntilReply` 用例，再加 1 个「测试源码不按条件跳过」
    守卫。`requestUntilReply` 用例用 `test/fixtures/fake-core.mjs`（读帧、延迟后回复的最小内核替身）验证：本地超时之后
    进程仍在、`isOutstanding` 仍为真；迟到的真实 Reply 到达后 resolve，`isOutstanding` 变假；另一条请求在回复前进程退出，
    则 reject 且 `isOutstanding` 变假；普通 `request()` 的超时行为不变。#2 的扫描此时只覆盖 `new AutomedSidecar(`、
    `new CoreSupervisor(`。
  - `test/ci-config.test.ts` +1：「本地门禁在 Web test 之前已构建内核」。

### M-01 验收契约

- 验收命令：`cargo build -p automed && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：lint、typecheck 无错误；Web 共 67 个用例全过（基线 38 + 29），skipped 0；desktop 仍是 104 个用例，0 失败。
- 负向对照：把 `FrameDecoder` 的长度读成 `readUInt32LE`，framing 的分片解码用例和 sidecar 的真实往返用例都会失败；把查找顺序里的 `local-bin` 与 `cargo` 对调，core-binary 的第 3 个用例会失败；按到达顺序而不是 `request_id` 关联回复，sidecar「乱序回复各归其主」会失败；删掉 ci.yml 里的 `cargo build -p automed` 或把它放到 `npm test` 之后，ci-config「测试前先构建内核」会失败；给真实内核用例加 `skipIf(process.env.CI)`，「测试源码不按条件跳过」守卫会失败。

### M-02 读写白名单迁移与路径拒绝

最新证据：docs/apps-web-的服务端托管-automed-2/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 结果：`server/gates/{read-gate,write-gate}.ts` 两份独立门禁，与 `READ_METHODS`、`role.rs`、`environment.rs`
  交叉核对。附件只收上传编号，`doc_refs` 非空一律拒绝；浏览器不能指定任意文件路径（`project.onboarding.save`
  那两个固定文档名这类枚举不算）。
- 覆盖：R-01（两份白名单）、R-04 第 4/5 条（网关层）、约束 3/8。前置：无（纯函数，与 M-01 没有依赖，编号在后只是顺序）。
- 新增测试（共 46 个）：
  - `test/read-gate.test.ts` 11 个：迁移 8 个，加参数长得像路径时拒绝、拒绝原因是中文、`project.probe`/`events.since`
    等内部方法浏览器不可读。
  - `test/write-gate.test.ts` 35 个：迁移 28 个，加从 ipc-gate 移来的组件名核对、附件必须是上传编号、`doc_refs` 非空被拒
    （三种路径写法）且空数组通过、拒绝原因是中文、解析 `dispatch.rs` 证明取 `path` 的方法不可达、两份门禁互不 import 且
    不 import desktop、长度上限钉死成字面值。

### M-02 验收契约

- 验收命令：`npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 113 个用例全过（67 + 46），skipped 0；desktop 仍是 104 个，0 失败。
- 负向对照：往写白名单里加 `project.add`，「project.add 不可达」和「取 path 的方法不可达」两条用例会失败；放开附件让它收路径，「附件必须是上传编号」和迁移后的 #11 会失败；放开非空 `doc_refs`，「doc_refs 非空被拒」会失败；读门禁 import 写门禁的任何导出，「互不 import」会失败；往读白名单里加一个不在 `READ_METHODS` 里的方法（如 `project.add`），交叉核对用例会失败。

### M-03 内核守护：崩溃重启、手动重启与 3 秒 tick

最新证据：docs/apps-web-的服务端托管-automed-2/evidence/M-03-r3-audit.md · 通过 · 审计 #3

- 结果：`core/restart-policy.ts`、`core/supervisor.ts`、`ticker.ts` 按方案 §3、§4 工作。
- 覆盖：R-01（core-restart）、R-03、R-05、约束 4/6。前置：M-01。
- 新增测试（共 25 个）：
  - `test/core-restart.test.ts` 6 个（原样迁移 5 个，另加 4999/5000ms 与非有限寿命边界）。
  - `test/core-supervisor.test.ts` 10 个：对账 tick 返回之后才算 connected；真实内核在注入时钟下活过 5 秒后被 SIGKILL，
    1 秒后重启且计数清零；`test/fixtures/automed-fail-fast.sh`（往 stderr 写一行 ERROR 后以 1 退出）连续 3 次就转
    offline，`reason` 引用 stderr，之后不再 spawn；offline 之后手动重启成功；手动重启仍失败时抛出内核的原因；运行中手动
    重启返回 `already_running`；重启延迟期间 `stop()` 后不再 spawn；二进制缺失时按快速失败计，测试进程不崩溃；对账 tick
    超过 `timeoutMs` 仍未回复时（`fake-core.mjs` 延迟回复），状态保持 `starting`、`isTickBusy()` 为真、再次 `restart()`
    返回 `already_running`，迟到的 Reply 到达后转 `connected` 且 `isTickBusy()` 变假；解析 `dispatch.rs` 的
    `DRIVER_LOCK_WAIT`，断言 `CALL_TIMEOUT_MS` 比它大。
  - `test/ticker.test.ts` 9 个：用 vitest 假定时器覆盖方案 §4 表中的 6 种情形；9000ms 内恰好 3 次；「tick 已本地超时但
    `isBusy()` 仍为真时，后续各拍都不发，`isBusy()` 变假后的下一拍恢复」；再加一个真实内核用例：用 `perl` 对
    `<home>/state/automed.sqlite3.driver.lock` 持有 `flock`，tick 返回 `skipped: true`，且日志函数一次都没被调用。

### M-03 验收契约

- 验收命令：`cargo build -p automed && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 138 个用例全过（113 + 25），skipped 0；desktop 仍是 104 个，0 失败。
- 负向对照：`MAX_QUICK_FAILURES` 改成 4，「连续 3 次转 offline」会因看到第 4 次 spawn 而失败；去掉在途标志，ticker「上一次未返回时不叠加」在 9 秒内会看到 3 次调用而不是 1 次；本地超时就清掉在途状态，「超时后仍不叠加」和 supervisor 的 `isTickBusy` 用例会失败；`skipped` 时记日志，「skipped 安静」和真实驱动锁用例都会失败；supervisor 不清理重启定时器，「延迟期间 stop」会看到新进程；`CALL_TIMEOUT_MS` 照搬 desktop 的 5000，`DRIVER_LOCK_WAIT` 交叉核对会失败。

### M-04 本机安全：Host、Origin 与 upgrade 守卫

最新证据：docs/apps-web-的服务端托管-automed-2/evidence/M-04-r4-audit.md · 通过 · 审计 #4

- 结果：`server/security.ts` 按方案 §7 实现，`createHttpServer` 装上 Host/Origin 中间件和 upgrade 守卫。全部用纯函数或
  假的 req/res/socket 测，不监听端口。没有任何令牌或 Cookie 代码。
- 覆盖：R-04 第 1/2/3 条、R-07、约束 5/8。前置：无（实现上独立；接入网关在 M-05）。
- 新增测试：`test/security.test.ts` 14 个：
  1. Host 接受：`127.0.0.1:<port>`、`localhost:<port>`、大写 `LOCALHOST:<port>`。
  2. Host 拒绝表：缺失、缺端口、端口不对、`[::1]:<port>`、`localhost.evil.com:<port>`、`evil.example:<port>`。
  3. Origin 接受：两个可信源。
  4. Origin 拒绝表（承接 ipc-gate #9）：缺失、`null`、`https://127.0.0.1:<port>`、其它端口、带路径、`autome://app`、
     `http://localhost.evil.com:<port>`。
  5. 中间件：GET、HEAD 只查 Host，带外站 Origin 的 GET 也放行。
  6. 中间件：POST、PUT、PATCH、DELETE、OPTIONS 缺 Origin 或 Origin 不对时回 403，`/api/*` 回 JSON，页面路径（如 POST `/`，
     即 RR action）回纯文本。
  7. R-07：Host 与 Origin 都合格、不带 Cookie、`Authorization` 和查询参数的 POST 放行到下一个中间件，响应没有 `Set-Cookie`。
  8. R-07：带 `?t=abc` 的 GET 照常放行、不回 303；带 Cookie 但 Origin 不对的 POST 照样 403。
  9. 任何拒绝响应都没有 `Access-Control-Allow-*` 头。
  10. upgrade 守卫：Host 不对、Origin 缺失、Origin 不对三种情况，都往假 socket 写 403 并销毁。
  11. upgrade 守卫：通过时转给取出来的监听器和 `registerUpgrade` 注册的处理器；两者都没有时回 404 并销毁。
  12. 开发模式下 `createHttpServer` 之后 `listenerCount('upgrade') === 1`（只建 server，不监听）。
  13. 所有拒绝原因都是中文（逐条匹配 CJK 字符）。
  14. 源码守卫：`apps/web/server/**/*.ts` 都不匹配方案 §7 给出的 Cookie/令牌正则。

### M-04 验收契约

- 验收命令：`npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 152 个用例全过（138 + 14），skipped 0；desktop 仍是 104 个，0 失败。
- 负向对照：Host 检查写成 `startsWith('localhost')`，拒绝表里 `localhost.evil.com:<port>` 那一行会失败；Origin 只在 `/api/*` 上检查，用例 6 里 POST `/` 会被放行而失败；保留一段 `?t=` 兑换（303 跳转）或往响应里写 `Set-Cookie`，用例 7、8 和源码守卫 14 会失败；守卫用 `prependListener` 而没有移除 Vite 的监听器，`listenerCount === 1` 会失败。

### M-05 网关接入 startServer 与端到端

- 结果：`gateway.ts` 与 `/api/read`、`/api/write` 接进 `startServer`。从 HTTP 请求到真实内核的整条链路可用，不需要任何凭据；
  `close()` 之后内核进程也退出。
- 覆盖：R-03（`core.restart` 经网关）、R-04 五条在 HTTP 层再验一遍、R-05（ticker 随服务启停）、R-07、约束 2/5/6。
  前置：M-01–M-04。
- 改动：`test/server.test.ts` 按方案 §8 改；sidecar #2 的扫描加入 `startServer(` 与 spawn `npm run dev`/`npm start`。
- 新增测试（共 21 个）：
  - `test/gateway.test.ts` 9 个，用假 supervisor，不监听端口：读转发；读被拒时内核收不到；写转发；`core.restart` 本地处理、
    不转发；6 个本地 op 回 501 且内核收不到；附件编号解析与越界路径拒绝；离线回 503；内核报错带原话和 `code` 回 409；
    本地等待超时回 504。
  - `test/gateway-http.test.ts` 10 个，监听端口、真实内核、临时 home：
    1. 不带任何凭据、Host 与 Origin 合格时 `config.get` 回 200；
    2. 不带任何凭据的 `config.set_loop` 写了之后读得到（R-07）；
    3. 缺 Origin、Origin 不对的写各回 403，读回的值没变；
    4. Host 不对时 GET 也回 403（用 `http.request` 自定义 Host 头）；
    5. `GET /healthz`、`GET /`、`/api/read`、`/api/write` 和一个 403 响应都没有 `Set-Cookie` 和 `Access-Control-Allow-Origin`（GET 另带一份 Origin 为另一 localhost 端口的请求），
       测试结束时临时 home 下没有 `web/` 目录；
    6. 不合格的 upgrade 回 403；
    7. 合格但没有处理器的 upgrade 回 404；
    8. `close()` 之后内核 pid 不存在；
    9. 二进制缺失时服务照常启动，`/api/read` 回 503，原因含「无法启动内核」；
    10. 坏 JSON 回 400，原因是中文。
  - `test/server.test.ts` 新增 2 个：开发模式下 Origin 是外站的 HMR upgrade 回 403；开发模式下 Host 为
    `evil.example:<port>` 的首页 GET 回 403。

### M-05 验收契约

- 验收命令：`cargo build -p automed && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm run build --prefix apps/web && npm test --prefix apps/desktop`
- 预期：Web 共 173 个用例全过（152 + 21），skipped 0；build 成功；desktop 仍是 104 个，0 失败；测试前后真实 `~/.autome/state/automed.sqlite3` 的 mtime 不变，`~/.autome/web` 的存在与否也不变（证据里给出测试前后 `stat` 的对比）。
- 负向对照：把 `core.restart` 当普通 op 转发，gateway.test 的「本地处理」会因内核收到命令而失败；把安全中间件挂在 `/api/*` 之后，「Origin 不对的写回 403」会拿到 200 且读回值已变；在写路径上重新要求任何凭据（Cookie、令牌、`Authorization`），「不带任何凭据的写读得到」会拿到 403；`close()` 不停 supervisor，「close 后 pid 不存在」会失败；W1 的 `launch()` 不传临时 `AUTOME_HOME`，sidecar #2 扩展版的扫描用例会失败。

## 人工验收清单

- H-02 运行 `npm run dev --prefix apps/web`，在浏览器打开 `http://127.0.0.1:7420/`，改一下 `app/routes/home.tsx` 的文字，页面不用手动刷新就更新（HMR 经过守卫之后仍然可用）。
- H-03 在别的网站页面（如 https://example.com）的开发者工具控制台执行 `fetch('http://127.0.0.1:7420/api/write', {method:'POST', body:'{}'})`，请求被拒，内核没有收到任何写操作。
- H-04 同一个开发会话里，用 `http://localhost:7420/` 也能打开面板；开发者工具的 Application 面板里本站没有任何 Cookie，Network 面板里所有响应都没有 `Set-Cookie`。

## Backlog

- B-02 `apps/desktop/src/login-path.js`（登录 shell 的 PATH）没有迁移。W10 用 launchd 自启动时，内核看到的 PATH 会缺 `claude`/`codex`，届时需要处理。
- B-03 W5 起的 loader 要拿到 `gateway`：`startServer` 已经返回它，但还需要在 `http.ts` 的 `createRequestHandler` 上加 `getLoadContext`。上游计划已允许 W5 修改 `server/http.ts`，验收里也已要求 `getLoadContext`，由 W5 实现。

## 争议项
