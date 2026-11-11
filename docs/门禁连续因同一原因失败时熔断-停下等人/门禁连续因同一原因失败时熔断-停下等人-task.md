# 任务：门禁连续因同一原因失败时熔断，停下等人

本任务的规则见 `docs/门禁连续因同一原因失败时熔断-停下等人/protocol/loop-protocol.md` 与
`docs/门禁连续因同一原因失败时熔断-停下等人/protocol/session-protocol.md`。

- 设计文档：`docs/门禁连续因同一原因失败时熔断-停下等人/门禁连续因同一原因失败时熔断-停下等人.md`
- 任务卡（用户写的，第一输入）：`docs/门禁连续因同一原因失败时熔断-停下等人/门禁连续因同一原因失败时熔断-停下等人-card.md`
- 任务整理日期：2026-11-11
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）

## 1. 需求原文

> 门禁连续因同一原因失败时熔断，停下等人

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§3 照任务卡整理。

## 2. 目标与背景

**背景（任务卡原文）。** T-14 的门禁因为同一个环境问题（缺依赖 `ws`）连续失败 17 次，每次退回实现、
会话确认无法修复后结束、门禁再跑，白白耗掉 17 个实现轮，直到预算用完才停下。

**目标。** 门禁失败后，内核不再无条件退回实现：同一任务、同一门禁节点连续 N 次因**同一原因**失败，
任务直接进入失败，停下等人；用户处理完环境问题后可以用 `rerun` 或 `extend` 继续。

任务卡规定的行为：

- 为每次失败的门禁计算「失败指纹」：失败的步骤名加上日志尾部归一化后的关键错误行（去掉路径、端口、耗时、
  哈希等易变部分）。
- 同一任务同一门禁节点连续 N 次（默认 3，`[gate] same_failure_limit` 可改，1–10）指纹相同，就不再退回实现，
  任务进入失败，原因「门禁连续 N 次因同一原因失败：<指纹摘要>」，并附最近一次日志路径。
- 中间只要有一次门禁通过或指纹不同，计数清零。
- 失败后可用 `rerun` 或 `extend` 继续（与其它失败一致）。

## 3. 验收标准

以下五条逐字取自任务卡「验收标准」。设计文档的里程碑必须逐条覆盖；用户在设计定稿时会对照它们批准或驳回。

1. 单测：指纹对路径、端口、耗时不同但错误相同的两份日志相等；对不同错误不相等。
2. e2e：假门禁连续 3 次输出同一错误 → 任务失败，原因含指纹摘要，且第 3 次之后没有再起实现会话。
3. e2e：错误交替变化时不熔断，仍按原逻辑退回实现。
4. 配置项越界时报中文错误。
5. 现有门禁相关测试不改照常通过。

补充说明（从任务卡和调研得出，不是新范围）：

- 第 2、3 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、
  真实门禁后台作业，只把模型换成替身）。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
  「第 3 次之后没有再起实现会话」要用账本里的会话记录（或等价的、不依赖时序猜测的证据）断言。
- 第 1 条的「两份日志」建议直接用 §6 记录的 T-14 真实日志形状来构造（不同 worktree 路径、不同 `Start at`
  时间、不同 `Duration`/`用时`、不同构建产物哈希），而不是只换一个数字的玩具日志。
- 第 4 条的「中文错误」沿用 `config_io.rs` 现有写法，例如 `[retry] attempts 必须是 0 到 10 的整数`；
  越界值包括 0、11、负数和非整数。
- 第 5 条：现有门禁相关的单元测试与 e2e（见 §6）**一行不改**就要通过。新增测试可以；若确有必要改已有测试，
  设计文档要逐条列出理由，且理由不能是「新行为与旧断言不符」。

## 4. 范围

在范围内：

- `crates/autome-domain`：失败指纹的计算（纯函数）、门禁失败后的状态转换（`task.rs` 的 `after_gate`，
  以及 Bug Loop `bug_loop.rs` 中对应的门禁失败分支——是否适用见约束 3）、新的失败原因、`GateConfig`
  新字段与默认值、`ExtendBudget` / `RerunFrom` 对新失败原因的处理；
