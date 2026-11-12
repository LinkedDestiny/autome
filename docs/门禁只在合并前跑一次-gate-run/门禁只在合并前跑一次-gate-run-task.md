# 任务：门禁只在合并前跑一次（`[gate] run = "before_merge"`）

本任务的规则见 `docs/门禁只在合并前跑一次-gate-run/protocol/loop-protocol.md` 与
`docs/门禁只在合并前跑一次-gate-run/protocol/session-protocol.md`。

- 设计文档：`docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run.md`
- 任务卡（用户写的，第一输入）：`docs/门禁只在合并前跑一次-gate-run/门禁只在合并前跑一次-gate-run-card.md`
- 任务整理日期：2026-11-12
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）

## 1. 需求原文

> 门禁只在合并前跑一次：[gate] run = before_merge，实现轮和修复轮之后不再跑完整门禁（Bug 修复轮只跑复现命令）

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§5 照任务卡整理。

## 2. 目标与背景

**背景（任务卡原文）。** 配了 `[gate]` 的需求任务，每个实现轮之后都跑一次完整门禁（Gate），合并前再跑一次
（MergeGate），合并后又跑一次（MainCheck）。Bug 任务每个修复轮之后跑 `( 复现命令 ) && ( 门禁命令 )`（FixGate），
合并前再跑一次。2026-11-12 实测：一天 102 次门禁，共 18.6 小时机器时间，中位数 10–13 分钟一次；T-35 一个任务过了
16 次门禁，T-37 过了 8 次。并行 3 个任务时 3–4 个完整 `ci-local` 同时跑，Rust 测试从约 3 分钟涨到 10 分钟，负载下的
超时误报又把任务退回实现轮（22 次失败占 4.7 小时）。用户拍板：**降低门禁运行次数，开发全部完成后再统一跑门禁，
避免门禁抢占**。

**目标。** 给项目配置加一个开关，让完整门禁只在合并前跑一次。任务卡规定的六件事：

1. **配置项。** 项目 `.autome/config.toml` 的 `[gate]` 新增 `run`：
   - `"each_round"`（缺省）：与今天逐事件相同；
   - `"before_merge"`：实现轮 / 修复轮之后不跑完整门禁，只在合并前（MergeGate）跑。

   其它值在解析时报错，文案与现有 `[gate]` 校验同一风格。全局配置不认这个键。
2. **需求 Loop（`before_merge`）。** 实现轮结束后直接去它在「没配门禁」时会去的地方（审计，或审计关闭时的下一步），
   不进 `Gate` 节点、不写 `gate.started`。所有里程碑完成、复盘、rebase 之后的 MergeGate 照常跑完整门禁；失败照现有
   路径退回实现轮并带 `Inject::GateFailed`，修好后再回到 MergeGate。
3. **Bug Loop（`before_merge`）。** FixGate **只跑复现命令**（验证这一轮修复确实让复现命令通过），不拼门禁命令；
   MergeGate 仍是 `( 复现命令 ) && ( 门禁命令 )`。126/127 不算失败、同因失败熔断等现有规则不变。
4. **简报里的宿主门禁证据。** 审计 / 复核简报：`before_merge` 下没有本轮门禁结果时，简报写明「本项目门禁只在合并前
   运行，本轮没有宿主门禁结果」，不留空、不沿用旧结果。金样测试覆盖。
5. **读者与文档。** `autome status <任务>` 的门禁行、`task.get.gate` 在没有结果时的显示不变；`autome run` 不新增事件。
   README「Host gate」一节与 `docs/development/technical-design.md` §8.1 各补一段，写明取舍：门禁次数从「每轮一次」
   降到「每个任务一次」，代价是门禁问题到合并前才暴露、会多退回一次实现轮。
6. **本仓库自己的配置不在本任务里改。** `.autome/config.toml` 由用户合并后自行加 `run = "before_merge"`。

## 3. 验收标准

