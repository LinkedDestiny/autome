# 合并后钩子 [merge] after：宿主在主检出上执行，失败只报警

status: 实现中
design-round: 1/15
implementation-round: 6/20
current-milestone: M-04
current-milestone-reopens: 0
convergence-mode: normal
next-action: 在允许本机监听与 Electron 启动的环境复验 M-04 全量 CI，核对门禁队列用例后取证待审

任务文件：`docs/合并后钩子-merge-after/合并后钩子-merge-after-task.md`

下文「范围 n」指任务文件 §3 第 n 条，「验收 n」指 §6 第 n 条，「约束 n」指 §5 第 n 条（按出现顺序：1 未配零差异、2 宿主与主检出、3 不阻塞且重启接上、4 超时 124、5 事件顺序、6 分层、7 替身对齐真实接口、8 fmt/clippy、9 仓库规则），「问题 n」指 §7 第 n 条开放问题（1 工作区、2 状态机表示、3 与 `[setup]` 先后、4 MainCheck 失败、5 主检出并发、6 事件形状、7 作业生命周期、8 Bug Loop、9 界面、10 权威文档）。代码位置以分叉点 `b4ea14b` 为准（HEAD `f6bd9bb` 之后只多了任务文档提交，代码相同）。

## 背景与现状

- 本仓库由 Autome 驱动开发。含 `crates/` 的任务合并后，驱动仍跑旧二进制，直到有人手动 `sh scripts/install-cli.sh` 并重启驱动；2026-11-11 漏装一次，T-16 因此被旧 bug 判协议失败。一般项目也需要「合并后做一件事」：部署、重建索引、发通知。
- 项目配置里今天没有任何钩子：`crates/autome-domain/src/config.rs` 与 `crates/automed/src/config_io.rs` 中没有 `[merge]`、hook、after_merge；`config_io.rs`、`config.rs`、README、技术设计文档里也没有 `"merge"` 这个键（已 grep 核对），新表不与现有配置冲突。
- 合并之后的状态机（`autome-domain/src/task.rs` 约 1291–1323 行）：`Merging` 成功后，配了 `[gate]` 进 `MainCheck`，否则直接进 `Cleanup`；`MainCheck` 无论结论如何都进 `Cleanup`（1444 行）；`Cleanup` 成功或失败都进 `Done`。所以 `Cleanup` 是合并后唯一必经的内核节点。
- 内核步骤每个 tick 重入（`scheduler.rs` 约 199–208 行：对每个停在 `is_core_step()` 节点上的任务调 `run_core_step`）。`[setup]`（T-27）已利用这一点：`run_core_step(Cleanup)` 开头（2927–2973 行）先做主检出准备，`Pending` 就返回、任务停在 Cleanup 等下一个 tick，`Failed` 就 `raise_alert("setup_failed")` 并写 `project.alert` 后继续 `cleanup(&layout)`。
- MainCheck（`run_gate`，3009 行起）在 `.worktree/_verify-<sha12>` 现建检出跑门禁，结论写 `gate.result{node:"main_check", ok, …}`；失败只 `raise_alert("main_gate_failed")`（3286 行），任务照常进 Cleanup。两种不跑门禁的情况写 `gate.skipped{node:"main_check"}`：工作区（没有单一合并提交）与 `_verify` 准备失败；门禁在任务停留期间被删出配置时不写事件、直接按通过推进（3054–3058 行）。
- 后台作业（`crates/automed/src/gate.rs`）：`Job::at(repo, task, node, key)` 的四个文件在 `.autome/output/gates/<task>/<node>-<key>.{log,pid,exit,started}`；`start` 以独立进程组跑 wrapper（`sh -c`），`poll_as(job, timeout, what)` 读 exit 标记、超时杀进程组并记 124（`TIMED_OUT`）、日志追加「<what>运行超过 N 分钟，已终止」、无标记无进程判 `Vanished`；`running(job)` 只读判断（含 wrapper 尚未写出 pid 的 10 秒空窗）；`stop_all` 杀该任务目录下所有存活 pid；`discard_node` 按 `<node>-` 前缀杀并删文件。core 重启后靠这些文件接上。
- 停止 / 取消都走 `dispatch.rs::stop_running_session`（1896 行），其中 `gate::stop_all` 与 `discard_node(.., "setup")`。在 Cleanup 上 `Pause` / `Stop` / `Cancel` 都被状态机接受（Cleanup 不 `awaits_user`）。
- `cleanup(&layout)`（3572 行）删任务 worktree 与分支，不动 `.autome/output/gates/`，所以作业日志在任务 Done 后仍在。
- 进入 Cleanup 时 `advance` 写一条 `task.updated{state: Active cleanup}`；Cleanup 结束写 `task.updated{state: done}`。**没有名为 `cleanup` 的事件**。
- 界面：桌面端警报标题 `ALERT_TITLES[alert.kind] || alert.kind`（`apps/desktop/renderer/screens/dashboard.js:114`），web 通知器对非 `main_gate_failed` 的 kind 显示「项目告警：<kind>」（`apps/web/server/notifier-rules.ts:31`）；`apps/` 下没有任何代码按事件 kind 穷举 `setup.*` / `gate.*`（grep 为空）。新警报 kind 与新事件不会让界面坏。
- 基线用例数（`cargo test --workspace --all-targets -- --list`，本轮实测）：autome-cli bin 25、autome-cli tests/cli 13、autome-domain 384、autome-domain tests/t14_gate_logs 1（默认忽略，需原始 T-14 日志）、automed lib 647、automed bin 0、end_to_end 103、prompt_rendering 16、real_cli 7，共 **1196**。现有用例名中含 `merge_after` 的为 0 个，所以本任务新增用例一律以 `merge_after` 入名，过滤 `merge_after` 即得新增数。

## 目标与非目标

目标（范围 1–7，验收 1–6）：

1. 项目 `.autome/config.toml` 可写 `[merge] after` 与 `after_timeout_minutes`。
2. 任务合并成功、MainCheck（若有）通过之后、清理任务 worktree 与分支之前，宿主在主检出 `project.path` 上以后台作业执行该命令，环境变量 `AUTOME_TASK`、`AUTOME_MERGE_COMMIT`。
3. 写 `merge.after_started` / `merge.after` / `merge.after_skipped` 事件，`autome run` 打印；失败（非零退出、超时）只报一条 `after_merge_failed` 项目警报，任务照常 Done。
4. 未配置时与今天逐字节同一行为。

