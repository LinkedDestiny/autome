# 任务：旧内核不能推进新账本——内核带构建标识、比账本旧就只读；面板找内核的顺序与安装方式对齐

本任务的规则见 `docs/旧内核不能推进新账本-内核带构建标识/protocol/loop-protocol.md` 与
`docs/旧内核不能推进新账本-内核带构建标识/protocol/session-protocol.md`。

- 设计文档：`docs/旧内核不能推进新账本-内核带构建标识/旧内核不能推进新账本-内核带构建标识.md`
- 任务卡（用户写的，第一输入）：`docs/旧内核不能推进新账本-内核带构建标识/旧内核不能推进新账本-内核带构建标识-card.md`
- 任务整理日期：2026-11-13
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `f40e8c64`

## 1. 需求原文

> 旧内核不能推进新账本：内核带构建标识，比账本旧就只读；面板找内核的顺序与安装方式对齐

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§5 照任务卡整理，§6–§7 是整理轮从调研得出的约束与
待定问题，§8–§9 是代码现状与项目背景。

## 2. 背景与目标

**背景（任务卡原文的事实）。** 2026-11-13 12:20，在 main 上按计划 §7.3 的说法用 `npm --prefix apps/web start` 起面板。
面板起的内核是 `target/debug/automed`——主检出里 11 小时前 cargo 留下的旧构建（早于 T-61、T-67、T-73、T-77）。它和
launchd 里的驱动轮流拿驱动锁，5 分钟内造成：

- T-59、T-54 被判「失败（实现）：配置问题：prompts/impl.md 里有 Autome 不认识的占位符 `{gate_line}`」——任务固定的
  协议是 v4 / v6，旧内核只认识到 v3；
- 新建的 T-81 被它固定在 `protocol/v3` 上（只能取消重提）；
- 它不认识 `[gate] run = "before_merge"`、快检、合并不占槽位这些后来的行为，没被及时发现的话还会按旧规则跑门禁、
  推进状态。

原因有两层：

1. `apps/web/server/core/binary.ts` 的查找顺序是 `AUTOMED_BIN` → 同包的 `automed` → `~/.local/bin/automed` → cargo
   构建目录。但 `scripts/install-cli.sh` 只装 `autome` 一个文件（内核是链接进 CLI 的库），`~/.local/bin/automed` 从来
   不存在，于是在源码检出里起面板一定落到 cargo 目录，那里是什么版本全凭运气。
2. 内核没有任何机制发现「我比这份账本旧」。任何旧二进制（旧的桌面端、没重装的 CLI、另一个 worktree 里的构建）接上
   同一个 `AUTOME_HOME` 都会照常推进。

**目标。** 两层各堵一次：

- 内核（库与二进制）带一个可比较的构建标识；账本记下推进过它的最新内核；比账本旧的内核只读——不推进、不判失败、
  不固定协议，写命令给出一句明确的话；确需回滚时有显式的逃生口并留下事件。
- 「装了 CLI 就一定有一个与 CLI 同版本、面板找得到的内核」；面板启动时说清自己用的是哪个内核；遇到「内核比账本旧」
  时横幅提示、写操作置灰、不刷日志。CLI 的 `autome run` 走同一段比较，被拒绝时不刷屏。

## 3. 要做什么（任务卡「要做什么」六条，逐条照录）

1. **构建标识。** `automed`（库与二进制）在构建时嵌入一个可比较的标识：建议用构建所在提交的提交时间戳 + 短 sha
   （`build.rs` 读 git；拿不到 git 时退回到包版本与构建时间，并标记为「未知来源」）。`autome --version`、
   `automed --version`、`env.get` / `core` 握手信息里都带上它。口径由设计轮定准，要求是「同一个提交构建出来的 CLI 与
   内核标识相同，后来的提交标识更大」。
2. **账本记下推进过它的最新内核。** 每次真正推进（`scheduler.tick` 拿到驱动锁、以及会改任务状态的写命令）时，把自己
   的标识与账本里记的比较：
   - 自己更新或相同：照常推进，并把账本里的值更新为自己的；
   - 自己更旧：**不推进**。`scheduler.tick` 返回 `skipped`，原因写明「这个内核（<标识>）比推进过账本的内核（<标识>）
     旧，已停止推进；请重新安装或设置 AUTOMED_BIN」；会改任务状态的写命令（创建、开始、批准、合并、rerun 等）同样
     拒绝并给这句话；只读方法照常工作。
   - 不能因此把任何任务判失败，也不能给任务固定协议。
3. **逃生口。** 确实要用旧内核接管（回滚）时：`AUTOME_ALLOW_OLDER_KERNEL=1` 或一条显式命令
   （`autome doctor --adopt-kernel` 之类，设计轮定名）把账本里的值降回当前内核，记一条事件。README 写明。
4. **面板找内核与安装对齐。**
   - `scripts/install-cli.sh` 与 `scripts/package-cli.sh` 把 `automed` 也装到 `$PREFIX`（或者 `autome` 提供一个等价的
     子命令让面板以 stdio 方式拉起内核——两种做法设计轮选一，要求是「装了 CLI 就一定有一个与 CLI 同版本、面板找得到
     的内核」）；
   - `binary.ts` 落到 cargo 构建目录时在启动日志里明确打一行「正在使用源码构建的内核：<路径>（<标识>）」；不管来源
     是哪一种，启动时都打出内核路径与标识；
   - 面板收到第 2 条的「内核比账本旧」时，顶部横幅显示这句话，写操作置灰（沿用内核离线时的样式），不再每 3 秒重试
     tick 刷日志。