以下六条逐字取自任务卡「验收标准」。设计文档的里程碑必须逐条覆盖；用户在设计定稿时会对照它们批准或驳回。

1. 单测（`autome-domain`）：`before_merge` 下实现轮完成后的转换等于没配门禁时的转换；MergeGate 通过 / 失败两条路与
   今天相同；`each_round` 下所有现有转换测试不改。
2. e2e（需求）：`run = "before_merge"`，两个里程碑的任务从创建到合并，门禁脚本恰好被调用一次（MergeGate），事件里
   没有 `node: gate` 的 `gate.started`；让门禁第一次失败 → 退回实现轮且 prompt 带门禁尾部 → 修好后再过 MergeGate → 合并。
3. e2e（Bug）：`before_merge` 下 FixGate 的命令只含复现命令，MergeGate 的命令含两者；修复没生效时 FixGate 失败退回修复。
4. 解析：`run = "sometimes"` 报错；缺省与显式 `each_round` 行为相同；`save_project` 不改写用户的 `[gate]`。
5. 简报金样测试覆盖第 4 条文案。
6. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

补充说明（从任务卡和调研得出，不是新范围）：

- 第 2、3 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、
  真实门禁后台作业，只把模型换成替身）。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
- 第 2 条「门禁脚本恰好被调用一次」要用不依赖时序猜测的证据断言（例如门禁脚本自己记下的调用次数、账本里的
  `gate.started` / `gate.result` 事件）。同一条里的两段是两个场景：一次全程通过（恰好调用一次）；一次 MergeGate 先失败
  再通过。注意合并后的 MainCheck 也会调用同一条门禁命令——它不在本任务范围内、行为不变，设计须说明「恰好一次」的
  计数怎样与 MainCheck 那一次区分开（例如按 `node` 数事件，或脚本按工作目录区分），不能靠关掉 MainCheck 来凑数。
- 第 4 条「`save_project` 不改写用户的 `[gate]`」：从路由界面保存角色配置后，用户手写的 `run = "before_merge"` 仍在文件里、
  重新加载后仍生效（现有测试 `a_gate_and_a_retry_policy_are_read_from_the_project_file` 对 `[gate]` 本身做了同样的验证）。
- 第 5 条的「金样」：仓库里现有的金样目录是 `crates/automed/tests/golden/`（`prompt_rendering.rs` 使用，说明见
  `tests/golden/README.md`）。设计须写明这条文案的金样放在哪、由哪个测试比对。
- 第 6 条：`sh scripts/ci-local.sh` 是本项目的 `[gate] command`，由 Autome 在实现轮之后自己跑（本仓库当前仍是
  `each_round`）。

## 4. 范围

在范围内：

- `crates/autome-domain`：`config.rs` 里 `run` 的类型、缺省值与合并（`GateConfig` 或与 `gate_same_failure_limit` 同样的
  旁挂字段，由设计决定）；`task.rs` 实现轮结束后的转换；`bug_loop.rs` 若需要（见约束 4）；相应单元测试。
- `crates/automed`：`config_io.rs` 解析与校验 `[gate] run`、`save_project` 保留它；`scheduler.rs` 的 `run_gate` 在 Bug Loop
  的 FixGate 上按 `run` 决定是否拼门禁命令；简报的宿主门禁证据（`scheduler.rs` 里组装 `BriefInput.gate` 的那段与
  `brief.rs`）；单元测试、金样测试与 e2e。
- 文档：`README.md`「Host gate, retries and cooldowns」一节（`[gate]` 示例加一行 `run`，并补一段取舍）；
  `docs/development/technical-design.md` §8.1「宿主门禁」补一段。
- 若 `run` 的值会经 `config.get` 之类的已有读接口出现在对外 JSON 里，设计须列出读者并确认它们不受影响。

不在范围内（任务卡「不做什么」，不要做）：