- `crates/automed`：`config_io.rs` 读写 `[gate] same_failure_limit` 与越界报错；`scheduler.rs` 的 `run_gate`
  把指纹、连续次数与日志路径交给状态机、写进 `gate.result` 事件；相应单元测试与 e2e；
- `crates/autome-cli`：`render.rs` 的 `failure()` 把新失败原因显示成「门禁连续 N 次因同一原因失败：<指纹摘要>」
  并带日志路径；
- 失败原因的读者：若新增了 `FailureReason` 的种类，`apps/desktop/renderer/lib/labels.js` 的 `failureReason()`
  要加对应分支（否则界面直接显示原始 `kind` 字符串）——只限这一处显示文案；
- 文档：README 里 `[gate]` 示例加一行 `same_failure_limit`；`docs/development/technical-design.md` §8.1
  「宿主门禁」一段补一句熔断规则。

不在范围内（不要做）：

- 自动修复环境问题（任务卡「不做什么」）；
- 改变门禁**通过**时的任何行为（任务卡「不做什么」）；
- 合并后的 `main_check`：它本来就不退回实现，失败只写项目报警；
- 改 `ci-local.sh` 的输出格式来迁就指纹——指纹要能处理任意 `[gate] command` 的输出；
- 新增 JSON-RPC 方法、改 `apps/web/`、`protocol/`、`plugins/`、`.autome/`；
- 顺手重构 `gate.rs`、`scheduler.rs` 或与本需求无关的代码。

## 5. 硬性约束

1. **门禁通过时零行为变化。** 通过之后去哪里（审计 / 无审计时的下一步 / 等你合并 / Bug Loop 的下一步）、
   `merge_gate` 同 sha 沿用的规则、`gate.reused` 事件，全部不变。
2. **未达到熔断次数时零行为变化。** 第 1…N−1 次失败照原逻辑退回实现（或 Bug Loop 的修复），注入的
   `Inject::GateFailed{head, command, tail}` 内容不变；实现预算耗尽仍按原逻辑报 `ImplBudget`。设计须写明
   预算耗尽与熔断同时成立时以哪个为准，并给出理由。
3. **作用范围要写清。** 「同一门禁节点」指 `Node` 的取值（`gate`、`merge_gate`、`fix_gate`……）。设计须逐个说明
   `gate`、`merge_gate`、`fix_gate`、`repro_check`、`main_check` 是否计数、是否熔断，并给出理由：
   `repro_check` 期望复现命令失败，`main_check` 失败不退回实现；Bug Loop 的 `fix_gate` / `merge_gate` 失败退回
   的是修复轮而不是实现轮，浪费方式相同。
4. **不算结果的运行不计数。** `gate::poll` 返回 `Vanished`（作业消失、重跑）或作业结束时 head 已变（作废重跑）
   时，现有代码不产出结果，也不能计入连续次数、不能清零。超时（退出码 124）是一次失败，设计须说明它的指纹。
5. **`autome-domain` 保持纯净。** 不做 I/O、不读文件、不碰 SQLite。指纹函数在 domain 里时接收日志文本作参数；
   「前几次的指纹」由 `automed` 从账本读出后交给状态机（`Context` 或 `Trigger` 上加字段），或由设计给出等价的
   纯净方案。
6. **计数的来源必须重启后仍正确。** 内核会在门禁运行中途重启（`gate.rs` 模块注释）；连续次数不能只存在内存里。
   现有每次门禁都写一条 `gate.result` 事件（`node`、`sha`、`ok`、`exit_code`、`command`、`log`、`tail`），可以
   在其上新增字段（例如指纹），**不能改已有字段的名字和含义**（审计简报的「宿主门禁」段在读它）。
