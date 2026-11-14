# 面板收尾批次：用上新的内核数据、补齐备注 / 排队顺序 / 依赖三个控制操作、任务详情页瘦身、测试补强、残留进程清理——一次设计、一次评审，按项分组里程碑依次实现

## 背景

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 的 W1–W12 已全部合并（main @ af3cfa5a）：`autome web` 能起面板，Electron 已是薄壳，浏览器端到端测试在门禁里。期间内核又合并了 T-53（排队顺序与依赖）、T-70（合并队列）、T-95（十一项内核补齐）、T-97（门禁步骤沿用），面板的页面是按更早的内核设计的，有一批「内核已经给了、面板还没用上」的地方。另外 T-96、T-97 各留了几条不阻塞的 Backlog。

按 2026-11-13 确认的做法，这些合成一个任务：一份设计、一次评审，批准后按项推进。

2026-11-14 用已安装的面板（`autome web`，内核 `0002310-…-af3cfa5a`）对真实账本（97 个任务）走查的结果：

- 各页都能打开；项目页 55 KB（T-96 之前 1.1 MB）；时间与用时已是本地时间与「5 分 43 秒」；「已取消（23）」折叠区在。
- 任务详情页偏大：T-96（58 个会话）一次返回 327 KB，其中 250 KB 是序列化的 loader 数据；T-97（27 个会话）192 KB。
- 任务页能做的操作是：开始、批准、驳回、补充信息、合并 / 授权合并 / 撤销授权、改建成需求、暂停、恢复、停止、取消、重跑、延长预算、表态（`apps/web/app/components/task-action.ts` 的 `WRITE_OPS`）。命令行有而面板没有的：`autome note`（T-95 的 D 项）、`autome queue move`（T-53）、`autome after` / `task new --after`（T-53）。这三个方法 `task.note`、`task.move`、`task.set_after` 已在面板的写白名单里（`apps/web/server/gates/write-gate.ts`），只是没有页面入口。
- `autome web --help` 报「无效参数：--help」。
- T-96 的一轮审计会话留下了一个 `autome-web /tmp/aud-… serve` 和它的 `autome core` 子进程（临时 `AUTOME_HOME`，没有碰真实账本），任务合并、worktree 清理后还活着，跑的是已被删除的 worktree 里的二进制。此前 `logtail-live-*` 测试进程也这样残留过多次（T-86 修的是测试自身）。

## 要做什么

六项，按 A → B → C → D → E → F 的顺序实现。一份设计文档覆盖六项，里程碑按项分组、标题以「A·」「B·」… 开头，总数不超过 9 个（本任务不适用「超过 6 个里程碑就拆分」的惯例）。前一项的里程碑全部审计通过后再开始下一项。

### A 面板用上 T-95 的内核数据

T-95 的 F、G 两项（设计文档在 `docs/内核补齐批次-2026-11-13-4/`）给了下面这些数据，面板当时按「不改页面」留着没用。字段名与口径以 main 上的内核为准，设计轮对真实内核实测后定页面怎么用：

任务详情页：

1. 文档标签页列出 Bug 文档、复盘教训、任务卡；任务合并完成、worktree 清理后仍能读文档（来源是默认分支），里程碑终态照常显示。
2. 改动标签页显示每个文件的改动类型（新增 / 删除 / 修改 / 改名），用内核给的删除文件数做「删除文件数异常多」的提示，去掉现在按「只删不增」做的近似。
3. 时间线改用按任务读事件的接口，去掉面板服务端在内存里维护全账本事件索引的做法（`apps/web/server/task-events.ts`）；用会话结束事件给会话条目定结束时刻与结束方式（正常 / 被停止 / 超时 / 崩溃）。

项目页与设置页：

4. 添加项目：`project.probe` 给出拒绝原因（已是项目、嵌套在项目里、包含项目）时，在填表之前就显示，不等到确认才报错。
5. Onboarding 会话已进账本：项目页 / 添加项目页用 `session.log` 显示它的日志，用事件得知结束；去掉服务端的轮询推送（`apps/web/server/onboarding-watch.ts`），前提是事件覆盖了它现在处理的全部情形，覆盖不了的如实保留并写明。
6. 设置页「内核实际生效的配置」显示全部字段，并标出哪些取的是默认值。
7. 一个项目的配置坏掉时：总览、侧栏照常显示其它项目；坏的那个项目带报警标记，项目页与设置页显示内核的校验原文。

### B 控制面补齐：备注、排队顺序、依赖