非目标：

- 合并前钩子、其它时机的钩子、多条命令、内置「安装 CLI 并重启驱动」特判（任务 §4）。
- 不改 `[gate]` / `[setup]` 的语义与它们的互斥规则（任务 §4）；不改 `protocol/`（内核行为，会话无需配合）；不改本仓库 `.autome/` 下任何文件（验收 6）。
- 工作区项目不执行钩子（§4 方案 7，给出确定行为）。
- 界面不新增展示（理由见背景最后两条；中文标签进 Backlog B-01）。
- 不检测钩子是否把主检出弄脏（见风险 R2）。

## 方案

### 1. 配置 `[merge]`（范围 1，约束 6）

```toml
[merge]
after = "sh scripts/install-cli.sh"   # 可省；用 sh -c 在主检出根目录跑
after_timeout_minutes = 10            # 可省，缺省 10
```

- 类型（`autome-domain/src/config.rs`，纯类型）：`MergeAfterConfig { command: String, timeout_minutes: u32 }`，`DEFAULT_MERGE_AFTER_TIMEOUT_MINUTES = 10`；`ProjectConfig.merge_after: Option<MergeAfterConfig>`（`#[serde(default, skip_serializing_if = "Option::is_none")]`，与 `setup` 相同）；`ResolvedConfig.merge_after` 在 `resolve()` 里照 `setup` 从项目配置拷贝。项目级，全局配置没有这一节。`ProjectConfig` / `ResolvedConfig` 在现有测试里只经 `..Default::default()` 或 `resolve()` 构造，加字段不改现有测试。
- 解析（`config_io.rs::parse_project`，放在 `[setup]` 与 `[gate]` 之间，规则与二者对齐）：
  - `[merge]` 不是表 → 报错「[merge] 不是一个表」；
  - `after` 缺省 → 等于没配（`merge_after = None`）；给了但不是字符串 → 报错「[merge] after 必须是字符串，例如 after = "make deploy"」；
  - `after` 去首尾空白后为空 → 等于没配，此时 `after_timeout_minutes` 不再校验（与 `[setup]` 空命令一致）；
  - `after_timeout_minutes` 缺省 10；给了必须是正整数，否则报错「[merge] after_timeout_minutes 必须是正整数」；上限截到 `u32::MAX`（同 `[gate]`）。
  - `[merge]` 里不认识的键忽略（与现有各表一致）。
- `save_project` 不需要改：toml_edit 原样保留不认识的表，用 C4 锁住。

### 2. 执行位置与时机：Cleanup 的前置子阶段（范围 2、3，回答问题 2）

- **不新增节点**。钩子是 `run_core_step(Cleanup)` 里排在 `[setup]` 主检出准备之后、`cleanup(&layout)` 之前的一个子阶段：作业在跑时返回、任务停在 Cleanup，下一个 tick 重入再看（与 `[setup]` 主检出准备同一形态）。
- 为什么是 Cleanup：它是合并后唯一必经的内核节点（MainCheck 只在配了 `[gate]` 时存在）；它在 MainCheck 之后、而 worktree / 分支的删除正是 `cleanup()` 做的事，所以「MainCheck 之后、清理之前」在这里天然成立。新增节点会改 `Node::ALL`、界面节点图、迁移与大量穷举测试，违背约束 1「现有测试不改」；Merging 不行（`merge_all` 重入会改写合并提交）；挂在 MainCheck 上则没配 `[gate]` 时不跑。
- **「cleanup 之前」的可观察定义**（约束 5、验收 1）：`merge.after` 的序号大于本任务 `gate.result{node:"main_check"}`（配了 `[gate]` 时）、小于本任务 `state` 为 Done 的那条 `task.updated`；并且钩子运行时任务 worktree 目录仍然存在。仓库里没有名为 `cleanup` 的事件，进入 Cleanup 的 `task.updated` 必然先于钩子（钩子就在这个节点里跑）；任务卡要的是「清理之前」，清理的动作与结果分别是 worktree 删除与 Done，二者都被上面的定义覆盖。E1 同时断言事件序号与「钩子看见 worktree 仍在」。
- 执行：`gate::start(&job, project.path, command, env)`，`job = Job::at(repo, task, "after_merge", <合并提交前 12 位>)`，即 `.autome/output/gates/<task>/after_merge-<sha12>.{log,pid,exit,started}`（范围 3，约束 2）；环境变量 `AUTOME_TASK=<task id>`、`AUTOME_MERGE_COMMIT=<task.merge_commit 全 40 位>`。不新增 wrapper，不改 `gate.rs` 的任何函数。
- 主检出此刻的 HEAD 是合并提交或其后代（并行任务可能在其后又合并），钩子看到的是主检出的当前内容；`AUTOME_MERGE_COMMIT` 告诉它本任务的合并提交是哪个。README 写明这一点。

### 3. 子阶段的完整流程（范围 2–4，约束 1、3、4）

新模块 `crates/automed/src/merge_after.rs`，入口 `phase(ctx, task, project, config: &MergeAfterConfig) -> Result<Phase>`，`Phase = Done | Pending`。`run_core_step(Cleanup)` 在 `resolved.merge_after` 为 `Some` 时调用它，`Pending` → 返回，`Done` → 照旧 `cleanup()`。为 `None` 时一行新代码都不进（约束 1）。

先定义两个只读判定（都在 `merge_after.rs`，均只查本任务）：

- **已结束** `concluded`：本任务已有 `merge.after` 或 `merge.after_skipped` 事件。
- **已准备** `prepared`：本任务目录 `.autome/output/gates/<task>/` 下存在标记文件 `after_merge.prepared`（注意是 `.` 不是 `-`，所以不会被 `discard_node(.., "after_merge")` 的 `after_merge-` 前缀删掉，也不是 `gate::Job` 的文件）。它表示「本任务 Cleanup 上的 `[setup]` 主检出准备段已经处理完」，由 `run_core_step` 在该段以 `Ready` 或 `Failed`（失败警报已写）结束后写下，或本段不存在（没配 `[setup]` / 工作区）时直接写下；准备 `Pending` 时不写。
- **已开始** `begun`：`concluded`，或 `prepared`，或本任务已有 `merge.after_started` 事件，或本任务目录下存在任一 `after_merge-*.started` 作业文件。`begun` 以「准备段是否处理完」为主，而不是「钩子是否启动」——钩子可能因主检出互斥（步骤 4）停在 `Pending`，此时没有任何启动痕迹，但准备段已经处理过（D1-P01）。

