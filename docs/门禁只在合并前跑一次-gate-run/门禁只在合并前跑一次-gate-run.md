# 门禁只在合并前跑一次（`[gate] run = "before_merge"`）

status: 实现中
design-round: 1/15
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01：在 autome-domain 加 GateRun 与实现轮结束后的转换，在 config_io 解析与校验 [gate] run

任务文件：`docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run-task.md`

任务卡：`docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run-card.md`

下文「目标 n」指任务文件 §2 第 n 条，「验收 n」指 §3 第 n 条，「约束 n」指 §5 第 n 条。代码位置以本 worktree 的基线提交
`28dad46` 为准，行号会漂移，以符号名为准。

## 背景与现状

- 配了 `[gate]` 的需求任务每个实现轮之后跑一次完整门禁（`Gate`），rebase 之后再跑一次（`MergeGate`），合并之后又跑一次
  （`MainCheck`）。Bug 任务每个修复轮之后跑 `( 复现命令 ) && ( 门禁命令 )`（`FixGate`），合并前再跑一次。2026-11-12 一天
  102 次门禁、18.6 小时机器时间；并行 3 个任务时多个 `ci-local` 互相抢占，负载下的超时误报又把任务退回实现轮。
- **配置。** `autome-domain/src/config.rs`：`GateConfig { command, timeout_minutes }`；`ProjectConfig.gate: Option<GateConfig>`
  （`None` = 没有门禁）；`same_failure_limit` 是旁挂字段 `ProjectConfig.gate_same_failure_limit: Option<u32>` →
  `ResolvedConfig.gate_same_failure_limit: u32`。测试里所有 `ProjectConfig { .. }` 字面量都带 `..Default::default()`，
  `ResolvedConfig` 全部经 `resolve()` 或 `ResolvedConfig { roles, ..c }` 构造；`GateConfig { .. }` 字面量有 5 处
  （`task.rs`、`bug_loop.rs`、`guards.rs`、`config_io.rs`、`scheduler.rs` 各一处）。
- **配置读写。** `automed/src/config_io.rs::parse_project` 的 `[gate]` 段：先校验 `same_failure_limit`，再取 `command`
  （缺失报错；去空白后为空 = 没有门禁），再取 `timeout_minutes`。`parse_global` 只读 `[loop]` 与 `[roles]`，全局文件里的
  `[gate]` 整张表被忽略。`save_project` 用 `toml_edit` 只改 `[loop]`、`[roles]`、`[bug]`，`[gate]` 不碰。
- **需求状态机。** `task.rs` 有三处 `ctx.config.gate.is_some()`：实现轮结束（`Node::Implement` 分支，约第 1152 行）→
  `Gate`；rebase 成功（约第 1262 行）→ `MergeGate`；合并成功（约第 1293 行）→ `MainCheck`。`after_gate`：
  `(Gate, Passed)` → 审计或 `after_implement_without_audit`；`(Gate|MergeGate, Failed)` → 回实现并注入
  `Inject::GateFailed`；`(MergeGate, Passed)` → `AwaitMerge`（已请求合并时直接 `press_merge`）。审计结束时全部里程碑已完成
  → `after_the_loop`（复盘开着进复盘，否则直接 rebase）。
- **Bug 状态机。** `bug_loop.rs`：`Node::Fix => core(Node::FixGate)`；FixGate / MergeGate 失败 → `start_fix` 注入
  `GateFailed`；FixGate 通过 → 复核。
- **调度器 `run_gate`。** Bug 任务在 `ReproCheck | FixGate | MergeGate` 上现场拼命令：有项目门禁且节点不是 ReproCheck 时
  `( {repro} ) && ( {gate.command} )`，否则只有复现命令。选不出门禁（配置被拿掉）→ 直接 `finish_gate(Passed)`，不写事件。
  作业 `gate::Job::at(repo, task, node, head12)`；启动时写 `gate.started { node, sha, command, log }`，环境变量带
  `AUTOME_GATE_NODE=<node>`；作业结束时写 `gate.result`，其中 **`command` 取的是这一次 tick 的配置**，不是作业启动时的
  命令。
- **MergeGate 沿用规则**（`run_gate` 内 `if !job.exists()` 之后）：最后一条 `gate.result` 满足 `ok == true`、
  `sha == 当前 head`，且（仅 Bug 任务）`command == 本次 MergeGate 的命令` → 写 `gate.reused` 并视为通过。**不看节点**；
  需求任务**不看命令**。
- **简报。** `write_brief` 只给需求 Loop 的角色写简报（Bug 会话被 `.filter(|_| !matches!(kind, SessionKind::Bug { .. }))`
  排除）；其中 `gate` 只给审计角色，取该任务**最后一条** `gate.result` 经 `render_gate_line` 渲染，不分节点、不看 sha。
  `brief.rs::build` 在 `gate` 为 `Some` 时追加 `## 宿主门禁` 段。Bug 的复核轮没有简报，`prompts/fix-review.md` 要求复核轮
  自己「独立复验复现命令与项目门禁」，内核不向它提供任何宿主门禁结果。
- **读者。** `task.get.gate` = 最后一条 `gate.result`（`dispatch.rs`）；CLI `autome status` 有对象才打印「门禁」行；
  `render.rs` 渲染 `gate.started/result/reused/skipped`。`project.get.config` 与 `config.get.resolved` 直接序列化
  `ResolvedConfig`。桌面端 `routing.js` 只看 `resolved.gate` 是否存在来画一个「门禁」框（注释写明「一个框代表三次运行」）；
  `task.js` 读 `task.get.gate`；Web 面板不读配置。
- **现有 e2e 样板**（`crates/automed/tests/end_to_end.rs`）：`write_gate_script` / `run_gated_task`、
  `a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again`（MergeGate 失败回路，
  修好后复盘重跑一次）、`stopped_gate_resumes_at_gate_and_discards_killed_result`（让门禁脚本阻塞、停在 Gate 的样板）、
  `bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate`。`World::configure` 只往配置文件末尾追加。