7. **旧账本照常可读。** 账本 schema 当前是 v5（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`）。最好不改 schema；
   若必须改，设计写明理由。没有指纹字段的旧 `gate.result` 事件必须能被正确处理（例如视为「指纹未知、不参与连续计数」），
   不能让正在跑的任务因升级而立刻熔断或报错。
8. **配置项。** `[gate] same_failure_limit`：整数 1–10，缺省 3；越界报中文错误（验收 4）。`save_project` 从路由界面
   保存时不得丢掉这一项（现有测试 `a_gate_and_a_retry_policy_are_read_from_the_project_file` 验证过 `[gate]` 本身
   会被保留，新字段同样要保留）。`scheduler.rs` 为 Bug Loop 现场拼出的 `GateConfig` 也要带上这个值。
9. **失败原因的形状。** 原因文案「门禁连续 N 次因同一原因失败：<指纹摘要>」，并附最近一次日志路径（任务卡原文）。
   `FailureReason` 是 `#[serde(tag = "kind")]` 的对外形状：可以新增种类，不能改已有种类的名字与字段。
   新增种类时逐条列出读者并同步：`crates/autome-cli/src/render.rs` 的 `failure()`、
   `apps/desktop/renderer/lib/labels.js` 的 `failureReason()`。
10. **失败后可继续（任务卡原文：与其它失败一致）。** 现状：`ExtendBudget` 只接受 `ImplBudget` / `DesignBudget`，
    其它失败报「只有轮次耗尽的失败可以追加轮次」；`RerunFrom` 只接受任务整理、六个角色节点与 `rebase`，不接受门禁节点。
    设计须让 `autome rerun` 与 `autome extend` 对新失败原因都有明确、可测的行为，并写明继续之后连续计数怎样算
    （例如用户介入后从零开始，还是沿用）。
11. **指纹必须真能区分 T-14 那种情形。** 见 §6：默认的 60 行尾部里**不含** `Cannot find package 'ws'` 这类关键错误行，
    只有易变的耗时、时间戳、构建哈希和 `FAILED  <步骤>` 汇总。设计须说明指纹读多少行、怎样从任意命令的输出里
    挑「失败的步骤名」与「关键错误行」、怎样归一化，并用 T-14 的真实日志验证（只读）：前 18 次失败的指纹应当相同，
    第 19 次（失败步骤从 `Web typecheck, Web test` 变成只剩 `Web test`）应当不同。
12. **不新增 JSON-RPC 方法、不改已有方法的参数。** 桌面端两份白名单与 `dispatch.rs` 的 `READ_METHODS` 由测试交叉核对。
13. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过；宿主门禁命令
    `sh scripts/ci-local.sh` 由 Autome 在实现轮之后自己跑。本任务改的正是门禁失败路径：实现期间如果本任务自己的门禁
    被环境问题卡住，按协议如实记录，不要为了让门禁变绿去改 `ci-local.sh` 或跳过步骤。
14. **并行任务。** 本仓库 `[loop] parallel = 3`，其它任务也可能改 `task.rs`、`scheduler.rs`。改动保持在必要范围内，
    不做无关的移动或重排，降低 rebase 冲突。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

**门禁作业（`crates/automed/src/gate.rs`）。**

- 每次运行是一个后台作业：`Job::at(repo, task, node, key)`，文件在主仓库
  `.autome/output/gates/<task>/<node>-<head 前 12 位>.{log,pid,exit,started}`。同一 head 不会被重复跑。
- `poll` 返回 `Running` / `Done{exit_code}` / `Vanished`；超时由内核杀进程组，写退出码 `TIMED_OUT = 124`，
  并在日志末尾追加「=== Autome：门禁运行超过 N 分钟，已终止 ===」。
- `tail(log, lines)`：取最后 `lines` 行，再按字节截到最多 6000 字节（按字符边界切，前面加「…」）。

**调度器（`crates/automed/src/scheduler.rs` 的 `run_gate`，约第 2715 行起）。**

- Bug Loop 时为 `repro_check` / `fix_gate` / `merge_gate` 现场拼一个 `GateConfig`（复现命令 + 项目门禁命令）。
- `MergeGate` 在 head 与上次通过的 `gate.result` 相同时直接沿用（`gate.reused`）。
- 作业结束后：`Vanished` → 清掉重跑；head 已变 → 清掉重跑；否则 `tail = gate::tail(&job.log, 60)`，写 `gate.result`
  事件（`node`、`sha`、`ok`、`exit_code`、`command`、`log`、`tail`），再 `finish_gate` 发 `Trigger::GateDone{node, outcome, status}`。