`phase` 的步骤：

1. `concluded` → `Done`（不再碰作业、不写事件）。这一条保证钩子在一个任务上至多得出一次结论：作业收完、事件写下之后，若 core 在推进到 Done 之前崩溃，下一 tick 重入不会再跑或再报。
2. 作业存在（`job.exists()`）→ 先**补开始事件**：本任务没有任何 `merge.after_started` 事件时（`gate::start` 成功后、写事件前 core 崩溃的断点，D1-P02），补写一条 `merge.after_started`，字段取自配置、`task.merge_commit` 与作业日志路径；然后 `gate::poll_as(&job, timeout_minutes, "合并后命令")`：
   - `Running` → `Pending`；
   - `Vanished`（机器睡眠、wrapper 被外部杀掉）→ `job.clear()`，返回 `Pending`；下一 tick 走第 4 步重新启动并再写一条 `merge.after_started`（与门禁 `Vanished` 同一处理：结果未知，不算失败）；
   - `Done { exit_code }` → 调 `Store::conclude_merge_after`（新增，`store.rs`，D1-P02）：**一个 SQLite 事务**里依次写 `project_alerts` 行（仅 `exit_code != 0`，含超时 124，kind `after_merge_failed`）、`project.alert { alert_id, kind: "after_merge_failed", task_id }`（与 `setup_failed` / `main_gate_failed` 同形）、最后写 `merge.after`（§5 字段）；要么全部提交，要么全部不提交。因此 `concluded`（有 `merge.after`）必然意味着失败警报已齐全：崩在事务之前 → 没有任何记录，下一 tick 作业文件仍在，重新 `poll` 得到同一结果再收一次，警报恰一条；事务提交后 → `concluded`，不再收集。返回 `Done`。作业文件保留（日志路径要留在事件与警报里）。
3. 计划：`autome_domain::merge_after::plan(workspace, merge_commit.is_some(), verdict)`（纯函数，§4）。`Skip { reason }` → 写 `merge.after_skipped { reason }`、返回 `Done`。
4. `Run` → **主检出互斥**（§6）：别的任务的钩子或主检出准备在跑 → `Pending`，不写事件；否则 `gate::start`、写 `merge.after_started`、返回 `Pending`。`gate::start` 与写事件之间崩溃：下一 tick 走第 2 步，由「补开始事件」补齐，不会重新启动。

`run_core_step(Cleanup)` 的整体顺序变为（只在配了 `[merge] after` 时与今天不同）：

1. `[setup]` 主检出准备（现有代码原样）——**但配了 `[merge] after` 且 `begun` 时整段跳过**（见 §7）；该段以 `Ready` / `Failed` 结束（或本段不存在）后，立刻写 `prepared` 标记，再进入第 2 步；`Pending` 则照旧返回；
2. 钩子子阶段 `phase`；
3. `cleanup(&layout)` → `CoreStepDone` → Done（现有代码原样）。

超时（约束 4）：`[merge] after_timeout_minutes`，`poll_as` 超时杀进程组、写 124、日志追加「合并后命令运行超过 N 分钟，已终止」，按失败处理（报警）。

失败语义（范围 4）：钩子失败或超时只报警，不撤回合并、不让任务失败、不退回任何轮次；任务的状态转移与今天完全相同（`CoreStepDone{Cleanup}` → Done）。警报 detail：

- 非零退出：`任务 T-x 合并后，在主检出里合并后命令 `<command>` 失败（退出码 n）。日志：<log>\n\n<tail 20 行>`
- 超时：`任务 T-x 合并后，在主检出里合并后命令 `<command>` 超时（超过 N 分钟，已终止，退出码 124）。日志：<log>\n\n<tail 20 行>`

### 4. 什么时候跳过：`merge_after::plan`（回答问题 1、4、8，约束 6）

`autome-domain/src/merge_after.rs`（纯函数，无 I/O）：

```rust
pub enum MainVerdict { NotRun, Passed, Failed, Skipped }
pub enum Plan { Run, Skip { reason: &'static str } }
pub fn plan(workspace: bool, has_merge_commit: bool, main: MainVerdict) -> Plan
```

按顺序判断：

| 条件 | 结果 | `reason` |
|---|---|---|
| 工作区项目 | Skip | `工作区项目不支持合并后命令` |
| 没有合并提交 | Skip | `没有单一的合并提交` |
| `main` = `Failed` | Skip | `验证 main 未通过` |
| `main` = `Skipped` | Skip | `验证 main 被跳过` |
| `main` = `NotRun` 或 `Passed` | Run | — |

`MainVerdict` 由 automed 从事件取：`store.last_event_in(task, ["gate.result", "gate.skipped"], "node", "main_check")`——没有 → `NotRun`（没配 `[gate]`，或门禁在停留期间被删）；`gate.result` 且 `ok == true` → `Passed`；`gate.result` 且 `ok != true` → `Failed`；`gate.skipped` → `Skipped`。一个任务至多合并一次、MainCheck 至多得出一次结论（`Vanished` 不写结论），所以「本任务最后一条」就是这次合并的结论。

- **MainCheck 失败时不跑（问题 4）**：任务卡写「验证 main 通过之后」；main 上的提交没过门禁时，部署 / 安装它正是钩子最不该做的事。`main_gate_failed` 警报已经告诉用户 main 有问题，`merge.after_skipped` 与 `autome run` 的「合并后命令跳过：验证 main 未通过」告诉用户钩子没跑；用户修好 main 后自己执行该命令。MainCheck 被跳过（`_verify` 准备失败）同理：main 没有被验证过，不算「通过」。
- **工作区跳过（问题 1）**：工作区按成员仓库各自合并，没有单一合并提交（MainCheck 今天也因此跳过）；`project.path` 是装着若干仓库的目录，不是一个检出；「按成员各跑一次」要回答哪个成员的提交、未改动成员跑不跑、命令在哪个目录跑，而配置只有一条命令，无论怎么选都是替用户猜。选跳过：每个任务恰好一条 `merge.after_skipped { reason: "工作区项目不支持合并后命令" }`（由 `concluded` 保证不重复），任务其余行为与没配相同。与 `[setup]` 的 `setup.skipped` 同形。不在解析时拒绝：解析不知道项目是不是工作区。按成员的钩子进 Backlog B-02。
- **Bug Loop 任务同样执行（问题 8）**：Bug 任务也经 `Merging → [MainCheck] → Cleanup` 合并到 main，「合并后需要做的事」（装新二进制、部署）与任务类型无关。子阶段按节点触发，不看 `kind`；E7 锁住。

