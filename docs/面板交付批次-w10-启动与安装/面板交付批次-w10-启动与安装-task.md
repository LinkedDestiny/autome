# 任务：面板交付批次——W10 启动与安装、真实数据走查收尾、W11 Electron 薄壳、W12 端到端与一屏验收

本任务的规则见 `docs/面板交付批次-w10-启动与安装/protocol/loop-protocol.md` 与
`docs/面板交付批次-w10-启动与安装/protocol/session-protocol.md`。

- 设计文档：`docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装.md`
- 任务卡（用户写的，第一输入）：`docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装-card.md`
- 用户指定的参考文档：`docs/plans/2026-11-10-web-dashboard-via-loop.md`（下称「计划」；§5.3、§5.5、§6 W10–W12、§7.3、§9）
- 任务整理日期：2026-11-14
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `f4a320d2`（任务卡写的 main 是 `fcabc5e1`）

## 1. 需求原文

> 面板交付批次：W10 启动与安装、真实数据走查收尾、W11 Electron 薄壳、W12 端到端与一屏验收——一次设计、一次评审，按项分组里程碑依次实现

任务卡正文比这句话具体得多；两者不一致时以任务卡为准。§2–§5 照任务卡整理（编号是整理轮加的，供里程碑引用），
§6–§7 是整理轮从调研得出的约束与待定问题，§8 是代码现状，§9 是前几个任务留下的教训，§10 是项目背景。

## 2. 背景与目标

计划的 W1–W9 已全部合并：外壳与总览、项目页与任务卡、任务详情、实时日志、添加项目 / 设置 / 环境页都在 main 上。
剩下的是交付：让用户一条命令起面板（W10）、把旧的 Electron 面板换成薄壳（W11）、用真实浏览器把整条链路验一遍
（W12）。按用户 2026-11-13 确认的做法，这三项加上一组走查发现的收尾问题合成**一个任务：一份设计、一次评审，
批准后按项推进**。

任务卡记录的现状（2026-11-14）：

- T-84 已合并：`autome core` 以 stdio 提供内核；`apps/web/server/core/binary.ts` 的查找顺序是 `AUTOMED_BIN` →
  同包的 `automed` → PATH 上的 `autome`（`core`）→ cargo 构建目录；面板启动时打出内核路径与构建标识；内核比账本旧
  时只读，面板显示横幅并置灰写操作。`scripts/package-cli.sh` 已有 `--binary` / `--out`；`scripts/ci-local.sh --package`
  已隔离 `AUTOME_HOME`。
- 起面板目前只能在源码检出里 `npm --prefix apps/web run build && AUTOME_WEB_PORT=<端口> npm --prefix apps/web start`；
  没有 `apps/web/bin/`，`autome` 没有 `web` 子命令，发布包里没有面板。
- `apps/desktop/` 仍是旧面板全套。`apps/web/e2e/` 不存在；W7、W9 各有几条「需要真实浏览器」的人工验收项没有自动化。
- 2026-11-14 用已安装的内核对真实账本（91 个任务）起面板走查，各页都能打开，发现的问题见 B 项。

**目标。** 四项，按 **A → B → C → D** 的顺序实现：

- **A（W10）启动与安装**——装好之后一条 `autome web` 就能在浏览器里打开面板；
- **B 真实数据走查收尾**——项目页变轻、时间好读、警告有中文、给浏览器用例备好选择器；
- **C（W11）Electron 薄壳**——`apps/desktop` 只剩启动服务、开窗口、目录选择与通知；
- **D（W12）端到端与一屏验收**——真实内核 + 假 CLI + 真实浏览器把需求与 Bug 两条链路验一遍，并接入门禁。

## 3. 任务要求（任务卡「要做什么」，逐条照录；设计文档的里程碑必须全部覆盖）

### 3.0 对设计轮与评审轮的总要求

- **总-1** 一份设计文档覆盖四项。里程碑按项分组、同一项的里程碑连续排列，标题以「A·」「B·」「C·」「D·」开头；
  每项 2–3 个里程碑，总数不超过 10 个。**本任务不适用「超过 6 个里程碑就拆分」的惯例（计划 §7.4），这是有意的。**
  每个里程碑能独立验收。
- **总-2** 前一项的里程碑全部审计通过后再开始下一项；后面的项不得成为前面验收的前提。
- **总-3** 不接触真实账本：实现轮与审计轮起面板、跑 `autome` / 测试一律用临时 `AUTOME_HOME`；不在 worktree 里不带
  临时 home 地跑任何会打开账本的命令。
- **总-4** 测试里起的服务、内核、假 CLI、浏览器进程，测试结束后不留存活（参照
  `apps/web/test/logtail-live-cleanup.test.ts` 的检查方式）。
- **总-5** 会话沙箱里不能监听本地端口、不能执行 `ps` 时，如实写明哪些验收命令没跑成、由宿主门禁复验，不要因此
  改弱测试，也不要用条件跳过（面板有不允许条件跳过的守卫）。
- **总-6** 不改 `crates/` 里与本任务无关的内核逻辑；A 项只在 `crates/autome-cli/src/` 新增 `web` 子命令。

### 3.1 A（W10）启动与安装

一句话需求：autome-web 启动器与发布包：start / stop / status / open、可选 launchd 自启动，面板进入 CLI 的 tar 包与
install-cli.sh，Rust CLI 新增 `autome web` 子命令转发。
目录：`apps/web/bin/`、`scripts/{install-cli,package-cli}.sh`、`crates/autome-cli/src/`（只新增 `web` 子命令）。

- **A-1** `autome web` 默认端口被占用时自动换一个，写进 `server.json`，再打开浏览器。
- **A-2** 重复执行时不起第二个服务，而是打开已有的那个。
- **A-3** `autome web service install|uninstall` 管理 `~/Library/LaunchAgents/` 下的条目。
- **A-4** 发布包包含 `autome` 与构建好的 `apps/web`（内核就是 `autome core`，T-84 已定，不再单独带 `automed`；
  `binary.ts` 的「同包」一档按发布包的实际布局对齐）；没有 Node ≥ 22.22 时只装命令行，并提示怎样装 Node。
- **A-5** 在空的 `PREFIX` 和空的 `AUTOME_HOME` 下，从 tar 包安装到打开面板全程走通（测试里「打开浏览器」用可注入的
  打开器，不真的弹窗口）。
- **A-6** 内核子进程拿到用户登录 shell 的 PATH（从 `apps/desktop/src/login-path.js` 迁移），launchd 自启动时也能找到
  `claude`、`codex`。
- **A-7** `autome web stop` 在面板的内核正跑着宿主门禁时给出提示并等它结束或要求 `--force`（面板的内核与
  `autome run` 轮流推进，直接杀掉会打断门禁）；`autome web status` 显示端口、内核路径与构建标识、是否有另一个驱动
  在推进。
- **A-8** README 与计划 §7.3 把「用 `npm --prefix apps/web start` 起面板」改成 `autome web`。

### 3.2 B 真实数据走查收尾（都只改 `apps/web/`）

- **B-1 项目页一次返回 1.1 MB。** 91 个任务的项目页 HTML 里标记只有 39 KB，其余是序列化的 loader 数据；每来一个
  相关事件就整页重验一次。要求：项目页 loader 只返回卡片实际要用的字段；已完成列默认只取最近若干张，其余按需加载
  （W6 的 Backlog B-04）；100 个任务的项目页响应不超过 200 KB（测试用夹具造 100 个任务断言大小）；同一秒内的多次
  事件合并成一次重验。
- **B-2 时间与用时不好读。** 任务详情时间线显示的是原始 UTC 时间戳（`2026-11-13T14:28:49Z → …`）和「用时 530 秒」。
  要求：全站时间按浏览器本地时区显示（当天只显示时分，跨天带日期），用时写成「8 分 50 秒」「1 小时 12 分」；
  服务端渲染与客户端水合不产生不一致（约定一种做法并有测试）。
