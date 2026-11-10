# 任务：Web 内核网关（W2）——apps/web 托管 automed、读写白名单、3 秒 tick 与 Host/Origin 本机检查

本任务的规则见 `docs/apps-web-的服务端托管-automed-2/protocol/loop-protocol.md` 与
`docs/apps-web-的服务端托管-automed-2/protocol/session-protocol.md`。

- 设计文档：`docs/apps-web-的服务端托管-automed-2/apps-web-的服务端托管-automed-2.md`
- 任务文件生成日期：2026-11-10；基线提交 `7b672c7`（main 为 `70839e6`）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支 `autome/apps-web-的服务端托管-automed-2`）
- 证据、简报、运行记录一律写在 `docs/apps-web-的服务端托管-automed-2/` 下（注意末尾的 `-2`）

## 1. 需求原文

> apps/web 的服务端托管 automed 子进程，提供带读写白名单的 RPC 网关、每 3 秒的 tick 和本机安全检查（鉴权只做 Host 与 Origin 检查，不做令牌和 Cookie；附件是上一版设计，除鉴权外已获批准、可沿用，其 Backlog B-01 纳入）

它是计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 中的 **W2 内核网关**。
上一次尝试（任务 T-11，文档目录 `docs/apps-web-的服务端托管-automed/`，分支已不存在，
代码没有进入 main）写出了完整设计，用户批准了其中除鉴权以外的全部内容。本任务是重做：
**以附件为基线，去掉令牌与 Cookie，把附件的 Backlog B-01 变成本任务的要求。**

附件（上一版设计全文，已随任务提交）：
`docs/apps-web-的服务端托管-automed-2/attachments/W2-design-v1-T-11.md`

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-05 原样取自计划 W2 的「验收」；R-06 是附件的 Backlog B-01；R-07 是需求原文括号里的
鉴权限定。编号供评审、审计追溯使用。

- **R-01 迁移。** 从 `apps/desktop/src` 迁移 framing、sidecar、core-restart 和两份白名单
  （`ipc-gate.js` 读、`write-gate.js` 写）到 `apps/web/server/`，原测试一起迁移到
  `apps/web/test/` 并通过。断言意图保留；每一处改动逐条列出并说明理由（附件 §6 的做法）。
- **R-02 automed 查找顺序：** `AUTOMED_BIN` → 同一发布包里的 → `~/.local/bin` → cargo 构建目录。
- **R-03 崩溃重启。** 内核崩溃后按原退避策略重启，连续失败就停下，等用户手动重启（`core.restart`）。
- **R-04 本机安全的每一条检查都有测试。** 检查项以计划 §5.5 的**当前文本**为准（2026-11-10
  提交 `70839e6` 改过，原文如下）：
  1. 只绑 `127.0.0.1`；Host 头必须是 `127.0.0.1:<port>` 或 `localhost:<port>`，防 DNS rebinding。
  2. 写请求和 WebSocket 升级都要检查 Origin，必须是本服务自己的 `http://127.0.0.1:<port>`
     或 `http://localhost:<port>`。
  3. 鉴权就到这一层为止：面板只是内核状态的展示和控制面，不做令牌、Cookie 和登录会话。
     跨站网页读不到响应（没有 CORS 头），也写不进来（Origin 不对）；本机进程本来就能直接用
     `autome` 命令行。
  4. 读写白名单从 `apps/desktop/src/{ipc-gate,write-gate}.js` 迁移，连同测试。读白名单以
     `dispatch.rs` 的 `READ_METHODS` 为准，测试解析源码交叉核对。
  5. 浏览器不能给出任意路径。添加项目先经服务端 `project.probe`；附件只能是上传后服务端
     生成的路径；日志路径只能来自内核。
- **R-05 tick。** 每 3 秒一次 `scheduler.tick`；返回 `skipped` 时安静跳过（不记日志、不算错误）。
- **R-06 CI 真跑内核用例（附件 Backlog B-01）。** 在 `.github/workflows/ci.yml` 的 `web` job
  里加 `cargo build -p automed`，并同步改 `apps/web/test/ci-config.test.ts`，使 CI 上的
  真实内核用例实际运行，而不是跳过。
- **R-07 鉴权只做 Host 与 Origin。** 不生成、不落盘、不校验任何访问令牌；不设置、不读取任何
  Cookie；没有 `?t=` 兑换、会话集合、登录会话。带正确 Host 与 Origin 的请求不需要任何凭据
  即可通过。