- **基线用例数**（`28dad46`，`-- --list` 计）：autome-domain 单测 384 + 集成 1；automed lib 650（其中 `config_io` 40、
  `scheduler` 87、`brief::tests` 20）；end_to_end 112；prompt_rendering 16。现有用例名里没有含 `before_merge` 的——下文
  新增用例名一律含 `before_merge`，并按里程碑用不重叠的前缀：M-01 的 automed 用例在 `config_io::tests::before_merge_*`，
  M-02 的 e2e 叫 `feature_before_merge_*`，M-03 的 e2e 叫 `bug_loop_before_merge_*`、单测在
  `scheduler::tests::before_merge_*` 与 `gate::tests::before_merge_*`，M-04 的 e2e 叫 `brief_before_merge_*`、单测在 `brief::tests::before_merge_*`。
  autome-domain 只有 M-01 加用例。各契约里的「共 N 个」是该里程碑完成时的总数，后续里程碑会再往上加。

## 目标与非目标

目标：

1. 项目 `[gate]` 新增 `run = "each_round" | "before_merge"`，缺省 `each_round`；其它值解析时报中文错误（目标 1、约束 2）。
2. `before_merge` 下需求任务实现轮结束后的转换与「没配门禁」逐字段相同；MergeGate / MainCheck 照旧（目标 2、约束 3、5）。
3. `before_merge` 下 Bug 任务 FixGate 只跑复现命令，MergeGate 仍跑两者（目标 3、约束 4）。
4. 只跑了复现命令的 FixGate、或中途切换 `run`，都不能让 MergeGate 被错误沿用（约束 6、10）；从未启用 `before_merge` 的任务，记账路径与今天逐字节相同（约束 1）。
5. 审计简报与 Bug 复核轮 prompt 在 `before_merge` 下没有本轮结果时写明「本项目门禁只在合并前运行，本轮没有宿主门禁结果」（目标 4、约束 8）。
6. README 与技术设计 §8.1 写明取舍（目标 5、约束 12）。

非目标（任务文件 §4「不在范围内」照抄，不做）：

- `each_round` 下任何行为不变；不做跨任务合并队列；不改 MainCheck（T-60）与门禁排队（T-47）；不改 `scripts/ci-local.sh`
  与会话自己跑测试的方式；不改本仓库 `.autome/`。
- 不新增 JSON-RPC 方法、不改参数、不新增事件种类；账本 schema 不变。
- 不改 `protocol/`、`plugins/`、`apps/desktop`、`apps/web`。逐条理由见方案 §7。
- 不给 Bug 复核轮新增简报文件、不改 `protocol/prompts/fix-review.md`；复核轮只由宿主在 prompt 末尾追加一段说明（方案 §6.3）。

## 方案

### 1. 配置（目标 1；约束 1、2、11）