- **B-3 配置警告是英文标识。** 项目设置页的配置警告原样显示内核的 `kind`，按 `crates/autome-domain/src/config.rs`
  的种类配中文说明，未知种类回退到原文（W9 的 Backlog B-07）。
- **B-4 人工验收项自动化的前置。** W7 的 H-03 / H-04（展开的日志在时间线新增条目时不被折叠、滚动位置不丢；
  1512×944 不出现页面滚动）与 W9 的 H-01 / H-02 / H-04（三页一屏、补全列表、复制按钮）需要的稳定选择器 /
  `data-testid` 在这一项里补齐，D 项的浏览器用例直接用。
- **B-5** 走查中各页（总览、项目页、新建任务卡、任务详情、添加项目、项目设置、环境）在真实数据下的其它明显问题，
  **设计轮自己再走一遍**（用临时 `AUTOME_HOME` 与夹具），发现的列进设计文档；属于内核数据缺口的写 Backlog，不改内核。

### 3.3 C（W11）Electron 薄壳

一句话需求：把 `apps/desktop` 改成薄壳：用 Electron 自带的 Node 运行 `apps/web` 服务、窗口加载面板、提供原生目录
选择与通知，删除旧渲染进程与 IPC 白名单。
目录：`apps/desktop/`、`scripts/package.sh`；`apps/web/` 里只动与薄壳对接的地方（`project.pick` 的处理函数、
`window.autome?.pickDirectory` / `notify` 的使用、`open.path` / `open.terminal` 如有页面需要则按 W9 约束注册）。

- **C-1** .app 不依赖系统 Node。
- **C-2** preload 只暴露 `pickDirectory()` 和 `notify()`。
- **C-3** 删除 `renderer/`、`src/{ipc-gate,write-gate,sidecar,framing}.js` 及对应测试；`scripts/ci-local.sh` 的
  Desktop 步骤随之调整。
- **C-4** 已经有 `autome web` 服务在运行时，直接连上它（读 A 项的 `server.json`）。
- **C-5** 打包后的 .app 能启动并显示面板（自动化到「主进程起得来、服务就绪、窗口加载的是面板地址」；真实窗口截图进
  人工验收清单）。
- **C-6** 薄壳带的内核走 `autome core` / 同包内核同一套查找，内核比账本旧时显示面板已有的横幅。

### 3.4 D（W12）端到端与一屏验收

一句话需求：用 Playwright 对真实 automed 和假 CLI 跑面板端到端测试（需求与 Bug 各一条），并逐页检查 1512×944 不出现
页面滚动，接入 CI。
目录：`apps/web/e2e/`、`.github/workflows/ci.yml`、`scripts/ci-local.sh`。

- **D-1** 需求：网页建卡（存为待开始）→ 开始 → 卡片实时换列 → 铃铛收到「等你批准」→ 批准 → 合并 → 完成，全程不
  手动刷新。
- **D-2** Bug：用命令行 `--kind bug --brief -` 建卡 → 面板里看到定位、复现检查、修复、复核逐轮出现在时间线上 → 合并。
- **D-3** 用命令行批准，浏览器 5 秒内更新。
- **D-4** 内核被杀掉后显示离线横幅，恢复后补齐期间的事件。
- **D-5** 每页 `scrollHeight ≤ clientHeight`（内部滚动区域除外），覆盖 W5–W9 的全部页面。
- **D-6** B-4 列出的 W7、W9 人工验收项各有一条浏览器用例。
- **D-7** 浏览器用例单独成一个门禁步骤：本机没有装 Playwright 浏览器时 `ci-local.sh` 明确报「缺浏览器」并给出安装
  命令（不静默跳过）；GitHub CI 里安装并运行。整个浏览器步骤在空闲机器上不超过 3 分钟。

## 4. 验收标准（任务卡原文）

- **验-1** 四个小节里各自的验收逐条成立；设计文档的里程碑验收契约逐条对应到这些条目，并标明属于哪一项。
- **验-2** 每一项的里程碑在后一项开始之前全部审计通过。
- **验-3** 新增的测试结束后不留存活进程；没有条件跳过的测试。
- **验-4** `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`npm --prefix apps/web` 的
  lint / typecheck / test / build、`sh scripts/ci-local.sh` 全绿。

设计轮写里程碑时，§3 的每一条（A-1…D-7）都要落到某个里程碑的验收契约上；需要真人的部分（如 C-5 的真实窗口截图）
按协议进 `## 人工验收清单`，不作里程碑的验收条件。

## 5. 范围

**在范围内（任务卡给出的目录，以设计为准）。**

| 项 | 目录 |
|---|---|
| A | `apps/web/bin/`（新建）、`scripts/install-cli.sh`、`scripts/package-cli.sh`、`crates/autome-cli/src/`（只新增 `web` 子命令）；文档 `README.md`、计划 §7.3；为做到 A-1/A-2/A-6/A-7 而必须动的 `apps/web/server/`（写 `server.json`、登录 PATH、`binary.ts` 的「同包」一档）及对应测试 |
| B | 只改 `apps/web/` |
| C | `apps/desktop/`、`scripts/package.sh`、`scripts/ci-local.sh` 的 Desktop 步骤；`apps/web/` 里只动与薄壳对接的地方 |
| D | `apps/web/e2e/`（新建）、`.github/workflows/ci.yml`、`scripts/ci-local.sh` |

**不做（任务卡「不做什么」，逐条照录）。**

- 不做角色、路由图、技能、协议页（D6）；不做配置的网页编辑；不做远程访问与登录。
- 不改内核的调度、协议、账本；内核数据缺口进 Backlog（任务详情与项目页缺的内核数据在另一个任务里做）。
- 不做 Windows / Linux 的启动器与薄壳（launchd、Terminal.app、.app 都只管 macOS；非 macOS 给出明确提示）。
- 不做自动更新。
- 不改 `.autome/`。

**整理轮补充的边界（从调研得出，不扩大范围；带「整理轮理解」的是解释，设计轮可据证据修正）。**

- `crates/automed`、`crates/autome-domain` 不改。A-7 要的「面板的内核是否正跑着门禁」「是否有另一个驱动在推进」
  只能从内核现有的读接口与文件得出；得不出的部分写清楚并进 Backlog，不为此加内核方法（见 §7 A8）。
- 整理轮理解：「只新增 `web` 子命令」包含它必需的 `USAGE` 文案、`run()` 里的分发，以及
  `crates/autome-cli/tests/` 里针对它的测试；不包含改其它动词的行为。
- 仓库的 `protocol/` 与本任务的协议副本都不改。
- launchd 里已有的驱动条目 `com.autome.loop`（`~/Library/LaunchAgents/`，仓库之外）是驱动本仓库 Loop 的那一个，
  会话不得读写、装卸它；A-3 新增的条目必须用不同的 label。
- D-7 的「单独一个门禁步骤」靠改 `scripts/ci-local.sh` 与 `ci.yml` 达成；项目的门禁配置在 `.autome/config.toml`
  里（`[gate] command = "sh scripts/ci-local.sh"`），不能改。
- README 里与旧桌面端有关的段落（`What it looks like` 的截图、`Layout` 的 `apps/desktop` 一条、`Packaging`）在 C 项
  之后会与事实不符。任务卡只在 A-8 点名了 README；C 项是否同步这些段落由设计轮定，不同步的写进 Backlog。

## 6. 硬性约束

1. **顺序与分组（总-1、总-2、验-2）。** 里程碑表里 A 的里程碑排在最前，然后 B、C、D；实现轮只推进最小编号的开放
   里程碑，所以编号顺序就是执行顺序。前面项的验收命令不得依赖后面项才有的东西（例如 A 的测试不能用 D 的 Playwright，
   B 的验收不能依赖 C 的薄壳）。反过来是允许的，而且是有意的：C-4 读 A 的 `server.json`，D 用 B-4 的选择器。
   每项 2–3 个里程碑、总数 ≤ 10；每个里程碑都要有协议要求的三行验收契约（验收命令、预期、负向对照）。
