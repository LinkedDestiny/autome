# 门禁只在合并前跑一次（`[gate] run = "before_merge"`）

status: 已完成
design-round: 3/15
implementation-round: 8/40
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

任务文件：`docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run-task.md`

任务卡：`docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run-card.md`

下文「目标 n」指任务文件 §2 第 n 条，「验收 n」指 §3 第 n 条，「约束 n」指 §5 第 n 条，「意见 n」指用户 2026-11-12 驳回设计时
那条意见的第 n 项（原文见本轮 prompt 与 `brief/plan-2.md`；它把范围扩到 `protocol/prompts`，覆盖任务文件 §4 里「不改会话自己跑
测试的方式」「不改 `protocol/`」两条——只在 `before_merge` 下、只到 prompt 模板、金样、协议版本与 protocol eval 为止）。
代码位置以本 worktree 的基线提交 `28dad46` 为准（`8eb77c9` 之前的提交都只改了任务文档），行号会漂移，以符号名为准。

## 背景与现状

- 配了 `[gate]` 的需求任务每个实现轮之后跑一次完整门禁（`Gate`），rebase 之后再跑一次（`MergeGate`），合并之后又跑一次
  （`MainCheck`）。Bug 任务每个修复轮之后跑 `( 复现命令 ) && ( 门禁命令 )`（`FixGate`），合并前再跑一次。2026-11-12 一天
  102 次门禁、18.6 小时机器时间；并行 3 个任务时多个 `ci-local` 互相抢占，负载下的超时误报又把任务退回实现轮。
- **会话自己也在全量跑。** 宿主之外，会话按 prompt 自己跑完整的项目测试体系，这些运行不经宿主、不受门禁排队（T-47）约束：
  `protocol/prompts/audit.md`「复验范围只有三样：项目测试体系……项目测试体系全量跑一次即可」；`fix.md`「让……复现命令通过，
  再运行配置的项目门禁」；`fix-review.md`「独立复验复现命令与项目门禁」；`impl.md` 不限制范围，实现轮按惯例自查时跑门禁命令；
  设计轮惯例把「全量回归」写成最后一个里程碑的验收命令。`loop-protocol.md`「审计造的检查归谁」一节也写着「项目测试体系
  （全量跑一次即可……）」，这一节会被摘进审计与实现轮的简报。宿主注入给修复轮的 `Inject::GateFailed` 文字要求「在同一命令下
  确认通过后提交」（需求）/「修复后用同一命令确认通过并提交」（Bug），也就是让修复轮自己再跑一遍整条门禁命令。
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
  `brief.rs::build` 在 `gate` 为 `Some` 时追加 `## 宿主门禁` 段。简报的「本轮相关的协议小节」逐字摘自任务固定的那一版
  `loop-protocol.md` / `session-protocol.md`（`brief-map.toml`）。Bug 的复核轮没有简报。
- **prompt 与协议版本。** 每个角色的 prompt 是任务固定的那一版协议里的 `prompts/<role>.md`（`PromptSpec.templates`，创建任务时
  整份复制进 `docs/<slug>/protocol/` 并记下 `protocol_ref`；协议「共同原则」第 6 条：本次运行不中途换版本）。`launcher.rs` 只做
  替换：`role_prompt` / `bug_prompt` 填 `{doc_dir}`、`{slug}`、`{budget_line}` 等，剩下任何 `{lower_snake}` 都报错
  （`check_no_placeholders_left`）；技能行与 `Inject` 文字由代码追加在模板之后。`PromptSpec` 没有 `Default`，字面量 4 处
  （`scheduler.rs` 的 `start_session` 与 `start_onboarding`、`launcher.rs` 测试模块、`tests/prompt_rendering.rs` 的 `spec()`）；
  `start_session` 构造它时手里已有 `resolved`。
- **金样。** `crates/automed/tests/golden/prompts/` 12 份，由 `tests/prompt_rendering.rs`（16 个用例）比对：五个角色的基准
  **就是模板原文**（`every_role_prompt_renders_exactly_what_the_constants_did` 比对「无预算渲染」与基准，刷新分支写入的是
  `seed().prompt(name)` 原文）；Bug 三份与 intake / onboarding 是渲染结果。
- **protocol eval。** `cargo run -p automed -- protocol eval protocol/`：第 1 层静态检查——内核契约区、正文大小（只计
  `loop-protocol.md` + `session-protocol.md`，上限 24576 字节，现为 19323 + 5123 = 24446，**只剩 130 字节**）、必含 / 禁含短语
  （`protocol/phrases.rs`）、模板占位符必须在 `KNOWN_PLACEHOLDERS`（9 个）里、`brief-map` 标题、eval 用例可读且 grader 带阳性
  对照、CHANGELOG 条目门槛（`behavioral` 必须带 `eval` 目录、证据至少两个任务）。`--changed` / `--behaviour` 用真实模型跑行为
  用例，要花钱。eval 用例（`protocol/evals/<name>/case.yaml`）的字段是封闭的 9 个，`eval_run::render_prompt` 按固定值填占位符。
  当前版本是 `CHANGELOG.md` 里编号最大的 `## protocol/v2`；`protocol/mod.rs` 的
  `the_version_in_force_is_the_newest_changelog_heading_over_the_seed` 与 e2e
  `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task` 各有一处断言写死 `protocol/v2`。
- **`bug-protocol.md` 第 40 行**（「复现与修复」节）：「宿主执行复现命令及配置的项目门禁，任一失败退回修复」。这份文件不进简报、
  不计入大小上限，Bug 会话按 prompt 去读它。
- **相邻任务。** T-62 正在改 `audit.md` / `retro.md` / `fix-review.md` 的范围规则；T-47、T-60 改 `run_gate` 一带。
- **读者。** `task.get.gate` = 最后一条 `gate.result`（`dispatch.rs`）；CLI `autome status` 有对象才打印「门禁」行；
  `render.rs` 渲染 `gate.started/result/reused/skipped`。`project.get.config` 与 `config.get.resolved` 直接序列化
  `ResolvedConfig`。桌面端 `routing.js` 只看 `resolved.gate` 是否存在来画一个「门禁」框（注释写明「一个框代表三次运行」）；
  `task.js` 读 `task.get.gate`；Web 面板不读配置。
- **现有 e2e 样板**（`crates/automed/tests/end_to_end.rs`）：`write_gate_script` / `run_gated_task`、
  `a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again`（MergeGate 失败回路，
  修好后复盘重跑一次）、`stopped_gate_resumes_at_gate_and_discards_killed_result`（让门禁脚本阻塞、停在 Gate 的样板）、
  `bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate`。`World::configure` 只往配置文件末尾追加。
- **基线用例数**（`28dad46`，`-- --list` 计）：autome-domain 单测 384 + 集成 1；automed lib 650；end_to_end 112；
  prompt_rendering 16。下文验收命令用到的过滤器在基线上的命中数：automed lib `config_io` 40、`scheduler` 87、`gate::` 11、
  `brief` 30、`launcher` 57、`protocol::` 61；end_to_end `gate` 11、`bug_loop` 14。现有用例名里没有含 `before_merge` 的——
  新增用例名一律含 `before_merge`，并按里程碑用不重叠的前缀：M-01 的 automed 用例在 `config_io::tests::before_merge_*`；
  M-02 的 e2e 叫 `feature_before_merge_*`；M-03 的 e2e 叫 `bug_loop_before_merge_*`、单测在 `scheduler::tests::before_merge_*`
  与 `gate::tests::before_merge_*`；M-04 的 e2e 叫 `brief_before_merge_*`、单测在 `brief::tests::before_merge_*`；M-05 的用例
  一律叫 `before_merge_prompt_*`（e2e 叫 `prompt_before_merge_*`）；M-06 的 e2e 叫 `protocol_before_merge_*`、金样用例叫
  `before_merge_*`。各契约里的「共 N 个」是该里程碑完成时的总数，后续里程碑会再往上加。

## 目标与非目标

目标：

1. 项目 `[gate]` 新增 `run = "each_round" | "before_merge"`，缺省 `each_round`；其它值解析时报中文错误（目标 1、约束 2）。
2. `before_merge` 下需求任务实现轮结束后的转换与「没配门禁」逐字段相同；MergeGate / MainCheck 照旧（目标 2、约束 3、5）。
3. `before_merge` 下 Bug 任务 FixGate 只跑复现命令，MergeGate 仍跑两者（目标 3、约束 4）。
4. 只跑了复现命令的 FixGate、或中途切换 `run`，都不能让 MergeGate 被错误沿用（约束 6、10）；从未启用 `before_merge` 的任务，记账路径与今天逐字节相同（约束 1）。
5. 审计简报与 Bug 复核轮 prompt 在 `before_merge` 下没有本轮结果时写明「本项目门禁只在合并前运行，本轮没有宿主门禁结果」（目标 4、约束 8）。
6. **完整的项目测试体系每个任务只跑一次——合并前、由宿主跑**（意见 1–4）。`before_merge` 下：实现轮、修复轮只跑本里程碑验收命令
   （Bug 是复现命令）与本轮改动直接相关的测试子集；审计轮、复核轮只做验收契约的负向对照与复验、自己构造的判别检查、相关测试
   子集，不因「没跑全量」而不给结论；设计轮不把「跑全量门禁」写成任何里程碑的验收命令；每个会话的 prompt 里由宿主渲染同一句
   「全量门禁（`<门禁命令原文>`）由宿主在合并前统一运行，本轮不要自己跑。」；MergeGate 失败后修复轮收到的注入文字不再要求它
   自己重跑整条门禁命令。