## 3. 附件的地位与必须的改动

附件是**已获批准的基线**：模块布局、automed 四级查找、内核守护、tick 的「上一次结束」定义与
不叠加规则、读写门禁的两处收紧（附件只收上传编号、`doc_refs` 非空一律拒绝）、网关路由与
本地 op 的 501、HTTP 接口与错误码、迁移表、里程碑划分与验收契约写法，都可以沿用，不必
重新论证。设计轮的工作是在它的基础上做下面两类改动，并把附件里所有受影响的地方改一致。
未受影响的部分若沿用，照样要写进本任务的设计文档（设计文档必须自成一体，不能写「同附件」）。

### 3.1 去掉令牌与 Cookie（R-04、R-07）

附件中至少以下位置依赖令牌或 Cookie，都要改或删（同类位置由设计轮自行查全）：

- §1 模块表 `security.ts` 一行：去掉「Cookie 解析、令牌（生成、落盘、兑换、轮换）」。
- §6 迁移表 ipc-gate #9 的理由：信任锚只剩 Origin + Host。
- §7 整节：检查表去掉「`?t=` 兑换」「会话 Cookie」两列；删「令牌」「开发时拿令牌」两段；
  upgrade 守卫的通过条件只剩 Host + Origin；中间件顺序去掉令牌兑换与 Cookie。
- §8：W1 `server.test.ts` 新增用例中「开发启动日志带一次性令牌地址」不再成立；
  「没有 Cookie 的 HMR upgrade 403」要改成以 Host/Origin 为判据。
- M-04、M-05 的新增测试、用例计数、预期与负向对照中所有令牌、Cookie、`SameSite`、
  「二次使用 403」「`~/.autome/web/token` 不被创建」相关条目。
- 人工验收 H-01（`?t=` 消失、Cookie 属性）删除；Backlog B-04（Cookie 只在内存）失效。
- 风险 2 中「Cookie、令牌兑换」的纯函数测试说明。

R-07 的反面也要有测试锁住（例如：响应不带 `Set-Cookie`；`<home>/web/` 下不产生令牌文件；
带正确 Host 与 Origin、不带任何凭据的写请求能到达网关），具体用例由设计决定。

### 3.2 B-01 纳入（R-06）

- 附件 §6 迁移表 sidecar #3 一行（「设了 `CI` 就让真实内核用例 `skipIf` 跳过」）、风险 1、
  非目标里「本任务不改 ci.yml」、Backlog B-01 本身，都要按 R-06 改写。
- 现状：`web` job 有 `defaults.run.working-directory: apps/web`、`timeout-minutes: 15`、
  没有 Rust 工具链与 `Swatinem/rust-cache` 步骤；`ci-config.test.ts` 的
  「CI 按顺序运行四个 Web 检查」用例**逐字断言** `run` 步骤列表正好是
  `npm ci, npm run lint, npm run typecheck, npm test, npm run build`，加构建步骤后必须同步改。
  `desktop` job 已有 `- name: Build the core the sidecar tests drive / run: cargo build -p automed`
  可作参照。构建步骤的工作目录、缓存、超时是否要调，由设计实测后决定。
- 仓库目前**没有 remote**，ci.yml 实际不会被执行；`ci-config.test.ts` 是它在本地唯一的守卫，
  所以 R-06 的验收以该测试为准。

### 3.3 其它沿用时要更新的地方

- 附件里的路径用的是旧目录 `docs/apps-web-的服务端托管-automed/`，本任务一律用
  `docs/apps-web-的服务端托管-automed-2/`。
- 附件「背景与现状」里的基线数字（Web 38 个用例、desktop 104 个、`READ_METHODS` 17 项、
  Vite 8.3.7 的 upgrade 监听器挂载时机等）是 `df63c20` 上测的；main 之后合并了 K0b 等改动，
  设计轮要在本 worktree 重测，以实测值为准，各里程碑预期的用例总数随之重算。

## 4. 范围

**可以改动的目录与文件：**

- `apps/web/server/`（计划 W2「目录」）——含改 `main.ts`、`http.ts` 接入网关；
- `apps/web/test/`（迁移来的测试、新测试、`test/fixtures/` 下的替身），含改 W1 的
  `server.test.ts`（接入网关后启动服务器就会启动内核，必须改用临时 `AUTOME_HOME`）
  与 `ci-config.test.ts`（R-06）；
