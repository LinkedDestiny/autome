# 任务：面板收尾批次——用上新的内核数据、补齐备注 / 排队顺序 / 依赖、任务详情页瘦身、测试补强、命令行小项、残留进程清理

本任务的规则见 `docs/面板收尾批次-用上新的内核数据-补齐备注/protocol/loop-protocol.md` 与
`docs/面板收尾批次-用上新的内核数据-补齐备注/protocol/session-protocol.md`。

- 设计文档：`docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注.md`
- 任务卡（用户写的，第一输入）：`docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注-card.md`
- 任务整理日期：2026-11-14
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `df71ea97`（任务卡写的 main 是 `af3cfa5a`，
  其后只有本任务的输入与协议固定两个提交）
- 本任务固定的协议版本：`protocol/v9`；账本 schema 现为 9（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`）

## 1. 需求原文

> 面板收尾批次：用上新的内核数据、补齐备注 / 排队顺序 / 依赖三个控制操作、任务详情页瘦身、测试补强、残留进程清理——一次设计、一次评审，按项分组里程碑依次实现

任务卡正文比这句话具体得多；两者不一致时以任务卡为准。§2–§5 照任务卡整理（编号是整理轮加的，供里程碑引用），
§6–§7 是整理轮从调研得出的约束与待定问题，§8 是代码现状，§9 是前几个任务留下的教训，§10 是项目背景。

## 2. 背景与目标

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 的 W1–W12 已全部合并：`autome web` 能起面板，Electron 已是薄壳，
浏览器端到端测试在门禁里。期间内核又合并了 T-53（排队顺序与依赖）、T-70（合并队列）、T-95（十一项内核补齐）、
T-97（门禁步骤沿用）。面板的页面是按更早的内核设计的，有一批「内核已经给了、面板还没用上」的地方；T-96、T-97 各留了几条
不阻塞的 Backlog。按用户 2026-11-13 确认的做法，这些合成**一个任务：一份设计、一次评审，批准后按项推进**。

任务卡记录的现状（2026-11-14，用已安装的面板对真实账本 97 个任务走查）：

- 各页都能打开；项目页 55 KB（T-96 之前 1.1 MB）；时间与用时已是本地时间与「5 分 43 秒」；「已取消（23）」折叠区在。
- 任务详情页偏大：T-96（58 个会话）一次返回 327 KB，其中 250 KB 是序列化的 loader 数据；T-97（27 个会话）192 KB。
- 任务页能做的操作是 `apps/web/app/components/task-action.ts` 的 `WRITE_OPS`：开始、批准、驳回、补充信息、合并 / 授权合并 /
  撤销授权、改建成需求、暂停、恢复、停止、取消、重跑、延长预算、表态。命令行有而面板没有：`autome note`（T-95 的 D 项）、
  `autome queue move`（T-53）、`autome after` / `task new --after`（T-53）。`task.note`、`task.move`、`task.set_after` 已在
  面板的写白名单里（`apps/web/server/gates/write-gate.ts`），只是没有页面入口。
- `autome web --help` 报「无效参数：--help」。
- T-96 的一轮审计会话留下了一个 `autome-web /tmp/aud-… serve` 和它的 `autome core` 子进程（临时 `AUTOME_HOME`，没有碰真实
  账本），任务合并、worktree 清理后还活着，跑的是已被删除的 worktree 里的二进制。此前 `logtail-live-*` 测试进程也这样残留过
  多次（T-86 修的是测试自身）。

**目标。** 六项，按 **A → B → C → D → E → F** 的顺序实现：

- **A 面板用上 T-95 的内核数据**——文档、改动类型、按任务读事件、会话结束方式、probe 拒绝原因、Onboarding 会话、全量配置、
  坏配置容错；
- **B 控制面补齐**——备注、排队顺序、依赖三个操作在面板上有入口；
- **C 任务详情页瘦身**——60 个会话的任务首次响应 ≤ 150 KB；
- **D 测试补强**——T-96、T-97 审计留下的五条不阻塞项逐条处理；
- **E 命令行与文档小项**——`autome web --help`、README 截图、驱动锁持有者；
- **F 清理任务时结束它留下的进程**。

## 3. 任务要求（任务卡「要做什么」与「对设计轮的要求」，逐条照录；设计文档的里程碑必须全部覆盖）

### 3.0 总要求

- **总-1** 一份设计文档覆盖六项。里程碑按项分组，标题以「A·」「B·」「C·」「D·」「E·」「F·」开头，**总数不超过 9 个**。
  **本任务不适用「超过 6 个里程碑就拆分」的惯例，这是有意的。**
- **总-2** 按 A → B → C → D → E → F 的顺序实现；前一项的里程碑全部审计通过后再开始下一项。
- **总-3** 内核改动只有 E-3 与 F 两处，账本 schema 至多递增一次，协议至多一个新版本；其余各项只改 `apps/web/`、
  `apps/desktop/`、`scripts/`、`README.md`、`crates/autome-cli/`。
- **总-4** 不接触真实账本：实现轮与审计轮起面板、跑 `autome` / 测试一律用临时 `AUTOME_HOME`。会话里起的服务、内核、浏览器，
  轮次结束前自己停掉。
- **总-5** 新增的 Web 测试文件按 T-97 的分组规则归入对应的 vitest project（`test-groups` 守卫会检查）；新增的门禁步骤按
  `scripts/ci/steps.sh` 的写法声明输入。
- **总-6** 会话沙箱里不能监听本地端口、不能执行 `ps` 时，如实写明哪些验收命令没跑成、由宿主门禁复验；不改弱测试，不加条件跳过。
- **总-7** 页面沿用既有 UI 规则：全中文、卡片优先低密度、1512×944 下页面本身不出现纵向滚动（内部滚动区除外）。

### 3.1 A 面板用上 T-95 的内核数据

T-95 的 F、G 两项（设计文档在 `docs/内核补齐批次-2026-11-13-4/`）给了下面这些数据，面板当时按「不改页面」留着没用。
**字段名与口径以 main 上的内核为准，设计轮对真实内核实测后定页面怎么用。**

任务详情页：

- **A-1** 文档标签页列出 Bug 文档、复盘教训、任务卡；任务合并完成、worktree 清理后仍能读文档（来源是默认分支），里程碑终态
  照常显示。
- **A-2** 改动标签页显示每个文件的改动类型（新增 / 删除 / 修改 / 改名），用内核给的删除文件数做「删除文件数异常多」的提示，
  去掉现在按「只删不增」做的近似。
- **A-3** 时间线改用按任务读事件的接口，去掉面板服务端在内存里维护全账本事件索引的做法（`apps/web/server/task-events.ts`）；
  用会话结束事件给会话条目定结束时刻与结束方式（正常 / 被停止 / 超时 / 崩溃）。

项目页与设置页：

- **A-4** 添加项目：`project.probe` 给出拒绝原因（已是项目、嵌套在项目里、包含项目）时，在填表之前就显示，不等到确认才报错。
- **A-5** Onboarding 会话已进账本：项目页 / 添加项目页用 `session.log` 显示它的日志，用事件得知结束；去掉服务端的轮询推送
  （`apps/web/server/onboarding-watch.ts`），前提是事件覆盖了它现在处理的全部情形，覆盖不了的如实保留并写明。
- **A-6** 设置页「内核实际生效的配置」显示全部字段，并标出哪些取的是默认值。
- **A-7** 一个项目的配置坏掉时：总览、侧栏照常显示其它项目；坏的那个项目带报警标记，项目页与设置页显示内核的校验原文。

### 3.2 B 控制面补齐：备注、排队顺序、依赖

面板定位是内核状态的展示与控制面，命令行能做的日常操作面板上也应能做：

- **B-1 备注**（`task.note`）：任务详情页能给进行中的任务留一句话，下一个实现 / 修复轮的简报会带上；已留的备注（含是否已送达）
  在页面上看得到。`--reopen` 的语义（T-95 D 项）由设计轮按内核实际行为决定是否提供入口。
- **B-2 排队顺序**（`task.move`）：项目页「待开始」列里排队中的任务能调整先后（移到最前、移到某张之前 / 之后）。不要求拖拽，
  按钮或菜单即可；键盘可操作。
- **B-3 依赖**（`task.set_after`）：新建任务卡时可以选「在某些任务合并之后开始」；任务详情页能修改或清空依赖；卡片上显示
  「等 T-xx 合并」。
- **B-4** 三个操作都走现有的写白名单与表单 action，失败时显示内核的原文；别处（命令行、另一个标签页）做的同类改动经事件
  实时反映。

### 3.3 C 任务详情页瘦身

- **C-1** 58 个会话的任务详情页现在 327 KB。要求：60 个会话、30 个里程碑证据的任务，详情页首次响应不超过 150 KB（用夹具造
  数据断言大小）；loader 只返回首屏要用的字段，较早的轮次、未展开的文档与改动按需加载。
- **C-2** 时间线新增条目时，展开的日志不被折叠、滚动位置不丢（W7 既有约束）继续成立；T-96 的一屏巡检与浏览器用例不回归。

### 3.4 D 测试补强

T-96、T-97 审计留下的不阻塞项，逐条处理：

- **D-1** `apps/web/e2e/live.spec.ts` 的离线恢复用例没有守住事件重放：关掉 eventbus 的 resume、drain 分发、恢复后的重验，
  用例仍然通过。改成离线前先让页面收到一条事件（`lastSeq` 非空），并给出负向对照——去掉重放时用例失败。
- **D-2** 一屏巡检 `oneScreen` 只查 `main` 及直接子元素；补一条对看板列体 `.board-column-body[data-scroll-region]` 的断言。
- **D-3** `scripts/ci/summary.mjs`：cargo 的 `Doc-tests` 段没有 `Running` 行，失败用例的目标名会沿用上一个目标。
- **D-4** `scripts/test/ci-reuse.test.mjs` 用例 ⑪ 的 `indexOf('==> Build core') < indexOf('==> Web test')` 在 Build core 缺席
  （-1）时也成立；补 `>= 0` 与输出断言。
- **D-5** ENOTEMPTY（T-97 D.3 没做完的部分）：Web 测试夹具清理临时目录时偶发 ENOTEMPTY，现在靠有界重试（5 次 / 100 ms）。
  按 T-97 设计文档 D.3 的办法做改前 / 改后各 20 次诊断，定位写入者；能证实就修根因并去掉重试，证实不了如实写明已排除的项，
  保留重试。

### 3.5 E 命令行与文档小项

- **E-1** `autome web --help`、`autome web -h`、`autome web help` 打印 `web` 子命令的用法（各子命令、`--force`、环境变量
  `AUTOME_WEB_PORT`），退出码 0；`autome help` 里的那一行不变。
- **E-2** README `What it looks like` 的截图还是旧桌面端。用 `apps/web/e2e/` 已有的 Playwright 与夹具数据加一个生成截图的脚本
  （不进门禁），产出总览、项目页、任务详情三张图并替换 README 里的旧图；脚本可重复执行。
- **E-3** `autome web status` 的「是否有另一个驱动在推进」现在只能从 tick 让路推断（T-96 BL-01；2026-11-14 实测 launchd 驱动
  刚重启时显示「最近 1 分钟没有发现其他驱动」）。内核在驱动锁里记录持有者（pid、命令、起始时间），提供只读查询；
  `autome web status` 与环境页据此给出确定答案。

### 3.6 F 清理任务时结束它留下的进程

- **F-1** 内核在清理任务 worktree 之前（合并后的清理、取消任务时的清理），找出仍在运行、且可执行文件或工作目录位于该 worktree
  之内的进程，先 SIGTERM、等待、再 SIGKILL，并记一条事件（结束了哪些进程：pid 与命令行摘要）。没有残留时不记。
- **F-2** 只认「位于该任务 worktree 之内」这一条判据，不按进程名猜；主检出、其它任务的 worktree、用户自己的进程一律不碰。
  判据在 macOS 上怎么可靠取得（`ps` / `lsof` / `proc_pidpath`），由设计轮实测后定；取不到时不杀，记一条警告。
- **F-3** 会话进行中不做这件事（实现轮、审计轮自己起的服务属于它自己）；只在 worktree 即将被删除时做。
- **F-4** 协议正文与 prompt 是否加一句「本轮起的服务、内核、浏览器，结束前自己停掉」由设计轮判断；要改就只动一个协议版本。

## 4. 验收标准（任务卡原文）

- **验-1（A）** 每一条有对应的页面测试；A-1、A-3、A-5 各有一条对真实内核的用例（Bug 任务的文档、已合并任务的文档、会话结束
  方式、Onboarding 日志）；去掉的服务端近似代码（全账本事件索引、Onboarding 轮询）确实删除，或在设计文档里写明保留的理由。
- **验-2（B）** 浏览器端到端用例各一条——留备注后下一轮简报里出现它；调整排队顺序后内核的启动顺序随之改变；设了依赖的任务在
  前置任务合并前不开始、合并后开始。命令行做的同类改动 5 秒内出现在页面上。
- **验-3（C）** 夹具断言 60 个会话的任务详情页首次响应 ≤ 150 KB；W7 的既有用例与 T-96 的一屏巡检不回归。
- **验-4（D）** 五条各自落在测试或证据里；D-1、D-4 给出负向对照；D-5 有结论。
- **验-5（E）** `autome web --help` 三种写法退出码 0 并打印用法；README 的三张截图由脚本生成并已替换；驱动锁持有者有 e2e
  （`autome run` 持锁时 `autome web status` 显示它的 pid 与命令）。
- **验-6（F）** e2e——任务的 worktree 里留一个存活进程，任务合并后的清理把它结束并记事件；主检出与另一个任务 worktree 里的
  同名进程不受影响；判据取不到时不杀并记警告。
- **验-7** 新增的测试结束后不留存活进程；没有条件跳过的测试。
- **验-8** `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`sh scripts/ci-local.sh` 全绿。