7. `each_round` 与没配门禁时，所有 prompt、简报逐字节不变：现有 12 份金样不动，新增 `before_merge` 的金样（意见 5）。
8. 协议升到 `protocol/v3`：五份模板（plan、impl、audit、fix、fix-review）加 `before_merge` 段，CHANGELOG 新节、protocol eval
   认得分段并校验它，五个行为用例（意见「范围」）；`bug-protocol.md` 第 40 行补一句「`before_merge` 下修复门禁只跑复现命令」
   （Backlog B-02，用户 2026-11-12 纳入，按协议单列为里程碑 M-07）。
9. README 与技术设计 §8.1 写明取舍：门禁次数降到每个任务一次；门禁问题与回归到合并前才由宿主统一发现、会多退回一次实现轮，
   会话不再各自兜底；Bug 修复轮只跑复现命令（目标 5、约束 12、意见 6）。

非目标（任务文件 §4「不在范围内」照抄，按意见调整的两条单独标出）：

- `each_round` 下任何行为不变（含 prompt 与简报的每一个字节）；不做跨任务合并队列；不改 MainCheck（T-60）与门禁排队（T-47）；
  不改 `scripts/ci-local.sh`；不改本仓库 `.autome/`。
- 「不改会话自己跑测试的方式」——**按意见调整**：只在 `before_merge` 下、只经 prompt 模板与宿主注入的文字改；`each_round` 不变。
- 「不改 `protocol/`」——**按意见调整**：改 `protocol/prompts/` 五份模板、`bug-protocol.md` 一句、`CHANGELOG.md`、`README.md`
  一句与 `evals/` 新增五个用例；**不改** `loop-protocol.md` 与 `session-protocol.md`（理由见方案 §6.6）。
- 不新增 JSON-RPC 方法、不改参数、不新增事件种类；账本 schema 不变。
- 不改 `plugins/`、`apps/desktop`、`apps/web`。逐条理由见方案 §8。
- 不给 Bug 复核轮新增简报文件；复核轮的口径写在它的 prompt 里（方案 §6.4）。
- 已固定在 `protocol/v2` 及更早版本的任务，模板里的会话规则不变（协议「共同原则」第 6 条）；宿主一侧的行为（不进 `Gate`、
  FixGate 只跑复现、审计简报的门禁证据、修复轮注入文字）对它们同样生效（方案 §6.8）。

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
     提供 `set_repro_only(bool)`（写入或删除，必要时先建目录）和 `repro_only() -> bool`。**`Job::clear` 的清单不加这一项**：
     `gate::start` 在写 `.started` 之前会调用 `job.clear()`（`gate.rs::start`：建目录 → `job.clear()` → 写 `.started` → 起进程），
     标记若在 `clear` 的清单里，`run_gate` 先写的标记就会被 `start` 自己删掉（评审 #2 R2-P01 的反例）。标记的生命周期因此只有两个
     操作者：`run_gate` 在每次启动前确定性地写或删；`discard_node` 的后缀清单加 `.repro-only`（丢弃整个节点的作业时一并清掉，
     不留孤儿）。`Job::clear`（`start` 内部、`Vanished`、head 移动三处调用）与 `start / poll` 的行为都不变。
  2. `run_gate` 在 `!job.exists()` 分支里、`gate::start` **之前**调用 `job.set_repro_only(x)`（它排在 `.started` 与进程之前，
     且 `start` 不碰它，所以任何存在 `.started` 的 `before_merge` 作业都带着标记），其中
     `x = bug && node == FixGate && resolved.gate.is_some() && resolved.gate_run == BeforeMerge`（即「这一次 FixGate 只拼了复现命令
     是因为新模式」）。无项目门禁的 Bug 任务本来就只跑复现命令，`x = false`，不写标记。标记是确定性地写或删，所以前一个作业留下的旧标记
     （`Vanished` / head 移动后 `clear` 不删标记）、或「写了标记但没来得及启动」的残留，都会在下一次启动前被覆盖；没有 `.started`
     的残留标记没有读者（记账只在作业存在时读标记）。
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

### 6. 宿主门禁口径：简报、prompt 与修复轮注入（目标 5、6、7；约束 8；意见 1–5）

#### 6.1 判定

「本轮门禁结果」= 最后一条 `gate.result` 且其 `sha` 等于写简报时的 `gate_head(layout)`（与门禁绑定 head 的同一个函数：
单仓库是检出 head，工作区是各仓库 head 的 sha256）。简报在本轮会话启动前、简报自己的提交之前计算，此时 head 就是
被审计的那一轮留下的提交。

#### 6.2 审计简报（`brief.rs` 新增纯函数，调度器调用）

`render_gate_line` 原样从 `scheduler.rs` 移到 `brief.rs`（改名 `gate_line_of_result`，`pub`，文字不改一个字），新增：

```rust
pub const GATE_ONLY_BEFORE_MERGE: &str = "本项目门禁只在合并前运行，本轮没有宿主门禁结果。\n";
pub fn gate_evidence(config: &ResolvedConfig, last: Option<&Value>, head: Option<&str>) -> Option<String>
```

- `config.gate` 为 `None`，或 `gate_run == EachRound` → `last.and_then(gate_line_of_result)`，与今天逐字节相同（约束 1）。
- `gate` 有、`gate_run == BeforeMerge`：`last` 的 `sha` 等于 `head` → `gate_line_of_result(last)`（例如 MergeGate 失败后实现轮没有
  新提交，那条失败结果确实就是本轮的）；否则（没有结果、结果是别的提交的、读不到 head）→ `Some(GATE_ONLY_BEFORE_MERGE)`。

调度器：`write_brief` 的调用方（已持有 `resolved` 与 `layout`）对审计角色算出 `gate_evidence(&resolved, last, gate_head(&layout).ok())`
传入；其它角色仍是 `None`。`brief::build` 不改——`Some` 时照旧出 `## 宿主门禁` 段，所以 `before_merge` 下审计简报永远有这
一段，不会缺失，也不会把上一次 MergeGate 的旧结果当本轮证据。简报的其余部分（含摘自协议的小节）在两种模式下都不变。

#### 6.3 宿主渲染的那一句（意见 3）

`launcher.rs` 新增：

```rust
/// before_merge 下每个会话都收到的同一句话，门禁命令原文照抄。
pub fn gate_line(command: &str) -> String {
    format!("全量门禁（`{command}`）由宿主在合并前统一运行，本轮不要自己跑。")
}
```

`PromptSpec` 新增 `gate_before_merge: Option<&'a str>`——**项目有门禁且 `run = "before_merge"` 时为 `Some(门禁命令)`，其余一律
`None`**。取值由 `autome-domain` 的 `ResolvedConfig::gate_before_merge(&self) -> Option<&str>` 给出（`gate` 为 `None` 或
`gate_run == EachRound` → `None`），调度器 `start_session` 对所有角色与 Bug 会话传 `resolved.gate_before_merge()`，
`start_onboarding` 传 `None`。这句话出现在两处，文字都来自 `gate_line()`：

1. 模板里 `before_merge` 段的 `{gate_line}` 占位符（§6.4、§6.5）——五份模板各一处；
2. `before_merge` 下修复轮的 `Inject::GateFailed` 文字（§6.7）。

不放进简报：Bug 会话没有简报，eval 用例也没有简报；放在 prompt 里，五个角色是同一条代码路径。

#### 6.4 模板里的 `before_merge` 段：渲染机制

模板里用整行的标记包住只在某种模式下出现的文字，写法与协议已有的 `<!-- kernel-contract: … -->` 同一风格：

```text
<!-- gate-run: each_round -->
……只在 each_round（含没配门禁）时出现的行……
<!-- gate-run: before_merge -->
……只在 before_merge 时出现的行……
<!-- /gate-run -->
```

规则（`autome-domain/src/protocol.rs` 新增纯函数 `select_gate_run(text: &str, run: GateRun) -> Result<String, String>`，不做 I/O）：

- 标记必须独占一行（去掉行尾空白后等于上面三种之一）。标记行连同它的换行符一起删掉。
- 一个 `gate-run: <模式>` 段从它的标记行开始，到下一个 `gate-run:` 标记或 `/gate-run` 为止；`each_round` 段与 `before_merge`
  段可以单独出现，也可以相邻成对出现，最后都必须由 `/gate-run` 收尾。模式名只认 `each_round` 与 `before_merge`。
  不成对、嵌套、未知模式 → `Err`，文字写明行号。
- 段内的行只在模式匹配时保留；段外的行原样保留。**一个模板没有任何标记时，两种模式的输出都等于原文**——固定在旧版本的
  任务因此不受影响（§6.8）。