- `GateOutcome::Failed{head, command, tail}` 里**没有日志路径**；日志路径只在 `gate.result` 事件里。
- `ctx.store.last_event(subject_id, kind)` 只取最后一条；要拿「最近 N 条同节点结果」需要新的读法或在事件里带计数。

**状态机（`crates/autome-domain/src/task.rs`）。**

- `after_gate`（约第 1300 行）：`(Gate, Passed)` → 审计或 `after_implement_without_audit`；`(Gate, Failed)` → 先查实现预算
  （`impl_round >= budget` 则 `Fail(ImplBudget)`），否则 `back_to_impl` 注入 `Inject::GateFailed`；`(MergeGate, Passed)` → `AwaitMerge`；
  `(MergeGate, Failed)` → `back_to_impl`；`(MainCheck, _)` → `Cleanup`。
- `Context { config, budget_n, decisions }`（约第 539 行）——没有任何历史信息。
- `FailureReason`（约第 195 行）：`design_budget`、`impl_budget`、`fix_budget`、`infeasible`、`protocol`、`session_crashed`、
  `rebase_conflict`、`config`、`core_step{node, detail}`。
- 失败面板：`(Failed, ExtendBudget)` 约第 930 行；`(Failed | Stopped, RerunFrom)` 约第 947 行（见约束 10）。
- Bug Loop（`crates/autome-domain/src/bug_loop.rs` 约第 224 行）：`fix_gate` / `merge_gate` 失败 → `start_fix` 注入 `GateFailed`。

**配置。**

- `crates/autome-domain/src/config.rs`：`GateConfig { command, timeout_minutes }`（约第 273 行），`DEFAULT_GATE_TIMEOUT_MINUTES = 30`；
  `ProjectConfig.gate` 只在项目级。
- `crates/automed/src/config_io.rs` 约第 271 行解析 `[gate]`：`command` 为空 → 没有门禁；`timeout_minutes` 必须是正整数。
  `[retry] attempts` 的越界写法「必须是 0 到 10 的整数」可作为文案范例。`save_project`（约第 415 行）用 `toml_edit` 写回。
  相关测试：`a_gate_and_a_retry_policy_are_read_from_the_project_file`、`a_gate_without_a_command_is_refused_and_an_empty_one_means_none`。

**失败原因的读者。**

- `crates/autome-cli/src/render.rs` 的 `failure()`（约第 72 行）按 `kind` 出中文，未知种类原样输出 `kind`。
- `apps/desktop/renderer/lib/labels.js` 的 `failureReason()`（约第 121 行）同样按 `kind` 分支，`default` 返回原始 `kind`。
- `apps/web/` 目前没有按 `kind` 渲染失败原因的代码。

**现有门禁测试（验收 5 要求不改）。**

- `gate.rs` 内的单元测试（通过/失败/超时/消失/尾部截断/丢弃节点）。
- `task.rs` 内 `a_requested_rebase_with_a_gate_still_runs_the_merge_gate`、`a_passed_merge_gate_on_a_requested_merge_goes_straight_to_merging`
  等；`bug_loop.rs` 约第 880–915 行的 `fix_gate` 用例。
- `crates/automed/tests/end_to_end.rs` 约第 2545 行起「The host-run gate」一节：`write_gate_script`（门禁脚本放在仓库外，
  有 `gate-ok` 文件就通过，否则输出 `gate says no: gate-ok is missing` 并失败）、`run_gated_task`、
  `a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`、
  `main_failing_its_gate_after_a_merge_raises_an_alert_the_user_can_dismiss`、`stopped_gate_resumes_at_gate_and_discards_killed_result`。
  新 e2e 可以照这个模式写一个「每次都输出同一错误」或「交替输出两种错误」的假门禁。

**T-14 的真实门禁日志（只读证据）。**

- 位置：主仓库 `/Users/dannie/project/autome-v2/.autome/output/gates/T-14/`（本 worktree 里没有 `.autome/output/`）。
  只读，不要改动、移动或删除。