设计轮写里程碑时，§3 的每一条（A-1…F-4）都要落到某个里程碑的验收契约上，并标明属于哪一项；需要真人的部分按协议进
`## 人工验收清单`，不作里程碑的验收条件。

## 5. 范围

**在范围内（任务卡给出的目录，以设计为准）。**

| 项 | 可以改的地方 |
|---|---|
| A、B、C | `apps/web/`（页面、服务端、测试、e2e）；薄壳若受影响才动 `apps/desktop/` |
| D | `apps/web/e2e/`、`apps/web/test/`、`scripts/ci/summary.mjs`、`scripts/test/`；D-5 的根因若在夹具或面板服务端则改那里 |
| E-1 | `apps/web/server/launcher.ts`（及 `bin/`）、`crates/autome-cli/src/`（`web` 子命令） |
| E-2 | `README.md`、生成截图的脚本（`apps/web/` 或 `scripts/` 下）、被替换的图片文件（现在在 `docs/images/`） |
| E-3 | 内核 `crates/automed/src/driver_lock.rs` 及其只读查询；`apps/web/`（`status` 与环境页）；`crates/autome-cli/` |
| F | 内核 `crates/automed/`（清理步骤与事件；域类型若需要则 `crates/autome-domain/`）；F-4 采纳时仓库的 `protocol/` |

**不做（任务卡「不做什么」，逐条照录）。**

- 不做角色、路由图、技能、协议页；不做配置的网页编辑；不做远程访问与登录。
- 不改调度、合并队列、门禁时机。
- 不做 Windows / Linux 的启动器与薄壳。
- 不处理 `npm start` / `npm run dev` 不写 `server.json`（开发模式，T-96 BL-06）。
- 不改 `.autome/`。

**整理轮补充的边界（从调研得出，不扩大范围；带「整理轮理解」的是解释，设计轮可据证据修正）。**

- A、B、C、D 四项不改 `crates/automed`、`crates/autome-domain`。实测发现内核给的数据与任务卡描述有出入、或缺某个字段时，
  按内核的实际行为设计页面，缺口写进 Backlog，不为此改内核（总-3）。
- 整理轮理解：E-2「替换 README 里的旧图」必然要改图片文件本身（现在是 `docs/images/*.png`），这不在总-3 列的目录里，但属于
  E-2 的字面要求；除图片外不动 `docs/` 下别的任务目录。
- 整理轮理解：E-3、F 改了内核的 RPC / 事件之后，`README.md` 的命令说明在范围内（总-3 点了名）；
  `docs/development/technical-design.md` 是否同步由设计轮定（T-95 的做法是同步），不同步的写进 Backlog。
- 本任务的协议副本 `docs/面板收尾批次-用上新的内核数据-补齐备注/protocol/` 不改。仓库根的 `protocol/` 只在 F-4 决定加那句话时
  才改，而且只新增一个版本（v10）。
- launchd 里的 `com.autome.loop`（驱动本仓库 Loop）与 `com.autome.web`（面板自启动，若用户装了）在仓库之外，会话不得读写、
  装卸、kickstart。
- `.github/workflows/ci.yml` 不在总-3 的目录清单里；它的步骤清单被 `apps/web/test/ci-config.test.ts` 钉住。新增门禁步骤若
  需要同步它，由设计轮写明并说明理由（见 §7 总-Q3）。

## 6. 硬性约束

1. **顺序与分组（总-1、总-2）。** 里程碑表里 A 的里程碑排最前，然后 B、C、D、E、F；实现轮只推进最小编号的开放里程碑，所以
   编号顺序就是执行顺序。前面项的验收命令不得依赖后面项才有的东西（例如 A、B 的验收不能依赖 C 的按需加载接口，
   A–E 的验收不能依赖 F 的清理）。总数 ≤ 9，六项各至少一个；每个里程碑都要有协议要求的三行验收契约（验收命令、预期、负向对照）。