- `.github/workflows/ci.yml`——**只改 `web` job**，只为 R-06；
- `scripts/ci-local.sh`——原则上不改。只有设计实测证明 ci-local 的 Web test 步骤跑之前拿不到
  可用的 `automed`、必须加一步构建才能与 ci.yml 保持一致时才改，并在设计里写明证据与理由。

**明确不做（出现即越界；均沿用附件「非目标」，去掉了其中「不改 ci.yml」一项）：**

- `events.since` 拉取、WebSocket 消息协议（W3）；通知、`web.sqlite3`（W4）；`server.json`（W10）；
- 页面与组件（W5 起）；`getLoadContext`（W5，附件 B-03）；
- 附件上传的实现（W6；本任务只留 `uploads.resolve` 接口，默认一律返回 null）；
- 日志跟随（W8）；目录补全与 `project.probe` 流程（W9）；`project.pick`、`open.*`、`env.*`
  等本地 op 的实现（W9 等，本任务回 501）；
- `autome web` 与发布包组装（W10）；迁移 `login-path.js`（W10，附件 B-02）；
- Electron 改造与删除 desktop 旧实现（W11）；
- 任何令牌、Cookie、登录会话、`?t=` 地址（R-07）。

## 5. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`。** 计划 D1：内核只在 K1、K2 里改，
   Web 任务不碰 `crates/`。只能**读**或**运行**它们（解析 `dispatch.rs`、构建并运行
   `target/debug/automed`、跑 desktop 测试）。desktop 那份白名单与测试原样保留，每个里程碑的
   `npm test --prefix apps/desktop` 结果不得退化。
2. **不新增依赖，不改 `apps/web/package.json` 与 `package-lock.json`**（附件已批准的决定）。
3. **两份门禁互相独立**：读、写门禁互不 import，也都不 import `apps/desktop` 的任何文件。
4. **tick 不叠加**：上一次 tick 没有真正结束（内核回了 Reply 或子进程已退出）就不发下一次；
   本地等待超时不算结束。网关不碰驱动锁，轮流推进完全交给内核的 `try_acquire`。
5. **服务只绑 `127.0.0.1`**，沿用 W1 的 `server/config.ts`（`HOST`、`resolvePort`，默认 7420，
   `AUTOME_WEB_PORT` 可覆盖）。
6. **测试不碰真实 `~/.autome`。** 任何启动内核或服务器的测试都用 `mkdtemp` 出来的临时
   `AUTOME_HOME`（或显式 `home`），不改 `process.env`；附件 sidecar #2 扩展版的扫描用例要锁住这条。
7. **服务端 TypeScript 走 Node 原生类型剥离**（ESM，`.ts` 后缀导入，无 tsx），与 W1 一致；
   测试用 vitest；`npm run lint`（`--max-warnings 0`）与 `npm run typecheck` 必须无错误。
8. **用户可见文字全中文**，专有名词、方法名、字段名保留原文（2026-09-15 定下的 UI 硬规则）。
   拒绝原因、离线原因、启动日志都算用户可见文字。
9. **宿主门禁会跑 ci-local。** `.autome/config.toml` 配置了 `[gate] command = "sh scripts/ci-local.sh"`
   （超时 30 分钟），每个实现轮之后、合并前、合并后在任务 worktree 里跑。新 worktree 首次运行
   要装 `apps/web/node_modules`；Web 步骤不得依赖交互或浏览器；整套 ci-local 仍需在超时内跑完。
