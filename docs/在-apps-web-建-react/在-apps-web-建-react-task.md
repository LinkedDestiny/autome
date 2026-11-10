# 任务：Web 面板骨架（W1）——apps/web 的 React Router v7 + 自定义 Node 服务器

本任务的规则见 `docs/在-apps-web-建-react/protocol/loop-protocol.md` 与
`docs/在-apps-web-建-react/protocol/session-protocol.md`。

- 设计文档：`docs/在-apps-web-建-react/在-apps-web-建-react.md`
- 任务文件生成日期：2026-11-10
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支 `autome/在-apps-web-建-react`）

## 1. 需求原文

> 在 apps/web 建 React Router v7 框架模式 + 自定义 Node 服务器的 TypeScript 工程，接入测试与 CI

它是计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 中的 **W1 骨架**。
后续 W2–W12 都建在这个工程上，所以本任务只交付「能跑、能测、能进 CI 的空骨架」，
不做任何面板功能。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

以下 R-01..R-04 原样取自计划 W1 的「验收」，编号供评审、审计追溯使用。

- **R-01 两种方式都能起服务。** 在 `apps/web` 下，`npm run dev` 与
  `npm run build && npm start` 都能启动服务，`GET /healthz` 返回 200。
- **R-02 Node 版本检查。** 启动时检查 Node ≥ 22；版本不够就用**中文**提示后以非零
  退出，不继续启动。
- **R-03 测试与 CI。** 用 vitest 跑测试；`scripts/ci-local.sh` 与
  `.github/workflows/ci.yml` 各加一个 Web 步骤，覆盖 lint、类型检查、测试、构建
  四项。
- **R-04 验证 `node:sqlite`。** 按计划 §5.3：验证 Node 内置的 `node:sqlite` 在
  **系统 Node** 与 **Electron 自带的 Node**（`apps/desktop` 依赖的 electron 43.2.0）
  下都能用；任一不行就退回 `better-sqlite3`，并在设计文档里写明结论与理由。
  验证结论应能被测试或脚本复现，而不只是一句话。

## 3. 范围

**可以改动的目录（计划 W1「目录」，越出即违反）：**

- `apps/web/`（新建）
- `scripts/ci-local.sh`
- `.github/workflows/ci.yml`

`apps/web` 自己的 `.gitignore`（例如 RR7 的构建产物 `build/`、`.react-router/`
生成类型）放在 `apps/web/` 内。仓库根 `.gitignore` 已忽略 `node_modules/` 与
`dist/`，不需要改。

**明确不做（属于后续任务，出现即越界）：**

- 托管 `automed` 子进程、RPC 网关、读写白名单、tick、Host/Origin/令牌检查（W2）；
- 事件总线、WebSocket、`useLiveEvents`（W3）；
- `web.sqlite3` 的表结构与通知器（W4）——R-04 只验证 `node:sqlite` 可用，不建业务表；
- 设计 token 迁移、项目导航、任何业务页面（W5 起）；
- 启动器、发布包、`autome web` 子命令（W10）；Electron 薄壳改造（W11）；
  Playwright 端到端（W12）。

## 4. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`。** 计划 D1：内核只在
   K1、K2 里改，Web 任务不碰 `crates/`。R-04 若需要用 Electron 的 Node 跑验证，
   只能**调用** `apps/desktop` 已安装的 electron（例如 `ELECTRON_RUN_AS_NODE=1`），
   不得修改 `apps/desktop` 的任何文件或依赖。
2. **一个包、一个进程**（计划 D5）：`apps/web` 同时包含 RR7 应用（框架模式，
   loader/action/SSR）与自定义 Node 服务器；后续 HTTP、WebSocket、sidecar、tick、
   通知都会挂在这个服务器上，骨架的结构要为此留出位置（例如 `server/` 与 `app/`
   分开），但不实现它们。
3. **TypeScript 工程**，测试框架用 **vitest**（R-03 指定）；lint 工具与类型检查方式
   由设计决定，但必须能在 CI 与 ci-local 中非交互运行、失败时非零退出。
4. **服务只绑 `127.0.0.1`**（计划 §5、§5.5）。计划中的默认端口是 7420；骨架阶段
   端口应可通过环境变量覆盖，便于测试使用随机/空闲端口。
5. **Node 版本：** 下限 22（R-02）。CI 使用 `actions/setup-node@v4` 的
   `node-version: '22'`；本机当前为 Node v26.5.0、npm 11.17.0。必须提交
   `apps/web/package-lock.json`，CI 用 `npm ci`。