2. **不接触真实账本、真实面板与真实安装位置（总-4）。** 本仓库正由已安装的 `~/.local/bin/autome`（launchd `com.autome.loop`，
   `autome run`）驱动自己的 Loop，账本在 `~/.autome`；用户还可能开着已安装的面板（`~/.autome/web/server.json`）。因此：
   - 任何手工验证与测试都把 `AUTOME_HOME`（面板还要 `HOME` / `userHome`）指到临时目录，不得省略、不得指向 `~/.autome`；
   - 拉真实内核的测试**显式**指定内核（现有写法：`binaryPath` / `AUTOMED_BIN` 指向 `<检出>/target/debug/automed` 或
     `target/debug/autome` + `['core']`）；缺省查找会落到 PATH 上已安装的 `autome`，那是另一个版本的内核；
   - `server.json` 只在临时 `AUTOME_HOME` 下读写；`stop` 类操作只给本测试自己起的进程发信号；不假定 7420 空闲，不连接、不停
     用户正在用的面板；不写真实的 `~/Library/LaunchAgents/`，不对真实 launchd 执行 `launchctl`；
   - 测试不真的打开浏览器窗口、Terminal.app 或系统对话框（现有注入点：打开器、终端、对话框）。
3. **F 的杀进程逻辑只许对着临时仓库验证。** F 的实现会给进程发 SIGTERM / SIGKILL。它的任何测试与手工验证，注册进测试账本的
   项目只能是测试自己在临时目录里建的仓库；**不得**把本 worktree、主检出 `/Users/dannie/project/autome-v2` 或它的
   `.worktree/` 下任何目录注册成测试账本里的项目或任务 worktree，也不得对着真实账本跑带 F 的内核。被结束的进程必须是测试
   自己起的；验-6 里「主检出与另一个任务 worktree 里的同名进程」同样是测试自己在临时目录里造的。
4. **内核改动只有两处（总-3）。** `crates/automed`、`crates/autome-domain` 的改动只服务 E-3 与 F。账本 schema 至多 9 → 10
   （不需要改表就不要改）；`protocol/` 至多新增 v10。连带的钉子：
   - `READ_METHODS`（`crates/automed/src/dispatch.rs`，现为 `[&str; 17]`）若因 E-3 增项，`apps/web/server/gates/read-gate.ts`
     的放行 / 保留清单与 `apps/web/test/read-gate.test.ts`、`write-gate.test.ts`（解析 `dispatch.rs` 核对）要同轮同步；
     `scripts/ci/steps.sh` 的 `Contract` 步骤 grep 这个常量名，名字不能改；
   - 改协议时：`protocol/CHANGELOG.md` 末尾加 `## protocol/v10`（每条改动写 `kind`、依据、预测）；`behavioral` 类改动必须带
     eval 并把它加进内核嵌入清单；验收命令要含 `cargo test -p automed --lib protocol::`、
     `cargo run -p automed -- protocol eval protocol/`、`cargo test -p automed --test prompt_rendering`
     （golden 差异核对后用 `AUTOME_UPDATE_GOLDEN=1` 接受）；`<!-- kernel-contract: … -->` 包住的区域不能动。
5. **测试不留存活进程（总-4、验-7）。** 凡是会起服务、内核、假 CLI、Electron、浏览器的新测试，都按现有方式断言无残留：
   给本次运行一个私有临时根目录，结束后用 `apps/web/test/fixtures/process-scan.ts` 的 `leftoverProcesses(root)`
   （`ps -axo pid=,ppid=,command=` 找命令行带这个根目录的进程及其子孙）断言为空；e2e 走 `e2e/harness/teardown.ts`。
   会话自己手工起的面板、内核、浏览器，本轮结束前停掉并在证据里写一行核对结果。
6. **没有条件跳过（总-6、验-7）。** `apps/web/test/core-sidecar.test.ts` 的守卫扫 `apps/web/test/*.ts` 与 `apps/web/e2e/`，
   出现 `test|it|describe.skip/skipIf/runIf/fixme/todo(` 就失败。平台前提用断言表达（`expect(process.platform).toBe('darwin')`），
   不用跳过。Rust 侧同理，不加 `#[ignore]` 与按环境变量提前返回。
7. **沙箱拦住时如实记录（总-6）。** 实现轮的沙箱可能拒绝 `listen(127.0.0.1)` 与 `ps`（T-96 的 25 个实现轮里 20 个如此，
   T-97 的 M-06 四轮如此）。遇到时：证据文件逐条列出因此没跑成的验收用例名与交给审计轮 / 宿主门禁执行的完整命令；
   不改弱测试、不加跳过。`npm run lint`、`npm run typecheck`、`cargo fmt`、`cargo clippy` 是静态检查，不受影响，当场跑。
8. **写白名单与读白名单不放松。** B 的三个操作走 `gateway.write` 与现有表单 action，参数校验沿用 `write-gate.ts` 已有规则
   （`task.note` 的 `text` ≤ 8000 字符、`reopen` 可选布尔；`task.set_after` / `task.create` 的 `after` 为 `T-N` 列表且有上限；
   `task.move` 的 `before` / `after` / `top` 恰好一个）。`events.since` 继续不对浏览器放行（`WITHHELD_READ_METHODS`，
   `read-gate.test.ts` 有「带 `subject` 也被拒」的用例）——A-3 的按任务读事件由面板服务端自己调内核。C 新增的按需加载接口
   同样只绑 `127.0.0.1`、过 Host / Origin 检查，不让浏览器给出任意路径（文档读取沿用 `pickDocument` 的「文档名不能是路径」）。
9. **UI 硬规则（总-7）。** 全中文，专有名词保留原文；卡片优先、低密度；1512×944 下页面本身不出现纵向滚动（看板列、时间线、
   文档、日志、设置卡片体可以内部滚动，内部滚动区带 `data-scroll-region`）；视觉沿用 `apps/web/app/styles/tokens.css` 的
   设计 token，不写内联样式（`styles.test.ts` 会拦）。新增的可交互元素键盘可达（B-2 点了名）。
10. **实时与既有浏览器行为不回归（C-2、B-4、验-2）。** 页面不手动刷新；事件到页面更新 ≤ 5 秒（生产 tick 间隔 3000 ms，
    重验有 1000 ms 合并窗口）；时间线新增条目时已展开的日志不折叠、滚动位置不丢；SSR 与水合一致（e2e 的 `consoleErrors`
    夹具对任何控制台错误判失败）。
11. **门禁步骤与测试分组的钉子（总-5）。** 新的 Web 测试文件必须出现在 `apps/web/test-groups.ts` 的恰好一个分组里
    （`unit` / `kernel` / `exclusive`；带内核、端口、进程、构建标记的不得进 `unit`，由 `test/test-groups.test.ts` 检查）。
    `scripts/ci/steps.sh` 里每个 `step` 声明输入目录；`apps/web/test/ci-config.test.ts` 钉住 Web 各步骤的写法与 `ci.yml` 的
    job（含 `e2e` job 限时 15 分钟）；`scripts/test/ci-*.test.mjs` 钉住步骤沿用的行为。改这些文件的里程碑同轮同步对应测试。
12. **时间预算。** 浏览器步骤 `npm run e2e` 的 `globalTimeout` 是 170 秒（`apps/web/playwright.config.ts`），T-96 审计实测一次
    149.80 秒、一次 170.64 秒（超限）——B、C、D 新增浏览器用例之前余量已经很小（见 §7 总-Q2）。宿主门禁整套上限 30 分钟，
    每轮快检预算 120 秒（`scripts/ci-quick.sh`，上限 10 分钟）。
13. **依赖变了，`node_modules` 要跟上。** worktree 里现在没有 `apps/web/node_modules`、`apps/desktop/node_modules`、`target/`。
    安装由 `scripts/ci/install-node.sh` 负责；改了 `package.json` 必须同轮提交 `package-lock.json`。本任务预计不需要新依赖
    （Playwright 1.64.1 已在），要加须说明理由。
14. **脚本的既有约定。** `scripts/*.sh`、`scripts/ci/*.sh` 为 `/bin/sh`（macOS 的 bash 3.2）而写：不用数组、`[[`、`local`；
    中文紧跟变量时写 `${var}`。断言子进程文本输出的测试，spawn 时显式设定 `NO_COLOR` / `FORCE_COLOR`（T-97 教训）。
15. **文案语言沿用现状。** 界面、CLI 输出、报错与日志用中文；代码注释与 `README.md` 用英文；设计文档、计划与
    `docs/development/technical-design.md` 用中文。
16. **设计文档的体量。** 六项合在一份文档里，协议的警告线是 ≤ 80KB、单轮增长 ≤ 10KB（T-96 的设计文档到了 99.8KB）；
    证据写进 `evidence/`，`## 里程碑` 一节只放那一张表，对比表与说明放别处。
17. **会话边界（AGENTS.md）。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改 `.autome/`；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成本轮后结束会话，不自行启动下一个会话。

## 7. 待设计轮回答的问题

任务卡留白、或与代码现状有出入的地方。只列问题与相关事实，不预设答案。设计文档「方案」一节须逐条给出结论与依据；
没有依据支持改动任务卡字面的，按任务卡字面做。

### 总

- **总-Q1 九个里程碑怎样分给六项。** 六项各至少一个，余量三个。A 有七条、跨任务详情 / 项目页 / 设置页 / 总览；B 有三个操作、
  三条浏览器用例；D 的五条互不相干。每个里程碑要能在合理轮次内完成，并且有一部分验收不依赖监听端口（§9 第 1 条）。