- 不改 `each_round`（缺省）下的任何行为，现有用例不改就通过；
- 不做跨任务的合并队列（几个任务一起 rebase、只跑一次门禁）——另提；
- 不改 MainCheck（T-60 处理合并后重复的那一次）、不改门禁排队（T-47）；
- 不改 `scripts/ci-local.sh`，不改会话自己跑测试的方式；
- 不改本仓库的 `.autome/config.toml`（也不许改 `.autome/` 下任何内容，见 §7 权威规则）。

调研补充的边界（同样不要做）：

- 不新增 JSON-RPC 方法、不改已有方法的参数；不新增事件种类；
- 不改 `protocol/`（协议源）、`plugins/`、`apps/desktop`、`apps/web`——除非设计能指出某个现有读者在 `before_merge` 下
  会显示错误内容，并逐条列出理由；
- 不顺手重构 `gate.rs`、`scheduler.rs`、`task.rs` 或与本需求无关的代码。

## 5. 硬性约束

1. **缺省零行为变化。** 不写 `run` 与写 `run = "each_round"` 完全等价，且与今天逐事件相同：节点序列、事件
   （`gate.started` / `gate.result` / `gate.reused` / `gate.skipped`）、注入、熔断计数、简报内容都不变。现有的单元测试与
   e2e **一行不改**就要通过；若确有必要改已有测试（例如结构体多了一个字段导致构造处编译不过），设计要逐条列出，
   且理由不能是「新行为与旧断言不符」。
2. **`run` 的取值与校验。** 只接受字符串 `"each_round"` 与 `"before_merge"`；其它值（含非字符串）在解析项目配置时报
   中文错误，风格与现有 `[gate]` 校验一致（现有文案：`[gate] timeout_minutes 必须是正整数`、
   `[gate] same_failure_limit 必须是 1 到 10 的整数`、`[gate] 需要 command，例如 command = "make check"`）。
   **全局配置不认这个键**——`[gate]` 整张表本来就只在项目级；设计须写明全局文件里出现它时的行为与现状一致（不因本任务
   新增读取路径）。设计还须写明 `command` 为空（现状：等于没有门禁）时 `run` 怎样处理——现有 `same_failure_limit`
   在 `command = ''` 时仍会被校验并保存，可作参照。
3. **需求 Loop 的转换等价于「没配门禁」。** `before_merge` 下，实现轮结束后的转换必须与 `ctx.config.gate == None`
   时逐字段相同（去审计；审计关闭时走 `after_implement_without_audit`，包括其中的预算判断）；不进 `Gate`、不发
   `RunCoreStep{Gate}`、不写 `gate.started`。**只有这一处**按「没配门禁」处理：rebase 之后进 MergeGate、合并之后进
   MainCheck 的两处判断（`task.rs` 里另外两处 `ctx.config.gate.is_some()`）不变。
4. **Bug Loop 的节点序列不变。** `Node::Fix => core(Node::FixGate)` 保持（现有测试 `bug_loop_fix_always_runs_the_fix_gate`
   断言：无论有没有项目门禁，修复后都进 FixGate）。`before_merge` 改变的只是 FixGate 这一次运行的**命令**：只有复现
   命令；超时仍取项目门禁的 `timeout_minutes`（没有门禁时取默认值）的现有规则不变。ReproCheck 本来就只跑复现命令，
   不变。MergeGate 仍是 `( 复现命令 ) && ( 门禁命令 )`。
5. **MergeGate 的一切照旧。** 通过 → 等你合并；失败 → 退回实现轮（Bug Loop：退回修复轮）并注入
   `Inject::GateFailed{head, command, tail}`；head 与上次通过的结果相同则沿用（`gate.reused`）。
   设计须说明并用测试钉住 `before_merge` 下这条路的完整回路：MergeGate 失败 → 实现轮（修的是已待审 / 已完成的里程碑，
   现有「必须有开放里程碑」守卫的豁免仍适用）→ **不经过 Gate** → 审计（或无审计时的下一步）→ … → 再次到 MergeGate。
   设计须逐节点写出这条回路在 `before_merge` 下实际经过哪些节点、是否重新经过复盘与 rebase，并指出它与 `each_round`
   下同一回路的差别只有「少了 Gate」这一处。
