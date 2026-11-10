# M-01 实现 #1：实现已落地，服务验收受环境阻塞

## 结果与范围

M-01 保持「开放」，不得标待审：没有取得真实 HTTP 验收通过证据。无上一轮审计、无审计缺陷回归需要搬入。
仅新增 apps/web 工程及本轮任务记录；未修改 CI、Desktop、Rust 或协议。M-02 的 Node 版本检查、M-03 sqlite、M-04 lint/CI 尚未实现。

交付 RR8 框架应用、自定义 express 服务器、固定回环地址、dev Vite 中间件与共享 HTTP server 的 HMR、prod 静态资源与 SSR、中文端口错误、信号退出清理、lockfile，以及 2 文件 12 个 vitest 用例。生产资源路径以模块目录解析，不依赖 cwd。

## 命令与证据

- `npm ci --prefix apps/web > .autome/output/m01.txt 2>&1 && npm test --prefix apps/web >> .autome/output/m01.txt 2>&1`：exit=1。npm ci 成功；测试 2 文件、总计 12 用例（上一轮基线 0，新增 12，来源为 config 5 + server 7），5 通过、7 因共享 beforeAll 启动失败而跳过，文件结果 1 通过 / 1 失败。没有减少或移除任何契约用例。
- `PATH=/opt/homebrew/opt/node@22/bin:$PATH npm test --prefix apps/web`：exit=1；Node 22.23.1 同样总计 12 用例，5 通过、7 跳过。日志 `.autome/output/m01-node22.txt`。相对系统 Node 运行总数差 0；相对上一轮基线仍 +12。
- 根因：dev 启动出现 `File watcher error: EMFILE: too many open files, watch` 及 `listen EPERM: operation not permitted 127.0.0.1`。单独用 node:net `createServer().listen(0, '127.0.0.1')` 复现 exit=1 / EPERM，日志 `.autome/output/m01-listen.txt`，证明监听权限阻塞独立于应用。未申请沙箱提权，当前权限模式不允许。
- `npm run build --prefix apps/web`：系统 Node 26.5.0 exit=0，日志 `.autome/output/m01-build.txt`。
- `PATH=/opt/homebrew/opt/node@22/bin:$PATH npm run build --prefix apps/web`：exit=0，日志 `.autome/output/m01-build-node22.txt`。
- apps/web 下 `npx react-router typegen && npx tsc --noEmit`：exit=0，日志 `.autome/output/m01-typecheck.txt`；仅验证现有工程类型，未推进 M-04 的 lint/CI。
- lockfile：react-router 8.5.1、@react-router/dev 8.5.1、vite 8.3.7、vitest 5.1.0、eslint 10.14.0、typescript 6.0.3；无 tsx。记录 `.autome/output/m01-dependencies.txt`。
- `git diff --check`：通过。

## 标待审之前的自审清单

本轮逐项执行可执行检查，但服务行为未取证，因此不标待审。

### 1. 情形表逐行

| 设计情形 | 检查与结果 |
|---|---|
| 未设端口 / 0 / 8123 / abc / 70000 | config 5 用例全通过 |
| dev healthz 200 与 JSON | 用例已纳入 test/server.test.ts，环境阻塞未执行 |
| dev 首页 200、中文、模块脚本 | 同上，未取证 |
| prod healthz 200 | 同上，未取证 |
| prod 首页及首个 assets JS 200 | 同上，未取证；构建成功不能替代 HTTP 证据 |
| prod 未知路径 404 | 同上，未取证 |
| address() 为 127.0.0.1 / IPv4 | 同上，未取证 |
| 端口占用非零退出且中文提示 | 同上，未取证 |
| dev / prod 均原生加载服务端 TS | Node 26 和 22 的 beforeAll 均走到监听阶段；真实服务启动未成功 |

### 2. 失败分支

- 非整数及超出范围：config 的 abc、70000 通过；补充边界检查见第 4 项。
- EADDRINUSE：项目用例已构造独占端口再启动子进程；因连测试端口都不能监听，未取证。
- 原生类型剥离负向对照：临时在 config.ts 加 `export enum UnsupportedMode { Dev }`，分别用 NODE_ENV=development/production 执行 `node server/start.js`，均 exit=1 且 stderr 含 ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX。日志 `.autome/output/m01-syntax-negative.txt`；通过 finally 还原文件。该检查不是 HTTP 通过证据。
- 去掉监听 host、同时去掉两个静态资源中间件的负向对照：未运行；环境不允许正向监听，无法取得有判别力的红绿对照。
- 拒绝授权、外部服务不可达：不适用，M-01 不含授权或外部服务；没有设计要求的读盘失败/超时业务分支。

### 3. 全部迁移

- dev 初始化 → 监听 → 请求；prod 构建 → 初始化 → 监听 → 请求：构建成功，两条真实启动的服务验收均未完成。
- 监听成功 → SIGINT / SIGTERM → 关闭 HTTP 与 Vite；监听失败 → 关闭 Vite → 非零退出：代码已实现；成功后的信号退出未取证。测试清理会对自己启动的进程组发 SIGTERM，不能代替正常服务退出验收。
- 不存在业务状态机或双向业务通道；HMR 浏览器行为保留 H-01 人工验收，未宣称通过。

### 4. 输入域边界

用 node:assert 直接导入 resolvePort 检查 12 个边界输入，全部通过，日志 `.autome/output/m01-boundaries.txt`：undefined→7420、0→0、65535→65535、0001→1；空串、-1、65536、1.5、前导空格、1e3、Infinity、超大整数字符串均拒绝。它们是临时自审断言，不计入 vitest 12 用例基线。日期/闰日/跨年不适用，本里程碑只有端口输入域。

## 下一步

在允许监听回环地址且文件监听器可用的执行环境重跑 M-01 验收，以及监听 host、静态资源的负向对照；成功后再标待审。本轮不启动下一会话。H-01/H-02 浏览器和 hydration 人工验收未执行。