5. **CLI 侧同样受保护。** `autome run` 用的是链接进去的内核，走同一段比较；被拒绝时 `autome run` 打一行原因后退出码
   非零（launchd 会重启它——设计轮确认不要形成每 10 秒一次的刷屏，例如退避或保持进程但不推进）。
6. **文档。** README「安装 / 升级」一节与 `docs/development/technical-design.md` 写明构建标识、谁能推进账本、逃生口；
   计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §7.3 里「用 `npm --prefix apps/web start` 起面板」一句补上
   「先重装 CLI」的前提。

## 4. 验收标准（任务卡原文，编号不变）

1. 单测：构建标识的解析与比较（含「未知来源」）；同一提交的 CLI 与内核标识相同。
2. e2e：先用「新」标识的内核推进一次，再用「旧」标识的内核（测试里注入标识）接同一个 `AUTOME_HOME`：
   `scheduler.tick` 返回 `skipped` 且带原因；`task.create`、`task.approve`、`task.merge`、`task.rerun_from` 被拒绝并给
   同一句话；`task.get`、`dashboard.get`、`events.since` 正常；账本里没有新增任何 `task.updated`，也没有任务被固定协议。
3. e2e：旧内核带逃生口启动后可以推进，事件里有记录；之后新内核再接上，账本的值回到新的。
4. e2e：排队中的任务在旧内核接上期间不被判失败（对应 T-59 / T-54 的现场：协议副本里有旧内核不认识的占位符）。
5. `install-cli.sh` 在空 `PREFIX` 下装完后，面板的 `resolveAutomedBinary` 找到的内核与 `autome --version` 标识一致
   （测试）；`package-cli.sh` 的 tar 包里有对应文件。
6. 面板：内核返回「比账本旧」时横幅与置灰有测试；启动日志含内核路径与标识。
7. `autome run` 被拒绝时不刷屏（测试断言一段时间内的输出行数上限）。
8. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

设计轮写里程碑时须让每条验收标准落到某个里程碑的验收命令上。

## 5. 范围

**在范围内（预计会动到的地方，以设计为准）。**

- `crates/automed`：构建标识（新增 `build.rs` 与承载它的模块；`entry::VERSION` 现在是唯一的版本常量）、
  `main.rs` 的 `--version` 与启动日志、`scheduler.rs` 的 `tick`、`dispatch.rs` 的 `handle_command` / `scheduler_tick` /
  `env_get`、`store.rs`（账本里记「最新内核」的地方）、对应单测与 `tests/end_to_end.rs`。
- `crates/autome-cli`：`--version` / `version`、`doctor`（若逃生口落在这里）、`run`（`drive`）被拒绝时的行为、
  `USAGE`、`tests/cli.rs`。若选「`autome` 提供 stdio 子命令」，还包括 `main.rs` 开头那段与 `automed` 对齐的子命令分发。
- `scripts/install-cli.sh`、`scripts/package-cli.sh`（及其内嵌的发布包 README 文案）。
- `apps/web`：`server/core/binary.ts`、`server/core/supervisor.ts`、`server/core/sidecar.ts`（若拉起方式变化）、
  `server/ticker.ts`、`server/main.ts`（启动日志）、`server/eventbus-protocol.ts`（`core` 消息）、`app/lib/live.tsx`
  （横幅与写闸门）及对应测试。
- 文档：`README.md`、`docs/development/technical-design.md`、`docs/plans/2026-11-10-web-dashboard-via-loop.md` §7.3。

**不做（任务卡「不做什么」，逐条照录）。**

- 不做账本格式的版本迁移框架；这里只管「谁能推进」。
- 不做多台机器之间的协调。
- 不改协议正文。
- 不处理 W10 的发布包结构（W10 沿用这里定下的安装位置即可）。

**整理轮补充的边界（从调研得出，不扩大范围）。**

- 桌面端（`apps/desktop`）不在任务卡的改动清单里：它带的旧内核由内核侧的比较保护，界面上怎样呈现不是本任务的验收项。
  缺省不改桌面端代码与 `scripts/package.sh`；但桌面端现有测试必须照常通过（见 §6 约束 7、8）。
- launchd 的 `com.autome.loop` 条目在仓库之外（`~/Library/LaunchAgents/`），会话不得改它；「不刷屏」只能由
  `autome run` 自身的行为达成。
- 「不改协议正文」包括仓库的 `protocol/` 目录与本任务的协议副本 `docs/<slug>/protocol/`。

## 6. 硬性约束

1. **被拒绝即无副作用。** 比账本旧的内核在 `scheduler.tick` 与写命令上被拒绝时：不写 `task.updated`、不改任何任务
   状态、不把任何任务判失败、不调用 `freeze_protocol`（不写协议副本、不提交、不写 `protocol_ref`）、不启动会话、
   不跑门禁、不动 worktree（任务卡第 2 条，验收 2、4）。比较必须发生在这些动作之前——`tick_locked` 的第一步就是
   `gate_queue::reclaim` 与首个 tick 的 `recover()`，都属于「推进」。