6. **沿用规则不能被旧结果骗过。** MergeGate 的沿用判断读的是「上次通过的 `gate.result`」与当前 head。`before_merge`
   下不再有 Gate 的结果，Bug Loop 下 FixGate 的结果只验证了复现命令。设计须核对现有沿用逻辑按什么匹配（节点？sha？
   命令？），并保证：**只跑了复现命令的 FixGate 通过，不能让同一 head 的 MergeGate 被沿用而跳过完整门禁**；
   需求任务中途把 `run` 从 `each_round` 改成 `before_merge`（或反过来）时也不会因此漏跑合并前的完整门禁。
7. **熔断规则不变。** 同一任务同一节点（`gate` / `merge_gate` / `fix_gate`）连续相同失败指纹达到
   `[gate] same_failure_limit` 时熔断，计数来源是账本里的 `gate.result`。`before_merge` 下 FixGate 只跑复现命令时照样
   计数；126/127 不算失败（Bug Loop 的现有规则）不变。不改 `gate.result` 已有字段的名字与含义。
8. **简报证据不留空、不沿用旧结果。** 现状：只有审计角色的简报带「## 宿主门禁」段，内容是该任务**最后一条**
   `gate.result`（不分节点、不看 sha）。`before_merge` 下这会出两种错：从没跑过门禁时整段缺失；MergeGate 失败退回后，
   下一轮审计会把上一次 MergeGate 的旧结果当成本轮证据。任务卡要求：`before_merge` 下没有**本轮**门禁结果时，简报写明
   「本项目门禁只在合并前运行，本轮没有宿主门禁结果」。设计须定义「本轮门禁结果」的判定（例如结果的 sha 等于本轮
   审视的 head），并写明 Bug Loop 的复核简报怎样处理（任务卡写的是「审计 / 复核简报」；现状复核简报是否带这一段、
   FixGate 只跑复现命令的结果怎样表述，都要写清，不能把「复现命令通过」说成「门禁通过」）。`each_round` 下简报内容
   不变（约束 1）。
9. **读者不变。** `autome status <任务>` 的「门禁」行来自 `task.get` 的 `gate` 字段（`dispatch.rs`：最后一条
   `gate.result`，没有则为 null，CLI 此时不打印这一行）——保持。`autome run` 的事件输出不新增种类。
   不新增 JSON-RPC 方法；桌面端两份白名单与 `dispatch.rs` 的 `READ_METHODS` 由测试交叉核对。
10. **正在运行的任务。** 配置每个 tick 重新解析。设计须说明用户在任务运行中途改 `run` 时的行为：任务正停在 `Gate`
    节点时切到 `before_merge`（现有先例：门禁在任务停在门禁节点时被从配置里拿掉，`run_gate` 视为通过并继续）、
    正在跑 FixGate 时切换、已暂停 / 已停止在 `Gate` 的任务恢复。要求是不卡死、不报协议失败、不漏跑合并前的完整门禁。
11. **`autome-domain` 保持纯净。** 不做 I/O、不读文件、不碰 SQLite；`run` 经 `ResolvedConfig` / `Context` 进状态机。
    账本 schema 不改。
12. **文档写明取舍。** README 与技术设计 §8.1 的新增段落必须包含：门禁次数从「每轮一次」降到「每个任务一次」；代价是
    门禁问题到合并前才暴露、会多退回一次实现轮；Bug 修复轮在 `before_merge` 下只跑复现命令。README 是英文的，
    技术设计是中文的，各按原文语言写。
13. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过。宿主门禁命令
    `sh scripts/ci-local.sh` 由 Autome 自己跑；不要为了让门禁变绿去改 `ci-local.sh` 或跳过步骤。