### 5. 事件与 `autome run`（范围 4，回答问题 6）

| 事件 | 字段 | `autome run` 打印（`crates/autome-cli/src/render.rs::event_line`） |
|---|---|---|
| `merge.after_started` | `command, commit, log` | `T-x 合并后命令开始：<command>  日志：<log>` |
| `merge.after` | `ok, exit_code, command, commit, log, tail` | `T-x 合并后命令通过（退出码 0）  日志：<log>` / `T-x 合并后命令未通过（退出码 n）  日志：<log>` |
| `merge.after_skipped` | `reason` | `T-x 合并后命令跳过：<reason>` |

- 结果事件 `merge.after` 含任务卡给的 `{ok, exit_code, log, tail}`，另加 `command`、`commit`（全 40 位合并提交）便于事后核对是哪条命令、哪次合并；`tail` 取日志末 20 行（与 `setup.result` 一致）。
- 任务卡要求「开始、结果两行」：开始行需要一个事件驱动，所以新增 `merge.after_started`（与 `setup.started` / `gate.started` 同一做法）。跳过时只有一行跳过。
- 事件都写在任务名下；警报的 `project.alert` 事件写在项目名下（现有做法）。

### 6. 主检出的并发（回答问题 5，约束 3）

- 并行任务（`[loop] parallel`）可能先后进入 Cleanup。两个钩子同时在主检出上跑（例如两次 `cargo install`）会互相踩；钩子又常依赖主检出准备装好的依赖。所以 §3 第 4 步启动前检查：`.autome/output/gates/` 下**其它任务**目录里，任一 `after_merge-*` 作业或 `setup-main` 作业 `gate::running(job)` 为真 → 本任务返回 `Pending`、不写事件，下一 tick 再看。`running` 只读、含 wrapper 启动空窗、作业一结束（有 exit 标记或进程已死）即不再占用，不依赖对方任务是否已收结果。
- 只在钩子一侧加等待，不改 `[setup]` 现有的「只等别人的 `setup-main`」规则（任务 §4「不改 `[setup]` 语义」）。剩余的窗口——别的任务在本任务钩子运行期间开始主检出准备——见风险 R4。
- 互斥只靠文件、每 tick 一次 `read_dir` 加若干 stat，不阻塞 tick / IPC。

### 7. 与 `[setup]` 的先后（回答问题 3）

- **准备在前，钩子在后**：钩子常要用准备装好的依赖（例如 `npm ci` 之后再构建部署）；反过来，钩子不会为准备提供什么。
- **准备失败时钩子照跑**：`setup_failed` 警报已经单独报出；准备与钩子是用户分别配置的两件事，很多钩子（本仓库的 `install-cli.sh` 只需要 cargo）与准备的依赖无关。若钩子确实需要那些依赖，它会失败并再报一条 `after_merge_failed`——两条警报都如实反映主检出的状态，比静默不跑可查。
- **准备段处理完后不再进入准备**（§3 顺序第 1 条）：今天准备失败后同一次调用里就 `cleanup()`，准备在一个任务的 Cleanup 上至多跑一次；加了钩子之后任务会在 Cleanup 上多停几个 tick（钩子在跑，或被别的任务的钩子/准备互斥挡在 `Pending`），若每个 tick 都先 `ensure(main)`，准备失败（不写 `main.fp`）会在每个 tick 重跑并重复报警，准备成功而期间别的任务改了 `watch` 也会重新准备。所以以 `prepared` 标记为界（它在准备段结束的那一次调用里就落盘，早于钩子是否启动，因此覆盖「钩子因互斥而 Pending」的窗口）：标记一旦存在，本任务的 Cleanup 不再调用准备。准备本身的判定、记录、互斥一概不动。只有「失败警报写下、标记落盘」之间崩溃才会让准备在重启后再跑一次——与今天「警报写下、`cleanup()` 完成之前崩溃」的窗口同形，不新增。E5 锁住钩子无需等待的场景，L5 锁住互斥等待场景（D1-P01）。

### 8. 作业生命周期（回答问题 7，约束 3）

| 情形 | 处理 |
|---|---|
| 暂停（Cleanup 上 `Pause`） | 作业在后台继续；恢复后 `Requeued → Active{Cleanup}`，tick 重入 `phase`，第 2 步收同一个作业（与门禁 / 主检出准备相同） |
| 停止（`Stop`） | `stop_running_session` 已有 `gate::stop_all` 杀进程组；新增一行 `discard_node(repo, task, "after_merge")` 删作业文件。恢复后 `begun`（已有 `merge.after_started`）→ 不重新准备；作业不存在 → 重新启动钩子，第二条 `merge.after_started`。被杀那次没有结论事件 |
| 取消（`Cancel`） | 同样经 `stop_running_session` 杀掉并删文件；任务 Cancelled，不写结论事件、不报警（用户主动取消） |
| core 重启 | 作业是独立进程组，文件都在；tick 重入 `phase`，第 2 步（补开始事件后）`poll` 接上，不会重新启动（钩子若自己重启了驱动，例如安装新 CLI 后 `launchctl kickstart`，新驱动按同一方式接上）。`prepared` 标记也在文件里，重启后不会重新准备 |
| 作业消失（`Vanished`） | 清作业、下一 tick 重启，见 §3 第 2 步 |
| 结论已写、推进 Done 前崩溃 | `concluded` → 直接 `cleanup()`，不重跑、不重报（L2 锁住）；结果事件与失败警报同事务，不存在「结果在、警报缺」（S1 锁住） |
| `gate::start` 后、开始事件前崩溃 | 作业存在、无 `merge.after_started` → 第 2 步补一条（L4 锁住），不重新启动 |
| 停留期间配置删掉了 `[merge] after` | 视为没配：子阶段不再进入、直接清理；在途作业在后台跑完，不收结果（与「门禁在停留期间被删出配置」同一处理） |

### 9. 没配 `[merge] after` 时零差异（约束 1，验收 3）

`run_core_step` 的新分支以 `resolved.merge_after.is_some()` 为第一判断；为 `None`（没有 `[merge]`、没有 `after`、`after` 为空白）时：不查事件、不 stat 作业、不写 `prepared` 标记、不写事件、`[setup]` 段按今天的代码原样执行、Cleanup 在同一次调用里完成，不新增节点停留。`stop_running_session` 新加的 `discard_node(.., "after_merge")` 在没有此类文件时是空操作（`discard_node` 对不存在的目录返回 `Ok`，对不匹配前缀的文件不动）。现有用例一个不改，E3 断言没有任何 `merge.after*` 事件与 `after_merge-*` 文件。