- 共 21 次失败的 `gate` 运行，随后 4 次 `gate` 通过、2 次 `merge_gate` 通过、1 次 `main_check` 通过（任务卡说的是 17 次，
  以日志为准时是 21 次；差异不影响需求）。
- 第 1–18 次失败：日志约 1317 行，汇总里 `FAILED  Web typecheck`、`FAILED  Web test`；第 19–21 次只剩 `FAILED  Web test`。
- 关键错误行在日志第 1218–1254 行附近，例如
  `server/eventbus-ws.ts(1,33): error TS2307: Cannot find module 'ws' or its corresponding type declarations.`、
  `Error: Cannot find package 'ws' imported from /Users/dannie/project/autome-v2/.worktree/<另一个任务>/apps/web/server/eventbus-ws.ts`。
  **它们都不在最后 60 行里。**最后 60 行是 vitest 汇总（`Start at  23:22:08`、`Duration  14.02s …`）、Web build 输出
  （`build/client/assets/home-C_YV8pw7.js` 这类带哈希的文件名、`✓ built in 243ms`）、`ci-local.sh` 的汇总
  （`  ok      <步骤>` / `  FAILED  <步骤>`、`有步骤未通过，用时 129s。`）。
- `scripts/ci-local.sh` 每一步以 `==> <步骤名>` 开头、以 `    ok (Ns)` 或 `    FAILED (Ns)` 结束，并在末尾汇总；这是本项目
  自己的门禁格式，**不是**所有项目都有——别的项目的 `[gate] command` 可能只是 `make check`。

**权威规格。** `docs/development/technical-design.md` §8.1「宿主门禁」（第 254 行）：失败回实现轮，注入 `GateFailed{head, command, tail}`；
每次门禁写一条 `gate.result` 事件。`README.md` 第 110–135 行左右记录了 `[gate]` / `[retry]` 的配置写法。

## 7. 项目背景摘要（2026-11-11 整理时）

**仓库。** 单个仓库 `autome-v2`。Autome 2.0：一个 macOS 桌面应用加命令行，把一句话需求在专用 worktree 里经由 Loop
（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 → rebase → 合并前门禁 → 等你合并 → 合并 →
验证 main → 清理）推进成合并提交。Bug 任务走另一条 Loop（定位 → 复现检查 → 修复 → 修复门禁 → 复核 …）。
**内核独占节点转换**：会话只写设计文档，下一个节点由 `automed` 读状态块决定。本仓库正在用自己的 Loop 开发自己。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置覆盖与校验、设计文档解析、
    任务状态机（`task.rs`）、Bug Loop 状态机（`bug_loop.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git（`git.rs`）、会话启动器（`launcher.rs`）、
    调度器（`scheduler.rs`）、宿主门禁（`gate.rs`）、配置读写（`config_io.rs`）、JSON-RPC 派发（`dispatch.rs`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析与输出（`render.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`），两套 IPC 白名单；本任务最多改 `renderer/lib/labels.js` 一处文案。
- `apps/web` — React Router + 自定义 Node 服务器的面板（vitest）；本任务不动。
- `protocol/` — Loop 协议源（编译进 `automed`）；本任务不动。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 三份 ADR；
  `docs/plans/` 按日期的决策记录。

**测试命令。**

```sh
cargo test                                   # domain、core、端到端
cargo test -p automed --test end_to_end      # 只跑端到端
cargo clippy --all-targets -- -D warnings    # CI 中警告即错误
cargo fmt --all -- --check
npm test --prefix apps/desktop               # 改了 labels.js 时
sh scripts/ci-local.sh                       # 全量本地 CI（Rust + 桌面端 + Web），即本项目的 [gate] command
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记，只把模型换成替身；测试数据库由
`AUTOMED_DB_PATH` 各自隔离。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下的其它目录、
  不推送；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，
  不要进任务文档目录。
- `.autome/rules/`：只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`，`timeout_minutes = 30`；
  `[retry] attempts = 2`，`backoff_seconds = 60`。