14. **并行任务与 rebase。** 本仓库 `[loop] parallel = 3`。任务卡点名的相邻任务：T-47（门禁排队，分支
    `autome/宿主门禁按机器排队-gate`）与 T-60（跳过重复的验证 main，分支
    `autome/合并提交的树与合并前门禁通过的树相同时跳过验证`）都改 `scheduler.rs` 的 `run_gate` 一带与 `[gate]` 配置。
    改动保持在必要范围内，不做无关的移动或重排，降低 rebase 冲突。不要去读写那两个分支的 worktree。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

行号是 2026-11-12 本分支（基于 `43baf7d`）上的位置，会随 rebase 漂移，以符号名为准。

**配置（`crates/autome-domain/src/config.rs`）。**

- `GateConfig { command, timeout_minutes }`（约第 287 行）；`ProjectConfig.gate: Option<GateConfig>`，`None` 表示没有门禁。
- `same_failure_limit` 没有放进 `GateConfig`，而是旁挂：`ProjectConfig.gate_same_failure_limit: Option<u32>`（约第 269 行）、
  `ResolvedConfig.gate_same_failure_limit: u32`（约第 378 行），在 `resolve` 里取缺省 3（约第 445 行）；单测
  `same_failure_limit_defaults_and_project_override`。
- `GateConfig` 在测试里有多处按字段字面量构造（如 `bug_loop.rs` 的 `bug_loop_fix_always_runs_the_fix_gate`），往结构体里
  加字段会让这些构造处编译不过——这关系到约束 1 的「现有测试不改」，是选「放进 `GateConfig`」还是「旁挂」时要权衡的。

**配置读写（`crates/automed/src/config_io.rs`）。**

- `[gate]` 解析约第 312–340 行：先读 `same_failure_limit`（越界报 `[gate] same_failure_limit 必须是 1 到 10 的整数`），
  再读 `command`（缺失报 `[gate] 需要 command，例如 command = "make check"`；空白 → 没有门禁），再读 `timeout_minutes`
  （`[gate] timeout_minutes 必须是正整数`）。
- 相关测试：约第 761–800 行的 `same_failure_limit` 三个用例；`a_gate_and_a_retry_policy_are_read_from_the_project_file`
  （约第 959 行，含「从路由界面保存后 `[gate]` 仍在」）；`a_gate_without_a_command_is_refused_and_an_empty_one_means_none`。
- `save_project` 用 `toml_edit` 写回，保留用户的注释与顺序。

**需求状态机（`crates/autome-domain/src/task.rs`）。**

- 实现轮结束（约第 1151 行）：`if ctx.config.gate.is_some() { → Gate, RunCoreStep{Gate} }`，否则审计开启 → `Audit`，
  审计关闭 → `after_implement_without_audit(status, budget, ctx)`（约第 737 行：没有开放里程碑 → `after_the_loop`；
  预算耗尽 → `Fail(ImplBudget)`；否则再起实现轮）。
- 另两处 `ctx.config.gate.is_some()`：rebase 之后进 `MergeGate`（约第 1262 行）、合并之后进 `MainCheck`（约第 1293 行）。
- `after_gate`（约第 1380 行）：`(Gate, Passed)` → 审计或无审计的下一步；`(Gate, Failed)` → 退回实现并注入 `GateFailed`；
  `(MergeGate, Passed)` → `AwaitMerge`（用户已请求合并时直接 `Merging`，约第 1356 行）；`(MergeGate, Failed)` → 退回实现；
  `(MainCheck, _)` → `Cleanup`。
- 现有相关单测：`a_requested_rebase_with_a_gate_still_runs_the_merge_gate`、
  `a_passed_merge_gate_on_a_requested_merge_goes_straight_to_merging`、约第 1570–1760 行的同因失败熔断用例、
  约第 1820–1915 行的 MergeGate 通过 / 失败用例。