面板定位是内核状态的展示与控制面，命令行能做的日常操作面板上也应能做：

1. **备注**（`task.note`）：任务详情页能给进行中的任务留一句话，下一个实现 / 修复轮的简报会带上；已留的备注（含是否已送达）在页面上看得到。`--reopen` 的语义（T-95 D 项）由设计轮按内核实际行为决定是否提供入口。
2. **排队顺序**（`task.move`）：项目页「待开始」列里排队中的任务能调整先后（移到最前、移到某张之前 / 之后）。不要求拖拽，按钮或菜单即可；键盘可操作。
3. **依赖**（`task.set_after`）：新建任务卡时可以选「在某些任务合并之后开始」；任务详情页能修改或清空依赖；卡片上显示「等 T-xx 合并」。
4. 三个操作都走现有的写白名单与表单 action，失败时显示内核的原文；别处（命令行、另一个标签页）做的同类改动经事件实时反映。

### C 任务详情页瘦身

- 58 个会话的任务详情页现在 327 KB。要求：60 个会话、30 个里程碑证据的任务，详情页首次响应不超过 150 KB（用夹具造数据断言大小）；loader 只返回首屏要用的字段，较早的轮次、未展开的文档与改动按需加载。
- 时间线新增条目时，展开的日志不被折叠、滚动位置不丢（W7 既有约束）继续成立；T-96 的一屏巡检与浏览器用例不回归。

### D 测试补强

T-96、T-97 审计留下的不阻塞项，逐条处理：

1. `apps/web/e2e/live.spec.ts` 的离线恢复用例没有守住事件重放：关掉 eventbus 的 resume、drain 分发、恢复后的重验，用例仍然通过。改成离线前先让页面收到一条事件（`lastSeq` 非空），并给出负向对照——去掉重放时用例失败。
2. 一屏巡检 `oneScreen` 只查 `main` 及直接子元素；补一条对看板列体 `.board-column-body[data-scroll-region]` 的断言。
3. `scripts/ci/summary.mjs`：cargo 的 `Doc-tests` 段没有 `Running` 行，失败用例的目标名会沿用上一个目标。
4. `scripts/test/ci-reuse.test.mjs` 用例 ⑪ 的 `indexOf('==> Build core') < indexOf('==> Web test')` 在 Build core 缺席（-1）时也成立；补 `>= 0` 与输出断言。
5. ENOTEMPTY（T-97 D.3 没做完的部分）：Web 测试夹具清理临时目录时偶发 ENOTEMPTY，现在靠有界重试（5 次 / 100 ms）。按 T-97 设计文档 D.3 的办法做改前 / 改后各 20 次诊断，定位写入者；能证实就修根因并去掉重试，证实不了如实写明已排除的项，保留重试。

### E 命令行与文档小项

1. `autome web --help`、`autome web -h`、`autome web help` 打印 `web` 子命令的用法（各子命令、`--force`、环境变量 `AUTOME_WEB_PORT`），退出码 0；`autome help` 里的那一行不变。
2. README `What it looks like` 的截图还是旧桌面端。用 `apps/web/e2e/` 已有的 Playwright 与夹具数据加一个生成截图的脚本（不进门禁），产出总览、项目页、任务详情三张图并替换 README 里的旧图；脚本可重复执行。
3. `autome web status` 的「是否有另一个驱动在推进」现在只能从 tick 让路推断（T-96 BL-01；2026-11-14 实测 launchd 驱动刚重启时显示「最近 1 分钟没有发现其他驱动」）。内核在驱动锁里记录持有者（pid、命令、起始时间），提供只读查询；`autome web status` 与环境页据此给出确定答案。

### F 清理任务时结束它留下的进程

- 内核在清理任务 worktree 之前（合并后的清理、取消任务时的清理），找出仍在运行、且可执行文件或工作目录位于该 worktree 之内的进程，先 SIGTERM、等待、再 SIGKILL，并记一条事件（结束了哪些进程：pid 与命令行摘要）。没有残留时不记。
- 只认「位于该任务 worktree 之内」这一条判据，不按进程名猜；主检出、其它任务的 worktree、用户自己的进程一律不碰。判据在 macOS 上怎么可靠取得（`ps` / `lsof` / `proc_pidpath`），由设计轮实测后定；取不到时不杀，记一条警告。
- 会话进行中不做这件事（实现轮、审计轮自己起的服务属于它自己）；只在 worktree 即将被删除时做。
- 协议正文与 prompt 是否加一句「本轮起的服务、内核、浏览器，结束前自己停掉」由设计轮判断；要改就只动一个协议版本。