2. **不接触真实账本与真实安装位置（总-3）。** 本仓库正由已安装的 `~/.local/bin/autome`（launchd `com.autome.loop`，
   `autome run`）驱动自己的 Loop，账本在 `~/.autome`。因此：
   - 任何手工验证与测试都把 `AUTOME_HOME` 指到临时目录，不得省略、不得指向 `~/.autome`；起面板、起薄壳同样；
   - 跑 `scripts/install-cli.sh` 必须同时设 `PREFIX` 与 `AUTOME_HOME` 为临时目录——脚本结尾会执行 `autome doctor`，
     它会打开账本并跑迁移；缺省 `PREFIX` 还会覆盖正在驱动 Loop 的 `~/.local/bin/autome`；
   - 面板里拉真实内核的测试必须**显式**指定内核（`binaryPath` / `AUTOMED_BIN`，现有测试用
     `process.env.AUTOMED_BIN || <仓库>/target/debug/automed`）。`binary.ts` 的缺省查找会落到 PATH 上已安装的
     `autome`——那是另一个版本的内核（T-84 的教训，见 §9）；
   - `server.json` 只在临时 `AUTOME_HOME` 下读写；测试里的 `stop` 只能给本测试自己起的进程发信号，不得按真实
     `~/.autome/web/server.json` 里的 pid 发信号；测试不得假定 7420 空闲，也不得连接或停掉用户正在用的面板；
   - `autome web service install|uninstall` 的测试不得写真实的 `~/Library/LaunchAgents/`，不得对真实 launchd 执行
     `launchctl bootstrap/bootout/kickstart`——目录与 `launchctl` 都要可注入；
   - 测试不得真的打开浏览器窗口、Terminal.app 或系统对话框（打开器、终端、对话框都注入替身）。
3. **测试不留存活进程（总-4、验-3）。** 凡是会起服务、内核、假 CLI、Electron、浏览器的新测试，都要有一条按
   `logtail-live-cleanup.test.ts` 的方式检查的断言：给本次运行一个私有临时根目录，结束后用 `ps -axo pid=,ppid=,command=`
   找命令行里带这个根目录的进程及其子孙，必须为空。
4. **没有条件跳过（总-5、验-3）。** `apps/web/test/core-sidecar.test.ts` 的「测试源码不按条件跳过真实内核用例」
   会扫 `apps/web/test/*.ts`，出现 `test|it|describe.skip/skipIf/runIf/todo(`、`skip:` / `todo:`、`process.env.CI`
   就失败。平台前提用断言表达（`install-alignment.test.ts` 的写法：`expect(process.platform).toBe('darwin')`），
   不用跳过。这个守卫目前不扫 `apps/web/e2e/`；验-3 对 e2e 同样成立（见 §7 D8）。
5. **沙箱拦住时如实记录（总-5）。** 实现轮的沙箱可能拒绝 `listen(127.0.0.1)` 与 `ps`（W7、W9 的全部实现轮都是
   `EPERM`，审计轮可以）。遇到时：证据文件逐条列出因此没跑成的验收用例名，交审计轮 / 宿主门禁复验；不改弱测试、
   不加跳过。`npm run lint`、`npm run typecheck`、`cargo fmt`、`cargo clippy` 是静态检查，不受影响，实现轮当场跑。
6. **内核只读不改（总-6）。** `crates/` 里只有 `crates/autome-cli/src/` 的 `web` 子命令（及 §5 所说的测试）。
   `READ_METHODS` 的常量名与内容不动：`apps/web/test/read-gate.test.ts`、`write-gate.test.ts` 解析 `dispatch.rs`
   核对它，`ci-local.sh` 的 `Contract` 步骤 grep 这个常量名。
7. **本机安全不放松（计划 §5.5）。** 只绑 `127.0.0.1`；Host 与 Origin 检查跟着**实际监听的端口**走（自动换端口后
   仍然成立；薄壳窗口加载的必须是服务自己的 `http://127.0.0.1:<端口>`）；不加令牌、Cookie、登录。浏览器不能给出
   任意路径：原生目录选择器选出的路径仍要过服务端的路径校验与 `project.probe`。薄壳的 preload 只暴露两个函数（C-2）。
   A 项若给服务加新的 HTTP 接口（例如供 `status` 用），同样受这几条约束。
8. **UI 硬规则继续有效。** 全中文，专有名词保留原文；卡片优先、低密度；1512×944 下页面本身不出现纵向滚动（看板列、
   时间线、日志面板可以内部滚动）；视觉沿用 animal-island 的设计 token（已在 `apps/web/app/styles/tokens.css`）。
9. **CI 步骤清单有测试钉着。** `apps/web/test/ci-config.test.ts` 断言 `ci.yml` 的 `web` job（`macos-14`、限时 15 分钟、
   `run` 步骤恰为 `npm ci` → `npm run lint` → `npm run typecheck` → `cargo build -p automed -p autome-cli` →
   `npm test` → `npm run build`）、每个 job 都有正数 `timeout-minutes`、`ci-local.sh` 的 web 段恰好四行
   `run 'Web <lint|typecheck|test|build>'` 且位于 contract 之后、package 之前、`CLI package` 步骤隔离 home 与 prefix。
   改这两个文件的里程碑要在同一轮同步这份测试。
10. **依赖变了，`node_modules` 要跟上。** `ci-local.sh` 与每轮快检都只在 `apps/web/node_modules`（或
    `apps/desktop/node_modules`）**不存在**时才安装。新增依赖（D 的 Playwright，C 可能的打包依赖）之后，worktree 里
    已有的 `node_modules` 是旧的，快检的 typecheck 与合并前门禁会因缺模块失败；合并之后主检出同理（合并后 main 验证
    跑的是同一条 `ci-local.sh`）。改了 `package.json` / `package-lock.json` 的那一轮必须把锁文件一起提交，并让
    worktree 的 `node_modules` 与之一致；主检出与门禁怎样跟上由设计轮给出做法（见 §7 D2）。
11. **脚本的既有约定。** `install-cli.sh` / `package-cli.sh` / `ci-local.sh` 为 `/bin/sh`（macOS 的 bash 3.2）而写：
    不用数组、`[[`、`local`；中文紧跟变量时写 `${var}`；安装用「写新文件再 `mv`」，打包用复制而不是原地改名
    （macOS 对已校验签名的二进制的文件身份敏感，改名副本曾实测被 SIGKILL——需要第二个名字时先实测）。
    `--from` 安装（含 `.sha256` 校验）、`package-cli.sh --binary` 不触发 cargo 构建（`install-alignment.test.ts`
    的 PATH 里故意没有 cargo，也没有 node / npm）这两点要继续成立。
12. **时间预算。** 浏览器步骤空闲机器上 ≤ 3 分钟（D-7）；宿主门禁整套 `ci-local.sh` 上限 30 分钟，每轮快检上限
    10 分钟；`ci.yml` 的 `web` job 限时 15 分钟（被测试钉住）。新增的安装 / 打包 / 薄壳测试放进缺省门禁还是
    `--package` 一档，要按这些预算取舍并写明。
13. **文案语言沿用现状。** 界面、CLI 输出、报错与日志用中文；代码注释与 `README.md` 用英文；计划文档与
    `docs/development/technical-design.md` 用中文。
14. **设计文档的体量。** 四项合在一份文档里，协议的警告线是 ≤ 80KB、单轮增长 ≤ 10KB；证据写进 `evidence/`，
    `## 里程碑` 一节只放那一张表。
15. **会话边界（AGENTS.md）。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改
    `.autome/`；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成本轮后结束会话，
    不自行启动下一个会话。

## 7. 待设计轮回答的问题

