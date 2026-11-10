# Web 面板骨架：apps/web 的 React Router 8 框架模式 + 自定义 Node 服务器

status: 实现中
design-round: 2/15
implementation-round: 2/20
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: 先确认宿主门禁的 Rust PID 清理断言阻塞；解除后在允许回环监听的环境复跑 M-01 验收与负向对照

任务文件：`docs/在-apps-web-建-react/在-apps-web-建-react-task.md`。
本任务的规则见 `docs/在-apps-web-建-react/protocol/loop-protocol.md` 与
`docs/在-apps-web-建-react/protocol/session-protocol.md`。

## 背景与现状

- `apps/web` 已有 RR8 工程与 dev/prod 服务入口，真实 HTTP 验收待取证；仓库根没有 `package.json`，不是 npm workspace。`apps/web` 是独立包，
  自带 `package-lock.json`。
- `scripts/ci-local.sh` 按 job 分节（rust / desktop / contract / package），每行
  `run '<名称>' …`，每步都跑、最后汇总；Desktop 节「`node_modules` 不存在才装」。
  宿主门禁在每个实现轮后跑它（超时 30 分钟）。
- `.github/workflows/ci.yml` 每个 job 有 `timeout-minutes`、`runs-on: macos-14`
  （`contract` 与 `cli-release` 用 ubuntu），Node job 用 `actions/setup-node@v4`
  `node-version: '22'`。仓库无 remote，ci.yml 从未真正执行。
- 本机：系统 Node 26.5.0、npm 11.17.0；另有 Homebrew `node@22`（22.23.1）。
- 本 worktree 的 `apps/desktop/node_modules/electron` **没有二进制**（无 `path.txt`、无
  `dist/`）；它的 `index.js` 在缺二进制时会联网下载，本环境下载失败。主检出
  （`git rev-parse --git-common-dir` 的上一级）里的 `apps/desktop` 装有完整的 Electron 43.2.0。

**用户决定（2026-11-10 驳回设计时给出，覆盖任务文件中对应的两处）：** Node 下限为
**22.22.0**（`node:sqlite` 免 flag，且是 React Router 8 的要求）；采用 `react-router@8` 与
`@react-router/*@8` 框架模式，而不是任务需求原文的 v7；工具链取与 22.22 兼容的当前大版本；
服务端 TS 用 Node 原生类型剥离，不用 tsx。任务 R-02、硬性约束 5 的「下限 22」与需求原文的
「React Router v7」按此解读；裁决 D1-P01 的「回到 22.0」由这条用户决定取代。

设计期查到的依赖事实（2026-11-10，`npm view` 与一次实装）：

- `react-router`、`@react-router/{dev,node,express}` 的 latest 是 **8.5.1**，`engines.node` 均为
  `>=22.22.0`。`@react-router/dev@8.5.1` 的 peer：`vite ^7 || ^8`、`typescript ^5.1 || ^6 || ^7`；
  `react-router` 的 peer：`react`/`react-dom >=19.2.7`。
- 工具链当前大版本：`vite@8.3.7`（`^20.19 || >=22.12`）、`vitest@5.1.0`（`^22.12 || ^24 || >=26`）、
  `eslint@10.14.0` 与 `@eslint/js@10`（`^20.19 || ^22.13 || >=24`），都覆盖 22.22。
- `typescript` latest 是 7.0.2，但 `typescript-eslint@8.72.2`（eslint peer 含 `^10`）要求
  `typescript <6.1.0`，所以 TS 取 **6.0.3**，这是唯一不取最新大版本的工具。
- 用 §2 的依赖组合在空包里实装：265 个包，无 ERESOLVE；lockfile 中**所有**带 `engines.node`
  的包都满足 22.22.0（`semver.satisfies` 逐个核对，0 个例外）。
- 同一空包在 Homebrew Node 22.23.1 上实测（产物在 `.autome/output/plan2-probe/`，非交付物）：
  `start.js` 动态 `import('./main.ts')`、`.ts` 之间带扩展名互相 import、`import type` 均由原生
  类型剥离直接运行，**无 ExperimentalWarning**；Vite 中间件模式 + `ssrLoadModule('virtual:react-router/server-build')`
  的 dev 路径 `/healthz` 200、首页 SSR 含占位文字与 `<script type="module"`；`react-router build` 后生产
  路径 `/healthz` 200、首页引用的 `/assets/*.js` 200、未知路径 404；`react-router typegen` 与
  `tsc --noEmit`（TS 6.0.3、`erasableSyntaxOnly`）通过。RR 默认 `entry.server` 仍引用 `isbot`。