- **总-Q2 浏览器用例的时间预算。** 现有 e2e 四个文件 10 处 `test(`，其中一屏巡检按页面参数化（T-96 审计的计数是 20 条），
  整步上限 170 秒且已贴着上限。本任务至少新增：验-2 的三条 + 「命令行改动 5 秒内出现」、D-1 改造、D-2 一条、C-2 的不回归。要定：新增用例放进
  现有 `Web e2e` 步骤还是另起步骤、上限调不调、怎样不让单次波动把审计打回（T-96 L-03）。
- **总-Q3 `ci.yml` 与门禁步骤。** 新增步骤（若有）在 `steps.sh` 声明哪些输入；`ci.yml` 要不要同步；E-2 的截图脚本「不进门禁」
  ——它放在 `apps/web/` 下会不会被 eslint / tsc / vitest 的 include 收进去、放在 `scripts/` 下会不会让 `Scripts` 步骤或
  「未归属路径」检查（`ci-local.sh --check-claims`）有反应。
- **总-Q4 设计轮的实测。** 任务卡要求 A 的页面用法、F 的判据都「实测后定」。实测需要 `(cd apps/web && npm ci)`、
  `cargo build -p automed -p autome-cli`、临时 `AUTOME_HOME`、显式的内核路径，并能监听端口与执行 `ps`；做不到时如实写明没测到
  哪些，不凭文档下结论。实测产物写进 `.autome/output/`。

### A

- **A-Q1 文档清单（A-1）。** 内核事实：`task.get` 的 `documents[]` 每条带 `label`、`source`（`worktree` | `default_branch`）与
  `absolute`；顺序是任务卡、设计文档 / Bug 文档、任务文件、评审、裁决、审计、运行记录、复盘教训、证据；worktree 不在时读默认
  分支上 `<文档根>/<slug>/` 的文件（只列主检出磁盘上确实存在的），`status_block` 从默认分支解析；取消且未合并的任务是空清单。
  面板事实：`task-docs-model.ts` 的 `docsView` 固定放一个「任务卡」首标签（显示 `brief ?? request`），并在没有文档时显示
  「任务已完成，工作目录已清理，文档暂不可读。」「内核尚未提供 Bug 文档，当前只显示任务卡。」；读文档走
  `server/task-docs.ts`（按 `absolute` 读盘，上限 200 KB）。要定：内核现在自己列任务卡，固定首标签怎样与之合并；两句过时的
  提示去留；`source` 要不要在页面上标出来；默认分支来源的文档在主检出被改动 / 文件不存在时显示什么；取消任务的呈现；
  验-1 的两条真实内核用例（Bug 任务的文档、已合并任务的文档）怎样造数据。
- **A-Q2 改动类型（A-2）。** 内核事实：`task.changes` 的 `files[]` 每项加 `status`（`added|deleted|modified|renamed`）、改名带
  `from`，顶层 `deleted_count`（工作区项目在 `repos[]` 各有一份并汇总）；分支已删且有 `merge_commit` 时用合并提交给同样结构并带
  `merged: true`。面板事实：`task-changes-model.ts` 的 `TaskChanges` 类型没有这些字段；`changesView` 用
  `added === 0 && deleted > 0` 数「疑似删除」，提示条件是 `suspected >= 5 || (suspected >= 2 && suspected * 2 >= files.length)`；
  `loadTaskDetail` 只在合并段（`inMergeSegment`）才请求 `task.changes`。要定：「异常多」用 `deleted_count` 的什么阈值；
  改名的呈现；已合并任务要不要显示改动（内核现在给得出）；工作区项目的呈现。
- **A-Q3 按任务读事件（A-3）。** 内核事实：`events.since {after_seq, limit, subject?}`，带 `subject` 时只回该 subject 的事件，
  `latest_seq` 仍是全账本最新；`session.*` 事件的 subject 是任务 id。面板事实：`server/task-events.ts`（48 行）拉全账本事件、
  按 subject 分组缓存（每组上限 5000，过滤 `task.head` / `integrity_warning` / `task.lessons`，账本回退时重置），经
  `services.taskEvents.forTask(id)` 给 `loadTaskDetail`；`main.ts` 创建它、`load-context.ts` 声明类型、
  `test/task-events.test.ts` 测它。要定：删除后 `forTask` 的等价物放在哪、分页（`limit`）与截断提示（现有 `truncated`）怎样
  保留、每次重验都重读还是按 `after_seq` 增量、与 C 的「较早轮次按需加载」怎样衔接（A 的验收不得依赖 C）。
- **A-Q4 会话结束方式（A-3）。** 内核事实：`session.ended` 载荷 `{session_id, kind, round, end: exited|stopped|timed_out|crashed,
  exit_code, duration_ms, cost_usd}`，同一会话只写一次；T-95 之前结束的会话没有这条事件。面板事实：时间线
  `task-timeline-model.ts` 现在用会话行的 `ended_at`、`lifecycle.state === 'killed'`（「已被停止」）与窗口内事件拼摘要。
  要定：四种结束方式的中文、与会话行数据不一致或缺事件（旧会话）时以谁为准、验-1 的真实内核用例覆盖哪几种结束方式
  （超时与崩溃怎样在测试里造出来）。
- **A-Q5 probe 拒绝原因（A-4）。** 内核事实：`project.probe` 回包带 `rejection: {kind: not_a_directory|already_project|
  nested_in_project|contains_project, message, project_id?} | null`，`message` 与 `project.add` 的原文相同。面板事实：
  `routes/projects.new.tsx`（85 行）按 `checked.probe` 决定是否显示确认表单。要定：四种 `kind` 各显示什么、
  `already_project` 要不要给出跳到那个项目的链接、未知 `kind` 回退到 `message`。
- **A-Q6 Onboarding（A-5）。** 内核事实：Onboarding 会话写 sessions 行（`task_id` 为空），`session.started` / `session.ended`
  的 subject 是**项目 id**；`project.get` 带 `onboarding_session: {id, lifecycle, started_at, ended_at} | null`；`session.log`
  按会话 id 取得到。面板事实：`server/onboarding-watch.ts`（45 行）读项目目录下
  `.autome/output/sessions/onboarding/<id>.{prompt,pid,exit}`，给出 `none | running | ended（带 exit_code）| interrupted`
  （pid 已死且没有退出标记、且 prompt 文件超过 10 秒）以及读盘出错的 `error`；每个 tick 检查一次，结束时经 WebSocket 推
  `onboarding.ended`（`admin-live.ts` 据此重验）；`settings-data.ts` 调 `admin.onboarding.status / watch`。要定并实测：
  事件总线会不会把 subject 为项目 id 的事件送到浏览器、路由的 `shouldRevalidate` 认不认；`interrupted`（会话进程消失而没被
  收割）在内核里对应什么、多久能得知；命令行起的 Onboarding（`autome project onboard`）同样覆盖；哪些情形事件覆盖不了——
  覆盖不了的保留并写明（任务卡原话）。日志显示复用哪个组件（任务详情的 `SessionLog`）以及「项目页 / 添加项目页」各放哪里。
- **A-Q7 全量配置（A-6）。** 内核事实：`config.get` 回包有 `resolved`（全部生效字段）、`defaults`（取内置缺省的键名列表）、
  `global`、`scope`、`violations`。面板事实：`config-view.ts` 的 `configRows(resolved)` 把对象拍平，标签取自
  `admin-labels.ts` 的 `CONFIG_LABELS`（没有就显示键名），角色单独成行；设置页的配置卡片是内部滚动区。要定：`defaults` 里键名
  的写法与拍平后的 `key` 怎样对上（实测）、默认值怎样标、全部字段的中文标签补到什么程度（未知键回退键名）。
- **A-Q8 坏配置（A-7）。** 内核事实：`dashboard.get` 的 `projects[]` 每项 `{project, config_error}`；`project.get` 在配置坏时
  回 `config: null` 与 `config_error`；`project.list` 不读配置；T-95 把「`task.get` 对坏配置项目失败」留在了 Backlog。
  面板事实：`board-data.ts` 的 `loadBoard` 对每张可见卡调 `task.get`，**任何一张失败整页就只剩一条错误**；`settings-data.ts`
  在 `config.get` 失败时只显示它的 `error`；`shell-data.ts` / 总览没有读 `config_error`。要定并实测：坏配置项目下
  `task.get`、`config.get`、`config.validate` 各回什么；项目页在 `task.get` 失败时怎样仍显示卡片与校验原文（不改内核）；
  报警标记放在总览卡片、侧栏、项目页标题的哪里；与现有项目级报警（`project.alert.dismiss`）怎样区分。

### B

- **B-Q1 备注的入口与可见范围（B-1）。** 内核事实（`dispatch.rs` 的 `task_note`）：空白与超过 8000 字符拒绝；已完成 / 已取消
  拒绝（「任务已经完成，备注不会再被任何一轮读到；有新问题请新建任务」）；其余未结束状态都收；`AwaitMerge` 不带 `reopen`
  拒绝并提示加 `--reopen`（带上则先记备注再送回实现 / 修复轮，Bug 预算用完时拒绝并提示先 `autome extend`）；
  `Merging / MainCheck / Cleanup` 照收但回包带 `warning`、事件带 `late: true`。`task.get` 顶层
  `notes: [{n, text, at, delivered: null | {session_id, role, round, at}}]`；事件 `task.note_added`、`task.note_delivered`。
  收件轮：需求的实现轮、Bug 的定位轮与修复轮。要定：任务卡说「进行中的任务」——草稿、排队、失败、已停止时入口显不显示；
  等你合并时提供不提供「送回实现轮」的入口（任务卡交给设计轮按内核实际行为决定）；`warning` 怎样显示；备注列表放在详情页
  哪里、已送达怎样标（送达到哪一轮）；它加进 `WRITE_OPS` / `OP_RULES` 还是独立表单。