`launcher` 的 `role_prompt` 与 `bug_prompt` 在做任何替换之前先调用它：`gate_before_merge` 为 `Some` 用 `BeforeMerge`，否则用
`EachRound`；`bug_prompt` 在按 `{request}` 切分之前调用（用户原文里的字符不会被当成标记）。

**门禁命令是数据，不是模板（评审 #2 R2-P02）。** 命令原文可以含花括号（如 `ci_script=scripts/ci-local.sh; sh "${ci_script}"`，或字面的
`{slug}`），`config_io` 只要求它非空。所以 `gate_line(cmd)` 的结果**不能**参与 `{doc_dir}` / `{slug}` 等替换，也不能进
`check_no_placeholders_left`——与 `bug_prompt` 对用户 `{request}` 的做法同源：**先切分、逐段验证、最后拼入**。实现为把模板在数据槽
（`{request}`，仅 Bug；`{gate_line}`，仅 `BeforeMerge`）处切开，每一段单独做已知占位符替换并 `check_no_placeholders_left`，再用对应数据
（用户需求原文 / `gate_line(cmd)`）把各段按原顺序拼回；拼回的数据不再被扫描或替换。`role_prompt` 同样改为「按 `{gate_line}` 切分 → 逐段
替换并验证 → 拼入」（它没有 `{request}` 槽）。`EachRound` 时 `before_merge` 段已被删掉、`{gate_line}` 不是数据槽，模板若在段外写了
`{gate_line}`，它留在某一段里，`check_no_placeholders_left` 照旧报错——真正的未知占位符仍然被拒绝。`eval_run::render_prompt` 同样：
先替换模板里的已知占位符，最后才把 `gate_line(cmd)` 拼入。`Inject::GateFailed` 与技能行本来就在模板渲染之后由代码追加（§6.7），
命令原文照抄，不受影响。intake 与 onboarding 不经过这一步（它们不认 `gate_before_merge`）。

于是 `each_round` / 没配门禁时的输出 = 删掉所有 `before_merge` 段与标记行之后的模板，与今天的模板逐字节相同——这正是现有 12 份
金样一份不改就能通过的原因（§6.9）。

**protocol eval 第 1 层**（M-05）：

- `KNOWN_PLACEHOLDERS` 加 `gate_line`（9 → 10）。
- 每份 prompt 模板（六个角色、三个 Bug、intake、onboarding）在两种模式下 `select_gate_run` 都必须成功，否则失败并报行号。
- `select_gate_run(text, EachRound)` 的结果里不得再有 `{gate_line}`——即 `{gate_line}` 只能写在 `before_merge` 段里。

**eval 用例**（M-05）：`case.yaml` 新增可选字段 `gate_command`（封闭字段 9 → 10）。`eval_run::render_prompt` 增加这个参数：
有值时按 `BeforeMerge` 选段并填 `{gate_line}`，没有时按 `EachRound`——现有 14 个用例没有这个字段，渲染结果不变。

#### 6.5 五份模板的 `before_merge` 段（M-06，`protocol/v3`）

下面「原行」都是今天模板里的一整行，改法是把它包进 `each_round` 段、紧接着写 `before_merge` 段；「插入」是只有
`before_merge` 段、没有 `each_round` 对应物。`{gate_line}` 渲染为 §6.3 那一句。

- **`prompts/plan.md`**（意见 4）。在「每个里程碑在表外有一个固定小节……三行缺一，评审轮会判「里程碑不可执行」。」那一行之后插入：

  ```text
  <!-- gate-run: before_merge -->
  - {gate_line}全量回归由合并前门禁承担：不要把跑全量门禁写成任何里程碑的验收命令，也不要单设「全量回归」里程碑；验收命令只写能区分本里程碑对错的测试子集。
  <!-- /gate-run -->
  ```

- **`prompts/impl.md`**（意见 1）。在「本轮产出：推进**编号最小的「开放」里程碑**……一轮只推进一个里程碑。」段落与其后的空行之后插入：

  ```text
  <!-- gate-run: before_merge -->
  {gate_line}本轮只跑本里程碑的验收命令和与本轮改动直接相关的测试子集，自审清单与「用例总数」都以这些命令为准；不要自己跑项目门禁命令。

  <!-- /gate-run -->
  ```

- **`prompts/audit.md`**（意见 2）。原行「复验范围只有三样：项目测试体系、本里程碑的验收命令、本轮自己新造的检查。项目测试体系全量跑
  一次即可……下一轮实现轮要把它搬进项目测试体系。」包进 `each_round` 段；`before_merge` 段为同一行，只把开头两句换成：

  > 复验范围只有三样：本里程碑验收契约的负向对照与验收命令、本轮自己新造的判别检查、与本里程碑改动直接相关的测试子集。{gate_line}也不要因为没跑全量而不给结论：全量回归由合并前门禁承担，简报所摘协议里「项目测试体系（全量跑一次即可……）」一句在本项目不适用。

  其后「**不要重跑前几轮留在 `.autome/output/` 里的临时检查**……」直到行尾与原行逐字相同。

- **`prompts/fix.md`**（意见 1）。原行「以最小改动修复根因，让定位时的同一个复现命令通过，再运行配置的项目门禁。不得修改、删除、跳过或
  弱化定位轮的回归测试。」包进 `each_round` 段；`before_merge` 段：

  > 以最小改动修复根因，让定位时的同一个复现命令通过，再跑与本轮改动直接相关的测试子集。{gate_line}不得修改、删除、跳过或弱化定位轮的回归测试。

- **`prompts/fix-review.md`**（意见 2、3；目标 4 对复核的要求）。原行「独立复验复现命令与项目门禁，并检查定位到修复的提交差异：根因是否
  站得住、回归测试是否有意义、改动范围是否最小。」包进 `each_round` 段；`before_merge` 段：

  > 本项目门禁只在合并前运行，本轮没有宿主门禁结果；修复门禁只跑了复现命令，它通过不等于项目门禁通过。{gate_line}独立复验复现命令和与本轮改动直接相关的测试子集，不要因为没跑全量而不给结论；并检查定位到修复的提交差异：根因是否站得住、回归测试是否有意义、改动范围是否最小。

  这取代上一版 §6.3 的「宿主在复核 prompt 末尾追加说明」：同一口径，去掉了「请你自己复验项目门禁」；不再需要
  `PromptSpec.gate_note` 与 `brief::bug_review_gate_note`。固定在旧版本、模板里没有这一段的任务，由 §6.8 的一条宿主兜底补同一件事实。「本项目门禁只在合并前运行，本轮没有宿主门禁结果」与审计简报的
  `GATE_ONLY_BEFORE_MERGE` 是同一句（M-06 的 phrase 规则钉住）。这段文字不引用任何账本事件，所以不会把上一次 MergeGate 的失败
  当作本轮证据；`before_merge` 下复核轮之前宿主只跑过只含复现命令的 FixGate，这句话因此总是成立。

- **`bug-protocol.md` 第 40 行**（Backlog B-02，用户 2026-11-12 纳入；单列为 M-07，不在 M-06 里做）：

  > 宿主执行复现命令及配置的项目门禁，任一失败退回修复；项目 `[gate] run = "before_merge"` 时修复门禁只跑复现命令，项目门禁留到合并前门禁。预算耗尽由 Autome 停下，用户可追加轮次。

  这份文件不进 prompt、不进简报、不计入大小上限，改它不影响任何渲染字节。同一里程碑在 `CHANGELOG.md` 的 `## protocol/v3` 节
  （M-06 建立）追加 G-6 条目，并在 `phrases.rs` 的 `REQUIRED` 加一条：`bug-protocol.md` 必含「修复门禁只跑复现命令，项目门禁留到合并前门禁」
  （`why` 引用 B-02 与目标 3），让这句话受 protocol eval 第 1 层保护。

review、adjudicate、retro、locate、intake、onboarding 六份模板不改：前三者不跑测试；定位轮的任务是写一个稳定失败的回归测试，
它的 prompt 本来就没有让它跑项目门禁。

T-62 同时在改 `audit.md` / `fix-review.md` 的范围规则：后合入的一方 rebase 时把两边文字合进同一行——`each_round` 段取 T-62 的
新文字（即 T-62 合入后 `each_round` 的金样），`before_merge` 段在 T-62 的新文字上做上面同样的替换（R5）。

#### 6.6 为什么不改 `loop-protocol.md` / `session-protocol.md`

`loop-protocol.md`「审计造的检查归谁」一节写着「复验范围就三样：项目测试体系（全量跑一次即可，不必反复单跑子集）……」，
它被逐字摘进审计与实现轮的简报。改它会改变 `each_round` 下的简报字节（意见 5），而且两份文件合计 24446 / 24576 字节，
只剩 130 字节。所以这一节保持原样，由 `audit.md` 的 `before_merge` 段明写「简报所摘协议里那一句在本项目不适用」——prompt 是
本轮的直接指令，比简报里的通则更具体。

#### 6.7 修复轮的注入文字（意见 1）