### 10. 分层与改动清单（约束 6）

- `autome-domain`：`config.rs`（`MergeAfterConfig`、默认值、两个字段、`resolve()` 一行）；新模块 `merge_after.rs`（`MainVerdict`、`Plan`、`plan`）。无 I/O、无 async、无 SQLite。
- `automed`：`config_io.rs`（解析）；`store.rs`（新增 `conclude_merge_after`：一个事务写失败警报、`project.alert`、`merge.after`；既有方法不动）；新模块 `merge_after.rs`（`phase`、`concluded`、`prepared`、`begun`、`verdict`、互斥扫描、补开始事件）；`scheduler.rs` 的 `run_core_step` 一处接入；`dispatch.rs::stop_running_session` 一行；`lib.rs` 注册模块。
- `autome-cli`：`render.rs` 三种事件的打印。
- 文档：README `[gate]` 示例块与说明条目；`docs/development/technical-design.md` §3.3、§8.1、§9（回答问题 10：这三节分别列配置文件内容、宿主作业与 Cleanup 步骤、解析规则，`[setup]` 当时也同步写进去了，不同步会让权威文档与代码不一致）。
- 不改：`gate.rs`、`setup.rs`、`task.rs` 状态机、`store.rs` 的既有方法、`protocol/`、`apps/`（问题 9：界面对新 kind 已有回退，见背景）、本仓库 `.autome/`。

### 11. 否决的做法

- **新增 `AfterMerge` 节点**：改 `Node::ALL`、节点图、迁移与穷举测试，违背约束 1；且它与 Cleanup 之间没有用户可见的区别。
- **钩子在 MainCheck 里跑**：没配 `[gate]` 就没有 MainCheck。
- **按工作区成员各跑一次**：见 §4，配置只有一条命令，各种取法都是替用户猜。
- **MainCheck 失败照跑**：与任务卡「验证 main 通过之后」相反，且会把没通过门禁的 main 部署出去。
- **准备失败跳过钩子**：把两件独立配置的事绑在一起，本仓库的主用例（装 CLI）会因无关的 `npm ci` 失败而静默不跑。
- **以 `[setup]` 的互斥同时等钩子**：改变 `[setup]` 语义，任务 §4 禁止。

## 风险与验证安排

- R1 「cleanup 之前」被理解为「进入 Cleanup 节点之前」：§2 给出可观察定义并说明为什么在 Cleanup 内（任务文件问题 2 自己也把「`Cleanup` 步骤内的前置子阶段」列为候选）；E1 用钩子在运行时记录 worktree 是否存在，直接证明钩子发生在清理动作之前。
- R2 钩子改动主检出的被跟踪文件：主检出变脏，下一次合并被「主工作树不干净」挡住。README 写明钩子应只写被 gitignore 的产物（或主检出之外）；不做检测（Backlog B-03）。E1 断言主检出仍干净。
- R3 钩子运行期间任务占着并行槽、worktree 推迟到钩子结束才删：与门禁 / 主检出准备相同，接受；超时缺省 10 分钟封顶。
- R4 钩子运行期间别的任务开始主检出准备（`[setup]` 的互斥只看 `setup-main`）：两者可能同时在主检出上跑。不改 `[setup]` 语义就无法关闭这个窗口；后果只是两件宿主作业并发，失败照常报警。进 Backlog B-04。
- R5 被暂停任务的在途钩子挂住时，要等任务恢复（`poll` 才会按超时杀）才释放互斥，期间阻塞别的任务的钩子（不阻塞其它任何事）。与 `[setup]` 的 B-05 同类，进 Backlog B-05。
- R6 本仓库是主要受益者，但本任务不改 `.autome/config.toml`（验收 6），由用户合并后自行配置；M-04 的验收命令核对 `.autome/` 相对分叉点无改动。
- 验证安排：每个里程碑的验收命令跑整个受影响的测试目标并报总数与基线差，再按 `merge_after` 过滤报新增数；e2e 用真实 git、真实 wrapper、真实 `sh -c`，只替换模型 CLI（约束 7），沿用 `World` / `write_gate_script` / `run_gated_task` / `setup_fixture` / `Workspace` / Bug Loop 夹具。钩子脚本写在仓库外：把 `$PWD|$AUTOME_TASK|$AUTOME_MERGE_COMMIT|wt=<present|absent>` 追加到 `<sandbox>/hook-runs`，在主检出里写被 gitignore 的 `hook-out/stamp`，按需以指定退出码失败或 `sleep`。最后一个里程碑跑 `sh scripts/ci-local.sh`（fmt、clippy、全部 Rust 测试、desktop、web；约束 8）。
- 新增用例合计 23 个：domain 4、automed lib 10、end_to_end 8、autome-cli 1；全工作区由 1196 到 **1219**。若实现期 rebase 带进 main 的新用例，总数相应增加，过滤 `merge_after` 的数不变；证据里分别报两者。

验收与里程碑的对应：验收 1 → M-02 E1；验收 2 → M-02 E2；验收 3 → M-02 E3；验收 4 → 每个里程碑跑整个目标、M-04 的 ci-local；验收 5 → M-04；验收 6 → M-04。约束 1 → E3；约束 3 → L2、L3、L4、L5、S1、E8；约束 4 → L1；约束 5 → E1；约束 6 → M-01；约束 7 → M-02、M-03；约束 8 → M-04。问题 1 → E6、D4；问题 3 → E5、L5；问题 4 → E4、D3；问题 5 → L3；问题 7 → L2、L4、S1、E8；问题 8 → E7。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | [merge] 配置解析、MergeAfterConfig 与跳过判定 plan（domain + config_io） | 0 | |
| M-02 | 已完成 | Cleanup 前置钩子作业：主检出执行、结果事件与警报、超时、重入与零差异、run 打印 | 0 | |
| M-03 | 已完成 | 跳过与协作：MainCheck 失败、与 [setup] 先后、工作区、Bug 任务、主检出互斥、停止与恢复 | 0 | |
| M-04 | 开放 | README 与技术设计文档、.autome 未改核对、全量回归 | 0 | |

## 验收契约

### M-01 验收契约

最新证据：docs/合并后钩子-merge-after/evidence/M-01-r2-audit.md · 通过 · 审计 #1