**Bug 状态机（`crates/autome-domain/src/bug_loop.rs`）。**

- `Node::Locate` 且已定位 → `core(ReproCheck)`；`Node::Fix => core(Node::FixGate)`（约第 234 行）。
- `GateDone` 于 `FixGate | MergeGate`（约第 278 行）：失败 → `start_fix` 注入 `GateFailed`；FixGate 通过 → 复核（审计关闭时
  也必须复核，见 `bug_loop_passed_fix_gate_requires_review_even_when_audit_is_off`）；复核通过 → `core(Rebase)` → MergeGate。
- 现有相关单测：`bug_loop_fix_always_runs_the_fix_gate`、`bug_loop_failed_fix_gate_returns_to_fix_with_the_tail`、
  `same_failure_bug_gate_trips_and_rejects_other_nodes`。

**调度器（`crates/automed/src/scheduler.rs`）。**

- `run_gate`（约第 3127 行）。Bug Loop 且节点是 `ReproCheck | FixGate | MergeGate` 时现场拼 `GateConfig`：复现命令取自
  `bug.located`（ReproCheck）或 `bug.reproduced`（其余）事件的 `command`；有项目门禁且节点不是 ReproCheck 时命令为
  `( {repro} ) && ( {gate.command} )`，否则只有复现命令；`timeout_minutes` 取项目门禁的，没有则取默认值。
  **本任务第 3 条要改的就是这个 `match`。**
- 选不出门禁（配置被拿掉）→ `finish_gate(.., GateOutcome::Passed)`。
- 作业：`gate::Job::at(repo, task, node, key)`，文件在主仓库 `.autome/output/gates/<task>/<node>-<head 前 12 位>.*`；
  结束后取 `gate::tail(log, 60)`，写 `gate.result`（`node`、`sha`、`ok`、`exit_code`、`command`、`log`、`tail` 及熔断用的
  指纹 / 计数字段），再 `finish_gate` 发 `Trigger::GateDone`。
- 简报的门禁证据（约第 2922 行）：`gate: match role { Role::Audit => last_event(task, "gate.result").and_then(render_gate_line), _ => None }`
  ——只给审计角色、取最后一条、不分节点不看 sha。`brief.rs`（约第 283 行）在 `input.gate` 为 `Some` 时追加
  `\n## 宿主门禁\n\n` 加这段文字，为 `None` 时整段不出现。

**读者。**

- `dispatch.rs` 约第 1635 行：`task.get` 的 `"gate": last_event(task, "gate.result")`。
- `crates/autome-cli/src/main.rs` 约第 667 行：`gate` 是对象时打印「门禁      <节点> 在 <sha> 上通过/未通过（<命令>）  日志 <路径>」，
  否则不打印。`render.rs` 约第 299–321 行渲染 `gate.started` / `gate.result` / `gate.reused` / `gate.skipped` 四种事件。
- `apps/web/app/lib/labels.ts` 与桌面端只有节点名到中文的映射（`gate` → 门禁等），不读 `[gate]` 配置。

**现有 e2e（`crates/automed/tests/end_to_end.rs`）。**

- 「The host-run gate」一节约从第 2566 行起：`write_gate_script`（门禁脚本放在仓库外；有 `gate-ok` 文件就通过，否则失败）、
  `run_gated_task`、`a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`（约第 2618 行，其中断言审计简报含
  `## 宿主门禁` 与「通过」）、`main_failing_its_gate_after_a_merge_raises_an_alert_the_user_can_dismiss`、
  `two_tasks_waiting_to_merge_both_land_and_the_second_rebases_through_the_gate_by_itself`、
  `a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again`（约第 3564 行，
  MergeGate 失败回路的现成样板）。