**类型（`autome-domain/src/config.rs`）。** 旁挂，不进 `GateConfig`：

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateRun { #[default] EachRound, BeforeMerge }

// ProjectConfig
#[serde(default, skip_serializing_if = "Option::is_none")]
pub gate_run: Option<GateRun>,

// ResolvedConfig
#[serde(default, skip_serializing_if = "GateRun::is_each_round")]
pub gate_run: GateRun,          // resolve(): project.gate_run.unwrap_or_default()

impl ResolvedConfig {
    /// 实现轮结束后是否进 Gate：有门禁且 run = each_round。
    pub fn gates_each_round(&self) -> bool { self.gate.is_some() && self.gate_run == GateRun::EachRound }
}
```

选旁挂的理由：`GateConfig { command, timeout_minutes }` 字面量有 5 处（含 4 处测试），加字段会让它们编译不过，违反约束 1；
`ProjectConfig` / `ResolvedConfig` 的所有构造处都经 `Default` 或 `resolve`，加字段零改动。与 `gate_same_failure_limit`
同一写法。`skip_serializing_if` 让 `each_round` 下 `project.get.config` / `config.get.resolved` 的 JSON 与今天逐字节相同；
只有 `before_merge` 项目多出 `"gate_run": "before_merge"` 一个键。

**解析（`automed/src/config_io.rs::parse_project`）。** 在 `[gate]` 段里、紧接 `same_failure_limit` 之后、读 `command`
之前：

- `run` 缺省 → `gate_run = None`；
- 字符串 `"each_round"` / `"before_merge"` → `Some(..)`；
- 其它（含非字符串、大小写不同）→ 报错 `[gate] run 必须是 "each_round" 或 "before_merge"`。

放在读 `command` 之前，和 `same_failure_limit` 一样：`command = ''`（= 没有门禁）时 `run` 仍被校验并保存在
`ProjectConfig.gate_run` 里，但不起作用——`gates_each_round()` 先看 `gate.is_some()`，Bug 拼命令也先看 `resolved.gate`，
简报规则同样先看 `gate.is_some()`（§6）。即：没有门禁时，`run` 写什么都等于今天的「没有门禁」。

**全局配置。** `parse_global` 不读 `[gate]`，本任务不改它；全局文件里出现 `[gate] run = …`（任意值）照旧被忽略、不报错。
用一个用例钉住。

**保存。** `save_project` 不碰 `[gate]`，用户手写的 `run` 原样保留；用一个用例钉住「改角色后保存 → 文件里仍有
`run = "before_merge"`，重新加载仍是 `BeforeMerge`」。

### 2. 需求状态机（目标 2；约束 3、5）

`task.rs` 只改一处：`Node::Implement` 分支的 `if ctx.config.gate.is_some()` 改为 `if ctx.config.gates_each_round()`。
另外两处（rebase 之后进 MergeGate、合并之后进 MainCheck）保持 `ctx.config.gate.is_some()` 不动。`after_gate`、
`press_merge`、`after_the_loop`、`after_implement_without_audit` 都不动。

于是 `before_merge` 下实现轮结束的转换**就是**没配门禁时那段代码的输出（审计开 → `Audit`；审计关 →
`after_implement_without_audit`，含「无开放里程碑 → 复盘/rebase」「预算耗尽 → `Fail(ImplBudget)`」「否则再起实现轮，
吸收纳入的待决条目」），不发 `RunCoreStep{Gate}`。

**MergeGate 失败回路，逐节点（约束 5）。** `before_merge`，审计、复盘都开：

`MergeGate(Failed)` → `Implement`（注入 `GateFailed{head, command, tail}`；守卫 `repairing = true`，豁免「必须有开放里程碑」，
与今天相同）→ 实现轮结束 → **`Audit`** → 审计结束：全部 `已完成` → `Retro` → `Rebase` → `MergeGate` →
通过 → `AwaitMerge`（已请求合并则 `Merging`）；又失败 → 再回 `Implement`。审计若把里程碑退回 `开放`，则
`Audit → Implement → Audit …` 直到全部完成再走 `Retro → Rebase → MergeGate`。

审计关：`Implement` 结束 → `after_implement_without_audit`：无开放里程碑 → `Retro`（或直接 `Rebase`）→ `MergeGate`。

`each_round` 下同一回路是 `MergeGate(Failed) → Implement → Gate → Audit → Retro → Rebase → MergeGate`。两者差别只有
`Implement` 与 `Audit` 之间少了 `Gate`；复盘与 rebase 两种模式下都会重新经过（`after_the_loop` 不区分），这与现有 e2e
`a_gate_failing_after_the_automatic_rebase…`（修好后复盘轮第 10 步重跑）一致。

### 3. 停在 Gate 节点的任务（约束 10）

`before_merge` 下状态机不会再进 `Gate`，但切换前已经停在 `Gate`（正在跑、已暂停、已停止、或同因失败后用户「从门禁重跑 /
追加轮次」）的任务会被调度到 `run_gate(.., Node::Gate)`。`run_gate` 开头（`uses_bug_loop` 之后、拼命令之前）加一段：

> 需求任务、节点是 `Gate`、`resolved.gate.is_some()` 且 `resolved.gate_run == BeforeMerge` →
> `gate::discard_node(repo, task, "gate")`（杀掉在跑的包装进程组、删作业文件）→ 写 `gate.skipped { node: "gate",
> reason: "门禁只在合并前运行" }` → `finish_gate(.., GateOutcome::Passed)`。

`(Gate, Passed)` 的后继正是「没配门禁」时实现轮结束会去的地方（`after_gate` 与 §2 的分支同源：审计开 → `Audit`；
审计关 → `after_implement_without_audit`，状态块由 `finish_gate` 读设计文档）。这与「门禁在任务停在门禁节点时被从配置
里拿掉」的先例同一条路，只多了杀进程（不让一个没人读的 `ci-local` 继续抢占）和一条已有种类的 `gate.skipped` 事件
（`render.rs` 已会渲染，`autome run` 不新增种类）。各情形：

- 正在跑：下一个 tick 杀掉、跳过、进审计。
- 已暂停（`Paused{resume: Gate}`）→ 恢复：`Active{Gate}`，同上。
- 已停止（`Stopped{at: Gate}`）→ 恢复：状态机先发 `DiscardGateRuns`，再同上。
- 同因失败停在 `Gate` → 用户「从门禁重跑 / 追加轮次」：同上，直接放行到审计——用户已声明门禁只在合并前跑。
- 反向（`before_merge` → `each_round`）：下一个实现轮结束照常进 `Gate`，无特殊处理。

合并前的完整门禁不会因此漏掉：rebase 之后进 `MergeGate` 的判断只看 `gate.is_some()`（§2），沿用规则见 §5。

### 4. Bug Loop 的 FixGate（目标 3；约束 4、7）

`bug_loop.rs` 不改（`Node::Fix => core(Node::FixGate)` 保持）。`run_gate` 里 Bug 拼命令的 `match` 改为：

```rust
(Some(gate), n) if n == Node::MergeGate
    || (n == Node::FixGate && resolved.gate_run == GateRun::EachRound) => format!("( {repro} ) && ( {} )", gate.command),
_ => repro.to_owned(),
```

即 ReproCheck 只跑复现（不变）；FixGate 在 `before_merge` 下只跑复现；MergeGate 永远拼两者。超时仍取项目门禁的
`timeout_minutes`（没有门禁取缺省值），不变。FixGate 结果照写 `gate.result { node: "fix_gate", command: <复现命令>, … }`，
同因失败熔断按节点 `fix_gate` 计数照旧；126/127 的处理在下游，命令变短不影响它。

### 5. 沿用规则不被骗过（约束 6、10）

核对结果：沿用要求「最后一条 `gate.result` 通过 + sha 相同 +（Bug）命令相同」，不看节点。

- **需求任务，`before_merge`。** 不再产生 `Gate` 结果；最后一条 `gate.result` 只可能是 MergeGate 的（或切换前的
  `Gate` 结果）。`Gate` 跑的永远是完整的 `gate.command`，所以同一 sha 上沿用它仍是「完整门禁在这个提交上通过过」，
  与今天语义一致。中途 `each_round ↔ before_merge` 切换不会产生「不完整」的结果。无需改动。
- **Bug 任务，`before_merge`。** FixGate 的命令是 `repro`，MergeGate 的是 `( repro ) && ( gate )`，命令不等 → 不沿用 →
  MergeGate 一定真跑。无需改动。
- **Bug 任务，FixGate 正在跑时切换 `run`。** 这是唯一的漏洞：作业按 `node + head` 定位、不含命令，作业是按旧配置启动
  的，结果却按新配置的命令记录（`run_gate` 的 `result.command` 与 `GateOutcome::Failed.command` 都取本 tick 的 `gate.command`）。
  `before_merge → each_round` 时，一个只跑了复现命令的作业会被记成 `command = "( repro ) && ( gate )"`、`ok = true`；
  若随后 rebase 没有移动 head，MergeGate 会沿用它而跳过完整门禁。

  **修法：作业旁的标记文件，不依赖账本事件。** 不用 `gate.started` 事件回溯命令：`run_gate` 先 `gate::start`、后写事件，
  两步不原子，daemon 在两步之间退出就没有事件；同 node/head 的旧作业被丢弃后重启时，按 node/sha 取事件还会拿到上一次运行的命令。
  改为：
  1. `gate.rs` 的 `Job` 加一个旁路文件 `<node>-<head12>.repro-only`（目录与其它作业文件相同，作业路径与现有文件名都不变），
     提供 `set_repro_only(bool)`（写入或删除）和 `repro_only() -> bool`；`Job::clear` 与 `discard_node` 的两处后缀清单各加这一项。
  2. `run_gate` 在 `!job.exists()` 分支里、`gate::start` **之前**调用 `job.set_repro_only(x)`，其中
     `x = bug && node == FixGate && resolved.gate.is_some() && resolved.gate_run == BeforeMerge`（即「这一次 FixGate 只拼了复现命令
     是因为新模式」）。无项目门禁的 Bug 任务本来就只跑复现命令，`x = false`，不写标记。标记是确定性地写或删，所以前一个作业留下的旧标记、
     或「写了标记但没来得及启动」的残留，都会在下一次启动前被覆盖。
  3. 作业结束记账时，若 `node == FixGate` 且 `job.repro_only()`：`result.command` 与 `GateOutcome::Failed.command` 取**复现命令本身**
     （它是这个作业实际跑过的命令）；否则取本 tick 配置拼出的命令，**与今天完全相同**。
  4. 因为标记总是先于作业文件出现，任何以 `before_merge` 启动的 FixGate 作业都带标记，不存在「作业在、标记不在」的崩溃窗口；
     没有标记的作业要么是 `each_round` 启动的，要么是升级前就在跑的（那时只有 `each_round`）。

  逐情形（FixGate，Bug 任务，有项目门禁）：

  | 启动时 | 结束时配置 | 标记 | 记录的 `command` | MergeGate 沿用？ |
  |---|---|---|---|---|
  | `each_round`（完整命令） | `each_round`（含 A→B 改了门禁命令） | 无 | 本 tick 的完整命令，同今天 | 同今天 |
  | `each_round` | `before_merge` | 无 | 复现命令（本 tick 拼出的），比实际跑的弱，安全 | 否（命令≠完整命令） |
  | `before_merge`（只复现） | `before_merge` | 有 | 复现命令 | 否 |
  | `before_merge` | `each_round` | 有 | 复现命令（不是当前完整命令） | 否——MergeGate 真跑 |
  | `before_merge`，标记已写、`gate.started` 未写时 daemon 退出 | 任意 | 有 | 复现命令 | 否 |

  **只有 `before_merge` 下启动的 FixGate 才会有标记，所以从未写过 `run` 的任务（包括 `run = "each_round"` 与无项目门禁）记账路径与今天
  逐字节相同**——约束 1 的边界按「是否启用过新模式」划，不按「配置有没有在两个 tick 之间变过」划。需求任务、MergeGate、ReproCheck、
  MainCheck 都不读也不写标记。`gate.rs` 里只多这一个旁路文件和两个小函数，不动 `start / poll` 的行为。

### 6. 简报的宿主门禁证据（目标 4；约束 8）

#### 6.1 判定

「本轮门禁结果」= 最后一条 `gate.result` 且其 `sha` 等于写简报时的 `gate_head(layout)`（与门禁绑定 head 的同一个函数：
单仓库是检出 head，工作区是各仓库 head 的 sha256）。简报在本轮会话启动前、简报自己的提交之前计算，此时 head 就是
被审计的那一轮留下的提交。

#### 6.2 规则（`brief.rs` 新增纯函数，调度器调用）

`render_gate_line` 原样从 `scheduler.rs` 移到 `brief.rs`（改名 `gate_line`，`pub`，文字不改一个字），新增：

```rust
pub const GATE_ONLY_BEFORE_MERGE: &str = "本项目门禁只在合并前运行，本轮没有宿主门禁结果。\n";
pub fn gate_evidence(config: &ResolvedConfig, last: Option<&Value>, head: Option<&str>) -> Option<String>
```

- `config.gate` 为 `None`，或 `gate_run == EachRound` → `last.and_then(gate_line)`，与今天逐字节相同（约束 1）。
- `gate` 有、`gate_run == BeforeMerge`：`last` 的 `sha` 等于 `head` → `gate_line(last)`（例如 MergeGate 失败后实现轮没有
  新提交，那条失败结果确实就是本轮的）；否则（没有结果、结果是别的提交的、读不到 head）→ `Some(GATE_ONLY_BEFORE_MERGE)`。

调度器：`write_brief` 的调用方（已持有 `resolved` 与 `layout`）对审计角色算出 `gate_evidence(&resolved, last, gate_head(&layout).ok())`
传入；其它角色仍是 `None`。`brief::build` 不改——`Some` 时照旧出 `## 宿主门禁` 段，所以 `before_merge` 下审计简报永远有这
一段，不会缺失，也不会把上一次 MergeGate 的旧结果当本轮证据。

#### 6.3 Bug Loop 的复核轮

现状：Bug 会话没有简报（见背景），复核 prompt（`protocol/prompts/fix-review.md`）不指向任何简报文件、不带宿主门禁结果，只要求复核轮
自己复验复现命令与项目门禁。任务卡要求「审计 / 复核简报」都写明，所以复核轮也要交付这句话；但新增简报要改 `protocol/`（范围外），
因此改由宿主在复核轮 **prompt 末尾**追加一段，通道与 `Inject::GateFailed` 给修复轮的文字相同：

- `launcher::PromptSpec` 新增 `gate_note: Option<&'a str>`；`build_prompt` 在 inject 段之后、返回之前，`Some` 时追加 `"\n" + 文字`。
- `brief.rs` 新增 `pub fn bug_review_gate_note(config: &ResolvedConfig) -> Option<String>`：`config.gate` 为 `Some` 且
  `gate_run == BeforeMerge` 时返回
  `"本项目门禁只在合并前运行，本轮没有宿主门禁结果。修复门禁只跑了复现命令，它通过不等于项目门禁通过；请你自己复验项目门禁，不要把复现命令通过当作门禁通过。\n"`，
  其余（`each_round`、没有项目门禁）返回 `None`。文案以 `GATE_ONLY_BEFORE_MERGE` 的常量句开头，保证与审计简报是同一句话。
- 调度器构造 `PromptSpec` 处（`start_session`）：只有 `kind == SessionKind::Bug { step: BugStep::FixReview }` 才传
  `bug_review_gate_note(&resolved)`；定位轮、修复轮、其它角色与 `start_onboarding` 一律 `None`。`each_round` 下复核 prompt 逐字节不变。
- 「不沿用旧结果」：这段文字不引用任何账本事件，不会把上一次 MergeGate 的失败当作本轮证据；MergeGate 失败回修复轮时修复轮的
  `Inject::GateFailed` 照旧带门禁尾部，那是修复轮要读的原文，不是复核轮的「宿主门禁结果」。

FixGate 的结果仍只出现在账本 `gate.result`（`node: fix_gate`、`command` 是复现命令本身，§5）以及 `autome status` 的门禁行
「修复门禁 在 <sha> 上通过（<复现命令>）」——这一行今天在没配项目门禁的 Bug 任务里就是这样显示的，节点名与命令都如实，
不会被读成完整门禁通过。

金样：`crates/automed/tests/golden/prompts/fix-review-before-merge.md`，由 `prompt_rendering.rs` 里新增的
`before_merge_fix_review_prompt_matches_its_golden` 比对（`spec()` 里 `gate_note` 设为 `bug_review_gate_note(before_merge 配置)`，
沿用现有 `golden()` / `maybe_update_golden()`，这两个函数不改）。

#### 6.4 金样

新目录 `crates/automed/tests/golden/briefs/`，文件 `audit-before-merge.md`：用固定输入调用 `automed::brief::build`，
其中 `gate = gate_evidence(before_merge 配置, 一条 sha 不同的旧 merge_gate 失败结果, Some("<head>"))`，整份简报逐字节比对。
比对测试 `before_merge_audit_brief_matches_its_golden` 放在 `tests/prompt_rendering.rs`，新增一对只读 `golden/briefs/` 的
辅助函数（现有 `golden()` / `maybe_update_golden()` 不改），同样受 `AUTOME_UPDATE_GOLDEN=1` 控制；
`tests/golden/README.md` 补一段说明 `briefs/`。

### 7. 读者（约束 9；任务 §4 末条）

| 读者 | `before_merge` 下看到的 | 处置 |
|---|---|---|
| `task.get.gate` / `autome status` 门禁行 | 最后一条 `gate.result`；合并前没有结果时为 null，不打印 | 不改 |
| `autome run` 事件输出 | 只少了 `node: gate` 的 `gate.started/result`；切换时多一条已有种类 `gate.skipped` | 不改 |
| `project.get.config` / `config.get.resolved` | 多 `"gate_run": "before_merge"`；`each_round` 下无此键 | 不改（读者按键取值，不枚举键） |
| 桌面端 `routing.js` 路由图 | 仍画「实现 → 门禁 → 审计」一个框（注释：一框代表三次运行） | 不改，进 Backlog B-01 |
| 桌面端 `task.js` 主机标签 | 读 `task.get.gate`，同第一行 | 不改 |
| Web 面板 | 不读配置，只有节点名映射 | 不改 |
| `protocol/bug-protocol.md` 第 40 行「宿主执行复现命令及配置的项目门禁」 | `before_merge` 下修复门禁只跑复现，项目门禁在合并前门禁跑，失败同样退回修复 | 不改，进 Backlog B-02 |

路由图不改的理由：它是配置编辑界面上的示意图，不是运行状态；它本来就把 MergeGate、MainCheck 画在同一个框里，框上的
副标题「合并前、合并后再跑」在 `before_merge` 下仍然成立；`apps/desktop` 在任务范围外。协议不改的理由：`protocol/` 在
范围外，且复核 prompt 本来就要求会话自己复验项目门禁，措辞差异不会让任何一轮做错事。桌面端两份白名单与
`READ_METHODS` 不变（不新增方法）。

### 8. 文档（目标 5；约束 12）

- `README.md`「Host gate, retries and cooldowns」：`[gate]` 示例加一行
  `run = "each_round"          # the default; "before_merge" runs the full gate only once, before the merge`；
  「The gate.」后补一段英文：`before_merge` 把门禁次数从每轮一次降到每个任务一次（once per task）；代价是门禁问题要到
  合并前才暴露（not until the merge），会多退回一次实现轮；Bug 修复轮在 `before_merge` 下只跑复现命令（only the repro command），
  完整门禁留给合并前门禁。
- `docs/development/technical-design.md` §8.1「宿主门禁」段后补一段中文，含：`[gate] run`、门禁次数从「每轮一次」降到
  「每个任务一次」、门禁问题到合并前才暴露、会多退回一次实现轮、Bug 修复轮只跑复现命令、简报文案、切换时的
  `gate.skipped`。

### 9. 现有测试改动清单（约束 1）

**断言一行不改。** 配置类新字段都是旁挂 `Option` / 有 `Default`，所有构造处已带 `..Default::default()` 或经 `resolve()`；
`render_gate_line` 是私有函数，没有测试直接调用；`brief::build`、`golden()` 不改签名；`gate::Job` 只在 `Job::at` 里构造（一处，生产代码）。

唯一要动现有测试文件的是 `launcher::PromptSpec` 新增 `gate_note`（§6.3）：它没有 `Default`，4 处字面量都要补一行 `gate_note: None,`——
`scheduler.rs` 的 `start_session`（传入 `Bug{FixReview}` 的说明，其余 `None`）与 `start_onboarding`（`None`）两处生产代码，
`launcher.rs` 测试模块里的 `PromptSpec { .. }`（约 1128 行）与 `tests/prompt_rendering.rs` 的 `spec()` 辅助函数（约 52 行）两处测试代码。
后两处只加这一行，不改任何断言、不改任何金样；理由是「结构体多了一个字段导致构造处编译不过」，约束 1 明文允许。
实现期若发现还有其它必须改动的现有用例，在当轮证据里逐条列出，理由不能是「新行为与旧断言不符」。

### 10. 否决的做法

- **把 `run` 放进 `GateConfig`**：改 5 处字面量，违反约束 1。
- **在 Bug 状态机里让 FixGate 在 `before_merge` 下被跳过**：违反约束 4（节点序列不变）。
- **把命令哈希编进作业文件名以修 §5 的漏洞**：会改 `each_round` 下的作业路径，`end_to_end.rs` 里有用例按
  `Job::at(.., "gate", head12)` 拼路径。
- **为 MainCheck 计数而关掉它**：任务明令禁止；改用 `AUTOME_GATE_NODE` 与账本 `node` 字段区分。
- **FixGate 的命令从 `gate.started` 事件回溯**（本设计的前一版）：`gate::start` 与事件两步不原子，崩溃窗口里没有事件；同 node/head
  的旧作业被丢弃重启时还会取到上一次运行的命令；并且无条件改变了从未启用 `before_merge` 的 FixGate 记账（门禁命令 A→B 时今天记 B），
  违反约束 1。改用作业旁标记文件（§5）。
- **把 `gate.started` 写在 `gate::start` 之前来消除窗口**：`gate::start` 失败（`无法启动门禁`）时会留下一条没有作业的启动事件，
  改变 `each_round` 下的事件序列；且仍要按 node/sha 回溯，解决不了同 head 旧作业的问题。
- **给 Bug 复核轮新增简报文件**：要在 `fix-review.md` 里加指向简报的句子，改 `protocol/`（范围外）；宿主 prompt 末尾追加（§6.3）达到同样效果。

## 风险与验证安排

- **R1 MergeGate 的偶发失败代价变大。** `each_round` 下 MergeGate 偶发失败 → 实现轮 → `Gate` 在同一 head 上重跑通过 →
  之后 MergeGate 沿用；`before_merge` 下没有这一步，若实现轮没有新提交，同一 head 上的旧 merge_gate 作业会被直接读回
  （作业文件按 node+head 定位），再记一次同样的失败，直到同因熔断。缓解：协议要求每个实现轮写证据文件并提交（head 必变，
  作业重跑）；熔断后用户有「从门禁重跑」（`DiscardGateRuns`）。门禁总次数下降本身也降低负载超时的概率。这是任务卡接受的
  取舍（「会多退回一次实现轮」），不在本任务里改作业复用规则；写进 README / 技术设计。
- **R2 与 T-47、T-60 的 rebase 冲突。** 两者都改 `run_gate` 一带。缓解：`run_gate` 只加 §3 一段和 §4 的一个 match 臂、§5 的
  命令取值；`render_gate_line` 的移动在约 2060 行，离 `run_gate` 远；不做无关移动。
- **R3 e2e 计数靠时序。** 不靠：门禁脚本把 `$AUTOME_GATE_NODE` 追加到仓库外的计数文件，断言读文件行；同时断言账本里
  按 `node` 分组的 `gate.started` 数。MainCheck 的那次由 `main_check` 行区分，合并前断言「只有一行 merge_gate」，合并后断言
  「merge_gate 一行 + main_check 一行」。
- **R4 e2e 改配置。** `World::configure` 只追加；中途切换 `run` 的用例直接重写 `.autome/config.toml` 的完整内容并提交
  （写两个 `[gate]` 表会是 TOML 错误）。
- **验证顺序。** M-01 先把配置与纯状态机钉死（最便宜、其余全依赖它）；M-02 建需求 Loop 的完整端到端链路（任务卡验收 2
  的两个场景是本任务最大的不确定性：真实门禁作业 + 回路节点序列）；M-03 Bug；M-04 简报与金样；M-05 文档与全量门禁。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | [gate] run 配置解析与需求状态机转换 | 0 | |
| M-02 | 开放 | 需求 Loop 端到端：只在合并前跑门禁与停在 Gate 的任务 | 0 | |
| M-03 | 开放 | Bug Loop：FixGate 只跑复现命令且不被沿用骗过 | 0 | |
| M-04 | 开放 | 审计简报的宿主门禁证据与金样 | 0 | |
| M-05 | 开放 | README 与技术设计写明取舍并过全量门禁 | 0 | |

## 里程碑说明

- **M-01**（覆盖：目标 1、2；验收 1、4；约束 1、2、3、11。前置：无）。`config.rs` 加 `GateRun`、两个旁挂字段与
  `gates_each_round()`；`task.rs` 改实现轮结束那一处；`config_io.rs` 解析与校验 `run`。新增用例（名字都含 `before_merge`）：
  - autome-domain 6 个：`config` 1 个（缺省 `EachRound`、项目覆盖、`each_round` 下 `ResolvedConfig` 的 JSON 没有 `gate_run` 键）；
    `task` 5 个：① 审计开时实现轮结束的 `Transition` 与 `gate: None` 配置下的逐字段相等；② 审计关时按「有开放里程碑 /
    无开放里程碑且复盘开 / 复盘关 / 预算耗尽 / 有纳入的待决条目」五种状态块逐一与 `gate: None` 相等；③ rebase 成功仍进
    `MergeGate`、合并成功仍进 `MainCheck`；④ `MergeGate` 通过（含已请求合并）/ 失败的转换与 `each_round` 配置下相等；
    ⑤ 停在 `Gate` 的任务收到 `GateDone{Passed}` 仍按 `after_gate` 走。
  - automed lib（`config_io`）5 个：缺省与显式 `each_round` 解析后 `resolve` 结果相等、`before_merge` 被读到；`"sometimes"`、
    `1`、`true` 都报 `[gate] run 必须是 "each_round" 或 "before_merge"`；改角色后 `save_project` 文件里仍有
    `run = "before_merge"` 且重载仍生效；`command = ''` 时 `run` 仍被校验并保存、`gate` 为 `None`；全局文件里
    `[gate] run = "sometimes"` 被忽略、`load_global` 成功。
- **M-02**（覆盖：目标 2；验收 2；约束 3、5、10。前置：M-01）。`run_gate` 加 §3 那一段。end_to_end 新增 4 个：
  ① `before_merge` 两个里程碑的任务从创建到 `await_merge`：计数文件只有一行 `merge_gate`，账本 `node: gate` 的
  `gate.started` 为 0、`merge_gate` 为 1；合并后完成，计数文件为 `merge_gate`、`main_check` 两行；② MergeGate 第一次失败
  → 回实现且该轮 prompt 含门禁尾部 → 修好 → 审计 → 复盘 → rebase → MergeGate 通过 → 合并完成；断言失败之后的节点序列
  （`task.updated` 的 `node`）为 `implement, audit, retro, rebase, merge_gate, await_merge` 且不含 `gate`；③ `each_round` 下任务
  停在一个阻塞的 `Gate` 上，改写配置为 `before_merge` → 任务进 `audit`、包装进程组已不在、有 `gate.skipped{node: gate}`，
  之后照常跑 MergeGate 到 `await_merge`；④ 同 ③ 但先 `task.stop`，改配置后 `task.resume` → 进 `audit`。
- **M-03**（覆盖：目标 3、4；验收 3；约束 1、4、6、7、10。前置：M-01）。`run_gate` 的 Bug 命令 `match`、`gate.rs` 的 `.repro-only`
  标记（`Job::set_repro_only / repro_only`，`clear` 与 `discard_node` 的清单）与 FixGate 结果的命令取值（§4、§5）。新增：end_to_end 2 个——① `before_merge` + 项目门禁：FixGate 的 `gate.started.command` 等于复现命令
  （不含 `&&`），MergeGate 的含 `( sh regression.sh ) && (` 与项目门禁命令；项目门禁第一次在 MergeGate 失败 → 回修复且
  prompt 含项目门禁输出 → 修好后 MergeGate 通过到 `await_merge`；② `before_merge` 下修复没生效 → FixGate 失败回修复，
  prompt 含 `regression-output` 与 `2/3`。automed lib（`scheduler`）4 个——③ FixGate 作业按 `before_merge` 启动、结束前配置
  改成 `each_round`：`gate.result.command` 是复现命令，失败时 `GateOutcome::Failed.command` 同；④ 接着在同一 head 上跑 MergeGate：没有 `gate.reused`，有一条
  `merge_gate` 的 `gate.started`；⑤ 崩溃窗口（D1-P01）：`before_merge` 下启动 FixGate 后删掉账本里的 `gate.started`（模拟标记已写、事件未写），
  再切 `each_round`、作业结束：`gate.result.command` 仍是复现命令，随后同 head 的 MergeGate 不沿用；同一 node/head 先留一个 `each_round`
  的完整命令旧事件、再以 `before_merge` 重启并删去新事件，结果同样记复现命令；⑥ 约束 1 边界（D1-P03）：从未写过 `run`，FixGate 以项目门禁 A 启动、
  结束前改成 B：`gate.result.command` 与今天一样是 `( repro ) && ( B )`，且该作业目录里从未出现 `.repro-only` 文件。gate.rs 另有 1 个单测放在
  `gate::tests::before_merge_*`：`set_repro_only(true/false)` 往返、`Job::clear` 与 `discard_node` 都会删掉它。
- **M-04**（覆盖：目标 4；验收 5；约束 8。前置：M-02、M-03）。§6：`brief.rs` 的 `gate_line` / `gate_evidence` / `bug_review_gate_note` / 常量，
  `launcher::PromptSpec.gate_note` 与 `build_prompt` 追加，调度器调用处（审计简报与 `Bug{FixReview}` 的 prompt），两份金样与 `golden/README.md`
  的说明；§9 的 4 处 `gate_note: None`。新增：automed lib（`brief`）4 个——无结果与旧 sha 结果都出常量文案；同 sha 结果出
  `gate_line`；`each_round` 与「没有门禁 + before_merge」时与 `last.and_then(gate_line)` 相等；`bug_review_gate_note` 只在「有项目门禁 + before_merge」
  返回 `Some` 且含常量句与「通过不等于项目门禁通过」。prompt_rendering 2 个（审计简报金样、Bug 复核 prompt 金样）。
  end_to_end 2 个——① M-02 ② 的场景里，第一轮审计与 MergeGate 失败修好后那一轮审计的简报都含「本项目门禁只在合并前运行，
  本轮没有宿主门禁结果」，后者不含失败那次的 sha 前 10 位；② `brief_before_merge_bug_review_prompt_says_no_host_gate_result`：M-03 ① 的 Bug 场景里，
  第一次复核轮收到的 prompt 含该句与「只跑了复现命令」。
- **M-05**（覆盖：目标 5；验收 6；约束 12、13。前置：M-01..M-04）。README 与技术设计 §8.1 两段（§8）；跑全量。

## 验收契约

### M-01 验收契约

- 验收命令：`cargo test -p autome-domain before_merge && cargo test -p automed --lib config_io::tests::before_merge && cargo test -p autome-domain && cargo test -p automed --lib config_io`
- 预期：autome-domain 过滤 `before_merge` 6 个通过；automed lib 过滤 `config_io::tests::before_merge` 5 个通过；autome-domain 单测共 390 个通过（基线 384 + 6）、集成 1 个通过；`config_io` 45 个通过（基线 40 + 5）；现有用例一个不改。
- 负向对照：`task.rs` 实现轮结束处仍写 `ctx.config.gate.is_some()` 时，task ①② 两个用例失败（转换去了 `Gate`）；解析对未知字符串不报错时，「`"sometimes"` 报错」用例失败。

### M-02 验收契约

- 验收命令：`cargo test -p automed --test end_to_end feature_before_merge && cargo test -p automed --test end_to_end`
- 预期：过滤 `feature_before_merge` 4 个通过；end_to_end 共 116 个通过（基线 112 + 4），现有 112 个一行不改。
- 负向对照：撤掉 M-01 的 `task.rs` 改动时，① 的计数文件出现 `gate` 行、② 的节点序列含 `gate`，两例失败；不加 §3 那一段时，③ ④ 的任务停在 `gate`（阻塞脚本不退出），断言节点为 `audit` 失败。

### M-03 验收契约

- 验收命令：`cargo test -p automed --test end_to_end bug_loop_before_merge && cargo test -p automed --lib scheduler::tests::before_merge && cargo test -p automed --test end_to_end && cargo test -p automed --lib scheduler`
- 预期：end_to_end 过滤 `bug_loop_before_merge` 2 个通过；`scheduler::tests::before_merge` 4 个通过；`gate::tests::before_merge` 1 个通过；end_to_end 共 118 个通过；`scheduler` 共 91 个通过（基线 87 + 4）。
- 负向对照：Bug 命令 `match` 不看 `gate_run`（FixGate 照旧拼两者）时，① 的 FixGate 命令断言失败、且因 `gate-ready` 缺失 FixGate 先失败使节点序列不符；FixGate 结果仍取当前配置的命令（不看 `.repro-only`）时，③ ⑤ 的 `command` 断言失败、④ 出现 `gate.reused` 而失败；无条件改用 `gate.started` 事件的命令（前一版做法）时，⑥ 的 `command` 断言失败；`.repro-only` 对所有 FixGate 都写时，⑥ 的「从未出现标记文件」断言失败。

### M-04 验收契约

- 验收命令：`cargo test -p automed --lib brief::tests::before_merge && cargo test -p automed --test prompt_rendering && cargo test -p automed --test end_to_end brief_before_merge && cargo test -p automed --test end_to_end a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`
- 预期：`brief::tests::before_merge` 4 个通过；prompt_rendering 共 18 个通过（基线 16 + 2 金样）；e2e 过滤 `brief_before_merge` 2 个通过；现有 `a_failing_gate_sends…`（`each_round` 审计简报含 `## 宿主门禁` 与「通过」）1 个通过、未改动。
- 负向对照：`before_merge` 下仍取最后一条 `gate.result` 不看 sha 时，「旧 sha 结果出常量文案」单测与 e2e 中修好后那一轮审计简报的断言失败（简报里出现失败那次的 sha 与「未通过」）；把常量文案改动一个字时两份金样比对失败；复核 prompt 在 `each_round` 下也追加说明时，现有 `fix-review.md` 金样失败；调度器对非 `FixReview` 的 Bug 会话也传说明时，`fix.md` / `locate.md` 金样（以及 e2e 的现有 Bug 用例）失败。

### M-05 验收契约

- 验收命令：`for p in 'run = "each_round"' before_merge 'once per task' 'not until the merge' 'only the repro command'; do grep -qF "$p" README.md || { echo "README 缺: $p"; exit 1; }; done; for p in before_merge 每个任务一次 合并前才暴露 多退回一次实现轮 只跑复现命令; do grep -qF "$p" docs/development/technical-design.md || { echo "技术设计缺: $p"; exit 1; }; done; cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && sh scripts/ci-local.sh`
- 预期：两组 grep 全部命中、无输出；fmt / clippy 退出码 0；`ci-local.sh` 退出码 0，其中 autome-domain 390 + 1、automed lib 664（650 + M-01 的 5 + M-03 的 4 + 1 + M-04 的 4）、end_to_end 120（112 + 4 + 2 + 2）、prompt_rendering 18 全部通过。
- 负向对照：README 少写「only the repro command」一句时 grep 循环打印「README 缺: only the repro command」并以 1 退出；任一新代码有 clippy 警告时 `-D warnings` 失败。

## 人工验收清单

- H-01 合并后在本仓库 `.autome/config.toml` 的 `[gate]` 加 `run = "before_merge"`，用真实模型跑下一个需求任务：`autome run` 输出里实现轮之后没有「门禁」行，只在 rebase 之后出现一次「合并前门禁」，合并后一次「验证 main」。

## Backlog

- B-01 桌面端路由图在 `before_merge` 下仍把「门禁」框画在实现与审计之间；可按 `resolved.gate_run` 把框挪到 rebase 之后。不在本任务范围（`apps/desktop`），不影响任何运行状态显示。
- B-03 README / 技术设计写「每个任务一次」时，紧邻注明它指正常通过路径上的合并前宿主门禁：MainCheck 仍在合并后运行，MergeGate 失败会退回实现轮后重试（评审 #1 N-01）。属于 M-05 的写作细节，不触发轮次。
- B-02 `protocol/bug-protocol.md` 第 40 行「宿主执行复现命令及配置的项目门禁」可在下一版协议里补一句 `before_merge` 下修复门禁只跑复现命令。不在本任务范围（`protocol/`）。

## 争议项