覆盖范围 1，约束 6，问题 1、4 的纯判定部分；前置：无。新增用例（名字都含 `merge_after`）：
autome-domain：D1 `merge_after_config_is_carried_into_the_resolved_config`（有与无两种）、D2 `merge_after_runs_when_the_main_check_passed_or_was_not_configured`（`NotRun`、`Passed` → `Run`）、D3 `merge_after_is_skipped_when_the_main_check_failed_or_was_skipped`（`Failed` → 「验证 main 未通过」，`Skipped` → 「验证 main 被跳过」）、D4 `merge_after_is_skipped_in_a_workspace_or_without_a_merge_commit`（工作区配四种 verdict 都是「工作区项目不支持合并后命令」；单仓无合并提交 + `Passed` → 「没有单一的合并提交」）。
automed lib `config_io.rs`：C1 `merge_after_section_is_read_with_command_and_timeout`（`after` 去空白、`after_timeout_minutes = 3`）、C2 `merge_after_section_defaults_to_10_minutes_and_a_blank_or_missing_after_means_none`（缺 `after_timeout_minutes` → 10；`after = "  "` 且 `after_timeout_minutes = 0` → `None` 不报错；`[merge]` 空表 → `None`）、C3 `merge_after_section_refuses_a_non_table_a_non_string_after_or_a_bad_timeout`（`merge = 1`、`after = 3`、`after_timeout_minutes` 为 0 / -1 / `"10"`，各得对应报错文字）、C4 `merge_after_section_survives_save_project`。

- 验收命令：`cargo test -p autome-domain > .autome/output/m01-domain.txt 2>&1; tail -3 .autome/output/m01-domain.txt; cargo test -p automed --lib > .autome/output/m01-lib.txt 2>&1; tail -3 .autome/output/m01-lib.txt; cargo test -p autome-domain merge_after 2>&1 | grep 'test result'; cargo test -p automed --lib merge_after 2>&1 | grep 'test result'`
- 预期：autome-domain 388 个通过（基线 384 + D1–D4），automed lib 651 个通过（基线 647 + C1–C4），0 失败；过滤 `merge_after` 在 domain 得 4 个、在 automed lib 得 4 个。现有用例一个不改。
- 负向对照：`resolve()` 不拷贝 `merge_after` → D1 失败；`Failed` 判成 `Run`（只看「有没有跑过 MainCheck」）→ D3 失败；先判 verdict 再判工作区 → D4 的「工作区 + Passed」得 `Run` 而失败；缺省超时写成 30（抄 `[setup]`）→ C2 失败；空白 `after` 被当成命令或仍校验超时 → C2 失败；`[merge] = 1` 被静默接受 → C3 失败。

### M-02 验收契约

最新证据：docs/合并后钩子-merge-after/evidence/M-02-r3-audit.md · 通过 · 审计 #2

覆盖范围 2、3、4（结果、警报、打印），验收 1、2、3，约束 1、2、3、4、5、7，问题 2、6；前置：M-01。新增用例：
end_to_end：E1 `merge_after_runs_in_the_main_checkout_after_the_main_check_and_before_cleanup`（沿用 `run_gated_task_with(..., false, Some(false))` 配 `[gate] reuse_for_main_check = false`，确保实际重跑 MainCheck，再配 `[merge] after = "sh <hook.sh>"`；`task.merge` 后 settle：任务 `done`；主检出有 `hook-out/stamp`；`hook-runs` 恰 1 行，`$PWD` 等于主检出的规范路径、`AUTOME_TASK` 等于任务 id、`AUTOME_MERGE_COMMIT` 等于 `task.merge_commit`、`wt=present`；序号 `gate.result{node:"main_check",ok:true}` < `merge.after_started` < `merge.after{ok:true,exit_code:0}` < state 为 Done 的 `task.updated`；`merge.after.log` 文件存在且 `tail` 含脚本输出；主检出 `git::is_clean`；无警报）、E2 `merge_after_failure_raises_one_alert_and_the_task_is_still_done`（钩子 `exit 6`：任务 `done`，任务 worktree 与分支已删；打开的警报恰 1 条、kind `after_merge_failed`、detail 含命令、「退出码 6」与 `after_merge-` 日志路径；`merge.after{ok:false,exit_code:6}` 恰 1 条）、E3 `merge_after_events_are_absent_when_no_merge_after_is_configured`（两种夹具都跑到 `done`：不写 `[merge]`；写 `[merge]\nafter = "  "`。`merge.after_started` / `merge.after` / `merge.after_skipped` 计数均为 0，没有 `after_merge_failed` 警报，`.autome/output/gates/<task>/` 下没有 `after_merge-*` 文件）。
automed lib `scheduler.rs` 测试（沿用 `setup_in_the_main_checkout_waits_while_another_tasks_main_setup_is_running` 的 World、Cleanup 任务与直接调 `run_core_step` 的做法）：L1 `merge_after_past_its_timeout_is_killed_with_124_and_reported_as_a_failure`（`after = 'sleep 30'`、`after_timeout_minutes = 1`；看到 pid 后把 `.started` 改写为 `1`，循环 `run_core_step` 直到 Done：`merge.after{ok:false,exit_code:124}`、警报 1 条且 detail 含「超时」、日志含「合并后命令运行超过 1 分钟」、`sleep` 进程已不存活）、L2 `merge_after_in_flight_is_waited_for_collected_once_and_never_rerun`（`after = 'sleep 1; echo hook-done'`：第一次调用后 `merge.after_started` 1 条、任务仍在 Cleanup、worktree 仍在；作业未完时再调，仍在 Cleanup、`merge.after_started` 仍 1 条——同一 tick 重入即 core 重启后的接上路径；循环到 Done 后 `merge.after` 1 条；再把任务状态改回 `Active{Cleanup}`（模拟结论已写、推进前崩溃）调一次：任务 Done、`merge.after_started` 与 `merge.after` 仍各 1 条、无警报）、S1 `merge_after_failure_alert_and_result_event_are_written_in_one_transaction`（`store.rs` 测试：调 `conclude_merge_after` 失败路径，`project_alerts` 恰 1 条 `after_merge_failed`、`project.alert` 与 `merge.after` 各 1 条；再在 `events` 表装一个只拦 `merge.after` 的 `BEFORE INSERT` 触发器（沿用 `reject_brief` 的做法）重试：返回错误，且 `project_alerts` 与 `project.alert` 没有新增——证明不会留下「警报在、结果缺」或相反）、L4 `merge_after_started_event_is_backfilled_when_the_job_exists_without_one`（直接对 Cleanup 任务 `gate::start` 一个 `after_merge-<sha12>` 作业、不写任何事件，模拟 `start` 后崩溃；调 `run_core_step`：`merge.after_started` 恰 1 条且 command / commit / log 正确，进程只有一个——没有重新启动；循环到 Done 后 `merge.after` 1 条）。
autome-cli `render.rs`：R1 `merge_after_events_are_printed_by_autome_run`（开始、通过、未通过、跳过四行；含命令、退出码、日志路径与跳过原因）。