10. 不修改 `.autome/` 下任何内容；不切分支、不推送、不操作 `.worktree/` 下的其它目录；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`（已 gitignore），不进任务目录。

## 6. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（任务整理 → 设计 → 评审/裁决 → 实现/审计 → 合并）驱动
Agent 会话开发软件的本地工具，仅支持 macOS。本仓库用自己的 Loop 开发自己；计划
`docs/plans/2026-11-10-web-dashboard-via-loop.md` 要把 Electron 渲染进程换成浏览器面板：
`apps/web` 里的 Node 服务托管 Rust 内核 `automed`，对浏览器提供 HTTP + WebSocket。

**技术栈与布局：**

- Rust workspace（toolchain 1.93.1，edition 2024）：
  - `crates/autome-domain` —— 纯类型与状态转换；
  - `crates/automed` —— SQLite 账本、Git、会话启动、调度、宿主门禁、协议，framed JSON-RPC over
    stdio（4 字节大端长度前缀）。`src/dispatch.rs`：`pub const READ_METHODS`（读白名单的源头）、
    分派表、`str_param(p, "path")` 取路径的方法（目前是 `project.probe`、`project.add`）；
    `driver_lock.rs`：`<db>.driver.lock` 上的 `flock`，`scheduler.tick` 用非阻塞 `try_acquire`，
    拿不到回 `skipped: true`；`config_io.rs::default_global_dir`：有 `AUTOME_HOME` 用它，否则 `$HOME/.autome`。
  - `crates/autome-cli` —— `autome` 命令行。
- `apps/desktop` —— Electron 外壳，纯 CommonJS，`npm test` = `node --test`。迁移来源：
  `src/{framing,sidecar,core-restart,ipc-gate,write-gate}.js` 及同名测试；tick 循环与写操作分派
  写在 `main.js`（`callCore`、`registerWriteChannel`、`startSidecar`、`restartCore`、`startTicking`），
  不是独立模块。`src/login-path.js` 不迁移。
- `apps/web` —— W1 已建：React Router 8 框架模式 + express 5 自定义服务器，TypeScript ~6.0，
  Vite 8，vitest 5，ESLint 10，`engines.node >= 22.22.0`。现有文件：
  `server/{config,http,main}.ts`（`startServer({port, mode})` 返回 `{server, close}`；
  `createHttpServer({mode})` 开发模式挂 Vite middleware 与 HMR，生产模式挂构建产物）、
  `server/{start,node-version}.js`；`app/` 只有一个占位首页；`test/` 下 5 个测试文件
  （ci-config、config、node-version、server、sqlite-probe）。本 worktree 尚未安装 `node_modules`。
- `protocol/` —— Loop 协议与 prompt 模板（编进二进制）；`plugins/` —— 1.x 插件。
- `scripts/` —— `ci-local.sh`（本地跑 CI，门禁用它）、`package.sh`、`package-cli.sh`、`install-cli.sh`。

**本机（生成时）：** Node v26.5.0；`target/debug/automed` 存在；`~/.local/bin/automed` 不存在
（`~/.local/bin/autome` 存在）。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、
  `npm test --prefix apps/web`、`npm run build --prefix apps/web`（首次先 `cd apps/web && npm ci`）
- Desktop：`npm test --prefix apps/desktop`
- 内核：`cargo build -p automed`；`cargo fmt --all -- --check`、
  `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets`
- 全部：`sh scripts/ci-local.sh`（门禁用的就是它）
- CI：`.github/workflows/ci.yml`，job：`rust`、`web`、`desktop`、`package`、`cli-package`、
  `cli-release`（仅 tag）、`contract`；仓库无 remote，ci.yml 不会被执行，ci-local 是唯一真正
  跑起来的那份，两者要保持一致。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，无具体规则）；仓库内无
  `docs/agent-project-profile.md`；
- `docs/development/requirements.md`、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2 决策 D1–D6、§5 架构、
  §5.5 本机安全、§6 W2（以及 W3、W5、W6、W8、W9、W10、W11 的边界，用于判断什么不归本任务）；
- 附件 `docs/apps-web-的服务端托管-automed-2/attachments/W2-design-v1-T-11.md`（见 §3）。

**前车之鉴（W1 复盘教训与 T-11 的经历，供设计参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`），审计轮可以。W1 六个实现轮都撞上，
  T-11 停在 M-01 也是因为它。安全检查、门禁、路由尽量写成纯函数或用假的 req/res/socket 测；
  需要监听的用例集中在少数文件。实现轮被环境阻断时，在证据里写明被拦住的用例数和
  不依赖监听的那部分结果，交审计轮复验。
- 宿主门禁在基线上可能本来就不全绿。与本任务改动无关、按范围约束不修的门禁失败，要在设计文档
  `## Backlog` 写一条：失败用例全名、断言原文、机制。不要在本任务里修 `crates/` 或 `apps/desktop/`。
- 报验收结果时报 Web 与 desktop 的**用例总数**及与上一轮基线的差，`skipped` 必须为 0
  （本机有二进制时真实内核用例不允许跳过）。