`Inject::GateFailed` 的文字由 `launcher::build_prompt` 写，不在模板里。今天它要求修复轮「在同一命令下确认通过后提交」（需求）/
「修复后用同一命令确认通过并提交」（Bug），也就是自己再跑一遍整条门禁命令。`gate_before_merge` 为 `Some(cmd)` 时换成下面两段，
为 `None` 时文字一字不改：

- 需求：

  > Autome 在提交 `{short}` 上运行了项目门禁 `{command}`，没有通过。输出末尾（原文）：（尾部代码块，与今天相同）本轮先从输出里找出失败的用例或步骤，单独复现并修好它，再跑与本轮改动直接相关的测试子集确认通过后提交，然后照常结束会话。{gate_line}本轮是修复轮，不推进新的里程碑：已是 `待审` 的里程碑保持 `待审`，implementation-round 照常加一，照常写本轮的证据文件。门禁结果以 Autome 在合并前重跑为准，不以你在证据里写的结论为准。

- Bug：

  > Autome 在提交 `{head}` 上运行门禁 `{command}`，没有通过。输出末尾（原文）：（尾部代码块，与今天相同）修复后用复现命令和与改动直接相关的测试子集确认通过并提交；{gate_line}不要修改回归测试。

  FixGate 在 `before_merge` 下只跑复现命令，它失败时 `{command}` 就是复现命令，这段话同样成立。

这是宿主代码，对固定在任何协议版本的任务都生效。

#### 6.8 固定在旧版本的任务

协议「共同原则」第 6 条：本次运行始终使用任务里那一份规则。固定在 `protocol/v2` 的任务没有分段标记，`select_gate_run` 原样返回，
它的会话规则保持 v2 的样子（会话自己全量跑——多花机器时间，但不会做错事）。宿主一侧的五件事——不进 `Gate`（§2）、FixGate 只跑
复现命令（§4）、审计简报的门禁证据（§6.2）、修复轮注入（§6.7）、下面的旧 Bug 复核说明——不看协议版本，对它们同样生效。用户在本任务合并后才给本仓库加
`run = "before_merge"`，之后新建的任务固定 `v3`。

**旧 Bug 任务的复核说明（评审 #2 R2-P03）。** 宿主让 FixGate 只跑复现命令，对固定在 v2 的 Bug 任务同样生效；v2 的 `fix-review.md` 没有
缺证说明，`FixGate` 通过后的复核轮也没有失败注入（`bug_loop.rs` 以 `None` 启动），约束 8「复核没有本轮门禁结果时必须写明」会因此漏掉这类任务。
兜底做在 `launcher::bug_prompt`，不新增 `PromptSpec` 字段（`gate_before_merge` 已经在手）：`step == FixReview`、
`spec.gate_before_merge` 为 `Some`、且**模板原文不含 `<!-- gate-run: before_merge -->`**（即新版模板已经自带 §6.5 的说明，不再重复）
时，在渲染结果之后追加一行 `BUG_REVIEW_GATE_NOTE`：

> 本项目门禁只在合并前运行，本轮没有宿主门禁结果；修复门禁只跑了复现命令，它通过不等于项目门禁通过。

这行只陈述事实，不要求复核轮改变自己的复验范围（v2 模板仍让它自己复验项目门禁，规则按协议冻结不变）；它与 §6.5 `fix-review.md` 的
`before_merge` 段前半句同文。`gate_before_merge` 为 `None`（`each_round`、没有门禁）时不追加，输出逐字节不变。新版（v3）模板含该标记，
渲染不变。

#### 6.9 金样

- **现有 12 份**（`golden/prompts/*.md`）一份不改：`each_round` / 没配门禁的输出与今天逐字节相同。`every_role_prompt_renders_…`
  的刷新分支今天写入模板原文；模板有了分段之后，原文不再等于 `each_round` 的基准，所以刷新分支改为写入
  `select_gate_run(模板, EachRound)`（保留 `{budget_line}` 等占位符，与今天的基准同形）。断言不改（§10）。
- **新增 `before_merge` prompt 金样 5 份**：`golden/prompts/{plan,impl,audit,fix,fix-review}-before-merge.md`，门禁命令取固定的
  `make check`，占位符按现有 `rendered()` / `bug_rendered()` 的哨兵还原。比对用例在 `tests/prompt_rendering.rs`：
  `before_merge_role_prompts_match_their_goldens`（plan、impl、audit）、`before_merge_bug_prompts_match_their_goldens`（fix、
  fix-review），以及 `before_merge_leaves_the_other_prompts_byte_identical`（review、adjudicate、locate、locate-card、intake、
  onboarding 在 `Some("make check")` 下的渲染等于各自现有金样；retro 没有金样，比对 `Some` 与 `None` 两次渲染相等）。沿用 `golden()` / `maybe_update_golden()`，这两个函数不改。
- **审计简报金样**：新目录 `golden/briefs/`，文件 `audit-before-merge.md`：用固定输入调用 `automed::brief::build`，其中
  `gate = gate_evidence(before_merge 配置, 一条 sha 不同的旧 merge_gate 失败结果, Some("<head>"))`，整份简报逐字节比对。比对用例
  `before_merge_audit_brief_matches_its_golden` 放在 `tests/prompt_rendering.rs`，新增一对只读写 `golden/briefs/` 的辅助函数，
  同样受 `AUTOME_UPDATE_GOLDEN=1` 控制。
- `tests/golden/README.md` 补两段：`*-before-merge.md` 是 `before_merge` 模式的渲染结果；`briefs/` 是什么。

### 7. 协议版本与 protocol eval（目标 8；M-06）

- `CHANGELOG.md` 末尾新增 `## protocol/v3`，一句说明（「`[gate] run = "before_merge"` 下会话不再自己跑全量门禁」）与六个条目
  （G-1..G-5 由 M-06 写入；G-6 是 B-02 那一句，由 M-07 追加到同一节）：

  | id | kind | clause | eval |
  |---|---|---|---|
  | G-1 | behavioral | `prompts/impl.md` | `evals/impl-before-merge-runs-only-the-milestone-checks/` |
  | G-2 | behavioral | `prompts/audit.md` | `evals/audit-before-merge-concludes-without-the-full-gate/` |
  | G-3 | behavioral | `prompts/plan.md` | `evals/plan-before-merge-keeps-the-full-gate-out-of-milestones/` |
  | G-4 | behavioral | `prompts/fix.md` | `evals/fix-before-merge-runs-only-the-repro-and-related-tests/` |
  | G-5 | behavioral | `prompts/fix-review.md` | `evals/fix-review-before-merge-concludes-without-the-full-gate/` |
  | G-6 | clarify | `bug-protocol.md` | null |

  证据：`[T-35 一个任务过了 16 次门禁, T-37 过了 8 次, 2026-11-12 用户驳回 门禁只在合并前跑一次-gate-run 设计]`；
  `predicted_impact: {metric: total_turns, direction: down, scope: task, horizon: 3}`（G-6 为 `direction: flat`）。
- 五个行为用例各一个目录：`case.yaml`（`gate_command: sh full-gate.sh`；fix / fix-review 用 `role: impl|audit` + `prompt:`，与现有
  Bug 用例同写法）、`scaffold.sh`、`fixture/eval-case/`（一个最小任务目录，里程碑验收命令是 `sh check-m01.sh`；`full-gate.sh` 每次被
  执行就往 `.autome/output/full-gate-ran` 追加一行）、`graders/behavior.md`。grader 机检：`full-gate-ran` 不存在、transcript 里没有执行
  `full-gate.sh`；同时本轮该做的事做了（实现轮标 `待审`、审计轮动了里程碑表、设计轮的验收命令里没有 `full-gate.sh`、修复轮写了修复说明、
  复核轮写了 `复核: 通过|打回`）。阳性对照：会话跑了 `full-gate.sh`（即 v2 模板 / 没有 `gate_command` 时的行为）→ 判 0。
- `phrases.rs` 的 `REQUIRED` 加：plan / impl / audit / fix / fix-review 五份模板必含 `<!-- gate-run: before_merge -->`；
  fix-review 必含「本项目门禁只在合并前运行，本轮没有宿主门禁结果」；audit 与 fix-review 必含「不要因为没跑全量而不给结论」。
  每条的 `why` 引用意见 1–3。
- `protocol/README.md`「文件」表 `prompts/*.md` 一行补一句：`<!-- gate-run: … -->` 包住的行按项目 `[gate] run` 取舍。
- 版本断言两处 `protocol/v2` → `protocol/v3`（§10）。行为用例要真实模型、要花钱，不做里程碑验收，列入人工验收清单 H-02。

### 8. 读者（约束 9；任务 §4 末条）