- 验收命令：`cargo test -p automed --test end_to_end > .autome/output/m02-e2e.txt 2>&1; tail -3 .autome/output/m02-e2e.txt; cargo test -p automed --lib > .autome/output/m02-lib.txt 2>&1; tail -3 .autome/output/m02-lib.txt; cargo test -p autome-cli > .autome/output/m02-cli.txt 2>&1; grep 'test result' .autome/output/m02-cli.txt; cargo test -p automed --test end_to_end merge_after 2>&1 | grep 'test result'; cargo test -p automed --lib merge_after 2>&1 | grep 'test result'`
- 预期：end_to_end 106 个通过（基线 103 + E1–E3），automed lib 655 个通过（M-01 后 651 + L1、L2、S1、L4），autome-cli 两个目标 26 + 13 个通过（bin 基线 25 + R1），0 失败；过滤 `merge_after` 在 e2e 得 3 个、在 automed lib 得 8 个。
- 负向对照：钩子放在 `cleanup()` 之后或 Done 之后 → E1 的 `wt=absent` 且序号断言失败；钩子在任务 worktree（`layout.cwd`）里跑 → E1 的 `$PWD` 与 `hook-out/stamp` 位置不对；`AUTOME_MERGE_COMMIT` 给成任务分支 head → E1 失败；失败时让任务 `Failed` 或退回 → E2 状态不是 `done`；失败时报两次警报（收集后不靠 `concluded` 防重入）→ E2 / L2 计数为 2；未配也写事件或在 Cleanup 多停（比如每次都启动作业）→ E3 失败；超时用缺省 30 或没走 `poll_as` → L1 一直 Running 而超时断言失败；每次重入都重新 `start` → L2 的 `merge.after_started` 为 2；只靠作业文件判断「已结束」而收完删文件 → L2 最后一段重新启动钩子；警报与结果事件分开写（先结果后警报，或各自独立事务）→ S1 的触发器重试后留下孤立警报或结果；作业已存在时不补开始事件 → L4 的 `merge.after_started` 为 0；补事件的同时又重新 `start` → L4 出现两个进程。

### M-03 验收契约

最新证据：docs/合并后钩子-merge-after/evidence/M-03-r4-audit.md · 通过 · 审计 #3

覆盖范围 5，问题 1、3、4、5、7、8，约束 3、7；前置：M-02。新增用例：
end_to_end：E4 `merge_after_is_skipped_when_the_main_check_failed`（`run_gated_task_with(..., false, Some(false))` 显式关闭 MainCheck 复用 + 合并前在沙箱放 `main-should-fail`，沿用 `write_gate_script` 的 `_verify` 失败分支：任务 `done`；`main_gate_failed` 警报 1 条、`after_merge_failed` 0 条；`merge.after_skipped{reason:"验证 main 未通过"}` 恰 1 条；`merge.after_started` 0 条；`hook-runs` 不存在）、E5 `merge_after_runs_after_the_main_checkout_setup_and_even_when_it_failed`（沿用 `setup_fixture` 配 `[setup]` 与 `[merge] after`，钩子脚本先 `sleep 1` 以确保至少一次 Pending 重入，并把主检出有没有 `deps/stamp` 记进 `hook-runs`。两段：a 准备成功——`setup.result{checkout:"main"}` 序号 < `merge.after_started`，钩子记录 `stamp=yes`；b 准备在主检出 `exit 4`——`setup_failed` 警报恰 1 条、`merge.after{ok:true}` 1 条、`setup-runs` 里主检出路径的行恰 1 行）、E6 `merge_after_is_skipped_in_a_workspace_with_one_event_per_task`（沿用 `Workspace` 夹具配 `[merge] after`：任务 `done`；`merge.after_skipped{reason:"工作区项目不支持合并后命令"}` 恰 1 条；`merge.after_started` 0 条；`hook-runs` 不存在）、E7 `merge_after_runs_for_a_bug_task_too`（沿用 `bug_loop_a_bug_runs_from_creation_to_a_merge_commit` 的步骤配 `[merge] after`，`task.merge` 后：任务 `done`、`merge.after{ok:true}` 1 条、`hook-runs` 1 行含该任务 id）、E8 `merge_after_stopped_by_the_user_is_killed_and_runs_again_on_resume`（钩子先把自己的 pid 写进沙箱，沙箱有 `slow` 时 `sleep 30`；看到 `merge.after_started` 后 `task.stop`：该 pid 不再存活、`gates/<task>/after_merge-*` 无文件、任务 `stopped`；删 `slow`、`task.resume` 后 settle：`merge.after_started` 2 条、`merge.after{ok:true}` 1 条、无警报、任务 `done`）。
automed lib `scheduler.rs`：L3 `merge_after_waits_while_another_tasks_hook_or_main_setup_is_running`（对两种「别人的作业」各跑三段，均对本任务调一次 `run_core_step(Cleanup)`：①另一任务 `T-2` 目录下只有刚写下的 `after_merge-<x>.started`（或 `setup-main.started`）、没有 pid——仍在 Cleanup、没有 `merge.after_started`；② pid 指向存活 `sleep`——同样等待；③杀掉并写 `.exit` 标记——本任务写 `merge.after_started`）、L5 `merge_after_waiting_for_another_hook_does_not_rerun_a_failed_main_setup`（D1-P01：T-1 在 Cleanup，配 `[setup]`（命令把一行追加进 `setup-runs` 后 `exit 4`）与 `[merge] after`；T-2 目录下有存活 `sleep` 的 `after_merge-<x>` 作业。连续对 T-1 调 `run_core_step(Cleanup)` 四次：每次都仍在 Cleanup；`setup-runs` 里主检出行恰 1 行，`setup_failed` 警报恰 1 条，`gates/<task>/after_merge.prepared` 存在，没有 `merge.after_started`。再杀掉 T-2 的 sleep 并写 `.exit`：下一次调用写 `merge.after_started`，循环到 Done 后 `merge.after{ok:true}` 1 条，`setup-runs` 与 `setup_failed` 仍各 1）。