任务卡留白、或与代码现状有出入的地方。只列问题与相关事实，不预设答案。设计文档「方案」一节须逐条给出结论与依据；
没有依据支持改动任务卡字面的，按任务卡字面做。

### A（W10）

- **A1 启动器与转发的分工。** `apps/web/bin/` 里的 Node 启动器与 Rust 的 `autome web` 各管什么：谁找 Node（≥ 22.22）、
  谁找装好的面板目录、谁起后台进程。事实：`autome` 的 `run()` 对**每个**动词都先 `Client::open()`（打开账本并跑
  迁移），`core` / `render-stream` / `protocol eval` 是在 `main()` 开头单独分发的；`web` 不在 `run()` 的允许列表里
  （现在是「未知命令」）；`Args` 把不认识的 `-x` 当开关。`autome web` 是否需要打开账本、`--force` 等参数怎样透传。
- **A2 发布包与安装布局。** tar 包现在只有 `autome` 与 `README.md`；`install-cli.sh` 的 `unpack` 只找名为 `autome`
  的可执行文件，装到 `$PREFIX`（一个 bin 目录，缺省 `~/.local/bin`）。「构建好的 `apps/web`」包含什么
  （`build/`、`server/*.ts`、`bin/`、`package.json`、运行时依赖——服务端在运行时 `import` `express`、`react-router`、
  `@react-router/{node,express}`、`react`、`react-dom`、`isbot`、`ws`；`server/*.ts` 靠 Node 原生类型剥离直接运行，
  先核实它对所在路径有没有限制）；面板装到哪里、`autome web` 在「检出里」与「安装后」各怎样找到它；
  `package-cli.sh` 怎样拿到构建好的面板而不破坏 `--binary` 不触发构建的约定；包的体积。
- **A3 `binary.ts` 的「同包」一档。** 现在是 `webRoot/../automed`（`source: 'bundle'`，不带参数）。任务卡要求按
  发布包实际布局对齐、且不再单独带 `automed`。新的判定、`source` 取值、`apps/web/test/core-binary.test.ts` 哪些
  断言要改；这一档 C-6 也要用（薄壳带的内核），A 定的布局不能让 C 无处落脚，但 A 的验收不得依赖 C。
- **A4 没有 Node ≥ 22.22 时。** `install-cli.sh` 怎样判定（PATH 上的 `node`？登录 shell 的 PATH？）、提示怎样写；
  只装了命令行之后执行 `autome web` 给什么；面板已装但运行时 Node 不够（现有 `server/start.js` 用
  `node-version.js` 检查并以中文退出，夹具 `test/fixtures/fake-node-21.7.3.mjs`、`fake-node-22.21.1.mjs`）。
- **A5 端口与 `server.json`。** 现状：缺省 7420（`server/config.ts` 的 `resolvePort`，`AUTOME_WEB_PORT`），占用时打
  「端口 N 已被占用」并退出码 1；仓库里**没有任何代码写 `server.json`**。计划 §5.3 定的位置与内容是
  `~/.autome/web/server.json`（实际端口、pid、启动时间）、日志 `~/.autome/web/logs/server.log`。已有的读者：
  `plugins/autome/skills/autome-task/scripts/panel-url.sh` 用 `sed` 取 `"port"`，再要求 `/healthz` 返回 `{"ok":true}`
  （测试 `plugins/autome/tests/card-panel.sh` 的 P3 / P4 / P10）。要定：谁写、何时写、字段（`status` 要的内核路径与
  构建标识放不放）、原子写入、陈旧文件怎样识别（pid 已死、pid 被别的进程复用、端口换了主人）、两个 `autome web`
  同时起的竞争、退出与崩溃后的清理；显式给了 `AUTOME_WEB_PORT` 而它被占用时是换还是报错；`npm start` / `npm run dev`
  起的服务写不写；`autome web` 是前台还是后台，`start` / `open` / 不带子命令各是什么语义。
- **A6 打开器。** macOS 用什么打开；测试怎样注入（A-5 要求可注入）；非 macOS 的提示。
- **A7 launchd 条目。** label（不得与 `com.autome.loop` 相同）、plist 内容（`ProgramArguments` 用哪些绝对路径、
  `KeepAlive`、日志、`AUTOME_HOME`）、`install` / `uninstall` 的幂等、怎样加载与卸载；launchd 给的 PATH 只有
  `/usr/bin:/bin:/usr/sbin:/sbin`，自启动时怎样找到 Node；测试怎样在不碰真实 `~/Library/LaunchAgents/` 与真实
  launchd 的前提下断言（约束 2）。
- **A8 `stop` 与 `status`（A-7）。** 事实：宿主门禁是独立进程组里的后台作业（`crates/automed/src/gate.rs`：包装脚本
  写 pid、把输出 tee 到日志、结束时写退出标记，文件在 `<仓库>/.autome/output/gates/<任务>/`；注释写明「重启的内核
  会在原处找到它们」），每个 tick 只是看文件；驱动锁是 `<账本>.driver.lock` 上按**每次操作**取放的 `flock`，
  不记录持有者；`scheduler.tick` 的返回有 `skipped`（布尔）与 `older_kernel`；`task.get` / `dashboard.get` 带 `gate`
  与 `gate_queue_position`；`/healthz` 只回 `{ ok: true }`；服务收到 SIGINT / SIGTERM 会走 `close()`（停 ticker、
  停内核）。要定并用实验核实：停掉面板的内核究竟会打断门禁的什么（任务卡的说法是「直接杀掉会打断门禁」）；
  「正跑着宿主门禁」从哪些现有读接口判定；「等它结束」的呈现与上限、`--force` 的行为；`status` 的「另一个驱动在
  推进」从哪来；`stop` / `status` 怎样与运行中的服务通信，发信号前怎样确认 pid 是自己的服务。
- **A9 登录 shell 的 PATH（A-6）。** `apps/desktop/src/login-path.js`（172 行：`parseLoginPath`、`mergePaths`、
  `shellQuery`、`spawnShell`、`resolveCorePath`、`corePathEnv`；3 秒超时；测试 `apps/desktop/test/login-path.test.js`）
  迁到 `apps/web` 的哪里、在哪一层生效（`startServer({ kernelEnv })` → `CoreSupervisor` → `sidecar.ts` 组装子进程
  环境）；从终端起与 launchd 起各怎样；用什么测试证明内核子进程真的拿到了（例如注入一个假登录 shell，再看内核能否
  找到只在那个 PATH 上的 `claude`）；桌面端那一份在 A 留着、到 C 再删，还是别的安排。
- **A10 A-5 的测试放在哪。** 「从 tar 包安装到打开面板」需要一个构建好的面板，而 `ci-local.sh` 与 `ci.yml` 里
  `Web test` 都排在 `Web build` 之前（顺序被 `ci-config.test.ts` 钉住）。测试自己构建、调整步骤顺序、还是放进
  `--package` 一档；它需要监听端口（约束 5）与真实内核（约束 2 的显式指定）。
- **A11 文档（A-8）。** 事实：`README.md` 里**没有**「`npm --prefix apps/web start`」这句（全文检索为空），
  也没有 `autome web`、7420；提到面板的只有 `### Upgrading, and which build may drive the ledger`（含内核查找顺序
  那一段，A3 改了之后要同步）。计划 §7.3 的那一句在 481 行。README 里 `autome web` 写在哪（英文）、`Layout` 要不要补
  `apps/web`；tar 包里的中文 README 在 `package-cli.sh` 的 heredoc 里，`install-alignment.test.ts` 断言它含
  `autome core` 与 `autome --version`。

### B 走查收尾