| 读者 | `before_merge` 下看到的 | 处置 |
|---|---|---|
| `task.get.gate` / `autome status` 门禁行 | 最后一条 `gate.result`；合并前没有结果时为 null，不打印 | 不改 |
| `autome run` 事件输出 | 只少了 `node: gate` 的 `gate.started/result`；切换时多一条已有种类 `gate.skipped` | 不改 |
| `project.get.config` / `config.get.resolved` | 多 `"gate_run": "before_merge"`；`each_round` 下无此键 | 不改（读者按键取值，不枚举键） |
| 桌面端 `routing.js` 路由图 | 仍画「实现 → 门禁 → 审计」一个框（注释：一框代表三次运行） | 不改（B-01，用户 2026-11-12 不纳入） |
| 桌面端 `task.js` 主机标签 | 读 `task.get.gate`，同第一行 | 不改 |
| Web 面板 | 不读配置，只有节点名映射 | 不改 |
| 会话（prompt） | 五份模板的 `before_merge` 段与 `gate_line` | 改（§6.5，M-06） |
| `protocol/bug-protocol.md` 第 40 行 | 补 `before_merge` 一句 | 改（B-02 纳入，M-07） |
| 协议页（`version_page.rs`） | 新版本 `protocol/v3` 一组 | 不改（按 CHANGELOG 读，不枚举版本） |

路由图不改的理由：它是配置编辑界面上的示意图，不是运行状态；它本来就把 MergeGate、MainCheck 画在同一个框里，框上的
副标题「合并前、合并后再跑」在 `before_merge` 下仍然成立；`apps/desktop` 在任务范围外。桌面端两份白名单与 `READ_METHODS`
不变（不新增方法）。

### 9. 文档（目标 9；约束 12；意见 6）

- `README.md`「Host gate, retries and cooldowns」：`[gate]` 示例加一行
  `run = "each_round"          # the default; "before_merge" runs the full gate only once, before the merge`；
  「The gate.」后补一段英文：`before_merge` 把门禁次数从每轮一次降到每个任务一次（once per task）；代价是门禁问题与回归要到合并前
  才由宿主统一发现（not until the merge），会多退回一次实现轮；会话不再各自兜底（sessions do not run the full gate themselves）——
  实现、审计、修复、复核轮只跑里程碑验收命令或复现命令与相关子集；Bug 修复轮在 `before_merge` 下只跑复现命令（only the repro command），
  完整门禁留给合并前门禁。
- `docs/development/technical-design.md` §8.1「宿主门禁」段后补一段中文，含：`[gate] run`、门禁次数从「每轮一次」降到
  「每个任务一次」、门禁问题与回归到合并前才暴露、会多退回一次实现轮、会话不再各自兜底（五份模板的 `before_merge` 段与
  `gate_line`）、Bug 修复轮只跑复现命令、审计简报与复核 prompt 的文案、切换时的 `gate.skipped`。
- B-03（「每个任务一次」的限定）用户 2026-11-12 不纳入；写作时照常描述 MergeGate 与 MainCheck，不额外承诺。

### 10. 现有测试改动清单（约束 1）

**断言一行不改**，以下三类除外，逐条列出：

1. **`launcher::PromptSpec` 新字段 `gate_before_merge`**（M-05）。`PromptSpec` 没有 `Default`，4 处字面量各补一行
   `gate_before_merge: None,`：`scheduler.rs` 的 `start_session`（传 `resolved.gate_before_merge()`）与 `start_onboarding`（`None`）
   两处生产代码；`launcher.rs` 测试模块里的 `PromptSpec { .. }`（约 1128 行）与 `tests/prompt_rendering.rs` 的 `spec()` 两处测试代码。
   后两处只加这一行。理由：结构体多了一个字段导致构造处编译不过——约束 1 明文允许。
2. **版本号断言 `protocol/v2` → `protocol/v3`**（M-06）。`protocol/mod.rs` 的
   `the_version_in_force_is_the_newest_changelog_heading_over_the_seed` 与 e2e
   `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task` 各一处。理由：这两处断言钉的是「当前协议版本号」，
   意见明确要求升协议版本，改的是期望的版本号本身，不涉及 `each_round` 下任何行为；同一用例的其它断言不动。
3. **`every_role_prompt_renders_exactly_what_the_constants_did` 的刷新分支**（M-06）。只在 `AUTOME_UPDATE_GOLDEN=1` 时执行的那一行，
   从写入模板原文改为写入 `select_gate_run(模板, EachRound)`（§6.9）。比对断言不改。

配置类新字段都是旁挂 `Option` / 有 `Default`；`render_gate_line` 是私有函数，没有测试直接调用；`brief::build`、`golden()` 不改签名；
`gate::Job` 只在 `Job::at` 里构造（一处，生产代码）；`case::Case` 在生产代码由 `case::parse` 构造，`eval_run::tests::case()` 另有测试字面量，新增字段需补 `None`；`eval_run` 的既有渲染测试调用需适配新增参数与 Result 返回值，原断言不变。实现期若发现还有其它必须改动的现有用例，
在当轮证据里逐条列出，理由不能是「新行为与旧断言不符」。

### 11. 否决的做法

- **把 `run` 放进 `GateConfig`**：改 5 处字面量，违反约束 1。
- **在 Bug 状态机里让 FixGate 在 `before_merge` 下被跳过**：违反约束 4（节点序列不变）。
- **把命令哈希编进作业文件名以修 §5 的漏洞**：会改 `each_round` 下的作业路径，`end_to_end.rs` 里有用例按
  `Job::at(.., "gate", head12)` 拼路径。
- **为 MainCheck 计数而关掉它**：任务明令禁止；改用 `AUTOME_GATE_NODE` 与账本 `node` 字段区分。
- **FixGate 的命令从 `gate.started` 事件回溯**：`gate::start` 与事件两步不原子，崩溃窗口里没有事件；同 node/head 的旧作业被丢弃重启时
  还会取到上一次运行的命令；并且无条件改变了从未启用 `before_merge` 的 FixGate 记账，违反约束 1。改用作业旁标记文件（§5）。
- **把 `gate.started` 写在 `gate::start` 之前来消除窗口**：`gate::start` 失败时会留下一条没有作业的启动事件，改变 `each_round` 下的
  事件序列；且仍要按 node/sha 回溯。
- **宿主在 prompt 末尾追加一段「以下覆盖上文」的说明、模板不动**（上一版 §6.3 的做法推广到所有角色）：模板里「全量跑一次即可」
  「再运行配置的项目门禁」仍在，同一个 prompt 前后矛盾；文字在代码里，不进协议版本，meta 任务改不了，也没有 eval 能测它。
- **新增 `{gate_scope}` 之类的占位符、`each_round` 时由代码把原句填回去**：把协议原文搬进代码，违背「文字在协议里、代码只做替换」
  （`launcher.rs` 对模板的约定）。
- **每种模式各一整份模板**（`prompts/before-merge/audit.md`）：五份全文重复，T-62 这类改范围规则的任务要改两遍。
- **改 `loop-protocol.md`「审计造的检查归谁」**：改变 `each_round` 下的简报字节，且大小上限只剩 130 字节（§6.6）。
- **把那一句只放进简报**：Bug 会话没有简报，eval 用例也没有简报（§6.3）。

## 风险与验证安排

- **R1 MergeGate 的偶发失败代价变大。** `each_round` 下 MergeGate 偶发失败 → 实现轮 → `Gate` 在同一 head 上重跑通过 →
  之后 MergeGate 沿用；`before_merge` 下没有这一步，若实现轮没有新提交，同一 head 上的旧 merge_gate 作业会被直接读回
  （作业文件按 node+head 定位），再记一次同样的失败，直到同因熔断。缓解：协议要求每个实现轮写证据文件并提交（head 必变，
  作业重跑）；熔断后用户有「从门禁重跑」（`DiscardGateRuns`）。门禁总次数下降本身也降低负载超时的概率。这是任务卡接受的
  取舍（「会多退回一次实现轮」），不在本任务里改作业复用规则；写进 README / 技术设计。
- **R2 与 T-47、T-60 的 rebase 冲突。** 两者都改 `run_gate` 一带。缓解：`run_gate` 只加 §3 一段和 §4 的一个 match 臂、§5 的
  命令取值；`render_gate_line` 的移动在约 2060 行，离 `run_gate` 远；`start_session` 只多一个字段；不做无关移动。
- **R3 e2e 计数靠时序。** 不靠：门禁脚本把 `$AUTOME_GATE_NODE` 追加到仓库外的计数文件，断言读文件行；同时断言账本里
  按 `node` 分组的 `gate.started` 数。MainCheck 的那次由 `main_check` 行区分，合并前断言「只有一行 merge_gate」，合并后断言
  「merge_gate 一行 + main_check 一行」。
- **R4 e2e 改配置。** `World::configure` 只追加；中途切换 `run` 的用例直接重写 `.autome/config.toml` 的完整内容并提交
  （写两个 `[gate]` 表会是 TOML 错误）。
- **R5 与 T-62 改同一批模板。** T-62 改 `audit.md` / `retro.md` / `fix-review.md` 的范围规则，本任务把 `audit.md`、`fix-review.md`
  各一整行包进分段。后合入的一方 rebase 时：`each_round` 段取 T-62 的新文字，`before_merge` 段在它上面做 §6.5 的替换，现有金样
  按 T-62 合入后的样子为准、`*-before-merge.md` 重新生成并看 diff。两边若都新增 `## protocol/v3`，后合入的一方把自己那节改成
  `## protocol/v4`，并把 §10 第 2 条的两处断言改成新号。
