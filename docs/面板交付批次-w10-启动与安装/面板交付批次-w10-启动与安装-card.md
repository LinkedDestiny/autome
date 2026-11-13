# 面板交付批次：W10 启动与安装、真实数据走查收尾、W11 Electron 薄壳、W12 端到端与一屏验收——一次设计、一次评审，按项分组里程碑依次实现

## 背景

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 的 W1–W9 已全部合并：外壳与总览、项目页与任务卡、任务详情、实时日志、添加项目 / 设置 / 环境页都在 main 上。剩下的是交付：让用户一条命令起面板（W10）、把旧的 Electron 面板换成薄壳（W11）、用真实浏览器把整条链路验一遍（W12）。

按用户 2026-11-13 确认的做法，这三项加上一组走查发现的收尾问题合成一个任务：一份设计、一次评审，批准后按项推进。

现状（2026-11-14，main @ fcabc5e1）：

- T-84 已合并：`autome core` 以 stdio 提供内核；`apps/web/server/core/binary.ts` 的查找顺序是 `AUTOMED_BIN` → 同包的 `automed` → PATH 上的 `autome`（`core`）→ cargo 构建目录；面板启动时打出内核路径与构建标识；内核比账本旧时只读，面板显示横幅并置灰写操作。`scripts/package-cli.sh` 已有 `--binary` / `--out`；`scripts/ci-local.sh --package` 已隔离 `AUTOME_HOME`。
- 起面板目前只能在源码检出里 `npm --prefix apps/web run build && AUTOME_WEB_PORT=<端口> npm --prefix apps/web start`；没有 `apps/web/bin/`，`autome` 没有 `web` 子命令，发布包里没有面板。
- `apps/desktop/` 仍是旧面板全套（`renderer/`、`src/{ipc-gate,write-gate,sidecar,framing}.js` 等）。
- `apps/web/e2e/` 不存在；W7、W9 各有几条「需要真实浏览器」的人工验收项没有自动化。
- 2026-11-14 用已安装的内核对真实账本（91 个任务）起面板走查，各页都能打开，发现的问题见下面 B 项。

## 要做什么

四项，按 A → B → C → D 的顺序实现：

- **A（W10）启动与安装**
- **B 真实数据走查收尾**
- **C（W11）Electron 薄壳**
- **D（W12）端到端与一屏验收**

对设计轮与评审轮的总要求：

1. 一份设计文档覆盖四项。里程碑按项分组、同一项的里程碑连续排列，标题以「A·」「B·」… 开头；每项 2–3 个里程碑，总数不超过 10 个（本任务不适用「超过 6 个里程碑就拆分」的惯例，这是有意的）。每个里程碑能独立验收。
2. 前一项的里程碑全部审计通过后再开始下一项；后面的项不得成为前面验收的前提。
3. 不接触真实账本：实现轮与审计轮起面板、跑 `autome` / 测试一律用临时 `AUTOME_HOME`；不在 worktree 里不带临时 home 地跑任何会打开账本的命令。
4. 测试里起的服务、内核、假 CLI、浏览器进程，测试结束后不留存活（参照 `apps/web/test/logtail-live-cleanup.test.ts` 的检查方式）。
5. 会话沙箱里不能监听本地端口、不能执行 `ps` 时，如实写明哪些验收命令没跑成、由宿主门禁复验，不要因此改弱测试，也不要用条件跳过（面板有不允许条件跳过的守卫）。
6. 不改 `crates/` 里与本任务无关的内核逻辑；A 项只在 `crates/autome-cli/src/` 新增 `web` 子命令。

### A（W10）启动与安装