- **B1 项目页的体积从哪来、砍到哪。** 事实：`loadBoard`（`app/components/board-data.ts`）取 `project.get` 的全部任务，
  再对每张可见的非草稿卡各调一次 `task.get`，把整份详情放进 `details`；卡片实际读的字段见
  `task-card-model.ts` 的 `cardView`（任务的 `id / title / kind / state / fix_round / completed_at / merge_commit`，
  详情的 `gate / gate_queue_position / queue_position / retry / sessions 里正在跑的那一个 / status_block`）；
  根路由的 `shell-data.ts` 也把各项目的任务放进同一份文档。要定：200 KB 量的是哪个响应（整页文档、重验的数据
  请求，还是两者）；「已完成列默认只取最近若干张」与现有的「近 14 天 + `?done=all` 查看全部」（计划 §3.2）怎样统一、
  若干是多少、列标题里的总数怎样保持正确；「按需加载」的形态（内核没有分页，裁剪在面板服务端做）；100 个任务的夹具
  怎样造（现有 `test/fixtures/w6-project.sh`、`w5-project-get.json`、`dashboard-double.ts`）。
- **B2 「同一秒内的多次事件合并成一次重验」的语义。** 现状：`app/lib/live-routes.ts` 的 `revalidateForEvents` 每收到
  一批 WebSocket 事件就重验一次，`board-live.ts` 对通知消息 `revalidateAll`。合并是前沿还是后沿触发、最长延迟多少
  （D-3 要求命令行批准后浏览器 5 秒内更新，tick 间隔 3 秒）；合并期间累积的事件不得丢（`pending` 决定哪些路由重验）；
  现有 `live-routes.test.ts`、`board-live-timing.test.ts` 哪些要改。
- **B3 时间与用时。** 事实：全站只有一个本地时间函数 `formatClock`（`app/lib/labels.ts:51`，取 `Date` 的本地时分）；
  时间线在 `task-timeline-model.ts` 里直接拼 `started_at → ended_at` 与「用时 N 秒」，轻量事件里还原样带出
  `not_before`、`until`。要定：「全站」包含哪些位置（设计轮列全）；当天 / 跨天 / 跨年的写法；用时的写法规则
  （不足 1 分钟、是否到小时就不写秒、超过一天）；SSR 与水合一致的做法（任务卡要求「约定一种做法并有测试」）以及
  随时间变化的文字怎样刷新。
- **B4 配置警告。** 事实：设置页 `routes/projects.$id.settings.tsx:78` 现在是 `<pre>{JSON.stringify(warning)}</pre>`，
  数据来自 `config.validate` 的 `violations`；`config.rs` 的 `ConfigViolation` 以 `kind` 标记，现有 8 种
  （`same_model`、`skill_not_visible`、`skill_not_found`、`parallel_out_of_range`、`design_rounds_out_of_range`、
  `session_minutes_out_of_range`、`budget_factor_out_of_range`、`both_default_models`），各带字段（角色、运行时、
  取值范围等）。每种的中文说明怎样用上这些字段；怎样用测试与 `config.rs` 交叉核对种类清单（参照 `labels.test.ts`
  对内核节点的做法），内核新增种类时回退到原文。
- **B5 选择器清单（B-4）。** 事实：`apps/web/app` 里现在**一个 `data-testid` 都没有**。命名规则与清单（至少覆盖
  W7 H-03 / H-04、W9 H-01 / H-02 / H-04，以及 D-1…D-5 要点到的看板列、任务卡、铃铛、横幅、时间线条目、日志面板、
  操作按钮）；B 的验收怎样在不依赖浏览器的前提下证明选择器存在且稳定。五条人工验收项的原文见 §8。
- **B6 设计轮自己的走查（B-5）。** 用临时 `AUTOME_HOME`、显式的 `AUTOMED_BIN` 与夹具，把七个页面在 100 个任务量级下
  走一遍；发现的问题列进设计文档并归到 B 的里程碑或 Backlog（内核数据缺口只进 Backlog）。走查需要先
  `(cd apps/web && npm ci)`、`cargo build -p automed -p autome-cli` 并能监听端口；做不到时如实写明没走到哪些页。
- **B7 D 的浏览器用例暴露出 B 范围内的产品缺陷时归谁。** 例如 H-03（时间线新增条目时已展开的日志不被折叠、滚动
  位置不丢）现在是否成立并没有自动化证据。约定：这类缺陷在 D 的里程碑里就地修（`apps/web/`），还是别的处理。

### C（W11）

- **C1 Electron 自带的 Node 能否原样跑这套服务。** 事实：`apps/desktop` 依赖 `electron 43.2.0`；服务要求
  Node ≥ 22.22（`start.js` 检查 `process.versions.node`）、`node:sqlite`、原生类型剥离运行 `server/*.ts`、
  `import.meta.dirname`；W1 留有 `npm run probe:sqlite --prefix apps/web`（`scripts/probe-sqlite.ts`，用
  `AUTOME_ELECTRON` 或 `apps/desktop/node_modules/electron` 定位 Electron）。这是 C 最大的技术不确定性，应在 C 的
  第一个里程碑里消除：服务跑在主进程里还是子进程里、asar 内的 `.ts` 能不能被剥离与加载、面板的 `build/` 与运行时
  依赖怎样进 .app。
- **C2 .app 里带什么内核。** 现状：`scripts/package.sh` 把 release 的 `automed` 放进 `Resources/core/automed`；
  `ci-local.sh --package` 的「Bundle carries a runnable core」与 `ci.yml` 的 `package` job 都执行
  `Resources/core/automed --help`。C-6 要求与 A3 的「同包」一档是同一套查找——定下 .app 里的布局并同步这两处检查。
- **C3 preload 与页面的对接。** 事实：`app/components/native-picker.ts` 约定 `window.autome.pickDirectory():
  Promise<string | null>`；`project.pick` 在写白名单与 `gateway.ts` 的 `localOps` 里，但没有注册处理函数（现在回
  501「网页端还不支持」，W9 的设计写明保持 501）；系统通知走 `app/components/bell-system.ts` 的 `SystemNotifier`
  （`permission / request / show`）。`pickDirectory()` 选出的路径怎样回到页面并仍过服务端校验（路径守卫只放行
  `$HOME` 之内）；`project.pick` 的处理函数到底需不需要；`notify()` 的参数与点击后的行为；主进程对 IPC 调用方与参数
  的校验、窗口的导航限制（外链、新窗口）；`open.path` / `open.terminal` 是否有页面需要（没有就不注册）。
- **C4 连接已有服务（C-4）。** 怎样用 `server.json` 判定「已有 `autome web` 在运行」；薄壳自己起的服务写不写
  `server.json`（写了之后 `autome web` 会复用它）；退出薄壳时停不停服务（连上的是别人起的就不该停）；已在运行的
  服务与薄壳自带面板版本不同时怎样。
- **C5 删什么、留什么、连带改什么。** 任务卡点名删除 `renderer/`、`src/{ipc-gate,write-gate,sidecar,framing}.js` 及
  对应测试。其余文件的去留要逐个交代：`main.js`（552 行，多为 IPC）、`preload.js`（165 行）、`src/app-protocol.js`
  （`autome://app` 协议，专为旧渲染进程）、`src/core-restart.js`（面板已有 `server/core/restart-policy.ts`）、
  `src/login-path.js`（A9）、`tools/screenshots.js` 与 `npm run shots`、`test/` 下的 `dom*.js`、`fixtures.js`、
  `labels-*.test.js`、`renderer-contract.test.js`、`packaging.test.js`、`app-protocol.test.js`、`core-restart.test.js`。
  **连带**：`apps/web/test/styles.test.ts` 直接读 `apps/desktop/renderer/style.css`（token 名值逐条对照）与
  `apps/desktop/renderer/fonts/nunito/`（字体与许可的哈希对照），删 `renderer/` 的那个里程碑必须同轮处理这份测试
  而不丢掉它守的东西；`apps/web/scripts/{electron-locate,probe-sqlite}.ts` 读 `apps/desktop/package.json` 的
  electron 版本；`package.json` 的 `build.files` / `extraResources`、`entitlements`；`ci-local.sh` 的 Desktop 与
  Contract 步骤的注释、`ci.yml` 的 `desktop` / `package` / `contract` job。