2. **只读照常。** `READ_METHODS`（`dispatch.rs`，现为 17 项）里的方法在旧内核上行为与现在一致（验收 2 点名
   `task.get`、`dashboard.get`、`events.since`）。
3. **一句话，处处相同。** tick 的原因与写命令的拒绝信息是同一句：「这个内核（<标识>）比推进过账本的内核（<标识>）
   旧，已停止推进；请重新安装或设置 AUTOMED_BIN」。面板横幅、`autome run` 的那一行显示的也是它；测试断言「同一句话」
   （验收 2）。措辞若要调整，由设计轮定一次并处处一致。
4. **同一提交，同一标识。** `autome` 与 `automed` 的标识来自 `automed` 库里的同一个来源（现在两者都读
   `automed::entry::VERSION`），不得各算各的；后来的提交标识更大（任务卡第 1 条，验收 1）。
5. **标识不得陈旧。** 一个因为构建缓存而停留在旧提交上的标识，就是本任务要防的那种事故的翻版（新代码报旧标识 →
   拒绝推进；或旧代码报新标识 → 照常推进）。`build.rs` 的重跑条件必须跟着 HEAD 走，并且在 worktree 里成立——
   worktree 的 `.git` 是一个文件，指向主仓库的 `.git/worktrees/<名字>/`，不能假设 `.git/HEAD` 存在。
6. **不得让本 worktree 构建出的二进制接触真实账本或真实安装位置。** 本仓库正由已安装的 `~/.local/bin/autome`
   （launchd `com.autome.loop`，`autome run`）驱动自己的 Loop，账本在 `~/.autome`。本任务的内核一旦对真实账本推进过
   一次，账本里记的「最新内核」就高于已安装的内核——若那时已安装的内核也带了这套比较，正在驱动本任务和其它任务的
   驱动会全部停下；若本任务还加了迁移，旧内核更是连账本都打不开（`refuse_a_database_that_is_not_ours`）。因此：
   - 任何手工验证与测试都把 `AUTOME_HOME` 指到临时目录（照 `crates/autome-cli/tests/cli.rs` 的 `Sandbox`、
     `apps/web/test/server.test.ts` 的 `tempHome()`），不得省略、不得指向 `~/.autome`；
   - 跑 `scripts/install-cli.sh` 必须同时设 `PREFIX` 为临时目录**和** `AUTOME_HOME` 为临时目录：脚本结尾会执行
     `autome doctor`，而 `doctor` 与其它命令一样先 `Client::open()`——打开账本并跑迁移；缺省 `PREFIX` 还会覆盖正在
     驱动 Loop 的 `~/.local/bin/autome`；
   - 不要在本 worktree 里不带临时 `AUTOME_HOME` 地跑 `sh scripts/ci-local.sh --package`（它的 `CLI package` 步骤会调
     `install-cli.sh`，只换了 `PREFIX`）；不带 `--package` 的 `ci-local.sh` 就是宿主门禁平时跑的那一套，其中的
     测试各自使用临时账本——本任务新增的测试也必须如此；
   - 不要在本 worktree 里起真实面板（`npm --prefix apps/web start` / `dev`）而不设临时 `AUTOME_HOME`。
7. **`skipped` 的现有形状不变。** `scheduler.tick` 的返回里 `skipped` 现在是布尔值，读它的有 `autome run`（`drive`）、
   Web 的 `ticker.ts` / `supervisor.ts`（`TickReport = { skipped?: boolean; errors?: string[] }`）与桌面端。原因只能
   以新增字段的方式带出，不得把 `skipped` 改成别的类型；「另一个驱动持锁」的跳过与「内核比账本旧」的跳过要能被
   调用方区分开。
8. **`READ_METHODS` 的常量名与现有内容不要动。** `apps/desktop/test/ipc-gate.test.js`、`apps/web/test/read-gate.test.ts`、
   `apps/web/test/write-gate.test.ts` 解析 `dispatch.rs` 核对它，`ci-local.sh` 的 `Contract` 步骤 grep 这个常量名。
   新增的方法若会改账本（例如逃生口），是写方法，不进 `READ_METHODS`；若经 Web 网关调用，还要过
   `apps/web/server/gates/write-gate.ts` 的白名单（任务卡未要求面板提供逃生口）。
9. **迁移只加不改。** 若「账本记下最新内核」需要改表：`store.rs` 顶部的规矩是 `SCHEMA_VERSION` 加一（整理时为 7），
   在 `migrate` 里加一个新的 `if version < N` 块，块内是包含 `PRAGMA user_version` 的单个事务，**不得编辑已有的迁移
   块**；迁移测试照现有 v6→v7 那条（`deps_store_migration_preserves_queue`：先把库降回上一版再重新打开）覆盖新版本。并行任务也可能占用同一个版本号——版本号以合并时 main 上的
   `SCHEMA_VERSION` 为准，rebase 后要核对。这不是「版本迁移框架」，只是沿用现有做法。
10. **没有记录的账本照常推进。** 升级后第一次接上的账本里没有「最新内核」的记录；此时任何带标识的内核都应当能
    推进并写下自己的值，现有 e2e（`end_to_end.rs` 183 个用例、`cli.rs` 30 个用例）与 Web 的真实内核测试一行不改就要
    通过——它们用的都是全新的临时账本。