## 对设计轮的要求

1. 内核改动只有 E3 与 F 两处，账本 schema 至多递增一次，协议至多一个新版本；其余各项只改 `apps/web/`、`apps/desktop/`、`scripts/`、`README.md`、`crates/autome-cli/`。
2. 不接触真实账本：实现轮与审计轮起面板、跑 `autome` / 测试一律用临时 `AUTOME_HOME`。会话里起的服务、内核、浏览器，轮次结束前自己停掉。
3. 新增的 Web 测试文件按 T-97 的分组规则归入对应的 vitest project（`test-groups` 守卫会检查）；新增的门禁步骤按 `scripts/ci/steps.sh` 的写法声明输入。
4. 会话沙箱里不能监听本地端口、不能执行 `ps` 时，如实写明哪些验收命令没跑成、由宿主门禁复验；不改弱测试，不加条件跳过。
5. 页面沿用既有 UI 规则：全中文、卡片优先低密度、1512×944 下页面本身不出现纵向滚动（内部滚动区除外）。

## 不做什么

- 不做角色、路由图、技能、协议页；不做配置的网页编辑；不做远程访问与登录。
- 不改调度、合并队列、门禁时机。
- 不做 Windows / Linux 的启动器与薄壳。
- 不处理 `npm start` / `npm run dev` 不写 `server.json`（开发模式，T-96 BL-06）。
- 不改 `.autome/`。

## 验收标准

1. A：每一条有对应的页面测试；A1、A3、A5 各有一条对真实内核的用例（Bug 任务的文档、已合并任务的文档、会话结束方式、Onboarding 日志）；去掉的服务端近似代码（全账本事件索引、Onboarding 轮询）确实删除，或在设计文档里写明保留的理由。
2. B：浏览器端到端用例各一条——留备注后下一轮简报里出现它；调整排队顺序后内核的启动顺序随之改变；设了依赖的任务在前置任务合并前不开始、合并后开始。命令行做的同类改动 5 秒内出现在页面上。
3. C：夹具断言 60 个会话的任务详情页首次响应 ≤ 150 KB；W7 的既有用例与 T-96 的一屏巡检不回归。
4. D：五条各自落在测试或证据里；D1、D4 给出负向对照；D5 有结论。
5. E：`autome web --help` 三种写法退出码 0 并打印用法；README 的三张截图由脚本生成并已替换；驱动锁持有者有 e2e（`autome run` 持锁时 `autome web status` 显示它的 pid 与命令）。
6. F：e2e——任务的 worktree 里留一个存活进程，任务合并后的清理把它结束并记事件；主检出与另一个任务 worktree 里的同名进程不受影响；判据取不到时不杀并记警告。
7. 新增的测试结束后不留存活进程；没有条件跳过的测试。
8. `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`sh scripts/ci-local.sh` 全绿。

## 参考

- 内核数据的来源：`docs/内核补齐批次-2026-11-13-4/`（T-95 设计文档的 F、G、D 三项）；T-53 的排队与依赖（`task.move`、`task.set_after`、`crates/automed/src/dispatch.rs`）。
- 面板：`apps/web/app/routes/`、`apps/web/app/components/{task-action.ts,task-ops.ts,task-detail-data.ts,task-timeline-model.ts,task-docs-model.ts,task-changes-model.ts,board-data.ts,settings-data.ts,config-view.ts}`、`apps/web/server/{task-events.ts,task-docs.ts,onboarding-watch.ts,gates/write-gate.ts,gates/read-gate.ts,launcher.ts,instance.ts}`、`apps/web/e2e/`。
- 已合并任务的设计文档与 Backlog：`docs/面板交付批次-w10-启动与安装/`（BL-01、BL-05、BL-10、BL-AU15）、`docs/降低宿主门禁开销-输入没变的步骤不重跑/`（BL-01 – BL-03、D.3）、`docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/`、`docs/w9-项目管理与环境-网页上添加项目-2/`。
- 内核：`crates/automed/src/driver_lock.rs`、`crates/automed/src/scheduler.rs` 里合并后清理与取消清理的步骤、`crates/autome-cli/src/`（`web` 子命令）。
- 门禁脚本：`scripts/ci/{lib.sh,steps.sh,summary.mjs}`、`scripts/test/`、`apps/web/test-groups.ts`。