- **R6 子集选得太窄。** 会话只跑「相关子集」，漏掉的回归要到 MergeGate 才暴露。这正是意见 6 要写明的取舍：回归到合并前才由宿主
  统一发现，会话不再各自兜底；MergeGate 失败走现有回路（§2），修复轮的注入文字（§6.7）指向失败的那部分。
- **R7 固定在旧版本的任务。** 见 §6.8：会话规则不变、宿主行为生效，不会卡住、不会漏跑合并前门禁（约束 10）。
- **R8 行为用例要花钱。** 五个行为用例的第 1 层（可读、带阳性对照、scaffold 存在）在 M-06 由 `protocol eval` 静态检查；真正用模型跑
  的「改前红、改后绿」列入人工验收清单 H-02，由用户在合并前决定是否花这笔钱。
- **验证顺序。** M-01 先把配置与纯状态机钉死（最便宜、其余全依赖它）；M-02 建需求 Loop 的完整端到端链路（任务卡验收 2
  的两个场景是本任务最大的不确定性：真实门禁作业 + 回路节点序列）；M-03 Bug；M-04 审计简报；M-05 先建分段渲染与静态校验，
  此时模板还没有分段，现有金样必须一字不差地通过——这一步单独证明「机制本身不改 `each_round` 的任何字节」；M-06 再改模板、升版本；
  M-07 补 `bug-protocol.md` 那一句（B-02）；M-08 文档。
- **本任务自己的验收命令只跑子集。** 本仓库仍是 `each_round`，宿主在每个实现轮之后跑 `sh scripts/ci-local.sh`（含 fmt、clippy、
  全部 Rust / 桌面端 / Web 测试）。所以下面各里程碑的验收命令只写能区分本里程碑对错的子集；「现有用例一行不改就通过」与任务验收 6
  （fmt / clippy / ci-local 全绿）由宿主门禁的结果证明——审计轮读简报「## 宿主门禁」一行，不自己再跑一遍。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | [gate] run 配置解析与需求状态机转换 | 0 | |
| M-02 | 已完成 | 需求 Loop 端到端：只在合并前跑门禁与停在 Gate 的任务 | 0 | |
| M-03 | 已完成 | Bug Loop：FixGate 只跑复现命令且不被沿用骗过 | 0 | |
| M-04 | 已完成 | 审计简报的宿主门禁证据与金样 | 0 | |
| M-05 | 已完成 | prompt 按门禁模式分段渲染、宿主门禁句与修复轮注入 | 0 | |
| M-06 | 已完成 | 协议 v3：五份模板的 before_merge 段、金样与 eval 用例 | 0 | |
| M-07 | 已完成 | bug-protocol 补 before_merge 下修复门禁只跑复现命令（B-02） | 0 | |
| M-08 | 已完成 | README 与技术设计写明取舍 | 0 | |

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
- **M-02**（覆盖：目标 2；验收 2；约束 3、5、10。前置：M-01）。`run_gate` 加 §3 那一段。end_to_end 新增 4 个，名字都含 `gate`：
  ① `feature_before_merge_two_milestones_run_the_gate_once_before_the_merge`：两个里程碑的任务从创建到 `await_merge`，计数文件只有
  一行 `merge_gate`，账本 `node: gate` 的 `gate.started` 为 0、`merge_gate` 为 1；合并后完成，计数文件为 `merge_gate`、`main_check`
  两行；② `feature_before_merge_a_failing_merge_gate_returns_to_implement_and_merges_after_the_fix`：MergeGate 第一次失败 → 回实现且
  该轮 prompt 含门禁尾部与门禁命令（只断言这两样，不断言注入的其余措辞——M-05 会改它）→ 修好 → 审计 → 复盘 → rebase →
  MergeGate 通过 → 合并完成；断言失败之后的节点序列（`task.updated` 的 `node`）为 `implement, audit, retro, rebase, merge_gate,
  await_merge` 且不含 `gate`；③ `feature_before_merge_switching_while_blocked_on_the_gate_skips_it`：`each_round` 下任务停在一个阻塞的
  `Gate` 上，改写配置为 `before_merge` → 任务进 `audit`、包装进程组已不在、有 `gate.skipped{node: gate}`，之后照常跑 MergeGate 到
  `await_merge`；④ `feature_before_merge_switching_while_stopped_at_the_gate_skips_it_on_resume`：同 ③ 但先 `task.stop`，改配置后
  `task.resume` → 进 `audit`。
- **M-03**（覆盖：目标 3、4；验收 3；约束 1、4、6、7、10。前置：M-01）。`run_gate` 的 Bug 命令 `match`、`gate.rs` 的 `.repro-only`
  标记（`Job::set_repro_only / repro_only`；`discard_node` 的清单加 `.repro-only`，`Job::clear` 不加，见 §5 第 1 点）与 FixGate 结果的命令取值（§4、§5）。新增：end_to_end 2 个——
  ① `bug_loop_before_merge_fix_gate_runs_only_the_repro_and_merge_gate_runs_both`：项目门禁 + `before_merge`，FixGate 的
  `gate.started.command` 等于复现命令（不含 `&&`），MergeGate 的含 `( sh regression.sh ) && (` 与项目门禁命令；项目门禁第一次在
  MergeGate 失败 → 回修复且 prompt 含项目门禁输出 → 修好后 MergeGate 通过到 `await_merge`；②
  `bug_loop_before_merge_an_ineffective_fix_fails_the_fix_gate`：修复没生效 → FixGate 失败回修复，prompt 含 `regression-output` 与 `2/3`。
  automed lib（`scheduler`）4 个——③ FixGate 作业按 `before_merge` 启动（走真实的 `run_gate → gate::start` 路径）：启动后、切换前作业目录里 `.repro-only` 存在且 `.started`
  存在（R2-P01：`start` 内部的 `clear` 没有删掉它）；结束前配置改成 `each_round`：`gate.result.command` 是复现命令，
  失败时 `GateOutcome::Failed.command` 同；④ 接着在同一 head 上跑 MergeGate：没有 `gate.reused`，有一条 `merge_gate` 的 `gate.started`；
  ⑤ 崩溃窗口（D1-P01）：`before_merge` 下启动 FixGate 后删掉账本里的 `gate.started`（模拟标记已写、事件未写），再切 `each_round`、作业
  结束：`gate.result.command` 仍是复现命令，随后同 head 的 MergeGate 不沿用；同一 node/head 先留一个 `each_round` 的完整命令旧事件、再以
  `before_merge` 重启并删去新事件，结果同样记复现命令；⑥ 约束 1 边界（D1-P03）：从未写过 `run`，FixGate 以项目门禁 A 启动、结束前改成 B：
  `gate.result.command` 与今天一样是 `( repro ) && ( B )`，且该作业目录里从未出现 `.repro-only` 文件。gate.rs 另有 1 个单测放在
  `gate::tests::before_merge_*`：`set_repro_only(true/false)` 往返、`gate::start` 之后标记仍在、`Job::clear` 不删它、`discard_node` 会删掉它。
- **M-04**（覆盖：目标 5；验收 5；约束 8。前置：M-02、M-03）。§6.1、§6.2、§6.9 的审计简报金样：`brief.rs` 的 `gate_line_of_result` /
  `gate_evidence` / `GATE_ONLY_BEFORE_MERGE`，调度器的审计简报调用处，`golden/briefs/audit-before-merge.md` 与 `golden/README.md` 的
  `briefs/` 说明。新增：automed lib（`brief::tests::before_merge_*`）3 个——无结果与旧 sha 结果都出常量文案；同 sha 结果出
  `gate_line_of_result`；`each_round` 与「没有门禁 + before_merge」时与 `last.and_then(gate_line_of_result)` 相等。prompt_rendering 1 个
  （`before_merge_audit_brief_matches_its_golden`）。end_to_end 1 个——`brief_before_merge_audit_brief_never_shows_a_stale_result`：
  M-02 ② 的场景里，第一轮审计与 MergeGate 失败修好后那一轮审计的简报都含「本项目门禁只在合并前运行，本轮没有宿主门禁结果」，
  后者不含失败那次的 sha 前 10 位。
