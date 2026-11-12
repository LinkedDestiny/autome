# 任务：合并提交的树与合并前门禁通过的树相同时，跳过「验证 main」

本任务的规则见 `docs/合并提交的树与合并前门禁通过的树相同时跳过验证/protocol/loop-protocol.md` 与
`docs/合并提交的树与合并前门禁通过的树相同时跳过验证/protocol/session-protocol.md`。

- 设计文档：`docs/合并提交的树与合并前门禁通过的树相同时跳过验证/合并提交的树与合并前门禁通过的树相同时跳过验证.md`
- 任务卡（用户写的，第一输入）：`docs/合并提交的树与合并前门禁通过的树相同时跳过验证/合并提交的树与合并前门禁通过的树相同时跳过验证-card.md`
- 任务整理日期：2026-11-12
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）

## 1. 需求原文

> 合并提交的树与合并前门禁通过的树相同时跳过验证 main，写 gate.reused，每次合并少跑一遍完整门禁

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§5 照任务卡整理。

## 2. 目标与背景

**背景（任务卡）。** 配了 `[gate]` 的任务合并要跑两次完整门禁：合并前门禁（`merge_gate`，在 rebase 后的
head 上）和合并后的验证 main（`main_check`，在合并提交上）。任务分支已经 rebase 到 main 最新提交之上时，
`--no-ff` 合并提交的树与分支 head 的树逐字节相同，第二次门禁是在同一份内容上重跑。

任务卡给出的实测（2026-11-12）：T-48 合并提交 `502e656` 的树 `a60b8ee` 等于其第二父提交（分支 head）的树；
T-42 `6fb7ef1` 同样（`1451f33`）。当天 102 次门禁共 18.6 小时，中位数 10–13 分钟一次；每次合并多付约 12 分钟
重复门禁并占一个门禁名额；重复的那一次只可能因环境抖动失败（当天 T-33、T-39 的 `main_gate_failed` 是误报）。

**目标。** 进入 `main_check` 时，如果合并提交的树与本任务最近一次**通过**的 `merge_gate` 所在提交的树相同，
就不启动门禁作业，写一条 `gate.reused` 事件，按通过推进到 Cleanup。每次这样的合并少跑一遍完整门禁。

任务卡规定的行为（六条）：

1. 进入 MainCheck 时，比较合并提交的树（`<merge>^{tree}`）与本任务最近一次**通过**的 MergeGate 所在提交的树。
   相同 → 不启动作业，写 `gate.reused { node: "main_check", sha: <合并提交>, from_sha: <MergeGate 的提交>, tree }`
   （沿用 MergeGate 同 sha 复用已有的 `gate.reused` 事件形状，字段只增不改），按通过推进到 Cleanup。
2. 树不同（合并时 main 又前进、产生了真正的合并）→ 行为与今天完全相同，照常跑 MainCheck。
3. 没有通过的 MergeGate 记录（例如门禁是任务停留期间才配上的）→ 照常跑。
4. Bug 任务的 MergeGate 命令是 `( 复现命令 ) && ( 门禁命令 )`，包含了门禁命令，同样适用。
5. `autome run` 打印一行「T-x 验证 main 沿用合并前门禁的结果（内容相同）」；`task.get.gate` 等读者按现有
   `gate.reused` 的处理显示。
6. 可关：`[gate] reuse_for_main_check = false` 时永远照常跑（缺省 true）。

## 3. 验收标准

以下五条逐字取自任务卡「验收标准」。设计文档的里程碑必须逐条覆盖。

1. e2e：任务 rebase 后过 MergeGate 再合并，没有任何 `gate.started{node: main_check}`，有一条
   `gate.reused{node: main_check}`，任务 Done；门禁脚本的调用次数比今天少一次。
2. e2e：按下合并后 main 再前进一步（走自动 rebase 之外、让合并提交的树与 MergeGate 的树不同的构造），
   MainCheck 照常运行。
3. e2e：`reuse_for_main_check = false` 时行为与今天逐事件相同。
4. 现有 MainCheck 相关用例里断言「合并后跑了门禁」的，按新缺省逐条说明并改写（不是删除）；其余现有用例不改。
5. `cargo test --workspace`、`sh scripts/ci-local.sh` 全绿；README Host gate 一节补一句。