- **B-Q2 「下一轮简报里出现它」怎样断言（验-2）。** 内核把未送达备注渲染成收件轮启动内容里的「操作者备注」段
  （`handoff::sections`），`session.started` 带 `notes: [n…]`。要实测：它落在 `docs/<slug>/brief/<角色>-<轮次>.md` 还是只在
  prompt 里、e2e 的假 CLI（`apps/web/test/fixtures/w7-flow.sh`，按 `.autome/fake/<slug>/<n>.sh` 逐步执行）怎样把它取出来断言；
  页面上「已送达」的变化同一条用例里一并看。
- **B-Q3 排队顺序（B-2）。** 内核事实：`task.move {task_id, before? | after? | top?}` 恰好一个；只有排队或待开始（草稿）的任务
  可以移动；不能以自己为参照，参照任务不存在或不在同一队列时拒绝；回包带 `queue_position` 与 `order`，事件 `task.moved`；`project.get` 的任务行与
  `task.get` 都带 `queue_position`。面板事实：`board-data.ts` 的 `boardColumns` 把非「已完成」的列按**任务编号倒序**排，
  「待开始」列（`todo`）同时装草稿与排队中的任务——现在这一列并不按队列顺序显示。要定：列内顺序改不改、草稿与排队中怎样区分、
  「移到某张之前 / 之后」的选择方式（按钮、菜单）与键盘操作、项目页 action（现在只认 `start` 与 `dismiss`）怎样加；
  验-2「启动顺序随之改变」的夹具怎样让任务真的排队（并行数、会话占位）。
- **B-Q4 依赖（B-3）。** 内核事实：`task.create {after: [T-N…]}`、`task.set_after {task_id, after}`（空列表即清空），事件
  `task.after_set`；可改时机（`autome-domain/src/deps.rs` 的 `check_editable`）：草稿 / 排队随时可改；需求任务开始之后只在设计批准
  之前可改（节点为整理 / 设计 / 评审 / 裁决 / 等你批准，且还没有过实现轮），之后回「依赖只能在设计批准之前修改」；Bug 任务
  开始之后不可改；已完成 / 已取消不可改；`check_after` 还拒绝跨项目、自依赖、成环等；任务 JSON 带
  `after` 与 `blocked_by: [{id, state, reason}]`（`reason` 在前置任务已取消 / 失败时非空，这时它永远等不到）。面板事实：
  `app/` 里没有任何地方读 `after` / `blocked_by`（依赖）；新建任务卡页是 `routes/projects.$id.tasks.new.tsx`（73 行）。
  要定：新建页的候选任务取哪些（同项目、未结束）、怎样选；详情页的修改入口在不可改时显示什么；卡片上「等 T-xx 合并」的写法
  （多个前置、前置已取消 / 失败时）；验-2 第三条用例的最短链路。
- **B-Q5 实时反映（B-4、验-2）。** 事实：任务页对 `task.` 前缀的事件重验（`TASK_PAGE_EVENT_PREFIXES`），项目页对
  `task. / project. / …` 重验，重验有 1000 ms 合并窗口。要定：三类事件（`task.note_added`、`task.note_delivered`、
  `task.moved`、`task.after_set`）各自触发哪些路由重验、被依赖方合并导致的「等 T-xx 合并」消失靠哪个事件；
  「命令行做的同类改动 5 秒内出现」各用哪条用例守。

### C

- **C-Q1 150 KB 量的是什么、砍哪里。** 事实：`loadTaskDetail` 现在返回整份 `task.get`（全部 `sessions`、`documents`、
  `decisions`、`brief`、`status_block`…）、整份 `config.get`、该任务的全部事件（`history`）、有重试时整份 `dashboard.get`、
  合并段的 `task.changes`、当前文档的解析结果（Markdown 节点树）。任务卡量的是「详情页首次响应」（SSR 的 HTML，
  327 KB 中 250 KB 是序列化的 loader 数据）。要定：重验时的数据请求算不算、要不要同样设上限；首屏到底要哪些字段（逐个组件
  列：标题、阶段条、提示、操作、时间线、右侧标签页）；「较早的轮次」的界线（按轮次数、按会话数）、按需加载的接口形态与
  URL 状态（刷新后保持）；文档与改动标签页未选中时不取；项目页当初怎样做到 ≤ 200 KB 的做法（`test/board-payload.test.ts`）
  能不能照搬。
- **C-Q2 夹具。** 「60 个会话、30 个里程碑证据」用替身内核回包还是真实内核 + 假 CLI 造；断言量的是哪个字节数
  （未压缩的响应体）；会话带多长的字段才算贴近真实（真实的 58 个会话是 327 KB）。
- **C-Q3 不回归（C-2）。** 事实：W7 H-03（展开日志、滚到中间、任务被推进后日志仍展开、位置不跳）的浏览器用例在
  `e2e/manual-items.spec.ts`；一屏巡检在 `e2e/layout.spec.ts` 与 `harness/browser.ts` 的 `oneScreen`。时间线改成部分加载后，
  「加载更早的轮次」与新增条目同时发生时这两条怎样继续成立；A-3 先改时间线数据来源、C 再改加载方式，两次改动的测试怎样
  不互相推翻。

### D

- **D-Q1 重放用例（D-1）。** 事实：`app/lib/live-client.ts` 记 `lastSeq`，重连后发 `{type: 'resume', after: lastSeq}`；
  `server/eventbus.ts` 有 `resume()` 与 `drain()`；恢复后 `live.tsx` 还会整体重验。T-96 审计 #18 实测：关掉 resume、drain
  分发、恢复后的 `revalidateAll`、重连的 `onReload`，现有 D-4 用例（杀内核 → 离线横幅 → 命令行建草稿卡 → 恢复后卡片出现）
  **仍然通过**。任务卡的改法是离线前先让页面收到一条事件。要定：改造后的用例到底守哪一条恢复路径、负向对照具体去掉哪段
  代码时变红；「恢复后的重验」本身就能让卡片出现时，用什么可观测量区分「重放了事件」与「整页重验了」。
- **D-Q2 列体断言（D-2）。** 事实：`oneScreen` 检查 `documentElement`、`[data-testid=shell-main]` 及其直接子元素，排除
  `[data-scroll-region]` 之内的元素；看板列体是 `routes/projects.$id.tsx:59` 的 `.board-column-body[data-scroll-region]`；
  e2e 夹具造了 30 张草稿卡让列体出现内部滚动。要定：断言的内容（存在、带标记、确实是它在滚而页面没滚）与负向对照。
- **D-Q3 Doc-tests（D-3）。** 事实：`summary.mjs` 的 `extract('cargo', …)` 只在 `Running <目标> (<路径>)` 行更新 `target`；
  cargo 的文档测试段以 `Doc-tests <crate>` 开头。测试在 `scripts/test/ci-summary.test.mjs`（夹具在 `scripts/test/fixtures/`）。
- **D-Q4 用例 ⑪（D-4）。** 事实：`ci-reuse.test.mjs` 第 106 行起；夹具里 `prep 'Build core'` 打印 `BUILD`、`step 'Web test'`
  打印 `WEB`。T-97 审计留过一个判别检查的写法（实现正确时绿、prep 永不执行时红）。负向对照按验-4 必须给。
- **D-Q5 ENOTEMPTY（D-5）。** 事实：`maxRetries: 5, retryDelay: 100` 现在出现在 10 个测试文件与 1 个夹具里
  （`board-detail-real`、`task-new-real`、`notifier-backfill-real`、`install-alignment`、`board-live`、`shell-live`、
  `eventbus-live`、`gateway`、`notifier-real`、`w9-cleanup`（逐字断言这组参数）、`fixtures/w9-replies.ts`）。T-97 D.3 的办法：
  在 `eventbus-live` 的 afterAll 里、`rm` 之前记录命令行含临时 home 的进程（`ps`）、`lsof +D <home>`、残留条目名（只写进
  `.autome/output/`），改前夹具并行跑 20 次、改后并行跑 20 次。T-97 的首要假设是 `test/fixtures/automed-pidfile.sh` 丢了
  `"$@"`、`--version` 探测因此起出第二个完整内核——**这个夹具已经修好**（现在传 `"$@"`，`--version` 不写 pid 文件），所以
  「改前 / 改后」各指什么需要重新定义。T-97 审计 #8 的观测是 8 次全量里 2 次红；按那个失败率，要有把握至少要连跑 11 次
  （T-97 L-04）。要定：改前 / 改后的定义、在哪种并发下复现、诊断需要 `ps` / `lsof`（实现轮沙箱可能不允许，由谁跑）、
  「证实」的判据、去掉重试时 `w9-cleanup.test.ts` 等断言怎样跟着改。