- **M-05**（覆盖：目标 5、6、7；意见 1、3、5；约束 1、8、11。前置：M-01）。§6.3、§6.4、§6.7、§6.8 的旧 Bug 复核说明：`autome-domain` 的
  `protocol::select_gate_run` 与 `ResolvedConfig::gate_before_merge()`；`launcher` 的 `gate_line()`、`PromptSpec.gate_before_merge`、
  `role_prompt` / `bug_prompt` 先选段再替换、`GateFailed` 两段 `before_merge` 文字；调度器 `start_session` 传值；§10 第 1 条的 4 处
  字面量；protocol eval 第 1 层的两项新检查与 `KNOWN_PLACEHOLDERS`；`case.yaml` 的 `gate_command` 与 `eval_run::render_prompt`。
  **本里程碑不改任何模板**，现有 12 份金样必须原样通过。新增（名字都含 `before_merge_prompt`）：
  - autome-domain 4 个：`protocol::tests` 3 个——① 没有标记的文本在两种模式下原样返回；② 成对段与单独的 `before_merge` 段在两种模式下
    的输出逐字节等于手写期望（含段前段后的空行、标记行连同换行一起删）；③ 不成对、嵌套、未知模式三种写法都 `Err` 且带行号。
    `config::tests` 1 个——`gate_before_merge()` 只在「有门禁 + before_merge」时为 `Some(command)`，没有门禁 + before_merge、each_round
    都为 `None`。
  - automed lib `launcher::tests` 6 个——① 用带两种段的合成模板起一个角色会话：`Some("make check")` 时输出 `before_merge` 段且含
    「全量门禁（`make check`）由宿主在合并前统一运行，本轮不要自己跑。」，`None` 时输出 `each_round` 段、没有任何标记行；另用门禁命令
    `ci_script=scripts/ci-local.sh; sh "${ci_script}"` 和 `echo {slug}` 各渲染一次（R2-P02）：渲染成功、命令原文逐字出现、`{slug}` 没有被替换成
    任务名；模板段外写了 `{gate_line}` 或别的未知占位符时仍报错；② 同 ① 但走
    `bug_prompt`（含同样两条带花括号的命令），且用户需求原文里写着 `<!-- gate-run: before_merge -->` 时原样保留；③ 需求 `GateFailed` 在 `Some` 下含尾部、`gate_line`，
    不含「在同一命令下确认通过」；在 `None` 下与今天逐字节相同；④ Bug `GateFailed` 同 ③（不含「用同一命令确认通过」）；⑤ intake、
    onboarding 在 `Some` 与 `None` 下输出相同；⑥ 旧 Bug 复核说明（R2-P03）：不含 `<!-- gate-run: before_merge -->` 的 `fix-review` 模板在 `Some` 下
    渲染结果末尾追加 `BUG_REVIEW_GATE_NOTE` 一行、`None` 下与今天逐字节相同；含该标记的模板在 `Some` 下该句只出现模板自带的一次；Fix、Locate 步骤
    不追加。
  - automed lib `protocol::` 5 个——`eval::tests` 3 个：不成对的标记被第 1 层拒绝并报文件与行号；`{gate_line}` 写在段外被拒绝；写在
    `before_merge` 段里的 `{gate_line}` 通过占位符检查。`case::tests` 1 个：`gate_command` 被读到，缺省为 `None`，其它未知字段仍报错。
    `eval_run::tests` 1 个：带 `gate_command` 的用例渲染出 `before_merge` 段与 `gate_line`，不带的渲染出 `each_round` 段，两种都没有标记行、
    没有残留占位符；`gate_command` 含 `{slug}` 时原文保留。
  - end_to_end 2 个——① `prompt_before_merge_feature_repair_round_runs_only_the_failing_part`：`before_merge` 下 MergeGate 失败后的修复实现轮
    prompt 含门禁尾部与 `gate_line`（门禁命令原文），不含「在同一命令下确认通过」；② `prompt_before_merge_bug_repair_round_runs_only_the_failing_part`：
    Bug 任务 MergeGate 失败后的修复轮 prompt 同理，不含「用同一命令确认通过」。
- **M-06**（覆盖：目标 5、6、7、8；验收 5；意见 1–5、范围。前置：M-05）。§6.5、§6.9、§7：五份模板的分段、
  `CHANGELOG.md` 的 `## protocol/v3`（G-1..G-5）、五个行为用例目录、`phrases.rs` 的新条目、`protocol/README.md` 一句、5 份 `before_merge` prompt 金样与
  `golden/README.md` 一段、§10 第 2、3 条。新增：prompt_rendering 3 个（§6.9 那三个）；automed lib 1 个——
  `protocol::eval::tests::before_merge_prompt_seed_segments_render_in_both_modes`：种子的五份模板在两种模式下都能选段，`before_merge` 输出各含一次
  `{gate_line}`、`each_round` 输出不含任何标记；实施提醒（评审 #3 N3-02）：`protocol/eval.rs` 的 `with_behavioral_entry` 与 `a_changed_run_covers_the_new_cases_and_the_three_baselines` 用合成的 `protocol/v3` / C-90 标签，种子新增 `protocol/v3` 后会命中种子节而取不到用例，须把两处合成标签换成不与种子冲突的标签，保留用例内容与断言强度，并按 §10 在实现证据中列出该调整；end_to_end 2 个——① `protocol_before_merge_feature_rounds_are_told_the_host_runs_the_full_suite`：
  `before_merge` 需求任务的设计轮、实现轮、审计轮 prompt 都含「全量门禁（`<门禁命令>`）由宿主在合并前统一运行，本轮不要自己跑。」，设计轮含
  「不要把跑全量门禁写成任何里程碑的验收命令」，审计轮含「不要因为没跑全量而不给结论」且不含「项目测试体系全量跑一次即可」；②
  `protocol_before_merge_bug_rounds_are_told_the_host_runs_the_full_suite`：`before_merge` Bug 任务的修复轮 prompt 含该句与「再跑与本轮改动直接相关的
  测试子集」、不含「再运行配置的项目门禁」；复核轮 prompt 含「本项目门禁只在合并前运行，本轮没有宿主门禁结果」与该句、不含「独立复验复现命令与项目门禁」。
- **M-07**（覆盖：目标 3、8；Backlog B-02（用户 2026-11-12 纳入）。前置：M-06）。§6.5 末条与 §7：`protocol/bug-protocol.md` 第 40 行改为
  §6.5 给出的那句；`CHANGELOG.md` 的 `## protocol/v3` 节追加 G-6（`kind: clarify`、`clause: bug-protocol.md`、`eval: null`、
  `predicted_impact.direction: flat`）；`phrases.rs` 的 `REQUIRED` 加 `bug-protocol.md` 一条。不改任何 prompt 模板、金样、
  `loop-protocol.md` / `session-protocol.md`。不新增用例：新短语由现有 `protocol::eval::tests::the_seed_passes_the_static_and_parse_layers`
  与 `running_against_the_seed_on_disk_is_clean` 校验，`every_requirement_carries_a_reason` 校验它的 `why`。
- **M-08**（覆盖：目标 9；验收 6；约束 12、13；意见 6。前置：M-01..M-07）。README 与技术设计 §8.1 两段（§9）。不新增用例。

## 验收契约

### M-01 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-01-r1-audit.md · 通过 · 审计 #1

- 验收命令：`cargo test -p autome-domain before_merge && cargo test -p autome-domain && cargo test -p automed --lib config_io`
- 预期：autome-domain 过滤 `before_merge` 6 个通过；autome-domain 单测共 390 个通过（基线 384 + 6）；集成共 1 个、默认忽略 1 个（t14_gate_logs 需原始 T-14 日志，非本轮新增忽略）；automed lib 过滤 `config_io` 45 个通过（基线 40 + 5，其中名字含 `before_merge` 的 5 个）；其余现有用例由宿主门禁在本轮 head 上的结果证明通过。
- 负向对照：`task.rs` 实现轮结束处仍写 `ctx.config.gate.is_some()` 时，task ①② 两个用例失败（转换去了 `Gate`）；解析对未知字符串不报错时，「`"sometimes"` 报错」用例失败。

### M-02 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 验收命令：`cargo test -p automed --test end_to_end gate`
- 预期：过滤 `gate` 共 15 个通过（基线 11 个现有门禁 e2e，含 `a_gate_failing_after_the_automatic_rebase…` 与 `stopped_gate_resumes_at_gate…`，加 4 个 `feature_before_merge_*`），现有 11 个一行不改。
- 负向对照：撤掉 M-01 的 `task.rs` 改动时，①② 的 `task.updated` 节点序列含 `gate`，两例失败（§3 跳过逻辑会阻止脚本执行，不能仅靠计数文件识别错误进入 Gate）；不加 §3 那一段时，③ ④ 的任务停在 `gate`（阻塞脚本不退出），断言节点为 `audit` 失败。

### M-03 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-03-r3-audit.md · 通过 · 审计 #3

- 验收命令：`cargo test -p automed --test end_to_end bug_loop && cargo test -p automed --lib scheduler && cargo test -p automed --lib gate::`
- 预期：end_to_end 过滤 `bug_loop` 16 个通过（基线 14 + 2 个 `bug_loop_before_merge_*`）；automed lib 过滤 `scheduler` 91 个通过（基线 87 + 4 个 `scheduler::tests::before_merge_*`）；过滤 `gate::` 12 个通过（基线 11 + 1）。
- 负向对照：Bug 命令 `match` 不看 `gate_run`（FixGate 照旧拼两者）时，① 的 FixGate 命令断言失败、且因 `gate-ready` 缺失 FixGate 先失败使节点序列不符；FixGate 结果仍取当前配置的命令（不看 `.repro-only`）时，③ ⑤ 的 `command` 断言失败、④ 出现 `gate.reused` 而失败；无条件改用 `gate.started` 事件的命令时，⑥ 的 `command` 断言失败；把 `.repro-only` 放进 `Job::clear` 的清单（`start` 内部会删掉它）时，③ 的「启动后标记存在」断言与 gate 单测「`gate::start` 之后标记仍在」失败；`.repro-only` 对所有 FixGate 都写时，⑥ 的「从未出现标记文件」断言失败。