## 目标与非目标

目标：交付「能跑、能测、能进 CI 的空骨架」，覆盖任务要求 R-01..R-04。

非目标（任务 §3 明确不做）：automed 托管、RPC 网关、白名单、tick、Host/Origin/令牌（W2）；
事件总线与 WebSocket（W3）；`web.sqlite3` 表结构与通知器（W4）；设计 token、导航、业务
页面（W5 起）；启动器与 `autome web`（W10）；Electron 薄壳（W11）；Playwright（W12）。
不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`、仓库根 `.gitignore`；用户专项门禁修复指令仅允许调整 Desktop app-protocol 的 Electron 加载时机。

## 方案

### §1 目录（D5：一个包、一个进程）

```
apps/web/
├── package.json · package-lock.json · .gitignore（build/ .react-router/）
├── tsconfig.json · eslint.config.js · vite.config.ts · react-router.config.ts · vitest.config.ts
├── app/                 RR8 应用：root.tsx、routes.ts、routes/home.tsx（中文占位页）
├── server/
│   ├── start.js         进程入口：纯 JS，先版本检查，再动态 import('./main.ts') 并调用 runFromEnv()
│   ├── node-version.js  MIN_NODE_VERSION 与 checkNodeVersion()，纯 JS、无任何 import
│   ├── main.ts          startServer(opts) 与 runFromEnv()：读环境、起服务、打日志、SIGINT/SIGTERM 优雅退出；模块顶层无副作用，测试可直接 import
│   ├── http.ts          createHttpServer({ mode })：express 应用 + /healthz + RR 处理器
│   └── config.ts        HOST = '127.0.0.1'、resolvePort(env)
├── scripts/
│   ├── probe-sqlite.ts  R-04 验证脚本（编排两种运行时）
│   ├── sqlite-check.mjs 在被测运行时里跑的检查本体（纯 ESM，Electron 内也能直接跑）
│   └── electron-locate.ts  定位 Electron 二进制（可单测的纯函数）
└── test/                vitest 用例与 fixtures/
```

后续 W2 的 gateway、W3 的 eventbus、W4 的 notifier 作为 `server/` 下的新模块挂进
`main.ts`；它们是长生命周期对象，所以挂在进程入口而不是 `http.ts` 的请求处理里。
`main.ts` 持有 `http.Server` 句柄，W3 的 WebSocket 升级挂在同一个 server 上。

### §2 依赖（package.json 用 `^`/`~` 限定大版本，lockfile 固定精确版本）

- 运行时：`react-router@^8.5.1`、`@react-router/node@^8.5.1`、`@react-router/express@^8.5.1`、
  `react@^19.2.7`、`react-dom@^19.2.7`、`express@^5`、`isbot@^5`（RR 默认 `entry.server` 需要）。
  不需要任何 TS 运行时加载器（§3）。
- 开发：`@react-router/dev@^8.5.1`、`vite@^8`、`vitest@^5`、`typescript@~6.0.3`、
  `eslint@^10`、`@eslint/js@^10`、`typescript-eslint@^8.72`、`globals`、`yaml`（M-04 的配置守卫）、
  `@types/node@^22`、`@types/react`、`@types/react-dom`、`@types/express`。
- **禁止**出现 `react-router@7`/`@react-router/*@7`、`tsx`、`typescript@7`（`typescript-eslint` 不支持）。
  `npm install` 时显式写 `@8`、`@^8`、`@^5`、`@^10`、`@~6.0.3`，不依赖 latest 标签漂移。
- `"type": "module"`，`"engines": { "node": ">=22.22.0" }`。

### §3 两条起服务路径（R-01）

- `npm run dev` = `node server/start.js`；`npm start` = `NODE_ENV=production node server/start.js`；
  `npm run build` = `react-router build`（产物 `build/client`、`build/server`）。
- 服务端 TypeScript 由 Node **原生类型剥离**执行（22.18 起默认开启，下限 22.22.0 必然具备）：
  `start.js` 版本检查通过后 `await import('./main.ts')`；不单独编译服务端、不用 tsx。
  相对 import 写 `.ts` 扩展名；tsconfig 开 `allowImportingTsExtensions`、`erasableSyntaxOnly`
  （禁用 enum、namespace、参数属性等需要转译的语法）、`verbatimModuleSyntax`（类型 import 必须写
  `import type`，否则剥离后运行时会去找不存在的导出）。
- 开发模式：`http.ts` 动态 `import('vite')`，`createServer({ server: { middlewareMode: true,
  hmr: { server: httpServer } } })`，请求经 `vite.middlewares` 后交给
  `createRequestHandler({ build: () => vite.ssrLoadModule('virtual:react-router/server-build') })`。
  HMR 挂在同一个 http.Server 上，**不另占 24678 端口**——并行 2 的两个 worktree 同时跑测试
  时，默认 HMR 端口会互相冲突。
- 生产模式：`express.static('build/client/assets', immutable, 1 年)` +
  `express.static('build/client', 1 小时)`，再交给 `createRequestHandler({ build: await import(build/server/index.js) })`（先 `await`，不能把 Promise 直接当 build）。
  路径以 `import.meta.dirname` 为基准，不依赖 cwd。生产路径不得 import `vite`。
- `GET /healthz` 在 RR 处理器之前由 express 直接返回 200 `{"ok":true}`。
- 监听：主机固定 `127.0.0.1`（不可配置，任务约束 4）；端口取 `AUTOME_WEB_PORT`，未设为
  7420，`0` 表示随机空闲端口；非整数或超出 0–65535 时 `resolvePort` 抛中文错误，进程以 1 退出。
- 启动成功打印一行 `autome-web 已启动：http://127.0.0.1:<实际端口>（开发模式|生产模式）`，
  测试据此取端口。端口被占（EADDRINUSE）时打印「端口 <n> 已被占用」并以 1 退出。
- 占位页：`<title>Autome 面板</title>`，正文中文一句「面板骨架已就绪」，无业务内容。

### §4 Node 版本检查（R-02）

- 下限 **22.22.0**（用户决定，见背景），单一来源 `server/node-version.js` 的
  `MIN_NODE_VERSION = '22.22.0'`，`package.json` 的 `engines.node` 必须是 `>=` 同一个值（测试核对）。
  §2 的全部依赖与 §3 的原生类型剥离都在这个下限上可用。
- `start.js` 只静态 import `./node-version.js`；`node-version.js` 无任何 import。版本不够时
  向 stderr 写「Autome Web 需要 Node.js 22.22.0 或更高版本，当前是 <版本>。请升级 Node.js 后重试。」
  并 `process.exit(1)`；够了才 `import('./main.ts')` 并调用 `runFromEnv()`。检查若放进 `main.ts`，
  旧 Node 会先因 `.ts` 无法加载或依赖的语法而崩，打不出中文提示。
- 版本取 `process.versions.node`，逐段按整数比较（不得按字符串比较）；不可解析的版本视为不够。
- 测试用 `node --import test/fixtures/fake-node-<v>.mjs server/start.js` 改写 `process.versions.node`
  （设计期已验证此法在 Node 26 上可行），生产代码里不留测试开关。
- 本机最接近下限的可实测版本是 Homebrew 22.23.1；22.22.0 本身无法实测，M-02 证据里如实写明
  「下限靠依赖 engines 声明、22.23.1 实测与版本检查单测支撑，未在 22.22.0 上运行」。

### §5 测试、lint 与类型检查（R-03）

- `npm test` = `vitest run`。`vitest.config.ts` 独立于 `vite.config.ts`，不加载 RR 插件
  （RR 的 Vite 插件在 vitest 下会报错）；`testTimeout` 60 秒、`fileParallelism: false`
  （服务端用例会起子进程和做构建）。
- 起子进程的用例用 `detached: true` 起进程组，结束时 `process.kill(-pid)`：`npm run` 下
  只杀 npm 会留下孤儿 node 占着端口。
- 生产路径用例在 `beforeAll` 里自己跑一次 `npm run build`，不复用可能过期的 `build/`。
- `npm run lint` = `eslint . --max-warnings 0`（flat config：`@eslint/js` recommended +
  `typescript-eslint` recommended；忽略 `build/`、`.react-router/`）。
- `npm run typecheck` = `react-router typegen && tsc --noEmit`；单一 tsconfig 覆盖 app/、server/、
  scripts/、test/ 与配置文件，`allowJs` + `checkJs` 让 `start.js` 与 `node-version.js` 也受检；
  §3 的 `erasableSyntaxOnly` 由这一步把关，写了需转译语法的服务端代码在 typecheck 就红。

### §6 CI 接入（R-03）

- `ci-local.sh` 在 contract 节之后、package 节之前加 `# --- job: web ---` 一节：
  `apps/web/node_modules` 不存在时 `run 'Web install' sh -c 'cd apps/web && npm ci'`；
  之后四行 `run 'Web lint'`、`run 'Web typecheck'`、`run 'Web test'`、`run 'Web build'`，
  各自 `npm run <x> --prefix apps/web`。「一个 Web 步骤」按现有结构解读为一个 job 节：
  rust 节也是一个 job 对应多行 `run`；拆成四行才保持「每步都跑」——合成一条 `&&` 链会让
  lint 失败遮住测试失败。不用 `npm install` 兜底：lockfile 已提交，`npm ci` 失败就是缺陷。
- `ci.yml` 加 `web` job：`timeout-minutes: 15`、`runs-on: macos-14`、checkout、
  `setup-node@v4`（`node-version: '22'`，解析为最新 22.x，高于 22.22.0；`cache: npm`、
  `cache-dependency-path: apps/web/package-lock.json`）、
  `working-directory: apps/web` 下 Install（`npm ci`）、Lint、Typecheck、Test、Build 五步。
- 因 ci.yml 不会被执行，用 `test/ci-config.test.ts` 解析 ci.yml（`yaml` 包）与 ci-local.sh
  文本，守住上面两条结构。
- Electron 那一腿（§7）不进 CI 与门禁：CI runner 与新 worktree 都没有 Electron 二进制，
  任务约束 7 不允许依赖外部条件。

### §7 `node:sqlite` 验证（R-04）

设计期预验证（同一段检查：建表、预处理语句写入、读回）：

- 系统 Node 26.5.0：可用，SQLite 3.53.3，无警告。
- Homebrew Node 22.23.1：可用，SQLite 3.53.2，打印 `ExperimentalWarning: SQLite is an experimental feature`。
- Electron 43.2.0（`ELECTRON_RUN_AS_NODE=1`，内置 Node 24.18.0，取主检出的二进制）：可用，SQLite 3.53.1。

**结论：采用 `node:sqlite`，不引入 `better-sqlite3`。** M-03 把它做成可复现的脚本与测试；
脚本结果若与此不符，实现轮就地改写本节结论并按任务要求退回 `better-sqlite3`。
`node:sqlite` 自 22.13 起免 flag，下限 22.22.0 覆盖它，W4 可直接使用；W4 只需决定 Node 22 线上的
ExperimentalWarning 是否在入口过滤。

复现方式：

- `npm run probe:sqlite`（= `node scripts/probe-sqlite.ts`，原生类型剥离）依次在两种运行时里跑
  `scripts/sqlite-check.mjs`：系统 Node（`process.execPath`），以及 Electron
  （`ELECTRON_RUN_AS_NODE=1 <二进制> sqlite-check.mjs`）。
- `sqlite-check.mjs` 在临时目录建文件库，`PRAGMA journal_mode=WAL` 须返回 `wal`，
  建表、预处理语句写入、关闭后重开读回，输出一行 JSON：运行时、Node 版本、Electron 版本、
  SQLite 版本、`ok`、stderr 中的警告。
- Electron 定位（`electron-locate.ts`，按序取第一个可用的）：环境变量 `AUTOME_ELECTRON`
  （二进制路径）→ 本 worktree 的 `apps/desktop/node_modules/electron` → 主检出的同一路径。
  某个包目录「可用」指 `path.txt` 存在且 `dist/<path.txt>` 存在。**不得 `require('electron')`**：
  缺二进制时它会联网下载，在本环境直接失败，也等于改了 `apps/desktop` 的安装内容。
- 跑出的 `process.versions.electron` 必须等于 `apps/desktop/package.json` 的
  `devDependencies.electron`（43.2.0），否则判失败——防止测到别的 Electron。
- 任一运行时失败，脚本以 1 退出；找不到 Electron 以 2 退出并用中文说明「未取证」，不算通过。

## 风险与验证安排

- RR8 框架模式 + 自定义服务器 dev/prod 两条入口不同 → 设计期已在空包上跑通两条路径（见背景）；
  M-01 仍最先做，两条路径都由起真实子进程的用例验收。
- 22 线可运行性：本机只有 22.23.1 与 26.5.0。M-01 实现轮用 Homebrew `node@22`（22.23.1）跑一遍
  完整验收作为 22 线证据；遇到某依赖在 22.x 线实际不兼容时，换该依赖的版本而不是抬高 `MIN_NODE_VERSION`。
- 原生类型剥离的语法边界：`erasableSyntaxOnly` 与 `verbatimModuleSyntax` 让 typecheck 先于运行拦下
  不可剥离的写法；server 用例都经真实 `node server/start.js` 子进程跑，不经 vite 转译，剥离失败会直接红。
- 首次门禁要 `npm ci`：M-04 验收从删掉 `apps/web/node_modules` 开始跑整套 ci-local，并记录
  Web 各步耗时，须远低于 30 分钟门禁超时。
- 依赖装错大版本（RR 7、TS 7、带进 tsx）：§2 写明；M-01 实现轮在证据里列出 lockfile 中 `react-router`、
  `@react-router/dev`、`vite`、`vitest`、`eslint`、`typescript` 的版本，并确认无 `tsx`。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | RR8 工程与自定义服务器：dev 与 build+start 两条路径起服务 | 0 | |
| M-02 | 开放 | 进程入口的 Node 版本检查 | 0 | |
| M-03 | 开放 | node:sqlite 双运行时验证脚本 | 0 | |
| M-04 | 开放 | lint、类型检查与 ci-local / ci.yml 接入 | 0 | |

## 里程碑说明

- M-01 覆盖 R-01、R-03 的 vitest 部分；前置：无。结果：`npm ci` 可从 lockfile 装好；
  `npm run dev` 与 `npm run build && npm start` 都能起，`/healthz` 200，首页 SSR 出中文占位页。
  新增 `test/config.test.ts`（5）与 `test/server.test.ts`（7）。
- M-02 覆盖 R-02；前置 M-01。结果：§4 的入口结构；新增 `test/node-version.test.ts`（12）。
- M-03 覆盖 R-04；前置 M-01。结果：§7 的脚本与 `test/sqlite-probe.test.ts`（6）；本文 §7 结论
  与脚本输出一致。
- M-04 覆盖 R-03；前置 M-01..M-03（ci 要覆盖全部用例，lint 要覆盖全部代码）。结果：§5 的 lint、
  typecheck，§6 的 ci-local 与 ci.yml；新增 `test/ci-config.test.ts`（6）。

### M-01 验收契约

最新证据：docs/在-apps-web-建-react/evidence/M-01-r2-impl.md · Desktop 修复通过，完整门禁仍阻塞，保持开放 · 实现 #2

- 验收命令：`npm ci --prefix apps/web > .autome/output/m01.txt 2>&1 && npm test --prefix apps/web >> .autome/output/m01.txt 2>&1; echo "exit=$?"; tail -15 .autome/output/m01.txt`
- 预期：exit=0；vitest 报 2 个文件、12 个用例全部通过（基线 0，新增 12）：config 5 个——未设端口→7420、`'0'`→0、`'8123'`→8123、`'abc'` 抛中文错误、`'70000'` 抛中文错误；server 7 个——dev `/healthz` 200 `{"ok":true}`、dev `/` 200 且含「Autome 面板」与 `<script type="module"`、prod `/healthz` 200、prod `/` 200 且其引用的第一个 `/assets/*.js` 返回 200、prod 未知路径 404、进程内 `startServer({ port: 0, mode: 'production' })` 的 `address()` 为 `127.0.0.1`/IPv4、端口被占时子进程非零退出且 stderr 含「已被占用」。
- 负向对照：`listen(port)` 不传主机时 `address()` 为 `::`，绑定用例失败；同时去掉 `build/client/assets` 与 `build/client` 两个 `express.static` 时，prod 资源用例拿到 404 失败（只去掉其一，另一个仍会服务该资源，不构成对照）；在 `server/config.ts` 里写一个 `enum`（不可剥离语法）时，经 `node server/start.js` 起的 dev/prod 子进程用例全部失败。

### M-02 验收契约

- 验收命令：`npm test --prefix apps/web > .autome/output/m02.txt 2>&1; echo "exit=$?"; tail -15 .autome/output/m02.txt`
- 预期：exit=0；3 个文件、24 个用例通过（基线 12，新增 node-version 12 个）：`checkNodeVersion` 8 个——9.9.0 ✗、21.7.3 ✗、22.9.0 ✗、22.21.1 ✗、22.22.0 ✓、22.23.1 ✓、26.5.0 ✓、`abc` ✗；拒绝消息为中文且含当前版本与「22.22.0」1 个；以伪造 21.7.3、22.21.1 启动 `server/start.js` 的子进程各 1 个——退出码 1、stderr 为中文提示、stdout 无「已启动」；结构 1 个——`start.js` 的静态 import 只有 `./node-version.js`、`node-version.js` 无 import、`engines.node` 等于 `>=` + `MIN_NODE_VERSION` 且 `MIN_NODE_VERSION` 为 `22.22.0`、`dev` 与 `start` 脚本都经 `server/start.js`。
- 负向对照：按字符串比较版本时 `'9.9.0' > '22.22.0'`、`'22.9.0' > '22.22.0'`，两者被放行，对应用例失败；只比较主版本（`>= 22`）时 22.9.0、22.21.1 被放行，对应用例与伪造 22.21.1 的子进程用例失败；把 `MIN_NODE_VERSION` 改成 22.0.0 时结构用例失败；把检查挪进 `main.ts` 或让 `start.js` 静态 import `./main.ts`，结构用例失败。

### M-03 验收契约

- 验收命令：`npm run probe:sqlite --prefix apps/web > .autome/output/m03-probe.txt 2>&1; echo "probe=$?"; npm test --prefix apps/web > .autome/output/m03.txt 2>&1; echo "exit=$?"; tail -8 .autome/output/m03-probe.txt; tail -15 .autome/output/m03.txt`
- 预期：probe=0，输出两行 `ok: true`：系统 Node（本机 26.5.0）与 Electron 43.2.0（内置 Node 24.18.0），各带 SQLite 版本；exit=0，4 个文件、30 个用例通过（基线 24，新增 sqlite-probe 6 个）：系统 Node 跑 `sqlite-check.mjs` 得 `ok: true` 且日志模式为 `wal`；定位 4 个（用临时目录造的假布局）——`AUTOME_ELECTRON` 优先、worktree 有 `path.txt` 缺 `dist` 时跳过且不触发下载并落到主检出、worktree 完整时优先 worktree、都没有时返回空；版本不等于 `apps/desktop/package.json` 的 43.2.0 时判失败 1 个。
- 负向对照：定位改用 `require('electron')` 时，「缺 dist」用例触发下载、抛错失败；`sqlite-check.mjs` 只测 `:memory:` 时 WAL 断言失败（内存库返回 `memory`）；不核对 Electron 版本时版本用例失败。

### M-04 验收契约

- 验收命令：`rm -rf apps/web/node_modules && sh scripts/ci-local.sh > .autome/output/m04.txt 2>&1; echo "exit=$?"; tail -25 .autome/output/m04.txt`
- 预期：exit=0；汇总里 Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract（新 worktree 里还有 Desktop install）与 Web install、Web lint、Web typecheck、Web test、Web build 全部 `ok`；Web test 步骤报 5 个文件、36 个用例通过（基线 30，新增 ci-config 6 个：ci.yml 有 `web` job 且 `timeout-minutes`、`macos-14`；`setup-node` 为 `'22'` 且 Install 为 `npm ci`；lint/typecheck/test/build 四步齐全；ci.yml 每个 job 都有 `timeout-minutes`；ci-local 仅在缺 `node_modules` 时 `run 'Web install'` 且用 `npm ci`；ci-local 四行 `run 'Web …'` 在 package 分支之外）；证据记录 Web 各步耗时。
- 负向对照：在 `app/routes/home.tsx` 加一个未使用的变量，ci-local 中 Web lint `FAILED`、其余 Web 三步仍各自跑完、总退出码 1；把四步合成一条 `&&` 链时 ci-config 的「四行 run」用例失败。

## 人工验收清单

- H-01 `npm run dev --prefix apps/web` 后用浏览器打开 `http://127.0.0.1:7420`，看到中文占位页；改 `app/routes/home.tsx` 的文字并保存，页面不整页刷新即更新（HMR 生效）。
- H-02 `npm run build --prefix apps/web && npm start --prefix apps/web` 后浏览器打开同一地址，页面正常显示，开发者工具控制台无 hydration 报错。

## Backlog

## 争议项