补充说明（从任务卡和调研得出，不是新范围）：

- 「e2e」指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、
  真实门禁后台作业，只把模型换成替身）。只写 `scheduler.rs` 内的单元测试不能代替它。
- 第 1 条的「调用次数少一次」要用门禁脚本自己留下的计数（或等价的、不依赖时序猜测的证据）断言，
  不能只看事件。
- 第 4 条：整理时查到的、与 `main_check` 直接相关的现有 e2e 在 §6 列出；设计轮要自己重新全量核对
  （`grep -n "main_check\|MainCheck"`），逐条写明「保持不变 / 改写为何、怎么改」。改写的方向是让用例在新缺省下
  仍然守住它原来要守的东西（例如显式构造树不同，或显式关掉开关），不是删掉断言。

## 4. 范围

在范围内：

- `crates/automed/src/scheduler.rs` 的 `run_gate`：`Node::MainCheck` 分支里的复用判断；
- `crates/automed/src/git.rs`：如需要，新增取提交树 id 的小函数（`git rev-parse <commit>^{tree}`）；
- `crates/autome-domain/src/config.rs` 与 `crates/automed/src/config_io.rs`：`[gate] reuse_for_main_check`
  的读取、缺省值（true）、解析与类型错误时的中文报错；
- `crates/autome-cli/src/render.rs`：`gate.reused` 且 `node == main_check` 时打印
  「T-x 验证 main 沿用合并前门禁的结果（内容相同）」；
- `gate.reused` 的其它读者：只在现有处理对新事件显示错误时才动（见 §6「读者」）；
- 测试：`end_to_end.rs` 新增三个 e2e（验收 1–3）、改写受新缺省影响的现有用例（验收 4）、相应单元测试；
- 文档：README「Host gate, retries and cooldowns」一节补一句并在 `[gate]` 示例里体现开关；
  若 `docs/development/technical-design.md` 的宿主门禁一节描述了「合并后再跑一次」，同步补一句。

不在范围内（任务卡「不做什么」，加上调研得出的边界）：

- 不改 MergeGate、Gate 的行为；不改 MainCheck 失败时「只报警」（`main_gate_failed`）的语义；
- 不比较除树以外的东西（提交信息、作者不影响门禁结果）；
- 工作区（workspace）项目今天就以 `gate.skipped` 跳过 MainCheck，保持不变；
- 不改状态机 `Merging → MainCheck → Cleanup` 的节点与转换（`crates/autome-domain/src/task.rs`）——复用
  是 `run_gate` 内部「按通过结束」的一条早退路径，与现有 MergeGate 复用、`gate.skipped` 同构；
- 不实现门禁排队 / 名额（那是 T-47 的任务，见 §6「并行任务」）；
- 不新增 JSON-RPC 方法；不改 `protocol/`、`plugins/`、`.autome/`；不顺手重构 `run_gate` 或 `gate.rs`。

## 5. 硬性约束

1. **事件形状只增不改。** 现有 MergeGate 复用写的是 `gate.reused { node, sha }`。新事件在此之上加
   `from_sha` 与 `tree`，不改名、不删字段、不改现有 MergeGate 复用事件的内容。
2. **树不同 / 无通过记录 / 开关关闭时，与今天逐事件相同。** 这三条路径上不许多写、少写或改写任何事件，
   `_verify-<sha>` 检出、准备命令（`[setup]`）、`main_gate_failed` 报警都照旧。
3. **复用判断在启动作业之前，也在（将来的）门禁准入之前，不占名额。** 复用时不应创建
   `.worktree/_verify-<sha>` 检出、不应在其中跑准备命令。设计轮要写清判断放在 `run_gate` 的哪一步，
   以及与 T-47 合入后的排队逻辑谁先谁后。
4. **「通过的 MergeGate」的认定要有据。** MergeGate 自己可能是复用的（`gate.reused{node: merge_gate, sha}`，
   没有对应的 `gate.result{node: merge_gate}`），也可能有多次（失败退回后重跑、rebase 后 head 变了）。
   设计轮必须说明：哪些事件算「本任务最近一次通过的 MergeGate」、取哪个提交的树、如何保证取到的是
   **合并前最后那一次**而不是更早一轮的记录。认不准就照常跑——宁可多跑，不可误跳。