### E

- **E-Q1 帮助由谁打印（E-1）。** 事实：`crates/autome-cli/src/web.rs` 找 Node（`AUTOME_NODE` 或 PATH）与面板目录
  （`AUTOME_WEB_ROOT`、`<exe>/web`、`<exe>/../lib/autome/web`、`<exe>/../../apps/web`），然后 `exec node bin/autome-web.js <参数>`；
  找不到 Node 或面板时打印安装提示、退出码 1。`apps/web/server/launcher.ts` 的 `launch()`：第一个参数以 `-` 开头时当作 `open`
  的参数，所以 `--help` / `-h` 落进「无效参数」，`help` 落进「未知面板命令」；子命令有 `open`（缺省）、`start`、`status`、
  `stop`、`service install|uninstall`，以及内部用的 `serve`；选项 `--port`、`--json`、`--force`、`--wait <秒>`、
  `--instance <id>`；环境变量 `AUTOME_WEB_PORT`、`AUTOME_WEB_DEFAULT_PORT`、`AUTOME_HOME`。`autome help` 里 `web` 那一行被
  `main.rs` 的单测逐字断言（任务卡要求不变）。要定：只装了命令行（没有 Node 或没有面板）时 `autome web --help` 也要退出码 0
  吗——这决定帮助放在 Rust 侧还是 Node 侧；用法文本列哪些（任务卡点名：各子命令、`--force`、`AUTOME_WEB_PORT`）；
  `autome web stop --help` 之类写在子命令之后的怎样处理。
- **E-Q2 截图脚本（E-2）。** 事实：README 第 10–48 行的 `What it looks like` 引用 `docs/images/` 下六张图（`dashboard`、
  `project`、`task`、`routing`、`skills`、`environment`），开头一段说截图由 `apps/desktop/tools/screenshots.js` 生成——
  **这个文件已不存在**。任务卡要三张（总览、项目页、任务详情）。面板没有路由图与技能页（「不做」），有环境页。e2e 夹具由
  `e2e/harness/{setup,serve,fixture}.ts` 起真实内核 + 假 CLI，数据里有任务编号、时间、临时路径。要定：另外三张旧图与对应
  段落的去留；脚本放哪、怎样运行（需要先 build、需要 Playwright 浏览器）、输出到哪；「可重复执行」的含义（每次都能跑成，
  还是产出逐字节稳定）；截图里的示例数据怎样不显得是测试垃圾（现有夹具有 30 张 `draft-N` 与 30 张已取消的卡）；截图是真人
  要看的东西——验收命令只能证明「脚本跑成、三个文件生成、README 引用的是它们」，观感进人工验收清单。
- **E-Q3 驱动锁持有者（E-3）。** 事实：`driver_lock.rs`（198 行）在 `<账本>.driver.lock` 上用 `flock`，**按每次 tick / 每条
  写命令取放**，不是进程存续期间一直持有，文件里什么都不写；`scheduler::tick` 拿不到锁时回 `skipped: true`；命令行的写命令
  等锁超过 2 秒会打印「等待驱动释放状态库…」。面板事实：`server/core/supervisor.ts` 的 `otherDriver()` 数最近 60 秒被让路的 tick，
  `/api/instance` 以 `other_driver: {skipped_last_minute, last_skipped_at}` 给出，`launcher.ts` 的 `status()` 据此打印；
  环境页（`routes/env.tsx`，62 行）现在没有任何关于驱动的内容。`autome doctor --json` 带 `driver_lock` 路径。要定：既然锁按
  操作取放，「持有者」记的是什么（当前这一刻的持有者、最近一次的持有者，还是「正在循环驱动的进程」的登记）；记在哪
  （锁文件内容、旁边的文件、账本——后者要动 schema）；持有者崩溃后陈旧记录怎样识别（pid 复用、起始时间）；面板自己的内核
  也是驱动，`status` 怎样区分「自己」与「另一个」；只读查询是新方法还是并进现有方法（`env.get` 等，见约束 4 的 `READ_METHODS`
  钉子）；验-5「`autome run` 持锁时 `autome web status` 显示它的 pid 与命令」在锁只持有一个 tick 的时长下怎样做成确定的用例；
  旧内核（没有记录）与新面板、新内核与旧面板并存时显示什么。

### F

- **F-Q1 判据怎样取得、覆盖到哪（F-1、F-2）。** 事实：T-96 留下的那个 `autome-web /tmp/aud-… serve` 是 **Node 进程**
  （`launcher.ts` 把 `process.title` 设成了 `autome-web <home> <动作>`）——它的可执行文件是系统的 `node`，不在 worktree 里；
  它是否满足「可执行文件或工作目录位于 worktree 之内」取决于它的工作目录。它的子进程 `autome core` 的可执行文件是 worktree
  里的 `target/debug/autome`。任务卡明确「只认位于该任务 worktree 之内这一条判据，不按进程名猜」。要实测并定：macOS 上取
  可执行文件路径与工作目录各用什么（`proc_pidpath`、`proc_pidinfo` 的 vnode 路径、`lsof -d cwd`、`ps`；`libc` 已是 workspace
  依赖）；别的用户的进程、已退出的僵尸、权限不足时各返回什么（「取不到时不杀，记一条警告」）；路径比较怎样做才不误伤——
  任务 worktree 是 `<主检出>/.worktree/<slug>`，**在主检出目录之内**，而主检出与其它任务的 worktree 不能碰；`/tmp` 与
  `/private/tmp` 这类符号链接、worktree 已被部分删除时的路径；按任务卡的判据 T-96 那个 Node 进程接不接得住，接不住的如实
  写明（不得为此改成按进程名或命令行匹配，除非设计给出仍属于「位于 worktree 之内」的依据并经评审）。
- **F-Q2 挂在哪些清理点。** 事实（`crates/automed/src/scheduler.rs`，行号为约数）：合并后的清理是 `cleanup(layout)`
  （约 5306 行，`Node::Cleanup` 的内核步骤，不强制删除，失败按非致命处理）；取消走 `cancel_task` → `discard_checkout`
  （约 5372 行，先归档文档再强制删除），`task.convert`（`dispatch.rs` 约 2631 行）也调 `discard_checkout`；工作区项目一个任务
  有多个检出（`layout.repos` 各一个 worktree，外加 `layout.cwd` 目录）。另有两处删 worktree 但不是「清理任务」：定位轮结束时
  删掉没用到的成员检出（约 1870 行）、合并验证用的临时检出 `verify_dir`（约 4820、4997 行）。取消之前内核已会停掉运行中的
  会话与门禁（`stop_running_session`、`gate::stop_all`；会话按进程组 `launcher::terminate_group` 先 TERM 后 KILL）。
  要定：哪几处算任务卡说的两种清理、另外两处碰不碰（任务卡只说合并后与取消时）；SIGTERM 之后等多久；清理步骤在持有驱动锁的
  tick 里同步执行，等待会不会拖住别的任务；进程结束不了时清理是继续还是失败。
- **F-Q3 事件与可见性。** 要定：事件名与载荷（任务卡：pid 与命令行摘要；没有残留时不记）、警告用什么事件、subject 是任务 id；
  `autome run` / `autome status` / 面板时间线要不要显示（面板显示它属于 `apps/web/`，F 排在最后，可以同项里做）；命令行摘要
  截多长、会不会把敏感参数写进账本。
- **F-Q4 验-6 的 e2e 怎样写。** 事实：内核的端到端测试在 `crates/automed/tests/end_to_end.rs`，CLI 的在
  `crates/autome-cli/tests/cli.rs`（真实二进制 + 沙箱 `AUTOME_HOME`）。要定：在任务 worktree 里留一个存活进程的做法（可执行
  文件在里面、工作目录在里面各一种）；「主检出与另一个任务 worktree 里的同名进程不受影响」的断言；「判据取不到」怎样在测试里
  造出来（注入点）；测试结束后这些进程由谁收（约束 5）；实现轮沙箱若不许 `ps` / 进程查询，哪些用例交审计轮跑。
- **F-Q5 协议要不要加那句话（F-4）。** 事实：仓库 `protocol/` 现为 v9；改协议的流程与钉子见约束 4；本任务自己用的是固定在
  任务目录里的 v9 副本，改了仓库的 `protocol/` 也只对之后新建的任务生效。任务卡的总-4 已经把这句话作为对本任务会话的要求。
  要定：加不加；加在 `loop-protocol.md` / `session-protocol.md` 还是哪几个角色的 prompt；属于 `behavioral` 还是别的 `kind`
  （前者要带改前红、改后绿的 eval）。

## 8. 代码现状（整理轮调研，基线 `df71ea97`；行号为约数，设计轮引用前自行复核）