### M-04 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-04-r4-audit.md · 通过 · 审计 #4

- 验收命令：`cargo test -p automed --lib brief && cargo test -p automed --test prompt_rendering && cargo test -p automed --test end_to_end brief_before_merge && cargo test -p automed --test end_to_end a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`
- 预期：automed lib 过滤 `brief` 33 个通过（基线 30 + 3 个 `brief::tests::before_merge_*`）；prompt_rendering 共 17 个通过（基线 16 + 1 个审计简报金样）；e2e 过滤 `brief_before_merge` 1 个通过；现有 `a_failing_gate_sends…`（`each_round` 审计简报含 `## 宿主门禁` 与「通过」）1 个通过、未改动。
- 负向对照：`before_merge` 下仍取最后一条 `gate.result` 不看 sha 时，「旧 sha 结果出常量文案」单测与 e2e 中修好后那一轮审计简报的断言失败（简报里出现失败那次的 sha 与「未通过」）；把常量文案改动一个字时审计简报金样比对失败；`each_round` 下也走新规则（例如无结果时也出常量）时，「each_round 相等」单测失败。

### M-05 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-05-r5-audit.md · 通过 · 审计 #5

- 验收命令：`cargo test -p autome-domain before_merge_prompt && cargo test -p automed --lib launcher && cargo test -p automed --lib protocol:: && cargo test -p automed --test prompt_rendering && cargo test -p automed --test end_to_end prompt_before_merge && git diff --quiet HEAD -- protocol/prompts crates/automed/tests/golden/prompts && test -z "$(git log --format=%h $(git merge-base HEAD main)..HEAD -- protocol/prompts crates/automed/tests/golden/prompts)"`
- 预期：autome-domain 过滤 `before_merge_prompt` 4 个通过；automed lib 过滤 `launcher` 63 个通过（基线 57 + 6）、过滤 `protocol::` 66 个通过（基线 61 + 5）；prompt_rendering 17 个全部通过、金样文件一字未改；e2e 过滤 `prompt_before_merge` 2 个通过；最后两条 git 检查无输出、退出码 0（本分支至此没有改过任何模板与 prompt 金样）。
- 负向对照：`select_gate_run` 删标记行时漏掉换行符，domain ② 的逐字节比对失败；`role_prompt` 忽略 `gate_before_merge`（永远按 `each_round` 选段）时，launcher ① 与 e2e ① 失败（没有 `gate_line`）；调度器给 `PromptSpec` 传 `None` 时，e2e ①② 失败；`GateFailed` 的 `before_merge` 文字仍写「在同一命令下确认通过」时，launcher ③ 与 e2e ① 失败；第 1 层不检查 `{gate_line}` 的位置时，eval「写在段外被拒绝」用例失败；把 `gate_line(cmd)` 先拼进模板再做占位符替换与检查时，launcher ①② 用 `${ci_script}` 的渲染报「不认识的占位符」、用 `{slug}` 的命令被改写而失败；旧模板的 Bug 复核不追加 `BUG_REVIEW_GATE_NOTE` 时，launcher ⑥ 失败，含标记的新模板里该句出现两次时 ⑥ 同样失败。

### M-06 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-06-r6-audit.md · 已完成 · 审计 #6

- 验收命令：`cargo test -p automed --test prompt_rendering && cargo test -p automed --lib protocol:: && cargo test -p automed --test end_to_end protocol_before_merge && cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task && cargo run -q -p automed -- protocol eval protocol/ && for f in adjudicate audit fix-review fix impl intake-card intake locate-card locate onboarding plan review; do git diff --quiet $(git merge-base HEAD main) -- crates/automed/tests/golden/prompts/$f.md || { echo "现有金样被改: $f"; exit 1; }; done`
- 预期：prompt_rendering 共 20 个通过（17 + 3 个 `before_merge_*`），其中现有 16 个比对的 12 份金样相对分支起点一字未改（for 循环无输出）；automed lib 过滤 `protocol::` 67 个通过（66 + 1）；e2e 过滤 `protocol_before_merge` 2 个通过、`every_round_is_handed_a_brief…`（版本断言改为 `protocol/v3`）1 个通过；`protocol eval` 退出码 0，报告里没有「失败」项，且列出 eval 用例 19 个（基线 14 + 5）、CHANGELOG 条目门槛齐全。
- 负向对照：把 `audit.md` 的 `each_round` 段改一个字时，现有 `every_role_prompt_renders_exactly_what_the_constants_did` 失败；`fix-review.md` 的 `before_merge` 段仍写「独立复验复现命令与项目门禁」时，`fix-review-before-merge.md` 金样比对与 e2e ② 失败；某份模板的 `before_merge` 段漏写 `{gate_line}` 时，`before_merge_prompt_seed_segments_render_in_both_modes` 与对应金样失败；CHANGELOG 没加 `## protocol/v3` 时，`every_round_is_handed_a_brief…` 的版本断言失败；某个 behavioral 条目的 `eval` 目录不存在或 grader 没有阳性对照时，`protocol eval` 以 1 退出。

### M-07 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-07-r7-audit.md · 已完成 · 审计 #7

- 验收命令：`grep -qF '修复门禁只跑复现命令，项目门禁留到合并前门禁' protocol/bug-protocol.md && grep -qF 'id: G-6' protocol/CHANGELOG.md && cargo test -p automed --lib protocol:: && cargo run -q -p automed -- protocol eval protocol/ && cargo test -p automed --test prompt_rendering && git diff --quiet $(git merge-base HEAD main) -- protocol/loop-protocol.md protocol/session-protocol.md`
- 预期：两条 grep 命中；automed lib 过滤 `protocol::` 67 个通过（与 M-06 相同，本里程碑不新增用例）；`protocol eval` 退出码 0、报告里没有「失败」项，「必含 / 禁含短语」条数比 M-06 完成时多 1；prompt_rendering 20 个通过（渲染字节不变）；最后一条 git 检查无输出、退出码 0（`loop-protocol.md` / `session-protocol.md` 未改）。
- 负向对照：`bug-protocol.md` 只加了 `REQUIRED` 条目、没改第 40 行时，第一条 grep 以 1 退出，且 `the_seed_passes_the_static_and_parse_layers` 与 `protocol eval`（报 `bug-protocol.md` 缺必含短语）失败；G-6 写成 `kind: behavioral` 而 `eval: null` 时，`protocol eval` 的 CHANGELOG 条目门槛以 1 退出；新 `REQUIRED` 条目的 `why` 不足 20 字节时 `every_requirement_carries_a_reason` 失败。

### M-08 验收契约

最新证据：docs/门禁只在合并前跑一次-gate-run/evidence/M-08-r8-audit.md · 已完成 · 审计 #8

- 验收命令：`for p in 'run = "each_round"' before_merge 'once per task' 'not until the merge' 'sessions do not run the full gate themselves' 'only the repro command'; do grep -qF "$p" README.md || { echo "README 缺: $p"; exit 1; }; done; for p in before_merge 每个任务一次 合并前才暴露 多退回一次实现轮 会话不再各自兜底 只跑复现命令; do grep -qF "$p" docs/development/technical-design.md || { echo "技术设计缺: $p"; exit 1; }; done; cargo fmt --all -- --check`
- 预期：两组 grep 全部命中、无输出；`cargo fmt --all -- --check` 退出码 0。本里程碑不新增用例；全量（autome-domain 394 + 1、automed lib 675、end_to_end 123、prompt_rendering 20，以及 clippy `-D warnings`、桌面端与 Web 测试）由宿主门禁 `sh scripts/ci-local.sh` 在本轮 head 上跑，审计轮以简报「## 宿主门禁」那一行（本轮 head 上「通过」）为任务验收 6 的证据，不自己再跑。
- 负向对照：README 少写「sessions do not run the full gate themselves」一句时 grep 循环打印「README 缺: sessions do not run the full gate themselves」并以 1 退出；技术设计少写「会话不再各自兜底」时打印「技术设计缺: 会话不再各自兜底」并以 1 退出；任一新代码没过 rustfmt 时 `--check` 以非 0 退出。

## 人工验收清单

- H-01 合并后在本仓库 `.autome/config.toml` 的 `[gate]` 加 `run = "before_merge"`，用真实模型跑下一个需求任务：`autome run` 输出里实现轮之后没有「门禁」行，只在 rebase 之后出现一次「合并前门禁」，合并后一次「验证 main」；实现轮与审计轮的会话记录里没有执行 `sh scripts/ci-local.sh`。
- H-02 合并前在本 worktree 跑 `cargo run -p automed -- protocol eval protocol/ --changed`（真实模型，要花钱）：新增的五个 `*-before-merge-*` 行为用例多数通过，三个基线用例不退步。

## Backlog

## 争议项