- **C6 C-5 自动化到哪一档。** 「主进程起得来、服务就绪、窗口加载的是面板地址」用什么驱动与观测；对打包后的 .app
  做（需要 `package.sh`，分钟级，首次要下载 electron-builder）还是对未打包的 `electron .` 做，各放进缺省门禁还是
  `--package` 一档；会话里跑不了 Electron 时怎样记录。已知坑：`node --test test/*.test.js` 会把 `test/` 下匹配的
  `.js` 都当测试；本机 headless 下 `capturePage()` 曾报 `UnknownVizError`（截图只能进人工验收清单）；macOS 对改名的
  二进制副本会 SIGKILL。

### D（W12）

- **D1 Playwright 的版本与浏览器。** 事实：`apps/web/package.json` 里没有 Playwright；整理时本机
  `~/Library/Caches/ms-playwright/` 里有 `chromium-1194`、`chromium-1243`、`chromium-1248` 及对应的
  `chromium_headless_shell-*`。D-7 要求缺浏览器时**明确失败**而不是跳过——而宿主的合并前门禁跑的正是
  `sh scripts/ci-local.sh`：选定的 Playwright 版本对应的浏览器修订号若不在本机，门禁必败、任务过不了合并。要定：
  版本、`ci-local.sh` 怎样检测与提示、`ci.yml` 里怎样安装与缓存；需要用户先装浏览器的话，写进设计文档最显眼处
  与人工验收清单。
- **D2 依赖怎样跟上（约束 10）。** 加了 Playwright 之后，worktree 与主检出里已有的 `apps/web/node_modules` 都是旧的；
  快检的命令在 `.autome/config.toml` 里（不能改），只在目录不存在时安装。给出做法（例如让 `ci-local.sh` 发现锁文件
  与已装依赖不一致时重装），并说明合并后主检出不会因此红。
- **D3 被测服务与内核怎样起。** 生产构建（要先 `npm run build`，见 A10 的步骤顺序）还是开发模式；端口取临时端口
  （Host / Origin 检查跟着实际端口）；临时 `AUTOME_HOME` 与临时 `HOME`（路径守卫只放行 `$HOME` 之内，临时仓库要在
  里面）；内核显式指定（约束 2）；会话不开终端（内核认 `AUTOMED_HEADLESS`）；假 CLI 经
  `AUTOMED_CLAUDE_BINARY` / `AUTOMED_CODEX_BINARY` 注入（`crates/automed/src/launcher.rs`），现成的分步假 CLI 在
  `apps/web/test/fixtures/w7-flow.sh`（按 `.autome/fake/<n>.sh` 逐步执行）。
- **D4 两条链路各要几轮、花多久。** 需求链路从整理到合并要经过十来个节点，每个节点一个假会话加若干个 3 秒的
  tick；Bug 链路类似。在 3 分钟的总预算里（D-7）怎样排：并行、缩短等待、还是复用同一个服务实例。
- **D5 D-3 与 D-4 的做法。** D-3：用 `target/debug/autome approve`（同一个临时 `AUTOME_HOME`）批准，量浏览器更新的
  时间（受 tick 间隔与 B2 的合并策略影响）。D-4：杀哪个进程；内核崩溃后面板按 `restart-policy.ts` 自动重启，
  「离线横幅」的可见窗口怎样做成确定的；「恢复后补齐期间的事件」里，离线期间的事件由谁产生（内核不在时命令行仍能
  写账本）。现成夹具：`test/fixtures/automed-pidfile.sh`（包一层真实内核并写出 pid）。
- **D6 「覆盖 W5–W9 的全部页面」的清单（D-5）。** 七条路由（`/`、`/projects/:id`、`/projects/:id/tasks/new`、
  `/tasks/:id`、`/projects/new`、`/projects/:id/settings`、`/env`）各取哪些状态（需求 / Bug 任务详情、展开日志、
  Onboarding 进行中、⌘K 面板、铃铛下拉等）；判据的确切写法（W9 H-01 用的是
  `.shell-main, .shell-main > *` 逐个 `scrollHeight <= clientHeight`，W7 H-04 用 `document.documentElement`）。
- **D7 五条人工验收项各自的浏览器用例（D-6）。** 原文见 §8；H-04（复制按钮）涉及剪贴板权限，H-02 还包含
  「普通浏览器里看不到『选择目录…』按钮」。
- **D8 门禁步骤的形态（D-7）。** `ci-local.sh` 里新步骤的位置（依赖 `cargo test` 先构建出内核、依赖面板构建与否）；
  `ci.yml` 里是新 job 还是并进 `web` job（后者的步骤清单与 15 分钟上限被 `ci-config.test.ts` 钉住）；
  `vitest.config.ts` 没有 `include` / `exclude`，`vitest run` 缺省会收 `**/*.{test,spec}.*`——`apps/web/e2e/` 里的
  文件不得被 vitest 误收；`tsconfig.json` 的 `include`（`app`、`server`、`scripts`、`test`、`*.ts`）与 eslint（`.`）
  对 `e2e/`、`bin/` 的覆盖；「没有条件跳过」的守卫是否延伸到 `e2e/`。
- **D9 浏览器用例的进程清理（总-4）。** 浏览器、服务、内核、假会话各由谁收，怎样按约束 3 断言。

## 8. 代码现状（整理轮调研，基线 `f4a320d2`；行号为约数，设计轮引用前自行复核）

**命令行 `crates/autome-cli`。** `src/main.rs`（1547 行）：`USAGE`（44 行起，`其他` 一组现有 `core`、`rpc`、
`version | help`；有断言其内容的单测，1516 行起）；`main()`（87 行起）先分发 `core`（`automed::entry::serve_stdio()`）、
`render-stream`、`protocol eval`，其余进 `run()`；`run()` 按动词列出允许的开关，随后对每个动词 `Client::open()`；
`doctor` 的 `--json` 输出含 `home / ledger / driver_lock / binary / kernel / environment / severity`。`src/args.rs`
（213 行，手写解析，`VALUED` 列出带值的开关）。`src/client.rs`：CLI 不拉起内核进程，直接链接 `automed` 库。
`tests/cli.rs`（1995 行）：`Sandbox` 用 `env!("CARGO_BIN_EXE_autome")` 跑真实二进制，`AUTOME_HOME` 指向沙箱。

**内核（只读参考）。** `crates/automed/src/entry.rs`：`serve_stdio`、`version_line`、`ledger_path`
（`AUTOMED_DB_PATH` 或 `AUTOME_HOME` 下）；环境开关 `AUTOME_ALLOW_OLDER_KERNEL`、`AUTOME_TEST_BUILD_ID`（仅 debug 构建）。
`launcher.rs`：`AUTOMED_HEADLESS`、`AUTOMED_CLAUDE_BINARY`、`AUTOMED_CODEX_BINARY`、`AUTOMED_GIT_BINARY`。
`dispatch.rs`：`scheduler_tick` 返回 `sessions_reaped / tasks_advanced / tasks_started / errors / skipped /
older_kernel / latest_seq / env_generation / env_probing`；`env.get` 的 `kernel` 含 `build_id`、`ledger_build_id`、
`may_advance`、`message`；`project.get` 返回 `project`、全部 `tasks`（含 `completed_at`）与 `config`。
`gate.rs`、`driver_lock.rs` 的相关事实见 §7 A8。`autome-domain/src/config.rs`：`ConfigViolation`（566 行起）、
`warnings()`（727 行，另一组非阻塞的中文提示）。