11. **逃生口必须是显式的，并留下事件。** 缺省行为是拒绝；降回账本的值只在用户明确要求时发生，并在事件流里记一条
    （验收 3）。之后更新的内核再接上，账本的值回到新的（验收 3）。测试用的「注入标识」通道不得成为生产环境里绕过
    比较的第二个逃生口（见 §7 问题 5）。
12. **`autome run` 被拒绝时不刷屏。** 驱动条目是 `KeepAlive = true`、`ThrottleInterval = 10`，标准输出与标准错误都
    写进 `~/.autome/state/loop-driver.log`；`autome run` 每次以非零码退出，launchd 10 秒后就会再起一次。无论选哪种
    做法，日志里这句话的出现频率要有上限并用测试钉住（验收 7）。
13. **安装脚本的既有约定。** `install-cli.sh` / `package-cli.sh` 为 `/bin/sh`（macOS 的 bash 3.2）而写：不用数组、
    `[[`、`local`；中文紧跟变量时写 `${var}`；安装用「写新文件再 `mv`」而不是原地覆盖（脚本注释：原地改写 macOS 已
    校验过签名的二进制，会让它的下一次运行被杀掉）；打包用复制而不是原地改名（链接器的 ad-hoc 签名属于它写出的那个
    文件）。若方案需要让同一个二进制以第二个名字出现（复制、硬链接或符号链接），先在本机实测它能正常运行再定——
    macOS 对签名与文件身份敏感，2026-08 曾实测到系统二进制的改名副本在启动后数百毫秒被 SIGKILL。`--from` 安装
    发布包的路径（`unpack` 现在只找 `autome`）与 `.sha256` 校验要继续成立。
14. **CI 步骤清单有测试钉着。** `apps/web/test/ci-config.test.ts` 断言 `.github/workflows/ci.yml` 与
    `scripts/ci-local.sh` 里的步骤与顺序；改这两个文件要同步那份测试。
15. **文案语言沿用现状。** 报错、界面、CLI 与日志文案用中文；代码注释与 `README.md` 用英文；
    `docs/development/technical-design.md` 与计划文档用中文。
16. **会话边界。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改 `.autome/`；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成本轮后结束会话，不自行启动
    下一个会话（AGENTS.md）。

## 7. 待设计轮回答的问题

任务卡留白或与代码现状有出入的地方。设计文档「方案」一节须逐条给出结论与依据；没有依据支持改动任务卡字面的，
按任务卡字面做。

1. **标识的口径。** 任务卡建议「提交时间戳 + 短 sha」。需要定下：
   - 文本格式与解析规则；用提交者时间（`%ct`）还是作者时间（rebase 之后两者不同）；
   - 比较规则：时间戳相同而 sha 不同时怎样（视为相同、视为不可比、还是按某个次序）；
   - 「未知来源」（拿不到 git：源码包、`cargo install` 自发布包等）与有来源的标识怎样比较，两个「未知来源」之间
     怎样比较——它算更旧、更新，还是只能靠逃生口；
   - 工作区有未提交改动（dirty）时标识怎样表示，是否参与比较；
   - `--version` 的输出形状：现在是 `autome 0.1.0` / `automed 0.1.0`，`install-cli.sh` 把 `"$dest" --version` 的输出
     原样印在「已安装」一行里，`ci-local.sh --package` 也调它。
2. **`build.rs` 怎样保证不陈旧（约束 5）。** 重跑条件用什么（`git rev-parse --git-dir` 得到的路径下的 `HEAD` 与
   它指向的 ref、packed-refs 等）；放在 `crates/automed` 一处还是两个 crate 各一份（约束 4 要求同源）；
   `cargo install --path crates/autome-cli`、发布包里没有 `.git` 时的退路；构建机没有 `git` 命令时不得让构建失败。
3. **「最新内核」记在账本哪里。** 现有表没有键值/元数据表（`projects`、`tasks`、`sessions`、`decisions`、`events`、
   `rule_proposals`、`rule_experiments`、`runtime_cooldowns`、`project_alerts`、`session_retries`、`task_briefs`、
   `task_after`）。新表（要迁移，约束 9）还是别的载体；比较与更新是否在驱动锁之内完成（两个驱动交替时不能出现
   「各自都以为自己最新」的窗口）；每个 tick 都「更新为自己的」时，值没变就不应产生写入或事件。
4. **本任务合并之前构建的内核不认识这套比较。** 它们照旧会推进（11-13 的现场就是这种内核）。加一个迁移会让它们被
   已有的 schema 检查挡在门外（打不开账本、进程退出、面板/launchd 反复重启）；不加迁移则它们不受保护。设计轮说明
   选择与后果，以及对「正在运行的已安装驱动」的上线顺序（合并 → 重装 CLI → kickstart）有没有要求写进 README。