- Bug Loop：`bug_loop_a_fix_whose_repro_command_still_fails_returns_to_fix`（约第 4253 行）、
  `bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate`（约第 4283 行，用
  `w.configure("[gate]\ncommand = …")` 配项目门禁，并断言 prompt 里复现输出在门禁输出之前）。新 e2e 可照这两个写。
- 金样：`crates/automed/tests/golden/prompts/*.md` 由 `tests/prompt_rendering.rs` 比对（`golden()` / `maybe_update_golden()`），
  说明见 `crates/automed/tests/golden/README.md`。目前没有针对简报「宿主门禁」段的金样。

**权威规格的现状文字。**

- `README.md` 第 107 行起「Host gate, retries and cooldowns」：`[gate]` 示例（`command` / `timeout_minutes` /
  `same_failure_limit`）与「The gate.」一段——写的是 "runs the command itself after every implementation round"。
- `docs/development/technical-design.md` §8.1（第 250 行起）「宿主门禁」一段：实现轮结束进入 Gate 节点 → MergeGate → MainCheck，
  以及熔断规则；§5（第 138 行）写明 Gate、MergeGate、MainCheck 只在项目配置了 `[gate]` 时经过。
- 本仓库 `.autome/config.toml` 的注释「每个实现轮之后、合并前、合并后，宿主在任务 worktree 里跑一遍本地 CI」描述的是
  `each_round`；本任务不改它。

## 7. 项目背景摘要（2026-11-12 整理时）

**仓库。** 单个仓库 `autome-v2`。Autome 2.0：一个 macOS 桌面应用加命令行（另有 Web 面板），把一句话需求在专用 worktree
里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 → rebase → 合并前门禁 → 等你合并 →
合并 → 验证 main → 清理）推进成合并提交。Bug 任务走另一条 Loop（定位 → 复现检查 → 修复 → 修复门禁 → 复核 → rebase →
合并前门禁 → …）。**内核独占节点转换**：会话只写文档与代码，下一个节点由 `automed` 读状态块决定。本仓库正在用自己的
Loop 开发自己。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置合并与校验（`config.rs`）、设计文档解析、
    任务状态机（`task.rs`）、Bug Loop 状态机（`bug_loop.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git（`git.rs`）、会话启动器（`launcher.rs`）、
    调度器（`scheduler.rs`）、宿主门禁作业（`gate.rs`）、宿主准备（`setup.rs`）、配置读写（`config_io.rs`）、简报（`brief.rs`）、
    JSON-RPC 派发（`dispatch.rs`）。测试在 `crates/automed/tests/`（`end_to_end.rs`、`prompt_rendering.rs`、`golden/`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析与输出（`main.rs`、`render.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`）；`apps/web` — React Router + Node 服务器的面板（vitest）。本任务预计都不动。
- `protocol/` — Loop 协议源（编译进 `automed`）；`plugins/` — Claude 插件。本任务不动。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 是 ADR；
  `docs/plans/` 是按日期的决策记录；`docs/<任务目录>/` 是各任务的文档。

**测试命令。**

```sh
cargo test                                   # domain、core、端到端
cargo test -p autome-domain                  # 只跑状态机与配置单测
cargo test -p automed --test end_to_end      # 只跑端到端
cargo test -p automed --test prompt_rendering  # 金样
cargo clippy --all-targets -- -D warnings    # 警告即错误
cargo fmt --all -- --check
sh scripts/ci-local.sh                       # 全量本地 CI（Rust + 桌面端 + Web），即本项目的 [gate] command
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记，只把模型换成替身；测试数据库由
`AUTOMED_DB_PATH` 各自隔离。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/` 下的任何内容；只在当前 worktree 内工作，不切分支、不操作 `.worktree/` 下的其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进
  `.autome/output/`，不要进任务文档目录。
- `.autome/rules/`：只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`（只读）：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`，`timeout_minutes = 30`；
  `[retry] attempts = 2`，`backoff_seconds = 60`。没有 `run`，即本任务自己的开发过程仍按 `each_round` 过门禁。