**面板服务端 `apps/web/server`。** `start.js`（Node 版本检查后加载 `main.ts`）；`main.ts`（98 行）：
`startServer({ port, mode, home, userHome, binaryPath, binaryArgs, kernelEnv, terminal })`——缺省
`resolveAutomedBinary()`，启动时 `console.error(describeKernel(...))`，监听 `HOST`（`127.0.0.1`）；`runFromEnv()`
读 `AUTOME_WEB_PORT` 与 `NODE_ENV`，成功后打「autome-web 已启动：http://127.0.0.1:<端口>（生产|开发模式）」，
`EADDRINUSE` 时打「端口 N 已被占用」并置退出码 1，SIGINT / SIGTERM 走 `close()`。`config.ts`：`resolvePort`
（缺省 7420，接受 0–65535）。`home.ts`：`AUTOME_HOME` 或 `$HOME/.autome`。`core/binary.ts`（21 行）：
`resolveAutomedBinary({ env, home, webRoot, exists })` → `{ path, args, source: 'env' | 'bundle' | 'cli' | 'cargo' }`，
顺序 `AUTOMED_BIN`（文件名是 `autome` 时带 `['core']`）→ `webRoot/../automed` → PATH 上第一个 `autome`、再
`~/.local/bin/autome`（都带 `['core']`）→ `webRoot/../../target/<AUTOMED_CARGO_PROFILE|debug>/automed`。
`core/identity.ts`：`describeKernel` 跑一次 `<内核> --version` 取构建标识，cargo 来源打「正在使用源码构建的内核：…」。
`core/supervisor.ts`（130 行）、`core/sidecar.ts`（322 行，`killIfRunning`）、`core/restart-policy.ts`。
`security.ts`：`acceptsHost` / `acceptsOrigin` 按实际端口判断。`http.ts`：`/healthz` 回 `{ ok: true }`；
`/api/read`、`/api/write`、`/api/paths`、`/api/uploads`。`gateway.ts`：`localOps = project.pick / project.confirm /
open.path / open.terminal / env.install / env.login`，没注册处理函数的回 501；`admin.ts` 注册了 `env.install`、
`env.login`、`project.confirm`。`paths.ts`：路径守卫只放行 `$HOME` 之内。`terminal.ts`：`createMacTerminal`
（`osascript` 开 Terminal.app，可注入 `execFile`）。`ticker.ts`：每 3 秒一次。`package.json`：`dev` =
`node server/start.js`、`start` = `NODE_ENV=production node server/start.js`、`build` = `react-router build`、
`test` = `vitest run`、`lint`、`typecheck`、`probe:sqlite`；`engines.node >= 22.22.0`。

**面板前端 `apps/web/app`。** 路由七条（`app/routes.ts`）：`/`（`_index.tsx`）、`/env`、`/projects/new`、
`/projects/:id`、`/projects/:id/settings`、`/projects/:id/tasks/new`、`/tasks/:id`。项目页
`routes/projects.$id.tsx`（63 行）：loader 并行取 `loadBoard` 与 `dashboard.get`；`shouldRevalidate` 对
`task. / project. / bug. / merge. / gate. / session. / runtime.` 前缀的事件重验。`components/board-data.ts`（53 行）、
`task-card-model.ts`（40 行）、`task-columns.ts`、`board-live.ts`；`lib/live.tsx`（`useLiveEvents`、`liveBanner`、
`writeGate`）、`lib/live-routes.ts`（36 行）、`lib/live-client.ts`。时间线 `components/task-timeline-model.ts`
（124 行）与 `task-timeline.tsx`（会话条目用 `<SessionLog defaultOpen={running}>`）。设置页
`routes/projects.$id.settings.tsx`（88 行）与 `components/settings-data.ts`。通知 `components/bell*.ts(x)`。
原生目录选择的预留 `components/native-picker.ts`（2 行）。

**面板测试 `apps/web/test`（112 个文件，vitest，`fileParallelism: false`，超时 60 秒）。** 与本任务最相关的：
`install-alignment.test.ts`（用 `package-cli.sh --binary target/debug/autome --out <临时>` 打包、空 `PREFIX` 安装、
`resolveAutomedBinary` 找到并真实握手）、`core-binary.test.ts`、`ci-config.test.ts`、`core-sidecar.test.ts`
（不许条件跳过的守卫，415 行）、`logtail-live-cleanup.test.ts`（进程清理的检查方式）、`fixtures/process-leak.ts`
（`leftoverProcesses` / `teardown`）、`styles.test.ts`（读桌面端的样式与字体，见 §7 C5）、`w9-layout.test.ts`、
`board-live-timing.test.ts`、`live-routes.test.ts`、`task-timeline*.test.ts`、`settings-config.test.ts`。
夹具 `fixtures/`：`w6-project.sh`、`w7-flow.sh`、`w7-states.sh`、`w5-states.sh`、`automed-pidfile.sh`、`fake-core.mjs`、
`fake-node-*.mjs`、`dashboard-double.ts` 等。约有 14 个测试文件会真的监听端口或起服务。

**旧桌面端 `apps/desktop`。** `main.js`（552 行：`BrowserWindow` 加载 `autome://app/index.html`，`autome:read` /
`autome:write` 两条 IPC，`pickProject()` 在 169 行用 `dialog.showOpenDialog`）、`preload.js`（165 行）、
`src/{app-protocol,core-restart,framing,ipc-gate,login-path,sidecar,write-gate}.js`、`renderer/`（`app.js`、
`index.html`、`style.css`、`fonts/`、`lib/`、`screens/` 九个页面）、`test/`（14 个文件，`node --test test/*.test.js`）、
`tools/screenshots.js`、`build/entitlements.mac.plist`；`package.json`：`electron 43.2.0`，`build.files` 列了
`main.js / preload.js / src/** / renderer/**`，`extraResources` 把 `core/` 放进包。任务卡「参考」里写的
`src/main/` 目录并不存在——目录选择的实现在 `main.js`，旧的界面内通知在 `renderer/lib/notify.js`。

**脚本与 CI。** `scripts/install-cli.sh`（169 行：`--from` / 检出内构建 / 下载发布三条路径，只支持 macOS，
装到 `${PREFIX:-$HOME/.local/bin}/autome`，结尾跑 `doctor`）。`scripts/package-cli.sh`（95 行：`--binary`、`--out`，
产出 `autome-<版本>-<架构>-apple-darwin.tar.gz` 与 `.sha256`，包内一个目录，含 `autome` 与 `README.md`）。
`scripts/package.sh`（50 行：release 构建 `automed` → `apps/desktop/core/` → `electron-builder --mac`）。
`scripts/ci-local.sh`（158 行，步骤依次为 Format、Clippy、Test、Desktop（缺 `node_modules` 时先装）、Contract、
Web lint / typecheck / test / build；`--package` 再加 Package、CLI package、Bundle carries a runnable core；每步都跑完，
任一失败则非零）。`.github/workflows/ci.yml`：job `rust`、`web`、`desktop`、`package`、`cli-package`、`cli-release`、
`contract`。`.githooks/post-commit` 会在提交后后台跑 `ci-local.sh`（本 worktree 的 `core.hooksPath` 指向它）。

**插件（只读参考）。** `plugins/autome/skills/autome-task/scripts/panel-url.sh` 读 `server.json` 的 `port`（见 §7 A5）。

**五条待自动化的人工验收项（原文）。**

- W7 H-03：展开一条日志并把日志滚到中间，用命令行推进或暂停这个任务，页面自动更新后那条日志仍展开、滚动位置没跳、
  时间线没有跳回顶部。
- W7 H-04：把浏览器窗口调到 1512×944，在开发者工具里量 `document.documentElement.scrollHeight ≤ clientHeight`，
  时间线、文档、日志各自内部滚动。
- W9 H-01：用临时 `AUTOME_HOME` 起面板，浏览器窗口 1512×944，分别打开 `/projects/new`、某项目的
  `/projects/<id>/settings`、`/env`，在控制台执行
  `[...document.querySelectorAll('.shell-main, .shell-main > *')].every(e => e.scrollHeight <= e.clientHeight)`，
  三页都为 `true`（内部滚动区除外）。