5. **哪些调用算「推进」。** 任务卡写的是「`scheduler.tick` 拿到驱动锁、以及会改任务状态的写命令」。`handle_command`
   现在对**所有**不在 `READ_METHODS` 里的方法取驱动锁，其中有不改任务状态的：`config.set_*`、`project.add` /
   `remove`、`project.onboarding.*`、`project.alert.dismiss`、`rules.*`、`task.archive` / `restore`、`task.retro`、
   `task.decide`、`task.set_after`、`task.move` 等。逐类定下拒绝还是放行，并说明依据（例如旧内核写的配置、
   旧内核 `project.add` 时跑的 `init` 脚手架是否算「按旧规则改了东西」）。另外两处不经 `handle_command` 的入口也要
   交代：`Store::open` 的 `migrate`（旧内核打开账本本身）、`recover()`（首个 tick 内）。
   测试怎样「注入标识」（验收 2）：进程内的 e2e（`World` 用 `Ctx::new(..).headless()` 加内存库）可以走 `Ctx` 上的
   字段，像 `launch_mode` 那样；但 `cli.rs` 跑的是真实的 `autome` 二进制、Web 测试拉起的是真实的 `automed`，
   验收 5、6、7 若要在进程级复现「旧内核」，需要别的通道——定下通道，并说明它为什么不构成约束 11 所说的第二个逃生口。
   「接同一个 `AUTOME_HOME`」在 `World` 现有的内存库上怎样成立（两个 `Ctx` 共用一个文件库，还是别的写法）。
6. **逃生口的形状。** `AUTOME_ALLOW_OLDER_KERNEL=1`、显式命令，还是两者都要；命令叫什么、挂在哪个动词下
   （`doctor` 现在只收 `--json`，且以环境探测结果决定退出码）；事件的种类名、`subject` 与 payload（旧值、新值、
   由谁触发）；环境变量常驻时是否每个 tick 都算一次「接管」——新驱动（launchd）与带逃生口的旧内核同时在跑，会把
   账本的值来回改写并各记一条事件，设计轮说明这种情形下的行为与事件数量上限。
7. **标识在哪些出口带出。** 任务卡写「`env.get` / `core` 握手信息」。内核现在没有握手方法：Web 的
   `CoreSupervisor.start()` 用第一次 `scheduler.tick` 当握手（成功即 `connected`），WebSocket 的 `hello` / `core`
   消息来自服务端自己的状态（`eventbus-protocol.ts`：`CoreMessage = { type: 'core'; status: 'connected' | 'offline';
   reason? }`）。定下：`env.get` 的 payload 加哪个字段（只增不改）；tick 的返回里带什么；`automed` 启动时的
   `automed core starting` 日志是否带标识；`autome doctor`（含 `--json`）是否显示自己的标识与账本里记的值。
8. **安装对齐选哪一种。** (a) 把 `automed` 也装到 `$PREFIX`；(b) `autome` 提供一个以 stdio 拉起内核的子命令。
   相关事实：`install-cli.sh` 在检出里只 `cargo build --release -p autome-cli`，`unpack` 只找名为 `autome` 的可执行
   文件；`package-cli.sh` 的 tar 包里只有 `autome` 与 `README.md`；`autome` 的 `main` 已经为了会话包装脚本而与
   `automed` 对齐了 `render-stream`、`protocol eval` 两个子命令（先例）；Web 的 `sidecar.ts` 以空参数
   `spawn(binaryPath, [])` 拉起内核；`binary.ts` 的 `local-bin` 一档写死 `~/.local/bin/automed`，不认 `PREFIX`；
   `bundle` 一档是 `apps/web` 上一级的 `automed`（W10 发布包的位置，本任务不处理其结构）；`cargo install --path
   crates/autome-cli` 只会装出 `autome`。设计轮选一并说明：自定义 `PREFIX` 下面板怎样找到内核（验收 5 是「空
   `PREFIX` 下装完后 `resolveAutomedBinary` 找得到」）；`binary.ts` 的新顺序与 `source` 取值；
   `apps/web/test/core-binary.test.ts` 现有 6 条断言哪些要改；桌面端的查找（`apps/desktop/src/sidecar.js`：
   `AUTOMED_BIN` → 包内 `Resources/core/` → cargo 目录）缺省不动。
9. **面板启动日志与标识从哪来。** 「不管来源是哪一种，启动时都打出内核路径与标识」「落到 cargo 目录时明确打一行
   『正在使用源码构建的内核：<路径>（<标识>）』」——标识是启动前跑一次 `<内核> --version` 取，还是连上之后从 RPC
   取；内核文件不存在或起不来时这行怎样写；日志现在统一走 `console.error`（`main.ts`）。
10. **面板怎样表示「内核比账本旧」。** 现在 `CoreStatus.state` 只有 `starting / connected / restarting / offline /
    stopped`，映射到 `CoreMessage` 的 `connected / offline`；`liveBanner` 的文案是「内核离线：<原因>，写操作暂不可用」，
    `writeGate` 只在 `connected` 时放行。任务卡要「横幅显示这句话，写操作置灰（沿用内核离线时的样式），不再每 3 秒
    重试 tick 刷日志」。定下：这是一个新状态还是带原因的 `offline`；横幅文案（是否仍带「内核离线：」前缀）；此时
    读请求照常可用（内核其实连着）；ticker 是停下、降频还是照常 tick 但不打日志，以及用户重装并重启面板之外有没有
    恢复路径（`core.restart` 现在在 `connected` 时直接返回 `already_running`）；直接经网关发来的写请求被内核拒绝时
    HTTP 层给什么（`gateway.ts` 现在把带 `code` 的内核错误映射为 409）。