**内核 `crates/automed`（A–D 只读；E-3、F 要改）。** `dispatch.rs`（7947 行）：`READ_METHODS: [&str; 17]`（约 3383 行：
`project.list / project.probe / project.get / project.onboarding.artefacts / task.get / task.changes / session.log /
dashboard.get / config.get / config.validate / env.get / env.install_recipe / skills.list / events.since / env.detect /
protocol.get / protocol.versions`）；`task_json_with_config`（约 690 行，任务 JSON 的 `after`、`blocked_by`）；
`project_get`（约 620–675 行，`queue_position`、`onboarding_session`、`config_error`）；`project_probe` 的 `rejection`
（约 1028 行）；`config_get` 的 `defaults`（约 1275 行）；`task_set_after`（约 1838 行）、`task_move`（约 1870 行）、
`task_note`（约 2040 行）；`task_get`（约 2087 行，`notes`、`documents` 的 `source`）；`task_changes`（约 2880–2955 行）；
`dashboard_get`（约 3019 行，逐项目 `config_error`）；`events_since` 的 `subject` 参数（测试在约 6990 行）。
`scheduler.rs`（13047 行）：`session.ended` 的写入（约 1589–1604 行）；清理与取消见 §7 F-Q2。`driver_lock.rs`（198 行）见
§7 E-Q3。`launcher.rs`：`terminate_group`（约 1290 行）。`store.rs`：`SCHEMA_VERSION = 9`，迁移按 `user_version` 逐块执行。
`gate.rs`：宿主门禁是独立进程组里的后台作业。域 crate `autome-domain/src/deps.rs`（416 行）：依赖校验与队列重排。

**命令行 `crates/autome-cli`。** `src/main.rs`（1663 行）：`USAGE`（44 行起；`web` 一行在「其他」组，被 1662 行的单测断言）；
`main()` 在 99 行把 `web` 分发给 `web::run`（不打开账本）；`note`、`after`、`queue move`、`task new --after` 的实现分别在
约 211、495、505、595 行。`src/web.rs`（51 行）见 §7 E-Q1。`tests/cli.rs`：`web_*` 六个用例（约 2199–2280 行，用假的
`autome-web.js` 与 `AUTOME_NODE`）。

**面板服务端 `apps/web/server`。** `main.ts`：`startServer({ mode, port, home, userHome, binaryPath, binaryArgs, kernelEnv,
tickIntervalMs, … })`，创建 `taskEvents`、`taskDocs`、`admin` 并注入路由上下文；`/api/instance`（`http.ts:41`）。
`launcher.ts`（164 行）、`instance.ts`（101 行）、`service.ts`（53 行，launchd 条目 `com.autome.web`）。`gateway.ts`、
`gates/read-gate.ts`（96 行）、`gates/write-gate.ts`（337 行，`ALLOWED_WRITE_OPS` 含 `task.note / task.move / task.set_after`，
`PROSE_FIELDS`、`MAX_PROSE_LENGTH = 8000`、`MAX_AFTER_IDS = 8`）。`task-events.ts`（48 行）、`task-docs.ts`（40 行，
`DOC_LIMIT_BYTES = 200 KB`）、`onboarding-watch.ts`（45 行）、`admin.ts`。事件：`eventbus.ts`、`eventbus-ws.ts`、
`eventbus-protocol.ts`；`ticker.ts`（生产 3000 ms）；`core/supervisor.ts`（`otherDriver()`）。

**面板前端 `apps/web/app`。** 路由七条（`routes.ts`）：`/`、`/env`、`/projects/new`、`/projects/:id`、
`/projects/:id/settings`、`/projects/:id/tasks/new`、`/tasks/:id`。任务详情：`routes/tasks.$id.tsx`（51 行，loader =
`loadTaskDetail`，action = `handleTaskAction`，三个标签页「文档 / 改动与合并 / 待你决定」）、
`components/task-detail-data.ts`（60 行）、`task-action.ts`（56 行，`WRITE_OPS` 十四项）、`task-ops.ts`（47 行，`OP_RULES`）、
`task-timeline-model.ts`（151 行）与 `task-timeline.tsx`、`task-docs-model.ts`（11 行）、`task-changes-model.ts`（41 行）、
`session-log.tsx`。项目页：`routes/projects.$id.tsx`（70 行）、`board-data.ts`（58 行）、`task-card-model.ts`、
`task-columns.ts`（列 `todo / running / needs_you / done`）。设置页：`routes/projects.$id.settings.tsx`（90 行）、
`settings-data.ts`（27 行）、`config-view.ts`（16 行）。添加项目：`routes/projects.new.tsx`（85 行）。新建任务卡：
`routes/projects.$id.tasks.new.tsx`（73 行）。环境页：`routes/env.tsx`（62 行）。实时：`lib/live-client.ts`、`lib/live.tsx`、
`lib/live-routes.ts`、`components/admin-live.ts`。选择器集中在 `lib/testids.ts`。

**面板测试。** `apps/web/test/`：133 个文件，vitest 5，按 `test-groups.ts` 分三个 project（`unit` 并行、`kernel` 并行、
`exclusive` 串行且排在最后），`test/fixtures/global-setup.ts`；守卫 `test/test-groups.test.ts`、`core-sidecar.test.ts`
（不许条件跳过）、`ci-config.test.ts`、`styles.test.ts`。与本任务直接相关的既有测试：`task-detail-data`、`task-events`、
`task-docs*`、`task-changes`、`task-timeline*`、`task-action`、`task-ops*`、`onboarding-watch`、`settings-config`、
`settings-onboarding*`、`add-project`、`board*`、`board-payload`（项目页体积）、`read-gate`、`write-gate`、`web-launcher*`、
`web-instance`、`web-stop-gate`。夹具：`fixtures/w7-flow.sh`（分步假 CLI）、`automed-pidfile.sh`、`process-scan.ts`、
`dashboard-double.ts` 等。`apps/web/e2e/`：`flows.spec.ts`（需求与 Bug 两条链路）、`live.spec.ts`（D-3、D-4）、
`layout.spec.ts`（一屏巡检）、`manual-items.spec.ts`（W7 / W9 的人工项）；`harness/` 用真实内核（`target/debug/autome core`）、
假 CLI、临时根目录，另起一个 3000 ms tick 的「live」实例；`npm run e2e` = `node scripts/e2e-preflight.ts && playwright test`
（缺浏览器 / 缺构建 / 缺内核时明确报错）；Playwright 1.64.1 需要 `chromium-1248`（本机已装）。

**薄壳 `apps/desktop`。** `main.js`、`preload.js`、`src/shell/`、`test/` 五个文件（`node --test`）；窗口加载面板地址，
本任务预计不需要动它。

**脚本与门禁。** `scripts/ci-local.sh`（参数 `--package / --all / --plan / --check-claims`）→ `scripts/ci/lib.sh` +
`scripts/ci/steps.sh`。`steps.sh` 现有步骤：`Format`、`Clippy`、`Test (unit, end to end, adapter flags)`（`cargo test
--workspace --all-targets --no-fail-fast`）、`Desktop`、`Contract`、`Scripts`（`node --test scripts/test/*.test.mjs`）、
`Web lint / typecheck / build / test / e2e`，以及 `prep`：`Desktop install`、`Build core`、`Web install`；每个 `step` 声明输入
目录，输入没变的步骤沿用上次结果（T-97）；`NO_GATE='docs .autome .claude AGENTS.md README.md'`。`scripts/ci-quick.sh`
是每轮快检（静态检查 + 与本轮改动相关的测试，预算 120 秒）。`scripts/ci/summary.mjs`（失败用例摘要）、`scripts/test/`
（`ci-claims / ci-install / ci-quick / ci-reuse / ci-summary / web-build / web-groups / web-pidfile`）。
`.github/workflows/ci.yml` 的 job：`rust / scripts / web / desktop / package / cli-package / cli-release / contract`
以及 `e2e`。`.githooks/post-commit` 会在提交后后台跑 `ci-local.sh`。

**文档。** `README.md`（英文；`What it looks like` 在 10–48 行，`autome web` 的说明在约 306–315 行）；
`docs/images/` 六张旧图；`docs/development/technical-design.md`（中文，RPC 与事件的说明）。

**本机现状（整理时）。** Node v26.5.0；worktree 里没有 `apps/web/node_modules`、`apps/desktop/node_modules` 与 `target/`
（第一次要 `npm ci` 与 cargo 构建）；本次整理轮的会话里 `listen(127.0.0.1:0)` 与 `ps` 都可用——这只说明这一轮，后面的实现轮
未必如此（§9 第 1 条）。整理轮没有运行任何测试，也没有对真实内核做实测；§7 里标「实测」的都留给设计轮。

## 9. 前车之鉴（来自已合并任务的复盘，逐条可核对）

1. **实现轮多半不能监听端口、不能 `ps`。** T-96 的 8 次审计退回全部落在实现轮因 `listen EPERM` / `spawnSync ps EPERM` 跑不成
   验收的里程碑上；T-97 的 M-06 有四个实现轮因此既没跑成全量也没标 `待审`。对策：设计时让每个里程碑都有一部分验收不依赖
   监听（模型层、纯函数、替身内核）；实现轮把跑不成的用例名与完整命令列进证据交给审计轮。
2. **审计退回要附跑绿过的判据。** T-96 的 M-07、M-09 各有一条断言连续两次退回：实现轮跑不了浏览器，只能照失败输出猜。
   审计退回时附上「审计本机已跑绿」的检查或被测元素的真实取值之后，下一轮都一次通过。