5. **比较的是树 id，由 Git 给出。** 用 `git rev-parse <commit>^{tree}`（或等价命令）取两边的树 id；
   不比较 sha、不比较 diff 输出、不自己算哈希。取树失败（提交不存在等）→ 照常跑。
6. **Bug 任务同样适用**（任务卡第 4 条）。注意现有 MergeGate 复用对 Bug 任务额外比较了 `command`；
   设计轮要说明 MainCheck 复用是否需要类似条件及理由。
7. **配置项。** `[gate] reuse_for_main_check`，布尔，缺省 true，项目级（与 `[gate]` 其它字段同处）；
   非布尔值报中文错误，写法沿用 `config_io.rs` 现有风格（例如 `[gate] same_failure_limit 必须是 1 到 10 的整数`）。
8. **连续失败熔断不受影响。** `record_gate_result` 把 `gate.reused` 当作一次通过来清零连续失败计数；
   `main_check` 本不计入熔断（`gate_failure::counts`），新事件不得改变任何节点的计数结果。
9. 仓库规则：不修改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不操作 `.worktree/` 下
   其它目录、不推送远端；超过 5 行的命令输出与临时验证产物写入 `.autome/output/`，不进任务文档目录。
10. 用户可见文案（CLI 输出、报错）用中文，与现有文案风格一致；代码注释沿用所在文件的语言（Rust 源码为英文）。

## 6. 项目背景摘要（2026-11-12 整理时）

**仓库。** `autome-v2`，单仓库。Autome 用自己的 Loop 开发自己。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，含 rustfmt、clippy），三个 crate：
  - `crates/autome-domain` — 纯域逻辑，无 I/O：任务状态机（`task.rs`）、Bug Loop（`bug_loop.rs`）、
    配置解析后的模型与合并规则（`config.rs`）、失败指纹（`gate_failure.rs`）等；
  - `crates/automed` — I/O 与 JSON-RPC：调度器（`scheduler.rs`，约 7600 行）、宿主门禁后台作业（`gate.rs`）、
    Git 封装（`git.rs`）、配置读写（`config_io.rs`）、事件与账本（`store.rs`，SQLite）、分发（`dispatch.rs`）；
  - `crates/autome-cli` — `autome` 命令，参数解析与打印（`render.rs`）。
- `apps/desktop` — Electron 外壳（`node --test`）；`apps/web` — React Router 面板 + Node 网关（vitest、eslint、tsc）。
- `protocol/` — 协议 seed；`plugins/` — Claude 插件；`docs/development/` — 需求与技术设计（权威）；
  `docs/<slug>/` — 各任务的文档目录。

**测试命令。**