- 验收命令：`cargo test -p automed --test end_to_end > .autome/output/m03-e2e.txt 2>&1; tail -3 .autome/output/m03-e2e.txt; cargo test -p automed --lib > .autome/output/m03-lib.txt 2>&1; tail -3 .autome/output/m03-lib.txt; cargo test -p automed --test end_to_end merge_after 2>&1 | grep 'test result'; cargo test -p automed --lib merge_after 2>&1 | grep 'test result'`
- 预期：end_to_end 111 个通过（M-02 后 106 + E4–E8），automed lib 657 个通过（655 + L3、L5），0 失败；过滤 `merge_after` 在 e2e 得 8 个、在 automed lib 得 10 个。
- 负向对照：MainCheck 失败照跑钩子 → E4 出现 `merge.after_started` 与 `hook-runs`；钩子排在准备之前 → E5a 的序号反了、钩子记录 `stamp=no`；准备失败就跳过钩子 → E5b 没有 `merge.after`；钩子开始后每次重入仍先 `ensure(main)` → E5b 的 `setup_failed` 警报 2 条以上、主检出准备行 2 行以上；工作区按成员去跑或每次重入都写跳过 → E6 计数不对；按任务类型排除 Bug → E7 没有 `merge.after`；停止时不 `discard_node("after_merge")` → E8「停止后 `after_merge-*` 无文件」的断言失败；不检查别的任务 → L3 ①② 出现 `merge.after_started`；互斥只看 pid 文件 → L3 ① 出现 `merge.after_started`；互斥不看 `setup-main` → L3 的准备那一组 ①② 失败；以「钩子是否已启动」而非 `prepared` 标记判定是否重入准备 → L5 的 `setup-runs` 多于 1 行、`setup_failed` 多于 1 条。

### M-04 验收契约

最新证据：docs/合并后钩子-merge-after/evidence/M-04-r6-impl.md · rebase 完成，集成复验受阻，保持开放 · 实现 #6

覆盖范围 6、7，验收 4、5、6，约束 8、9，问题 10；前置：M-03。改动：README 的 `[gate]` 配置示例块加 `[merge]` 两行，其下条目列表在 **Preparation** 之后加一条 **After merge**（时机、主检出、环境变量、超时缺省 10、失败只报警、MainCheck 未通过与工作区跳过、钩子只应写被 gitignore 的产物、停止后恢复会重跑）；`docs/development/technical-design.md` §3.3 配置示例加 `[merge]`、§8.1 加钩子作业与 Cleanup 前置子阶段、§9 加解析规则。不新增用例。

- 验收命令：`grep -c '\[merge\]' README.md docs/development/technical-design.md; grep -c 'AUTOME_MERGE_COMMIT' README.md; git diff --exit-code $(git merge-base HEAD main) -- .autome/ && echo AUTOME-DIR-UNCHANGED; cargo test --workspace --all-targets > .autome/output/m04-all.txt 2>&1; echo "test exit $?"; grep 'test result' .autome/output/m04-all.txt; sh scripts/ci-local.sh > .autome/output/m04-ci.txt 2>&1; echo "ci exit $?"; tail -20 .autome/output/m04-ci.txt`
- 预期：README 与技术设计文档里 `[merge]` 各至少 1 处，README 里 `AUTOME_MERGE_COMMIT` 至少 1 处；打印 `AUTOME-DIR-UNCHANGED`；`test exit 0`，各目标用例总数为 1219，其中 1218 通过、1 忽略（t14_gate_logs 原有可选检查，需 AUTOME_T14_GATE_DIR 指向原始 T-14 日志；基线 1196 + 本任务 23：domain 4、automed lib 10、e2e 8、cli 1；rebase 带进的 main 用例另计并在证据中列出）；`ci exit 0`，ci-local 的 Format、Clippy、Rust 测试、Desktop、Web 各步全部通过。
- 负向对照：README 只改了说明条目、没改示例块（或反之）→ `[merge]` 或 `AUTOME_MERGE_COMMIT` 计数为 0；会话改了本仓库 `.autome/config.toml` 去试钩子 → `git diff --exit-code` 非零、不打印 `AUTOME-DIR-UNCHANGED`；新代码有未格式化或 clippy 告警 → ci-local 的 Format / Clippy 步失败、`ci exit` 非零。

## 人工验收清单

- H-01 合并后由用户在本仓库 `.autome/config.toml` 配 `[merge] after = "sh scripts/install-cli.sh"`，再合并一个改了 `crates/` 的任务：`autome run` 里能看到「合并后命令开始」与「通过」两行，`~/.local/bin/autome` 的修改时间晚于合并时刻。需要用户的真实驱动与安装目录，会话内无法取证。
- H-02 让一次钩子失败（例如 `after = "exit 3"`），在桌面端仪表盘与 web 面板上确认：出现一条 `after_merge_failed` 警报（目前显示原始 kind），任务卡片显示已完成、可以忽略该警报（文案改进见 B-01）。

## Backlog

- B-01 桌面端 / web 面板为警报 `after_merge_failed` 加中文标题，并在任务时间线展示 `merge.after*` 事件（本任务不改界面，见非目标）。
- B-02 工作区项目的合并后命令：按成员仓库配置各自的命令，在各成员的主检出上运行。
- B-03 钩子跑完后检测主检出被跟踪文件是否变化，变了就写警告事件，提前暴露风险 R2。
- B-04 让 `[setup]` 的主检出准备也等待别的任务的 `after_merge-*` 作业（风险 R4）；需要另开任务，因为它改变 `[setup]` 的互斥语义。
- B-05 被暂停任务的在途钩子超时后要等任务恢复才会被杀、才释放主检出互斥（风险 R5）；可让互斥判定顺带按超时放行。
- B-07 M-04 的文档核对只统计 `README.md` 里 `[merge]` / `AUTOME_MERGE_COMMIT` 的出现次数，鉴别力不足以证明「示例块与条目都改了」；实现 / 审计时直接读示例块与条目核对，或把验收命令改成按段落匹配（评审 N1）。
- B-06 MainCheck 未通过而跳过钩子时，把「合并后命令未执行」追加进 `main_gate_failed` 警报的 detail，让用户在仪表盘上一眼看到。

## 争议项