- 一句话需求：autome-web 启动器与发布包：start / stop / status / open、可选 launchd 自启动，面板进入 CLI 的 tar 包与 install-cli.sh，Rust CLI 新增 `autome web` 子命令转发。
- 目录：`apps/web/bin/`、`scripts/{install-cli,package-cli}.sh`、`crates/autome-cli/src/`（只新增 `web` 子命令）。
- 验收：
  1. `autome web` 默认端口被占用时自动换一个，写进 `server.json`，再打开浏览器；
  2. 重复执行时不起第二个服务，而是打开已有的那个；
  3. `autome web service install|uninstall` 管理 `~/Library/LaunchAgents/` 下的条目；
  4. 发布包包含 `autome` 与构建好的 `apps/web`（内核就是 `autome core`，T-84 已定，不再单独带 `automed`；`binary.ts` 的「同包」一档按发布包的实际布局对齐）；没有 Node ≥ 22.22 时只装命令行，并提示怎样装 Node；
  5. 在空的 `PREFIX` 和空的 `AUTOME_HOME` 下，从 tar 包安装到打开面板全程走通（测试里「打开浏览器」用可注入的打开器，不真的弹窗口）；
  6. 内核子进程拿到用户登录 shell 的 PATH（从 `apps/desktop/src/login-path.js` 迁移），launchd 自启动时也能找到 `claude`、`codex`；
  7. `autome web stop` 在面板的内核正跑着宿主门禁时给出提示并等它结束或要求 `--force`（面板的内核与 `autome run` 轮流推进，直接杀掉会打断门禁）；`autome web status` 显示端口、内核路径与构建标识、是否有另一个驱动在推进。
  8. README 与计划 §7.3 把「用 `npm --prefix apps/web start` 起面板」改成 `autome web`。

### B 真实数据走查收尾

2026-11-14 对真实账本走查发现的问题，都只改 `apps/web/`：

1. **项目页一次返回 1.1 MB。** 91 个任务的项目页 HTML 里标记只有 39 KB，其余是序列化的 loader 数据；每来一个相关事件就整页重验一次。要求：项目页 loader 只返回卡片实际要用的字段；已完成列默认只取最近若干张，其余按需加载（W6 的 Backlog B-04）；100 个任务的项目页响应不超过 200 KB（测试用夹具造 100 个任务断言大小）；同一秒内的多次事件合并成一次重验。
2. **时间与用时不好读。** 任务详情时间线显示的是原始 UTC 时间戳（`2026-11-13T14:28:49Z → …`）和「用时 530 秒」。要求：全站时间按浏览器本地时区显示（当天只显示时分，跨天带日期），用时写成「8 分 50 秒」「1 小时 12 分」；服务端渲染与客户端水合不产生不一致（约定一种做法并有测试）。
3. **配置警告是英文标识。** 项目设置页的配置警告原样显示内核的 `kind`，按 `crates/autome-domain/src/config.rs` 的种类配中文说明，未知种类回退到原文（W9 的 Backlog B-07）。
4. **人工验收项自动化的前置。** W7 的 H-03 / H-04（展开的日志在时间线新增条目时不被折叠、滚动位置不丢；1512×944 不出现页面滚动）与 W9 的 H-01 / H-02 / H-04（三页一屏、补全列表、复制按钮）需要的稳定选择器 / `data-testid` 在这一项里补齐，D 项的浏览器用例直接用。
5. 走查中各页（总览、项目页、新建任务卡、任务详情、添加项目、项目设置、环境）在真实数据下的其它明显问题，设计轮自己再走一遍（用临时 `AUTOME_HOME` 与夹具），发现的列进设计文档；属于内核数据缺口的写 Backlog，不改内核。

### C（W11）Electron 薄壳

- 一句话需求：把 `apps/desktop` 改成薄壳：用 Electron 自带的 Node 运行 `apps/web` 服务、窗口加载面板、提供原生目录选择与通知，删除旧渲染进程与 IPC 白名单。
- 目录：`apps/desktop/`、`scripts/package.sh`；`apps/web/` 里只动与薄壳对接的地方（`project.pick` 的处理函数、`window.autome?.pickDirectory` / `notify` 的使用、`open.path` / `open.terminal` 如有页面需要则按 W9 约束注册）。
- 验收：
  1. .app 不依赖系统 Node；
  2. preload 只暴露 `pickDirectory()` 和 `notify()`；
  3. 删除 `renderer/`、`src/{ipc-gate,write-gate,sidecar,framing}.js` 及对应测试；`scripts/ci-local.sh` 的 Desktop 步骤随之调整；
  4. 已经有 `autome web` 服务在运行时，直接连上它（读 A 项的 `server.json`）；
  5. 打包后的 .app 能启动并显示面板（自动化到「主进程起得来、服务就绪、窗口加载的是面板地址」；真实窗口截图进人工验收清单）；
  6. 薄壳带的内核走 `autome core` / 同包内核同一套查找，内核比账本旧时显示面板已有的横幅。