3. **耗时类验收不要凭一次超时退回。** T-96 的 M-10 因 e2e 一次 170.64 秒被退回，下一轮零改动 149.80 秒通过，白花一对轮次。
4. **改了模块，跑引用它的全部测试。** T-96 的 M-04 / M-05 只跑验收契约列的文件，四条早已存在的测试被打破，直到宿主快检才发现；
   T-97 的 M-03 同样在验收子集之外留下两处回归。改 `apps/web/app` 的轮次跑 `npx vitest related --run <改动文件…>` 或
   `--project unit` 全量；改 `crates/<c>/src` 的轮次跑一次不带名字过滤的 `cargo test -p <c> --lib`。
5. **改了 RPC 回包形状，grep 全部使用方。** T-95 的四个里程碑各自审计通过，第一次合并前门禁在它们改过的回包上红了 7 个
   （含 Web 的五个 `*-real` 测试）。E-3、F 改回包或加事件时，`grep -rl` 方法名 / 字段名命中的 `crates/*/tests/` 与
   `apps/web/test/` 都要进验收子集。
6. **改协议要跑 `cargo test -p automed --lib protocol::`。** T-95 的 M-09 加了 eval 却没加进内核的嵌入清单，8 个失败被后面
   几轮当成「既有阻塞」一路带到合并前门禁。把失败标成「既有」之前，先查它是不是本分支引入的。
7. **负向对照照原文注入。** T-95 有两条负向对照按契约原文注入后并不变红（其一：去掉 `-M` 之后 git 默认仍检测改名）。
   写契约时就核实；实际做法与原文不同时并排写明。
8. **偶发失败的结论要够次数。** T-97 用「连续 5 次全绿」关闭了一个失败率约 25% 的问题，证据自己承认强度一般。按观测失败率
   p 取 n 使 (1-p)^n ≤ 0.05，或给出旧实现必红、新实现必绿的确定性回归（D-5 直接相关）。
9. **断言子进程输出时固定颜色环境。** T-97 的一条回归在会话里（带 `NO_COLOR=1`）全绿、在宿主门禁上红。
10. **改了测试运行器配置，用不带过滤的入口真实启动一次。** T-97 改 vitest 分组后只跑了 `--project unit`，`npm run test`
    整体起不来，审计才发现。
11. **改了 `.rs` 就 `cargo fmt --all`。** T-97 有一轮因一处换行被快检退回，多花两个会话。
12. **拉真实内核的夹具显式指定内核；`target/debug/` 可能是旧的。** 依赖新内核行为的测试先
    `cargo build -p automed -p autome-cli`（门禁里由 `prep 'Build core'` 保证）。
13. **已关闭里程碑的「最新证据」指针不要动；rebase 之后跑受影响包的全量。** 前者 T-84 三次被判协议失败；后者 T-84、T-96
    都在 rebase 轮只跑子集而让门禁连败。本任务周期长、六项依次做，中途 rebase 的概率高。
14. **验收命令按文件点名时别漏了新文件的分组。** 新增 Web 测试文件忘了进 `test-groups.ts`，`test-groups` 守卫会在全量里红。

## 10. 项目背景摘要（生成于 2026-11-14）

**这是什么。** Autome 2.0：一个驱动「设计 → 评审 / 裁决 → 实现 → 审计 → 合并前门禁 → 合并」Loop 的本地工具（Bug 另有一条
「定位 → 复现检查 → 修复 → 门禁 → 复核」的 Loop）。内核持有全部状态（SQLite 账本 + 事件流），以 stdio 上带长度前缀的
JSON-RPC 服务面板；命令行 `autome` 直接链接内核库、打开同一个账本，`autome core` 就是面板用的内核；多个驱动（`autome run`、
面板的内核）靠驱动锁轮流推进。面板（`autome web`）是内核状态的展示与控制面：Node 服务 + 浏览器页面，只绑 `127.0.0.1`，
鉴权只有 Host / Origin 检查。本仓库用 Autome 自己开发自己：`.autome/config.toml` 里 `[loop] parallel = 3`；
`[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`（每个任务只在合并前跑一次完整门禁）、
`timeout_minutes = 30`；每轮快检 `sh scripts/ci-quick.sh`（上限 10 分钟）；`[merge] auto = true`（过了合并前门禁自动合并，
设计批准仍要人）；`[retry] attempts = 2`。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：`crates/autome-domain`（纯域：状态机、
  配置形状与校验、状态块解析、依赖与队列规则，不做 I/O）、`crates/automed`（内核，库 + `automed` 二进制：调度、JSON-RPC 分发、
  账本、驱动锁、门禁、会话启动、Git、内置协议）、`crates/autome-cli`（`autome` 命令行，把 `automed` 当库链接；`web` 子命令转发
  给 Node 启动器）。
- `apps/web`——Node（≥ 22.22）服务（`server/`，TypeScript 由 Node 原生类型剥离直接运行：托管内核子进程、读写白名单、事件总线
  与 WebSocket、通知、日志跟随、`node:sqlite` 面板库、启动器）+ React Router 8 框架模式的面板（`app/`，SSR）；vitest 5、
  Playwright 1.64.1、eslint、tsc。
- `apps/desktop`——Electron 薄壳（起或连上面板服务、开窗口、原生目录选择与通知），`node --test`。
- `protocol/`——内置协议 seed（现为 v9，编译进内核）；`plugins/`——Claude Code / Codex / Cursor 插件；`scripts/`——门禁
  （`ci-local.sh`、`ci-quick.sh`、`ci/`、`test/`）、安装与打包；`.github/workflows/ci.yml`。
- 文档：`README.md`（英文）、`docs/development/{requirements,technical-design}.md`、`docs/plans/`、`docs/adr/`；每个任务的
  文档在 `docs/<slug>/`。任务卡点名的参考：`docs/内核补齐批次-2026-11-13-4/`（T-95，设计文档的 D、F、G 三项）、
  `docs/排队任务可以调整顺序并声明依赖-task/`（T-53）、`docs/面板交付批次-w10-启动与安装/`（T-96；Backlog BL-01、BL-05、
  BL-10、BL-AU15）、`docs/降低宿主门禁开销-输入没变的步骤不重跑/`（T-97；Backlog BL-01 – BL-03、设计 D.3）、
  `docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/`、`docs/w9-项目管理与环境-网页上添加项目-2/`。这些文档都很大
  （单份 40–100 KB），按节读，不要整份读。
- 状态位置：`$AUTOME_HOME`（缺省 `~/.autome`）下 `state/automed.sqlite3`（账本，旁边是 `.driver.lock`）、
  `state/web.sqlite3`（面板库）、`web/`（`server.json`、`launch.lock`、`logs/server.log`、`uploads/`）；任务 worktree 在
  `<项目>/.worktree/<slug>`。

**测试与检查命令（在 worktree 根目录运行）。**

- Rust：`cargo test --workspace --all-targets --no-fail-fast`（门禁同款）；单个 crate `cargo test -p automed --lib`；
  内核端到端 `cargo test -p automed --test end_to_end <用例名>`；CLI 集成 `cargo test -p autome-cli --test cli <用例名>`；
  `cargo fmt --all -- --check`；`cargo clippy --all-targets -- -D warnings`；协议：
  `cargo test -p automed --lib protocol::`、`cargo run -p automed -- protocol eval protocol/`、
  `cargo test -p automed --test prompt_rendering`。
- Web：新 worktree 先 `(cd apps/web && npm ci)`；`npm run lint|typecheck|build|test --prefix apps/web`；单个文件
  `npm run test --prefix apps/web -- test/<文件>`；单个分组 `(cd apps/web && npx vitest run --project unit)`；
  相关测试 `(cd apps/web && npx vitest related --run <源文件…>)`。拉真实内核的测试用 `target/debug/automed` 与
  `target/debug/autome`，先 `cargo build -p automed -p autome-cli`。
- 浏览器端到端：`npm run build --prefix apps/web && npm run e2e --prefix apps/web`（需要 Playwright 的 chromium；缺什么
  preflight 会报出来）；单个文件 `(cd apps/web && npx playwright test e2e/<文件>)`。
- 门禁脚本自身的测试：`node --test scripts/test/*.test.mjs`。Desktop：`npm test --prefix apps/desktop`。
- 整套本地 CI（即宿主门禁命令）：`sh scripts/ci-local.sh`（`--plan` 只看哪些步骤会跑，`--all` 强制全跑）。门禁由宿主在合并前
  跑一次；实现轮 / 审计轮跑里程碑验收命令与直接相关的测试。
- 输出长的命令重定向到 `.autome/output/` 只读头尾，例如
  `npm run test --prefix apps/web > .autome/output/web.txt 2>&1; tail -20 .autome/output/web.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、
  不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入 `.autome/output/`，不进任务
  文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/面板收尾批次-用上新的内核数据-补齐备注/protocol/`（见文首指路）。

**环境注意。** macOS（Darwin），系统 `sh` 是 bash 3.2；测试需要本机有 `git`；macOS 对二进制的签名与文件身份敏感（改名的
二进制副本曾实测被 SIGKILL，需要第二个名字时用符号链接并先实测）；`/tmp` 是 `/private/tmp` 的符号链接（F 的路径比较要
留意）；launchd 起的进程只有最小 PATH。