11. **`autome run` 被拒绝时的行为（任务卡第 5 条与验收 7 之间有张力）。** 任务卡正文写「打一行原因后退出码非零」，
    同时要求设计轮确认不刷屏，并给了两个例子：退避、或保持进程但不推进。在 launchd `KeepAlive` +
    `ThrottleInterval 10` 之下，「立刻非零退出」就是每 10 秒一行。设计轮选定并写明：`run`（常驻）、`run --once`、
    `run --until-idle` 各自怎样；保持进程时是否周期性重查（用户重装后旧进程不会自己变新，但逃生口可能让账本的值
    降下来）；`--json` 模式下这条信息的形状；现有的「另一个驱动…正在推进，本进程轮流推进」提示（`drive` 里对任何
    `skipped` 都打）不得用在这种情形上；验收 7 的测试怎样在不真的等几分钟的前提下断言行数上限。
12. **其它写命令在 CLI 上的呈现。** `autome task new` / `approve` / `merge` / `rerun` 等被拒绝时，现有路径是
    `CliError::Failed(message)` → `autome：<message>`、退出码 1。确认沿用，并覆盖 `task new` 这类在调用 `task.create`
    之前还做别的事（读文件、解析项目）的命令。
13. **验收 5 的测试放在哪、花多少时间。** 在检出里跑 `install-cli.sh` 意味着一次 `cargo build --release`（冷构建以
    分钟计）；宿主门禁 `sh scripts/ci-local.sh` 的上限是 30 分钟，每轮快检上限 10 分钟。是进缺省门禁、进
    `--package` 一档、还是用 `--from` 一个由 debug 产物拼出的 tar 包来测安装逻辑——说明取舍；无论哪种都受约束 6
    限制（临时 `PREFIX` 与临时 `AUTOME_HOME`）。
14. **文档落点。** README 没有名为「安装 / 升级」的小节：安装写在 `## The command line`（约 280–296 行），状态位置在
    `## Where your state lives`，桌面端安装在 `## Packaging`。定下新内容放哪（新小节或并入现有小节，README 为英文）；
    `technical-design.md` 落在 §8.1「驱动锁」、§13「持久化与恢复」、§14「IPC 接口」中的哪几处；计划 §7.3 的那一句
    在 481 行（「**W6、W7 合并后**，用 `npm --prefix apps/web start` 起面板…」）。

## 8. 代码现状（整理轮调研，行号为约数，以实际代码为准）

**版本与入口。**

- 仓库里没有任何 `build.rs`。唯一的版本常量是 `crates/automed/src/entry.rs` 末尾的
  `pub const VERSION: &str = env!("CARGO_PKG_VERSION");`（注释：The version both binaries report）。
- `crates/automed/src/main.rs`（130 行）：`--version | -V` 打印 `automed {VERSION}`（54 行）；`render-stream`、
  `protocol eval` 两个子命令；其余情况打开账本（`entry::ledger_path`：`AUTOMED_DB_PATH` 或 `AUTOME_HOME` 下）、
  建 `Ctx`、打一行 `automed core starting`（tracing 写 stderr），然后在 stdin/stdout 上读写带长度前缀的 JSON-RPC 帧。
- `crates/autome-cli/src/main.rs`（1509 行）：`main`（88 行）先分发 `render-stream` / `protocol eval`；`run`（115 行）
  处理 `--help`、`--version`（122 行，打印 `autome {VERSION}`），按动词列允许的开关，随后**对每个动词**都
  `Client::open()`（171 行）；`doctor`（237 行，直接调 `env_probe::probe_all()`，`--json` 输出 `home`、`ledger`、
  `driver_lock`、`binary`、`environment`、`severity`）；`drive`（969 行，`autome run`：循环 `scheduler.tick` →
  `events.since`，`skipped == true` 时打一次「另一个驱动…正在推进」，tick 报错即以 `CliError::Failed` 退出码 1 结束）；
  `trigger`（1215 行）是「解析任务名 → 调一个写方法 → 打印结果」的通用路径；`USAGE`（44 行，有断言其内容的测试）。
- `crates/autome-cli/src/client.rs`（187 行）：CLI 不拉起 `automed` 进程，而是链接同一个库、在本进程内调
  `dispatch::handle_command`；`Client::open` 用 `Store::open` 打开账本。

**驱动锁与推进入口。**

- `crates/automed/src/driver_lock.rs`（198 行）：账本旁的 `<db>.driver.lock` 上的 `flock`；`try_acquire`（不等）、
  `acquire_with_notice`（等）。内存库没有锁文件。
- `crates/automed/src/scheduler.rs`（10893 行）：`TickReport { sessions_reaped, tasks_advanced, tasks_started, errors,
  skipped: bool }`（90 行）；`tick`（127 行）——`ctx.driver_lock` 已有则直接跑，否则 `try_acquire`，拿不到返回
  `skipped: true`，拿到后调 `tick_locked`（155 行：`gate_queue::reclaim` → 首个 tick 的 `recover()`（170 行）→ 收会话
  → 重试 → 内核步骤 → 填槽位）；`freeze_protocol`（3345 行：任务没有 `protocol_ref` 或副本目录不存在时，把**本二进制
  内置的**协议复制进 `docs/<slug>/protocol/`、提交「固定协议」、写 `protocol_ref`——T-81 被固定到 v3 的地方）；
  `recover`（4686 行）。