- `cargo test --workspace`（任务卡点名）；
- `sh scripts/ci-local.sh` — 本项目的宿主门禁命令（`.autome/config.toml` 的 `[gate] command`，超时 30 分钟）。
  依次跑：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace --all-targets -- --nocapture`、`npm test --prefix apps/desktop`、契约检查、
  `apps/web` 的 lint / typecheck / test / build。每步都跑完，任一失败则退出码非零。

**权威规则文件。** 仓库根 `AGENTS.md`；`.autome/rules/`（目前只有一份说明性 README，没有具体规则）；
`docs/agent-project-profile.md` 不存在。

**与本任务直接相关的现状代码**（行号是整理时的，会漂移，以符号名为准）：

- `scheduler.rs::run_gate`（约 3127 行起）：
  - `Node::MainCheck` 分支：取 `task.merge_commit`；工作区项目或没有合并提交 → 写
    `gate.skipped { node, reason: "没有单一的合并提交" }` 并按通过结束；否则在 `.worktree/_verify-<sha 前 12 位>`
    （`verify_dir`）建游离检出，返回 `(检出目录, 合并提交)`。**检出是在复用判断之前建的**——本任务的判断要更早。
  - `if !job.exists()` 里现有的 MergeGate 复用：最近一条 `gate.result` 为 `ok` 且 `sha == head`
    （Bug 任务另要求 `command` 相同）→ 写 `gate.reused { node, sha }`，`finish_gate(.., Passed)`。
  - 之后是 `[setup]` 准备命令（MainCheck 用 `verify_record`）、`gate::start`、写 `gate.started`。
  - 作业结束：写 `gate.result`（`record_gate_result`）；MainCheck 删除验证检出，失败则 `raise_alert("main_gate_failed")`
    加 `project.alert` 事件；最后 `finish_gate`。
  - 没有配 `[gate]`（`selected_gate` 为 `None`）→ 直接按通过结束。Bug 任务在 MainCheck 用的是 `resolved.gate`
    （不带复现命令），在 MergeGate 用 `( 复现 ) && ( 门禁 )`。
- `scheduler.rs` 内已有围绕 `gate.reused` 的单元测试（搜 `count_events(&task.id, "gate.reused")`）。
- `autome-domain/src/task.rs`：`(Node::MainCheck, _) => Cleanup`，不论门禁结果。
- `autome-domain/src/config.rs`：`GateConfig { command, timeout_minutes }`；项目配置另有
  `gate_same_failure_limit: Option<u32>`，解析后为 `Resolved.gate_same_failure_limit`——新开关可照此模式。
  `config_io.rs` 在读 `[gate]` 表处解析 `same_failure_limit`（搜该字符串）。
- `git.rs`：没有取树 id 的函数；有 `head_sha`、`worktree_add_detached`、`worktree_remove`、`worktree_prune`。
- 读者：
  - `autome-cli/src/render.rs`：`"gate.reused"` 目前统一打印「{任务} {节点名}：提交 {sha} 已经通过过，沿用」；
    `node_label("main_check")` 为「验证 main」。任务卡要求 main_check 的复用打印另一句话。
  - `dispatch.rs`：`task.get` 的 `gate` 字段是 `last_event(task_id, "gate.result")`，只看 `gate.result`，
    不看 `gate.reused`。任务卡写的是「按现有 `gate.reused` 的处理显示」——设计轮核实现有处理是否已足够，
    不足再提最小改动。
  - `apps/desktop/renderer/screens/dashboard.js`、`lib/labels.js`，`apps/web/app/lib/labels.ts` 有
    `main_check` 的节点文案，整理时未见按 `gate.reused` 事件分支的代码。
- 现有与 `main_check` 直接相关的 e2e（`crates/automed/tests/end_to_end.rs`）：
  - 约 2660 行处断言 `result["node"] == "main_check"` 的用例（合并后跑了门禁）；
  - `setup_runs_in_the_merge_commit_checkout_before_the_main_check`；
  - `setup_failure_in_the_merge_commit_checkout_raises_an_alert_and_skips_the_main_check`；
  - `setup_in_the_main_checkout_runs_after_a_merge_only_when_the_watch_changed`、
    `setup_failure_in_the_main_checkout_raises_an_alert_and_the_task_is_still_done`（主检出的准备命令，
    是否受影响待设计轮核对）。
  这些用例在新缺省下很可能走到复用路径，正是验收 4 要逐条处理的对象。
- README「Host gate, retries and cooldowns」一节现有表述：「…and once more on `main` after the merge — that last
  run cannot undo the merge, so a failure there raises an alert…」，以及 Preparation 段提到
  「in the merge verification checkout」。

**并行任务（整理时都还没合进 main）。**

- T-47「宿主门禁按机器排队」（分支 `autome/宿主门禁按机器排队-gate`）：改同一个 `run_gate`，加门禁名额与排队。
  本任务整理时 main 上的 `run_gate` 里还没有排队代码。任务卡要求：复用判断在准入之前，不占名额。
  谁后合谁 rebase；设计要让这段判断是 `run_gate` 里一处独立的早退，冲突面尽量小。
- T-61（分支 `autome/门禁只在合并前跑一次-gate-run`）：从分支名看也在动门禁的运行时机，整理时刚开工，
  内容未知。设计轮开始时看一眼它是否已合入 main，若已合入，以合入后的 `run_gate` 为准重新核对本节。