### D（W12）端到端与一屏验收

- 一句话需求：用 Playwright 对真实 automed 和假 CLI 跑面板端到端测试（需求与 Bug 各一条），并逐页检查 1512×944 不出现页面滚动，接入 CI。
- 目录：`apps/web/e2e/`、`.github/workflows/ci.yml`、`scripts/ci-local.sh`。
- 验收：
  1. 需求：网页建卡（存为待开始）→ 开始 → 卡片实时换列 → 铃铛收到「等你批准」→ 批准 → 合并 → 完成，全程不手动刷新；
  2. Bug：用命令行 `--kind bug --brief -` 建卡 → 面板里看到定位、复现检查、修复、复核逐轮出现在时间线上 → 合并；
  3. 用命令行批准，浏览器 5 秒内更新；
  4. 内核被杀掉后显示离线横幅，恢复后补齐期间的事件；
  5. 每页 `scrollHeight ≤ clientHeight`（内部滚动区域除外），覆盖 W5–W9 的全部页面；
  6. B 项第 4 条列出的 W7、W9 人工验收项各有一条浏览器用例；
  7. 浏览器用例单独成一个门禁步骤：本机没有装 Playwright 浏览器时 `ci-local.sh` 明确报「缺浏览器」并给出安装命令（不静默跳过）；GitHub CI 里安装并运行。整个浏览器步骤在空闲机器上不超过 3 分钟。

## 不做什么

- 不做角色、路由图、技能、协议页（D6）；不做配置的网页编辑；不做远程访问与登录。
- 不改内核的调度、协议、账本；内核数据缺口进 Backlog（任务详情与项目页缺的内核数据在另一个任务里做）。
- 不做 Windows / Linux 的启动器与薄壳（launchd、Terminal.app、.app 都只管 macOS；非 macOS 给出明确提示）。
- 不做自动更新。
- 不改 `.autome/`。

## 验收标准

1. 四个小节里各自的验收逐条成立；设计文档的里程碑验收契约逐条对应到这些条目，并标明属于哪一项。
2. 每一项的里程碑在后一项开始之前全部审计通过。
3. 新增的测试结束后不留存活进程；没有条件跳过的测试。
4. `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`npm --prefix apps/web` 的 lint / typecheck / test / build、`sh scripts/ci-local.sh` 全绿。

## 参考

- 计划 §5.5（本机安全）、§6 W10 / W11 / W12、§7.3、§9。
- T-84 的产物：`crates/automed/src/entry.rs`（`serve_stdio`）、`crates/autome-cli/src/main.rs`（`core`）、`apps/web/server/core/binary.ts`、`apps/web/test/install-alignment.test.ts`、`scripts/package-cli.sh`、README「Upgrading, and which build may drive the ledger」。
- 面板现有页面与服务：`apps/web/app/routes/`、`apps/web/server/{main,http,gateway,admin,terminal,paths}.ts`、`apps/web/app/components/{board-data.ts,task-card-model.ts,task-timeline-model.ts}`。
- 旧面板：`apps/desktop/`（`src/login-path.js`、`src/main/` 里目录选择与通知的实现、`tools/`、`scripts/package.sh`）。
- 已合并任务的设计文档与人工验收清单：`docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/`、`docs/w9-项目管理与环境-网页上添加项目-2/`、`docs/旧内核不能推进新账本-内核带构建标识/`。
- 测试进程清理的写法：`apps/web/test/logtail-live-cleanup.test.ts`。