- `crates/automed/src/dispatch.rs`（6546 行）：`Ctx`（40 行，字段有 `store`、`autome_home`、`home`、`environment`、
  `launch_mode`、`recovered`、`gate_prev_tick`、`driver_lock`、`driver_lock_notice`；测试通过字段而不是进程级开关改
  行为）；`handle_command`（222 行：先核对 IPC 的 `PROTOCOL_VERSION`，然后对「不是 `scheduler.tick` 且不在
  `READ_METHODS`」的方法等驱动锁，`DRIVER_LOCK_WAIT` 现为 120 秒）；方法表 380–481 行；`scheduler_tick`（2761 行，
  返回 `sessions_reaped / tasks_advanced / tasks_started / errors / skipped / latest_seq / env_generation /
  env_probing`）；`env_get`（2787 行）与 `env_json`（2827 行：`probed / probing / generation / environment / severity /
  problems / can_run_anything / prerequisites`）；`READ_METHODS`（2982 行，17 项）；拒绝用 `rejected(..)` /
  `bad_params(..)` / `internal(..)` 构造 `DispatchError { code, message }`。
- `crates/automed/src/store.rs`（3808 行）：`SCHEMA_VERSION = 7`（70 行）；`Store::open`（193 行，打开即 `migrate`）；
  `migrate`（231 行，v7 块在 489 行）；`refuse_a_database_that_is_not_ours`（519 行：账本 `user_version` 高于本二进制
  的 `SCHEMA_VERSION` 时拒绝打开——这是现有唯一的「我比账本旧」检查，只在 schema 变过时才生效）；`append_event`
  （561 行，所有状态变化都经它进 `events`）；`latest_seq`、`events_since*`。

**Web 面板。**

- `apps/web/server/core/binary.ts`（18 行）：`resolveAutomedBinary({ env, home, webRoot, exists })` 返回
  `{ path, source: 'env' | 'bundle' | 'local-bin' | 'cargo' }`，顺序是 `AUTOMED_BIN` → `webRoot/../automed` →
  `~/.local/bin/automed` → `webRoot/../../target/<AUTOMED_CARGO_PROFILE|debug>/automed`（cargo 一档不检查存在）。
  测试 `apps/web/test/core-binary.test.ts`（6 条）。
- `apps/web/server/core/supervisor.ts`（110 行）：`CoreSupervisor`——`start()` 建 `AutomedSidecar`、立刻发一次
  `scheduler.tick` 当握手，成功即 `connected`；子进程退出按 `restart-policy.ts` 重启或转 `offline`；`call` / `tick` /
  `restart`。`TickReport = { skipped?: boolean; errors?: string[]; … }`。
- `apps/web/server/core/sidecar.ts`（320 行）：`spawn(binaryPath, [], …)`，`AUTOME_HOME` 由 supervisor 传入，stderr
  逐行交给 `onStderrLine`，`failureReason()` 取最后几行 stderr。
- `apps/web/server/ticker.ts`（24 行）：每 3 秒一次；`skipped` 时不打 `errors`；tick 抛错时打
  「[调度] tick 失败：…」。`apps/web/server/main.ts`（84 行）：`startServer` 缺省 `binaryPath =
  resolveAutomedBinary().path`，日志走 `console.error`，启动成功打「autome-web 已启动：…」，现在不打内核路径。
- `apps/web/server/gateway.ts`（60 行）：内核错误带 `code` → 409，超时 → 504，其余 → 503 并加「内核未连接：」前缀。
  `apps/web/server/eventbus-protocol.ts`：`mapCoreStatus` 把五种状态映射成 `connected` / `offline` 加原因。
- `apps/web/app/lib/live.tsx`：`liveBanner`（「内核离线：…，写操作暂不可用」）、`writeGate`（只在 `core.status ===
  'connected'` 时放行）、`WriteGateProvider` / `useWriteGate`（各写操作控件据此置灰）。相关测试：
  `live-banner.test.ts`、`live-status.test.ts`、`shell-live.test.ts`、`ticker.test.ts`、`core-supervisor.test.ts`、
  `server.test.ts`。Web 里拉真实内核的测试用 `process.env.AUTOMED_BIN || <仓库>/target/debug/automed`，依赖门禁里
  先跑过的 `cargo test`（`ci-config.test.ts` 断言了这个顺序）。

**安装与打包。**

- `scripts/install-cli.sh`：三条路径——`--from <tar 包或 URL>`（校验 `.sha256`，`unpack` 用
  `find … -name autome -perm -u+x` 取二进制）、在检出里（`cargo build --release -p autome-cli`，取
  `target/release/autome`）、其它地方下载发布包；装到 `${PREFIX:-$HOME/.local/bin}/autome`（先写 `.new.$$` 再 `mv`），
  打印 `"$dest" --version`，最后跑 `"$dest" doctor`。只支持 macOS。
- `scripts/package-cli.sh`：`cargo build --release -p autome-cli`，把 `autome` 与一份 README 放进
  `dist/autome-<version>-<target>/`，打成 tar 包并写 `.sha256`。
- `scripts/package.sh`（桌面端）：`cargo build --release -p automed`，把 `automed` 放进 app 包的
  `Resources/core/`。`scripts/ci-local.sh --package` 才会跑打包相关步骤。