6. **CI 结构与现有一致：** `ci.yml` 中每个 job 都有 `timeout-minutes`，runner 用
   `macos-14`（产品只支持 macOS）；ci-local 中的新步骤沿用其 `run '<名称>' …` 写法、
   「每步都跑、最后汇总」的语义，以及「`node_modules` 不存在才安装」的做法
   （参照现有 Desktop 步骤）。ci-local 新增步骤不得让其它步骤被跳过。
7. **宿主门禁会跑 ci-local。** 本项目 `.autome/config.toml` 配置了
   `[gate] command = "sh scripts/ci-local.sh"`（超时 30 分钟），每个实现轮之后、
   合并前、合并后都在任务 worktree 里跑。因此：新 worktree 第一次跑时
   `apps/web/node_modules` 不存在，ci-local 必须能自行安装；Web 步骤不得依赖
   交互、浏览器或网络以外的外部服务；整套 ci-local 仍需在门禁超时内跑完。
8. **UI 文字全中文**，专有名词保留原文（2026-09-15 定下的 UI 硬规则）。骨架如有
   占位页面，同样遵守。
9. 不修改 `.autome/` 下任何内容；不切分支、不推送、不操作 `.worktree/` 下的其它
   目录；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`（已 gitignore）。

## 5. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（设计 → 评审/裁决 → 实现/审计 → 合并）驱动
Agent 会话开发软件的本地工具，仅支持 macOS。本仓库也用自己的 Loop 开发自己。

**技术栈与布局：**

- Rust workspace（toolchain 1.93.1，edition 2024）：
  - `crates/autome-domain` —— 纯类型与状态转换，含设计文档状态块解析器；
  - `crates/automed` —— SQLite、Git、会话启动、调度、宿主门禁、协议与 eval、
    JSON-RPC 分发（`src/dispatch.rs` 的 `READ_METHODS` 是读白名单的源头）；
  - `crates/autome-cli` —— `autome` 命令行。
- `apps/desktop` —— Electron 43.2.0 外壳，纯 JS（无 TypeScript、无打包器），
  `npm test` = `node --test test/*.test.js`；`src/sidecar.js`、`framing.js`、
  `ipc-gate.js`、`write-gate.js` 是 W2 将迁移到 `apps/web` 的来源（本任务不迁移）。
- `apps/web` —— **尚不存在**，由本任务新建。仓库根没有 `package.json`，
  不是 npm workspace。
- `protocol/` —— Loop 协议与 prompt 模板（编进二进制）；`plugins/loop` —— 1.x 插件。
- `scripts/` —— `ci-local.sh`（本地跑 CI）、`package.sh`、`package-cli.sh`、
  `install-cli.sh`。
- `.githooks/post-commit` —— 若设置了 `core.hooksPath=.githooks`，每次提交后在后台跑
  ci-local（`NO_CI=1` 可跳过）。

**测试命令：**

- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace --all-targets`
- `npm test --prefix apps/desktop`
- 全部一次跑完：`sh scripts/ci-local.sh`（门禁用的就是它）；
  `sh scripts/ci-local.sh --package` 额外打包
- CI：`.github/workflows/ci.yml`，现有 job：`rust`、`desktop`、`package`、
  `cli-package`、`cli-release`（仅 tag）、`contract`。仓库目前没有 remote，
  ci.yml 实际上不会被执行，ci-local 是唯一真正跑起来的那份——两者必须保持一致。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，无具体规则）；
- `docs/development/requirements.md`、`docs/development/technical-design.md`；
- `docs/adr/0001..0003`；
- 本任务的上游计划：`docs/plans/2026-11-10-web-dashboard-via-loop.md`
  （§2 决策 D1–D6、§5 架构、§5.3 面板库、§6 W1）。仓库内无
  `docs/agent-project-profile.md`。

**已知风险（供设计参考，不是结论）：**

- `node:sqlite` 在 Node 22 早期版本需要 `--experimental-sqlite`，各版本行为与
  ExperimentalWarning 不同；Electron 是否编入 `node:sqlite` 需实测（R-04）。
- RR7 框架模式 + 自定义服务器在 dev（Vite 中间件）与生产（构建产物）两条路径上
  的入口不同，R-01 要求两条都能起。
- 门禁在全新 worktree 中首次运行要装依赖，Web 步骤的耗时会叠加在现有 1–2 分钟上。