- W9 H-02：添加项目页输入 `~/` 开头的路径时补全列表随输入出现，点选后填入输入框；在普通浏览器里看不到
  「选择目录…」按钮。
- W9 H-04：环境页点「复制」，剪贴板里是页面上的命令原文。

**本机现状（整理时）。** Node v26.5.0；worktree 里还没有 `apps/web/node_modules`、`apps/desktop/node_modules` 与
`target/`（第一次要 `npm ci` 与 cargo 构建）；本次整理轮的会话里 `listen(127.0.0.1:0)` 与 `ps` 都可用——这只说明
这一轮，后面的实现轮未必如此（§9 第 1 条）。

## 9. 前车之鉴（来自已合并任务的复盘，逐条可核对）

1. **实现轮多半不能监听端口、不能 `ps`。** W7 的 5 个里程碑、W9 的 8 个实现轮全部在 `listen EPERM` / `spawnSync ps
   EPERM` 下结束，没有一轮标成 `待审`，测试自身的缺陷只能等审计轮第一次真正跑到断言时才暴露（W9 M-02 因此 reopen）。
   对策：需要监听的验收，实现轮先把不需要监听的部分（纯函数、模型层、静态检查）跑绿并写明哪些用例没跑成；
   设计时尽量让每个里程碑都有一部分验收不依赖监听。
2. **静态检查当场跑。** W9 有一轮没跑 typecheck 就交了，被宿主快检退回，多花一轮。lint、typecheck、fmt、clippy
   不受沙箱影响。
3. **新写的测试自己触发守卫。** T-84 的 `install-alignment.test.ts` 用了 `test.skipIf(process.platform !== 'darwin')`，
   合并前门禁才被「不许条件跳过」的守卫拦下，多花一轮。标 `待审` 前跑一遍 `npm run test --prefix apps/web` 全量，
   证据里写总数、通过数与每个失败用例的名字。
4. **断言只针对被测代码自己写出或传入的值。** T-84 有个断言假定 `mktemp -d` 理会 `TMPDIR`，本机并不如此，reopen 一次。
5. **拉真实内核的夹具要显式指定内核。** T-84 rebase 之后，一个重启服务时没传 `binaryPath` 的夹具按新的查找顺序选中了
   已安装的旧 CLI，门禁连败。本任务还要再改一次查找顺序（A3）——改的那一轮要枚举全部 `startServer(` 与
   `new CoreSupervisor(` 的调用点。
6. **`target/debug/` 里的二进制可能是旧提交构建的。** 比较构建标识、或依赖新子命令的测试，先 `cargo build -p automed
   -p autome-cli`（门禁里 `cargo test` 在 Web 测试之前，已保证这一点）。
7. **已关闭里程碑的「最新证据」指针不要动。** T-84 有三轮把已完成里程碑的指针改指到实现证据，三次都被内核判协议
   失败。里程碑全部关闭后的实现轮（rebase、门禁修复）保持原审计证据指针。
8. **rebase 之后跑全量。** T-84 的 rebase 轮只跑了冲突相关的子集，main 带进来的测试在门禁上连败两次。本任务周期长、
   动的又是公共入口（启动、查找内核、CI 脚本），rebase 后要跑受影响包的全量。

## 10. 项目背景摘要（生成于 2026-11-14）

**这是什么。** Autome 2.0：一个驱动「设计 → 评审 / 裁决 → 实现 → 门禁 → 审计 → 合并」Loop 的本地工具（Bug 另有一条
「定位 → 复现检查 → 修复 → 门禁 → 复核」的 Loop）。内核持有全部状态（SQLite 账本 + 事件流），以 stdio 上的带长度
前缀的 JSON-RPC 服务面板；命令行 `autome` 直接链接内核库、打开同一个账本，`autome core` 就是面板用的内核。
本仓库用 Autome 自己开发自己：`.autome/config.toml` 里 `[loop] parallel = 3`；`[gate] command = "sh
scripts/ci-local.sh"`、`run = "before_merge"`（每个任务只在合并前跑一次完整门禁）、`timeout_minutes = 30`；每轮
快检 `quick` = `cargo fmt --check` + `cargo clippy -D warnings` +（缺 `apps/web/node_modules` 时 `npm ci`）+ Web
lint + typecheck，上限 10 分钟；`[merge] auto = true`（过了合并前门禁自动合并，设计批准仍要人）；
`[retry] attempts = 2`。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：
  `crates/autome-domain`（纯域：状态机、配置形状与校验、状态块解析，不做 I/O）、`crates/automed`（内核，库 +
  `automed` 二进制：调度、JSON-RPC 分发、账本、驱动锁、门禁、会话启动、Git、内置协议）、`crates/autome-cli`
  （`autome` 命令行，把 `automed` 当库链接）。
- `apps/web`——Node（≥ 22.22）服务（`server/`，TypeScript 由 Node 原生类型剥离直接运行，托管内核子进程、读写白名单、
  事件总线与 WebSocket、通知、日志跟随、`node:sqlite` 面板库）+ React Router 8 框架模式的面板（`app/`，SSR）；
  vitest、eslint、tsc。
- `apps/desktop`——Electron 43.2.0 的旧面板（本任务 C 项改成薄壳），纯 `node --test`。
- `protocol/`——内置协议 seed（不改）；`plugins/`——Claude Code / Codex / Cursor 插件；`scripts/`——`ci-local.sh`、
  `install-cli.sh`、`package-cli.sh`、`package.sh`；`.github/workflows/ci.yml`。
- 文档：`README.md`（英文）、`docs/development/{requirements,technical-design}.md`、`docs/plans/`（计划与决策记录）、
  `docs/adr/`；每个任务的文档在 `docs/<slug>/`。相关的已合并任务文档：
  `docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/`、`docs/w9-项目管理与环境-网页上添加项目-2/`、
  `docs/w6-项目页与任务卡-按待开始-进行中/`、`docs/旧内核不能推进新账本-内核带构建标识/`。
- 状态位置：`$AUTOME_HOME`（缺省 `~/.autome`）下 `state/automed.sqlite3`（账本）、`state/web.sqlite3`（面板库）、
  `web/`（`uploads/`，以及计划里的 `server.json`、`logs/server.log`）。

**测试与检查命令（在 worktree 根目录运行）。**

- Rust：`cargo test --workspace`（与门禁同款：`cargo test --workspace --all-targets -- --nocapture`）；
  CLI 集成 `cargo test -p autome-cli --test cli <用例名>`；`cargo fmt --all -- --check`；
  `cargo clippy --all-targets -- -D warnings`。
- Web：新 worktree 先 `(cd apps/web && npm ci)`；`npm run lint|typecheck|test|build --prefix apps/web`
  （单个文件：`npm run test --prefix apps/web -- test/<文件>`）。拉真实内核的测试用 `target/debug/automed` 与
  `target/debug/autome`，先 `cargo build -p automed -p autome-cli`。
- Desktop：`npm test --prefix apps/desktop`（先 `cd apps/desktop && { npm ci || npm install; }`）。
- 整套本地 CI（即宿主门禁命令）：`sh scripts/ci-local.sh`；`--package` 另跑打包（分钟级）。门禁由宿主在合并前跑
  一次；实现轮 / 审计轮跑里程碑验收命令与直接相关的测试即可。
- 输出长的命令重定向到 `.autome/output/` 只读头尾，例如
  `npm run test --prefix apps/web > .autome/output/web.txt 2>&1; tail -20 .autome/output/web.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入
  `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/面板交付批次-w10-启动与安装/protocol/`（见文首指路）。

**环境注意。** macOS（Darwin），系统 `sh` 是 bash 3.2；e2e 需要本机有 `git`；macOS 对二进制的签名与文件身份敏感
（约束 11）；launchd 起的进程只有最小 PATH（A-6 要解决的就是这个）。