- 本机现状：`~/.local/bin/autome` 存在，`~/.local/bin/automed` 不存在；launchd `com.autome.loop` 运行
  `/Users/dannie/.local/bin/autome run`，`KeepAlive = true`、`ThrottleInterval = 10`，工作目录是主检出，输出写
  `~/.autome/state/loop-driver.log`。

**现有测试夹具（改动时最可能碰到的）。**

- `crates/automed/tests/end_to_end.rs`（9512 行，183 个用例）：`World`（183 行；`Store::open_in_memory()`、
  `Ctx::new(..).headless()`、假 CLI），用例开头 `needs_git!()`。
- `crates/autome-cli/tests/cli.rs`（1629 行，30 个用例）：`Sandbox`（35 行）用 `env!("CARGO_BIN_EXE_autome")` 跑真实
  二进制，`AUTOME_HOME` 指向沙箱，假 CLI 脚本按步执行。
- `crates/automed/src/dispatch.rs` 的单测 `Sandbox`、`scheduler.rs` 的单测 `World`（`recover` 相关用例在 5251、
  10131 行附近）、`store.rs` 的迁移测试与「拒绝更高版本库」的测试。
- `apps/desktop/test/ipc-gate.test.js`：桌面端读白名单必须是内核 `READ_METHODS` 的子集。

## 9. 项目背景摘要（生成于 2026-11-13）

**这是什么。** Autome 2.0：一个驱动「设计 → 评审/裁决 → 实现 → 门禁 → 审计 → 合并」Loop 的本地工具。内核 `automed`
持有全部状态（SQLite 账本 + 事件流），通过 stdio JSON-RPC 服务 Electron 桌面端与 Web 面板；命令行 `autome` 直接链接
内核库、打开同一个账本。本仓库用 Autome 自己开发自己（`.autome/config.toml`：`[loop] parallel = 3`；
`[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、`timeout_minutes = 30`，每轮快检
`quick` = fmt + clippy + Web lint + typecheck、上限 10 分钟；`[merge] auto = true`；`[retry] attempts = 2`）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：
  - `crates/autome-domain`——纯域：状态机（`task.rs`、`bug_loop.rs`）、配置形状（`config.rs`）、状态块解析等。不做 I/O。
  - `crates/automed`——内核（库 + `automed` 二进制）：调度（`scheduler.rs`）、JSON-RPC 分发（`dispatch.rs`）、账本
    （`store.rs`）、驱动锁（`driver_lock.rs`）、门禁、会话启动（`launcher.rs`）、Git（`git.rs`）、内置协议
    （`protocol/`，`include_str!` 仓库根的 `protocol/`）。测试：`tests/end_to_end.rs`、`tests/prompt_rendering.rs`、
    `tests/real_cli.rs`。
  - `crates/autome-cli`——`autome` 命令行（`main.rs`、`render.rs`、`client.rs`、`args.rs`），把 `automed` 当库链接；
    测试 `tests/cli.rs`。
- `apps/web`——Node（>= 22.22）网关（`server/`，含 `core/`、`gates/`、通知器）+ React Router 8 面板（`app/`）；
  vitest、eslint、tsc。
- `apps/desktop`——Electron 外壳，纯 `node --test`（注意：`node --test` 会把 `test/` 下任意 `.js` 当测试跑）。
- `protocol/`——内置协议 seed（本任务不改）；`plugins/`——Claude 插件；`scripts/`——`ci-local.sh`、`install-cli.sh`、
  `package-cli.sh`、`package.sh`。
- 文档：`README.md`（英文）、`docs/development/technical-design.md`（§8.1 驱动锁、§13 持久化、§14 IPC）、
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`（§7.3）、`docs/adr/`。每个任务的文档在 `docs/<slug>/`。

**测试与检查命令（在 worktree 根目录运行）。**

- `cargo test --workspace`；与门禁同款：`cargo test --workspace --all-targets -- --nocapture`
- 单个 e2e：`cargo test -p automed --test end_to_end <用例名>`；CLI 集成：`cargo test -p autome-cli --test cli <用例名>`
- 单元测试：`cargo test -p automed --lib <名字>`、`cargo test -p autome-domain <名字>`
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`
- `npm run lint|typecheck|test|build --prefix apps/web`（单个文件：`npm run test --prefix apps/web -- <文件>`）；
  `npm test --prefix apps/desktop`。新 worktree 第一次要先 `(cd apps/web && npm ci)`。
- 整套本地 CI（即宿主门禁命令）：`sh scripts/ci-local.sh`——每一步都跑完，任一步失败则非零退出。门禁只在合并前由
  宿主跑一次；实现轮 / 审计轮跑里程碑验收命令与直接相关的测试即可。`--package` 见 §6 约束 6。
- 输出长的命令重定向到 `.autome/output/` 只读头尾，例如
  `cargo test -p automed --test end_to_end > .autome/output/e2e.txt 2>&1; tail -20 .autome/output/e2e.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入
  `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/旧内核不能推进新账本-内核带构建标识/protocol/`（见文首指路）。

**环境注意。** macOS（Darwin），系统 `sh` 是 bash 3.2；脚本按 POSIX `sh` 写。e2e 需要本机有 `git`。macOS 对二进制的
签名与文件身份敏感（约束 13）。
