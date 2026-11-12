# 任务：每轮快检（`[gate] quick`）——`before_merge` 下实现轮 / 修复轮之后由宿主跑一条静态检查

本任务的规则见 `docs/每轮快检-gate-quick-before/protocol/loop-protocol.md` 与
`docs/每轮快检-gate-quick-before/protocol/session-protocol.md`。

- 设计文档：`docs/每轮快检-gate-quick-before/每轮快检-gate-quick-before.md`
- 任务卡（用户写的，第一输入）：`docs/每轮快检-gate-quick-before/每轮快检-gate-quick-before-card.md`
- 任务整理日期：2026-11-13；任务号 T-67
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree），状态块不写 `repos:`
- 分支基线：`042fd06`（其下是 `1d06101`，T-45 的合并提交）。整理时 main 已前进到 `ca79dcc`（T-46），见约束 15

## 1. 需求原文

> 每轮快检：`[gate] quick = "…"`，`before_merge` 下实现轮 / 修复轮之后只跑一条几秒钟的静态检查

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§5 照任务卡整理，§6 是调研时在代码里看到的事实。

## 2. 目标与背景

**背景（任务卡原文）。** T-61 让项目可以配 `[gate] run = "before_merge"`：完整门禁（本仓库是 `sh scripts/ci-local.sh`，
约 3 分钟）每个任务只在合并前跑一次。代价是格式、lint、类型检查这类**几秒就能跑完**的问题也要等到合并前才暴露，暴露后
要多走「实现 → 审计 → 复盘 → rebase → 合并前门禁」一整圈。门禁关闭 / 合并前模式下已经出过两次：

- T-56 带进 main 的 `Web typecheck` 失败（14 处隐式 any），靠另开 T-65 修；
- T-61 自己的 `clippy::needless_borrow`（M-04 引入，经过 5 个审计轮没人发现），合并前门禁第一次运行才拦住，多花实现 #10、
  审计 #10、复盘 #3 和两次完整门禁，约 40 分钟。T-61 的复盘 L-07 记录：同一条 `cargo clippy` 在会话里只要 3.27 秒。

把这类检查交给 prompt 去「提醒会话自己跑」不可靠（L-07 的现象就是每一轮都写「由宿主门禁负责」）。宿主应当自己跑，
像完整门禁一样用退出码说话。

**目标。** 给项目配置加一条「快检」命令：`before_merge` 下，实现轮 / 修复轮之后由宿主跑它，合并前仍跑完整门禁。
任务卡规定的八件事：

1. **配置项。** 项目 `.autome/config.toml` 的 `[gate]` 新增可选键 `quick`（字符串，`sh -c` 在任务 worktree 里执行）和
   可选键 `quick_timeout_minutes`（缺省 5，范围与 `timeout_minutes` 的校验同一风格）。没配 `[gate] command` 时单独写
   `quick` 报错。全局配置不认这两个键。
2. **`run = "before_merge"` 且配了 `quick` 时：**
   - 需求 Loop：实现轮结束后进 `Gate` 节点，但跑的是 `quick` 命令；通过 → 去审计（或审计关闭时的下一步）；失败 → 照现有
     `Gate` 失败路径退回实现轮，prompt 带命令与日志尾部。
   - Bug Loop：FixGate 跑 `( 复现命令 ) && ( quick 命令 )`。
   - MergeGate 不变：完整门禁命令（Bug 是复现命令 + 完整门禁）。完整门禁是否包含 quick 的内容由项目自己决定，
     Autome 不拼接。
3. **`run = "each_round"`（缺省）时 `quick` 不生效**，行为与今天逐事件相同；配置解析通过，`autome doctor` / 配置校验
   不报错（可在 `config.get` 的 `violations` 之外给一句说明，不强求）。
4. **共用现有机制。** 快检与完整门禁共用宿主任务队列（T-47 的 `[gate] concurrency`）、同因失败熔断、126/127 处理、
   日志落盘位置（`.autome/output/gates/<任务>/`）、`AUTOME_GATE_LOG` / `AUTOME_GATE_NODE` 环境变量；另导出
   `AUTOME_GATE_KIND=quick|full`，让脚本能区分。
5. **读者能分清跑的是哪一种。** `gate.started` / `gate.finished` 事件带 `kind: "quick" | "full"`；`autome run` 的行写
   「快检开始 / 快检通过 / 快检未通过」；`autome status <任务>` 的门禁行与 `task.get.gate` 标明种类；审计 / 复核简报的
   「宿主门禁」段写明「本轮宿主只跑了快检：<命令>，退出码 N」，不让审计把快检当成完整门禁的证据。
   （任务卡写的 `gate.finished` 在代码里不存在，见 §6「事件」与约束 6。）
6. **prompt。** `before_merge` 段（T-61 引入的 `<!-- gate-run: before_merge -->` 分段与 `{gate_line}`）在配了 `quick` 时
   说明「宿主会在本轮结束后跑快检 `<命令>`，失败会退回」，没配时保持现有文案。协议正文有改动就按 `protocol/CHANGELOG.md`
   的规矩发新版本（独立的 `## protocol/vN` 标题）。
7. **文档。** README Host gate 一节与 `docs/development/technical-design.md` 对应小节各补一段：两级门禁的取舍与推荐
   （快检放格式 / lint / 类型检查，控制在 1 分钟内；测试留给完整门禁）。
8. **本仓库的 `.autome/config.toml` 不在任务里改**；合并后由用户（或其授权的操作者）加：
   `quick = "cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web"`。
   验收里请实测这条命令在热缓存下的耗时并写进证据。

## 3. 验收标准

以下六条逐字取自任务卡「验收标准」。设计文档的里程碑必须逐条覆盖；用户在设计定稿时会对照它们批准或驳回。

1. 单测（`autome-domain`）：`before_merge` + `quick` 下，实现轮完成 → `Gate`；`Gate` 通过 / 失败的转换与 `each_round` 下
   `Gate` 的转换相同；没配 `quick` 时与 T-61 现有行为逐条相同（现有用例不改就通过）。
2. e2e（需求）：`before_merge` + `quick`，两个里程碑的任务从创建到合并：快检脚本被调用 2 次（每个实现轮一次），完整门禁
   脚本恰好 1 次；让快检第一次失败 → 退回实现轮且 prompt 带快检尾部 → 修好后继续；事件里 `kind` 正确。
3. e2e（Bug）：FixGate 的命令含复现命令与快检、不含完整门禁；MergeGate 含复现命令与完整门禁。
4. 解析：只有 `quick` 没有 `command` 报错；`quick_timeout_minutes` 越界报错；`save_project` 不改写用户的 `[gate]`；
   `each_round` + `quick` 不跑快检。
5. 简报与 prompt 金样覆盖第 5、6 条文案；`autome run` / `status` 的渲染有单测。
6. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿——**实现轮自己在会话里跑前两条并把退出码
   写进证据**（本项目当前是 `before_merge`，宿主到合并前才跑）。

补充说明（从任务卡和调研得出，不是新范围）：

- 第 2、3 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、真实门禁
  后台作业，只把模型换成替身）。只在 `autome-domain` 或 `scheduler.rs` 里写单元测试不能代替它。
- 第 2 条是两个场景：一次全程通过（快检 2 次、完整门禁恰好 1 次）；一次快检先失败再通过。计数要用不依赖时序猜测的证据
  （脚本自己记下的调用、账本里的事件）。快检脚本与完整门禁脚本必须是**两个不同的脚本**，否则分不清谁被调用了几次。
  合并后的 MainCheck 也会调用完整门禁命令——它不在本任务范围内、行为不变；T-61 的同类用例
  `feature_before_merge_two_milestones_run_the_gate_once_before_the_merge` 让门禁命令把 `$AUTOME_GATE_NODE` 追加进一个
  文件、按节点数调用，以此把 MergeGate 的那一次与 MainCheck 的那一次分开。照这个办法做，不能靠关掉 MainCheck 来凑数。
- 第 2 条「每个实现轮一次」在快检失败的场景里不成立（修复那一轮之后还要再跑一次），断言按场景写。
- 第 4 条「`save_project` 不改写用户的 `[gate]`」：从路由界面保存角色配置后，用户手写的 `quick` / `quick_timeout_minutes`
  连同注释仍在文件里、重新加载后仍生效。现成样板是 `before_merge_save_role_preserves_gate_text_and_reload`。
- 第 5 条的金样目录是 `crates/automed/tests/golden/`（`prompts/` 与 `briefs/`，由 `tests/prompt_rendering.rs` 比对，说明见
  `tests/golden/README.md`，`AUTOME_UPDATE_GOLDEN=1` 接受差异）。现有 `*-before-merge.md` 金样对应「没配 `quick`」，
  **不许改动**（约束 1）；配了 `quick` 的文案另立金样。
- 第 6 条：本仓库当前 `run = "before_merge"` 且没有 `quick`，实现轮 / 审计轮之后宿主什么都不跑。所以每个改了 `.rs` 的
  实现轮都要在会话里跑 `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings`，把两条的退出码写进
  该轮证据；改了 `apps/web` 的还要跑 `npm run lint --prefix apps/web` 与 `npm run typecheck --prefix apps/web`。
  `sh scripts/ci-local.sh` 由宿主在合并前跑；会话沙箱里跑不通的步骤（T-61 记录过 Desktop SIGABRT、Web listen EPERM）
  照实逐步列出，不要写成「由宿主门禁负责」了事。
- 目标 8 要求的实测：在本 worktree、热缓存下跑那条四段命令，记录总耗时与各段耗时，写进证据文件（`evidence/`）。
  这是一次测量，不是把它配进 `.autome/config.toml`。

## 4. 范围

在范围内：

- `crates/autome-domain`：`config.rs` 里 `quick` 与 `quick_timeout_minutes` 的类型、缺省值与合并；`task.rs` 实现轮结束后
  的转换（`before_merge` + `quick` → `Gate`）；`bug_loop.rs` 若需要（见约束 4）；相应单元测试。
- `crates/automed`：`config_io.rs` 解析与校验两个新键、`save_project` 保留它们；`scheduler.rs` 的 `run_gate`（按节点与配置
  选命令和超时、`AUTOME_GATE_KIND`、事件里的 `kind`、沿用规则）；`gate.rs` 若作业需要记下自己的种类；简报与 prompt
  （`brief.rs`、`launcher.rs`）；`dispatch.rs` 的 `task.get.gate`（及可选的配置说明）；单元测试、金样与 e2e。
- `crates/autome-cli`：`render.rs` 的 `gate.started` / `gate.result` 行，`main.rs` 的 `autome status` 门禁行；渲染单测。
- `protocol/`（协议源，不是任务目录里那份副本）：`prompts/` 的 `before_merge` 分段、`bug-protocol.md` 里说「修复门禁只跑
  复现命令」的那一句，凡有改动就在 `CHANGELOG.md` 末尾加新的 `## protocol/vN`，并过 `protocol eval`。
- 文档：`README.md`「Host gate, retries and cooldowns」一节（`[gate]` 示例加两行，并补一段两级门禁的取舍与推荐）；
  `docs/development/technical-design.md` §8.1 补一段。
- `task.get.gate` 的其它读者（见 §6「读者」）：设计须逐个列出，并说明各自在快检结果下显示什么、改不改、为什么。

不在范围内（任务卡「不做什么」，不要做）：

- 不改 `each_round` 下的任何行为；
- 不自动从完整门禁里拆出快检，不解析 `ci-local.sh`；
- 不做「按改动文件选择快检命令」——一条命令，项目自己写条件；
- 不做跨任务合并队列；
- 不改本仓库的 `.autome/config.toml`（也不许改 `.autome/` 下任何内容，见 §7 权威规则）。

调研补充的边界（同样不要做）：

- 不改 MergeGate 与 MainCheck 跑什么命令；不改门禁排队（T-47）、MainCheck 沿用（T-60）、同因熔断的规则本身；
- 不改 `scripts/ci-local.sh`，不为了让门禁变绿去跳过其中的步骤；
- 不新增 JSON-RPC 方法、不改已有方法的参数；账本 schema 不改；
- 不改 `docs/每轮快检-gate-quick-before/protocol/`（本任务固定的协议副本）与 `protocol/` 里的 `kernel-contract` 区域；
- 不顺手重构 `gate.rs`、`scheduler.rs`、`task.rs`、`launcher.rs` 或与本需求无关的代码。

## 5. 硬性约束

1. **没配 `quick` 时零行为变化。** 不写 `quick` 的项目——无论 `each_round` 还是 `before_merge`——与今天逐事件相同：
   节点序列、事件（`gate.started` / `gate.result` / `gate.reused` / `gate.skipped` / `gate.queued`）的种类与次数、注入、
   熔断计数、简报与 prompt 的字节。现有单元测试、金样与 e2e **一行不改**就要通过；若确有必要改已有测试（例如结构体多了
   一个字段导致构造处编译不过），设计要逐条列出，且理由不能是「新行为与旧断言不符」。给现有事件加 `kind` 字段是任务卡
   明确要求的（目标 5），算可见变化：设计须列出受影响的现有断言与读者，并确认没有按整个载荷相等来比较的地方被打破。
2. **`each_round` + `quick`：解析通过、不跑快检。** 行为与没写 `quick` 的 `each_round` 逐事件相同；`Gate` / FixGate 跑的
   仍是完整门禁（Bug 是复现 + 完整门禁）。任务卡允许在 `violations` 之外给一句说明，不强求；给的话不能进 `violations`
   （那会让保存被拒绝），也不能让 `autome doctor` 报错。
3. **配置的取值与校验。** `quick` 只接受字符串；`quick_timeout_minutes` 缺省 5，校验与 `timeout_minutes` 同一风格（现有
   文案：`[gate] timeout_minutes 必须是正整数`）。错误用中文，风格与现有 `[gate]` 校验一致（另有
   `[gate] 需要 command，例如 command = "make check"`、`[gate] run 必须是 "each_round" 或 "before_merge"`）。
   「没配 `[gate] command` 时单独写 `quick` 报错」是任务卡原话。设计须逐一写明并测试下面这些任务卡没说的边角：
   `command = ''`（现状：等于没有门禁，但同表其它键仍被校验）时写了 `quick`；`quick = ''` 或只有空白；`quick` 不是字符串；
   只写 `quick_timeout_minutes` 没写 `quick`；只写 `concurrency` + `quick`（现状：只写 `concurrency` 可以不带 `command`）。
   **全局配置不认这两个键**：`parse_global` 现在只读 `[gate] concurrency`，其它键静默忽略（`run` 也是这样，见测试
   `before_merge_global_gate_run_remains_ignored`）——保持，不因本任务新增读取路径。
4. **状态机只多一处判断。** 需求 Loop：`Node::Implement` 完成后，`before_merge` + `quick` 进 `Gate` 并发
   `RunCoreStep{Gate}`，与 `each_round` 下相同；`after_gate` 里 `(Gate, Passed)` / `(Gate, Failed)` 两条路不改（失败路径
   含预算判断：`impl_round >= budget` 时 `Fail(ImplBudget)`）。rebase 之后进 MergeGate、合并之后进 MainCheck 的判断不变。
   Bug Loop 的节点序列不变：`Node::Fix => core(Node::FixGate)` 保持（现有测试 `bug_loop_fix_always_runs_the_fix_gate`），
   变的只是 FixGate 这一次运行的**命令**。
5. **合并前的完整门禁不能被快检结果顶替。** 现状（§6「沿用」）：需求任务的 MergeGate 只要「最后一条 `gate.result` 通过且
   `sha` 等于当前 head」就写 `gate.reused` 并直接通过，**不看命令也不看节点**。`before_merge` + `quick` 下，快检通过后
   head 没动就到 MergeGate 时（例如 rebase 无事可做），完整门禁会被跳过——整个任务一次完整门禁都没跑就到「等你合并」。
   设计必须堵住这条路并用测试钉住：**快检通过的结果不得让任何 MergeGate 被沿用**；MainCheck 的沿用
   （`main_check_reuse`，按 `merge_gate` 节点的结果与实际启动命令匹配）也不得被快检结果满足。Bug Loop 的同类用例
   `before_merge_fix_gate_result_cannot_be_reused_on_the_same_head` 是样板；`( 复现 ) && ( quick )` 的 FixGate 结果同样
   不得顶替 `( 复现 ) && ( 完整门禁 )` 的 MergeGate。
6. **`kind` 记的是实际跑了什么。** 代码里没有 `gate.finished` 这个事件；门禁结束写的是 `gate.result`（§6「事件」）。
   任务卡的意图是开始与结束两条事件都带 `kind`——设计按 `gate.started` 与 `gate.result` 落实，不新增事件种类，并在设计
   文档里写明这处与任务卡字面的出入。`kind` 必须来自**这次作业启动时**的事实，不是收结果那一刻的配置：配置每个 tick
   重新解析，作业在跑的时候用户可能改了 `run` / `quick`（T-61 为同一问题给 FixGate 加过落盘的 `repro-only` 标记）。
   `gate.result` 的 `command` 现有字段的名字与含义不改。设计还须写明任务卡没说的两种运行的 `kind`：只跑复现命令的
   ReproCheck，以及没配项目门禁（或 `before_merge` 没配 `quick`）时只跑复现命令的 FixGate——它们既不是快检也不是完整
   门禁；`AUTOME_GATE_KIND` 在这两种运行里导出什么也要写明。`gate.reused` / `gate.skipped` / `gate.queued` 是否带 `kind`
   由设计决定并说明。
7. **共用宿主机制，不另起一套。** 快检走 `run_gate` 现有的路径：准备命令（`[setup]`）在前、`gate_queue::admit` 排队
   （占 `[gate] concurrency` 的名额，FIFO）、后台作业与日志在 `.autome/output/gates/<任务>/`、结果绑定 head（head 变了
   作废重跑）、`AUTOME_TASK` / `AUTOME_HEAD` / `AUTOME_GATE_NODE` / `AUTOME_GATE_LOG` 照旧导出。同因失败熔断按节点计数
   （`gate` / `merge_gate` / `fix_gate`），快检失败照样计入所在节点；126/127 的现有处理不变。设计须核对并写明：同一节点
   上快检与完整门禁的结果（模式切换前后）会不会被算进同一条连续失败、作业文件名 `<节点>-<head 前 12 位>.*` 在种类
   切换时会不会把另一种的在途作业或旧日志当成自己的。
8. **超时。** 快检用 `quick_timeout_minutes`（缺省 5），完整门禁仍用 `timeout_minutes`。Bug Loop 的 FixGate 现在取项目
   门禁的 `timeout_minutes`；`before_merge` + `quick` 下它跑的是 `( 复现命令 ) && ( quick )`，复现命令可能是一条不快的
   测试——用哪个超时任务卡没说，设计须给出取值与理由。超时的现有表述（「门禁超时（运行超过 N 分钟，被内核终止）」）
   在快检上要说得通。
9. **不让快检被读成完整门禁。** 凡是把宿主结果告诉人或会话的地方，快检都要标明是快检：
   - 审计简报「## 宿主门禁」段：本轮只有快检结果时写明「本轮宿主只跑了快检：<命令>，退出码 N」，并让读者知道完整门禁
     只在合并前运行。现有那句「这是实现轮之外的一次独立运行。复验时可以引用它……」是写给完整门禁的，不能原样套在快检上。
     T-61 的规则保留：结果的 `sha` 不等于本轮 head 时不引用旧结果。MergeGate 失败退回后的那一轮审计，最后一条结果可能是
     完整门禁的也可能是快检的——设计须写明各种情形下这一段的文字。
   - Bug 复核：Bug 会话没有简报，「宿主门禁」的说明在 `fix-review` 的 prompt 里（模板正文与 `launcher.rs` 的
     `BUG_REVIEW_GATE_NOTE`）。现有文字「修复门禁只跑了复现命令」在配了 `quick` 后不再属实，须改成与事实相符的说法，
     同样不能把「复现 + 快检通过」说成「项目门禁通过」。
   - 失败注入：`Gate` / FixGate 上快检失败退回时，prompt 带命令与日志尾部（任务卡目标 2）。现有 `before_merge` 下的
     `Inject::GateFailed` 文字写的是「运行了项目门禁 `…`」「门禁结果以 Autome 在合并前重跑为准」，对快检失败不属实
     （快检在本轮结束后就会重跑），设计须给出快检失败时的文字；MergeGate 失败时的现有文字不变。
   - `autome run`：「快检开始 / 快检通过 / 快检未通过」；`autome status <任务>` 门禁行与 `task.get.gate` 标明种类。
10. **prompt 与协议版本。** 配了 `quick` 时，`before_merge` 段要说明「宿主会在本轮结束后跑快检 `<命令>`，失败会退回」；
    没配时字节不变（约束 1）。`{gate_line}` 由宿主填写，模板正文在 `protocol/prompts/`。改了 `protocol/` 下任何文件就要发
    新版本：在 `protocol/CHANGELOG.md` 末尾加独立的 `## protocol/vN`（整理时最大是 v4），每条改动一个块，写 `kind`、
    至少两个任务的 `evidence`（T-56/T-65 与 T-61 可用）、`predicted_impact`；`behavioral` 类必须带「改前红、改后绿」的
    eval 用例（`protocol/evals/`）；`cargo run -p automed -- protocol eval protocol/` 必须通过。`kernel-contract` 区域不能动。
    **已经固定了旧版协议的任务也会遇到新配置**（任务目录里的模板是创建时的副本，本任务自己就固定在 v4）：设计须写明
    旧模板下配了 `quick` 时会话读到什么，不能渲染失败、不能出现未填的占位符（T-61 有先例测试
    `before_merge_prompt_legacy_bug_review_gets_one_note`）。命令文本是数据：`quick` 里出现 `{slug}` 这类花括号不得被当成
    模板语法（现有测试 `before_merge_prompt_role_selects_sections_and_preserves_command_data`）。
11. **正在运行的任务。** 配置每个 tick 重新解析。设计须说明并测试用户在任务运行中途改配置时的行为，至少包括：任务停在
    `Gate`（在跑 / 排队 / 已暂停 / 已停止）时加上或拿掉 `quick`；`each_round` ↔ `before_merge` 切换时 `Gate` 上在途的
    作业属于另一种；FixGate 在跑时切换。现有先例：`before_merge` 且没配 `quick` 时任务停在 `Gate`，`run_gate` 清理作业、
    写 `gate.skipped`（原因「门禁只在合并前运行」）后按通过继续——没配 `quick` 时这条保持（约束 1）。要求是不卡死、
    不报协议失败、`kind` 不记错、不漏跑合并前的完整门禁。
12. **`autome-domain` 保持纯净。** 不做 I/O、不读文件、不碰 SQLite；配置经 `ResolvedConfig` / `Context` 进状态机。
    `ResolvedConfig` 会原样出现在 `project.get` 的 `config` 里：新增字段沿用 `gate_run` 的做法，缺省时不改变已有 JSON
    （样板测试 `before_merge_defaults_override_and_json_compatibility`），旧账本 / 旧 JSON 反序列化不受影响。
13. **文档写明取舍。** README 与技术设计 §8.1 的新增段落必须包含：两级门禁各自跑什么、什么时候跑；推荐快检只放格式 /
    lint / 类型检查并控制在 1 分钟内，测试留给完整门禁；`each_round` 下 `quick` 不生效；完整门禁是否包含快检的内容由项目
    自己决定，Autome 不拼接；`AUTOME_GATE_KIND`。两处现有文字里「Bug 修复轮只跑复现命令」的说法要同步改准。README 是
    英文的，技术设计是中文的，各按原文语言写。
14. **质量门。** 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过；实现轮按
    §3 第 6 条自己跑前两条并记退出码。不要为了让门禁变绿去改 `ci-local.sh` 或跳过步骤。
15. **并行任务与 rebase。** 本仓库 `[loop] parallel = 3`。整理时 main 比本分支多一个 T-46（`autome run` / `logs`
    支持 `--json`），它改了 `crates/autome-cli/src/render.rs`、`main.rs`、`tests/cli.rs`、`crates/automed/src/dispatch.rs`、
    `store.rs`——正是本任务要改 `autome run` / `status` 文案的地方，rebase 时会相遇。改动保持在必要范围内，不做无关的移动
    或重排；`--json` 输出里的事件载荷会带上新的 `kind`，设计须确认与 T-46 的输出约定不冲突（rebase 之后以 main 上的代码
    为准）。不要去读写其它任务的 worktree。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

行号是 2026-11-13 本分支（`042fd06`）上的位置，会随 rebase 漂移，以符号名为准。

**配置（`crates/autome-domain/src/config.rs`）。**

- `GateConfig { command, timeout_minutes }`；`ProjectConfig.gate: Option<GateConfig>`，`None` 表示没有门禁。
- `[gate]` 的其它键都是旁挂字段，不在 `GateConfig` 里：`ProjectConfig.gate_run: Option<GateRun>`、
  `gate_same_failure_limit`、`gate_reuse_for_main_check`、`gate_concurrency`；`ResolvedConfig` 里对应
  `gate_run: GateRun`（`#[serde(default, skip_serializing_if = "GateRun::is_each_round")]`）等。
- `GateRun { EachRound(缺省), BeforeMerge }`。`ResolvedConfig::gate_before_merge() -> Option<&str>`（有门禁且
  `before_merge` 时返回完整门禁命令，给 prompt 用）、`gates_each_round() -> bool`（有门禁且 `each_round`）。
- `GateConfig { .. }` 字面量构造有 9 处（`config.rs` 2，`task.rs`、`bug_loop.rs`、`brief.rs`、`config_io.rs`、
  `guards.rs`、`scheduler.rs`、`tests/prompt_rendering.rs` 各 1）——往结构体里加字段会让它们编译不过，关系到约束 1。
- 常量：`DEFAULT_GATE_TIMEOUT_MINUTES = 30`、`DEFAULT_GATE_SAME_FAILURE_LIMIT = 3`、`DEFAULT_GATE_CONCURRENCY = 1`。

**配置读写（`crates/automed/src/config_io.rs`）。**

- `parse_project` 的 `[gate]` 段（约第 318–366 行）依次读：`reuse_for_main_check`、`concurrency`、`same_failure_limit`、
  `run`、`command`、`timeout_minutes`。`command` 缺失报 `[gate] 需要 command，例如 command = "make check"`——唯一例外是
  只配了 `concurrency`（此时等于没有门禁）；`command` 去空白后为空 → 没有门禁，此时 `timeout_minutes` 不读也不校验，
  但前面几个键已经校验过。`timeout_minutes`：`as_integer().filter(|m| *m > 0)`，否则 `[gate] timeout_minutes 必须是正整数`。
- `parse_global`（约第 131 行）只读 `[gate] concurrency`，表里其它键忽略。
- 相关测试：`before_merge_parse_default_and_explicit_modes`、`before_merge_invalid_run_is_rejected`、
  `before_merge_save_role_preserves_gate_text_and_reload`、`before_merge_empty_command_still_validates_run`、
  `before_merge_global_gate_run_remains_ignored`、`a_gate_without_a_command_is_refused_and_an_empty_one_means_none`、
  `gate_concurrency_without_command_means_no_gate`。
- `save_project` 用 `toml_edit` 写回，保留用户的注释与顺序，不碰 `[gate]`。
- `autome doctor`（`autome-cli/src/main.rs::doctor`）只做环境探测，不读项目配置。项目配置的校验结果在 `project.get` 等
  接口的 `violations`（`config::validate(&resolved, &inventory)`，`dispatch.rs` 约第 548、1033 行），非空会拒绝保存
  （`reject_if_invalid`）。

**需求状态机（`crates/autome-domain/src/task.rs`）。**

- 实现轮结束（约第 1151 行）：`if ctx.config.gates_each_round() { → Gate, RunCoreStep{Gate} }`，否则审计开启 → `Audit`，
  审计关闭 → `after_implement_without_audit`。
- `after_gate`（约第 1380 行）：`(Gate, Passed)` → 审计，或无审计时 `after_implement_without_audit`（读不到状态块则拒绝）；
  `(Gate, Failed{head, command, tail})` → 预算用完则 `Fail(ImplBudget)`，否则回实现并注入 `Inject::GateFailed`；
  `(MergeGate, Passed)` → `AwaitMerge`；`(MergeGate, Failed)` → 回实现。
- T-61 的单测（约第 2167–2300 行）：`before_merge_implement_with_audit_equals_no_gate`、
  `before_merge_implement_without_audit_equals_no_gate`、`before_merge_rebase_and_merge_keep_full_gate_nodes`、
  `before_merge_merge_gate_outcomes_equal_each_round`、`before_merge_existing_gate_pass_still_uses_after_gate`。
  这些是「没配 `quick`」的行为，不许改。

**Bug 状态机（`crates/autome-domain/src/bug_loop.rs`）。**

- `Node::Fix => core(Node::FixGate)`（约第 234 行）；`GateDone` 于 `FixGate | MergeGate`（约第 278 行）：失败 → `start_fix`
  注入 `GateFailed`；FixGate 通过 → 复核；复核通过 → `core(Rebase)` → MergeGate。
- 126/127 的处理在 ReproCheck 一侧：`ReproDone` 里退出码 126/127 与超时不算「已复现」（约第 251 行；`scheduler.rs`
  约第 1788 行同一判断）。

**调度器（`crates/automed/src/scheduler.rs::run_gate`，约第 3233 行）。**

- 开头的特例：非 Bug 任务、节点是 `Gate`、有门禁且 `before_merge` → `gate::discard_node(.., "gate")`，写
  `gate.skipped {node: "gate", reason: "门禁只在合并前运行"}`，按通过继续。**配了 `quick` 时这里要改成真的跑快检。**
- Bug 任务在 `ReproCheck | FixGate | MergeGate` 上现场拼 `GateConfig`：复现命令取自 `bug.located`（ReproCheck）或
  `bug.reproduced`（其余）事件的 `command`；有项目门禁且（节点是 MergeGate，或节点是 FixGate 且 `each_round`）时命令为
  `( {repro} ) && ( {gate.command} )`，否则只有复现命令；`timeout_minutes` 取项目门禁的，没有则取默认值。
  其它情况用 `resolved.gate`；选不出门禁（配置被拿掉）→ 按通过继续。
- 作业：`gate::Job::at(repo, task, node, key)`，文件 `.autome/output/gates/<task>/<node>-<head 前 12 位>.{log,pid,exit,started}`；
  FixGate 另有落盘标记 `<…>.repro-only`（`Job::set_repro_only` / `repro_only`），用来在收结果时知道这次启动没带项目
  门禁，即使期间配置变了；此时 `gate.result.command` 记的是复现命令本身。
- 启动前：`[setup]` 准备（`setup::ensure_with_hold`）→ `gate_queue::admit(ctx, task, project, node, "gate", None)`
  （排不上写 `gate.queued {node, job, checkout, position}` 并返回）→ `gate::start(&job, &cwd, &gate.command, env)`，
  `env` 是 `AUTOME_TASK`、`AUTOME_HEAD`、`AUTOME_GATE_NODE`；`gate.rs::start` 另设 `AUTOME_GATE_COMMAND`、
  `AUTOME_GATE_LOG`、`AUTOME_GATE_PID`、`AUTOME_GATE_EXIT`。
- 超时在 `gate::poll(&job, gate.timeout_minutes)`；超时的退出码是 `gate::TIMED_OUT`（124），日志尾部带一行内核终止说明。

**事件。**

- 账本里的门禁事件种类：`gate.queued`、`gate.queue_dropped`、`gate.started`、`gate.result`、`gate.reused`、
  `gate.skipped`。**没有 `gate.finished`。**
- `gate.started` 载荷：`node`、`sha`、`command`、`log`。`gate.result`：`node`、`sha`、`ok`、`exit_code`、`command`、
  `log`、`tail`，失败且节点参与熔断时另有 `fingerprint`、`fingerprint_summary`、`same_failures`、`tripped`。
- 熔断（`record_gate_result`）：按节点找上一条 `gate.result` / `gate.reused` 算连续相同指纹；`gate_failure::counts(node)`
  决定哪些节点参与（`gate` / `merge_gate` / `fix_gate`）。

**沿用。**

- MergeGate（`run_gate` 内，约第 3355 行）：`last_event(task, "gate.result")` 的 `ok == true` 且 `sha == head`，并且
  （不是 Bug 任务 **或** `command` 等于本次要跑的命令）→ 写 `gate.reused {node, sha}`，直接通过。需求任务这一支不比命令、
  不比节点——约束 5 的来由。
- MainCheck（`main_check_reuse`，约第 3162 行）：取 `merge_gate` 节点最近的 `gate.result` / `gate.reused`（后者再按 `sha`
  找回原结果），要求通过、是合并提交的第二父提交、其前一条 `gate.started` 的 `node` / `sha` 与结果一致且 `command` 等于
  预期的完整命令、两棵树相同。注意 `gate.reused` 按 `sha` 找回的「原结果」不限节点。

**简报与 prompt。**

- `brief.rs::gate_evidence(config, last, head)`：没有门禁或 `each_round` → 最后一条 `gate.result` 渲染成一段；
  `before_merge` → 结果的 `sha` 等于 `head` 才渲染，否则写常量 `GATE_ONLY_BEFORE_MERGE`
  （「本项目门禁只在合并前运行，本轮没有宿主门禁结果。」）。调用处在 `scheduler.rs` 约第 2669 行，只给审计角色，取的是
  **不分节点**的最后一条 `gate.result`。
- `gate_line_of_result` 的文字：「Autome 在提交 `<sha>` 上运行了 `<命令>`：通过 / 未通过（退出码 N）。完整输出：`<日志>`。
  这是实现轮之外的一次独立运行。复验时可以引用它，但它不替代你自己的负向对照。」
- 简报只给非 Bug 会话写（`write_brief` 调用处过滤了 `SessionKind::Bug`）。Bug 复核的门禁说明在 prompt 里。
- `launcher.rs`：`SessionSpec.gate_before_merge: Option<&str>`（来自 `resolved.gate_before_merge()`）；`select_prompt` 按它
  是否为 `Some` 选 `<!-- gate-run: … -->` 分段（`autome_domain::protocol::select_gate_run`）；
  `gate_line(command)` =「全量门禁（`<命令>`）由宿主在合并前统一运行，本轮不要自己跑。」；
  `BUG_REVIEW_GATE_NOTE` =「本项目门禁只在合并前运行，本轮没有宿主门禁结果；修复门禁只跑了复现命令，它通过不等于项目门禁
  通过。」；`Inject::GateFailed` 的渲染分需求 / Bug、分有无 `gate_before_merge` 共四段文字（约第 478–525 行）。
- 模板里带 `before_merge` 分段的有五份：`protocol/prompts/plan.md`、`impl.md`、`audit.md`、`fix.md`、`fix-review.md`；
  `fix-review.md` 的 `before_merge` 段把「修复门禁只跑了复现命令」写死在正文里。`protocol/bug-protocol.md` 第 40 行：
  「项目 `[gate] run = "before_merge"` 时修复门禁只跑复现命令，项目门禁留到合并前门禁」。
- 金样：`crates/automed/tests/golden/prompts/{plan,impl,audit,fix,fix-review}-before-merge.md`、
  `golden/briefs/audit-before-merge.md`；测试在 `tests/prompt_rendering.rs`（`before_merge_audit_brief_matches_its_golden`、
  `before_merge_role_prompts_match_their_goldens`、`before_merge_bug_prompts_match_their_goldens`、
  `before_merge_leaves_the_other_prompts_byte_identical`）。
- 协议：`protocol/` 是源（编译进 `automed`），当前最大版本 `protocol/v4`；任务目录里的 `protocol/` 是 v4 的副本（多一份
  `contract.toml`，少 `evals/`）。改法见 `protocol/README.md`「怎么改」。

**读者。**

- `dispatch.rs` 约第 1661 行：`task.get` 的 `"gate": last_event(task, "gate.result")`——原样返回最后一条结果。
- `autome-cli/src/render.rs`（约第 335–361 行）：`gate.queued` →「排队等门禁（<节点>，入队时前面还有 k 个）」；
  `gate.started` →「<任务> <节点>开始：<命令> @ <sha>」；`gate.result` →「<任务> <节点>通过 / 未通过（退出码 N）  日志：…」；
  `gate.reused`、`gate.skipped` 各一行。节点名经 `node_label`（`gate` → 门禁、`merge_gate` → 合并前门禁、
  `fix_gate` → 修复门禁）。
- `autome-cli/src/main.rs` 约第 702 行：`autome status <任务>` 打印「门禁      <节点> 在 <sha> 上<结论>（<命令>）  日志 <路径>」。
- 桌面端 `apps/desktop/renderer/screens/task.js::hostTags`：读 `task.get` 的 `gate`，显示标签「<节点>通过 / 未通过 · <sha>」，
  悬停显示命令与日志——快检通过会被显示成「门禁通过」。`routing.js` 用 `resolved.gate.command` 画一个固定的门禁节点。
- Web `apps/web`：只有节点名映射（`app/lib/labels.ts`），没有读门禁结果的地方。
- main 上 T-46 新增了 `autome run --json` / `logs --json`（本分支还没有，rebase 后才看得到）。

**现有 e2e（`crates/automed/tests/end_to_end.rs`）。**

- 工具：`World::configure(toml)` 写项目配置；`write_gate_script`（脚本放在仓库外；工作目录里有 `gate-ok` 文件就通过）；
  `run_gated_task_with(w, request, count, reuse)`。
- T-61 的用例：`feature_before_merge_two_milestones_run_the_gate_once_before_the_merge`（约第 2567 行）、
  `feature_before_merge_a_failing_merge_gate_returns_to_implement_and_merges_after_the_fix`、
  `brief_before_merge_audit_brief_never_shows_a_stale_result`、
  `feature_before_merge_switching_while_blocked_on_the_gate_skips_it`、
  `feature_before_merge_switching_while_stopped_at_the_gate_skips_it_on_resume`、
  `bug_loop_before_merge_fix_gate_runs_only_the_repro_and_merge_gate_runs_both`（约第 4529 行）、
  `bug_loop_before_merge_an_ineffective_fix_fails_the_fix_gate`、`prompt_before_merge_*`、`protocol_before_merge_*`。
  新 e2e 可照它们写；它们本身不许改。
- `scheduler.rs` 内的单测：`before_merge_fix_gate_result_cannot_be_reused_on_the_same_head`、
  `before_merge_fix_gate_records_repro_across_mode_switch_even_on_failure`、
  `before_merge_fix_gate_crash_window_ignores_missing_and_stale_start_events`。

**权威规格的现状文字。**

- `README.md` 第 107 行起「Host gate, retries and cooldowns」：`[gate]` 示例六个键；「Host job queue」「The gate.」两条；
  `before_merge` 一段写着 "Bug fix rounds run only the repro command"。
- `docs/development/technical-design.md` §8.1（第 305 行起）：「宿主门禁。」一段与「合并前门禁模式。」一段（后者写「Bug
  修复轮只跑复现命令，FixGate 节点保留」「任务停在 Gate 时切换到此模式，宿主清理该节点作业并写 `gate.skipped` 后继续」）。
- `scripts/ci-local.sh` 的步骤：Format、Clippy、Test、Desktop、Contract、Web lint、Web typecheck、Web test、Web build。
  任务卡给本仓库拟的快检命令是其中 Format、Clippy、Web lint、Web typecheck 四步。

**T-61 的两条教训（`docs/门禁只在合并前跑一次-gate-run/lessons.md`）。**

- L-01：Autome 读的是**主检出**的 `.autome/config.toml`；worktree 里那份是分支基线的副本，可能已经过时。「没有门禁」和
  「有门禁但还没出结果」在简报里都是什么也不写，审计分不出来；9 个实现轮都写「由宿主门禁负责」，任务目录里没有任何
  一轮跑过 clippy / ci-local 的证据。
- L-07：合并前门禁第一次运行只有 Clippy 失败（一处 `needless_borrow`），同一条 clippy 在会话里只要 3.27 秒。提议：改了
  `.rs` 的实现轮必须在证据里记录 clippy 与 fmt 的退出码，不得以「由宿主门禁负责」跳过。本任务 §3 第 6 条即此。

## 7. 项目背景摘要（2026-11-13 整理时）

**仓库。** 单个仓库 `autome-v2`。Autome 2.0：一个 macOS 桌面应用加命令行（另有 Web 面板），把一句话需求在专用 worktree
里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 →〔门禁〕→ 审计 → 复盘 → rebase → 合并前门禁 → 等你合并 →
合并 → 验证 main → 清理）推进成合并提交。Bug 任务走另一条 Loop（定位 → 复现检查 → 修复 → 修复门禁 → 复核 → rebase →
合并前门禁 → …）。**内核独占节点转换**：会话只写文档与代码，下一个节点由 `automed` 读状态块决定。本仓库正在用自己的
Loop 开发自己。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置合并与校验（`config.rs`）、设计文档
    解析（`status_block.rs`）、任务状态机（`task.rs`）、Bug Loop 状态机（`bug_loop.rs`）、门禁排队与失败指纹的纯逻辑
    （`gate_queue.rs`、`gate_failure.rs`）、prompt 分段选择（`protocol.rs`）。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git（`git.rs`）、会话启动器与 prompt 渲染
    （`launcher.rs`）、调度器（`scheduler.rs`）、宿主门禁作业（`gate.rs`）、门禁排队（`gate_queue.rs`）、宿主准备
    （`setup.rs`）、配置读写（`config_io.rs`）、简报（`brief.rs`）、JSON-RPC 派发（`dispatch.rs`）。测试在
    `crates/automed/tests/`（`end_to_end.rs`、`prompt_rendering.rs`、`real_cli.rs`、`golden/`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析与输出（`main.rs`、`render.rs`、
    `tests/cli.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`）；`apps/web` — React Router + Node 服务器的面板（vitest、eslint、tsc）。
- `protocol/` — Loop 协议源（`loop-protocol.md`、`session-protocol.md`、`bug-protocol.md`、`prompts/`、`brief-map.toml`、
  `evals/`、`CHANGELOG.md`），编译进 `automed`；`plugins/` — Claude 插件，本任务不动。
- `docs/development/requirements.md`、`docs/development/technical-design.md` 是规格权威；`docs/adr/` 是 ADR；
  `docs/plans/` 是按日期的决策记录；`docs/<任务目录>/` 是各任务的文档。

**测试命令。**

```sh
cargo test                                     # domain、core、端到端
cargo test -p autome-domain                    # 状态机与配置单测
cargo test -p automed --lib                    # 调度器、配置读写、简报、启动器的单测
cargo test -p automed --test end_to_end        # 端到端
cargo test -p automed --test prompt_rendering  # prompt 与简报金样（AUTOME_UPDATE_GOLDEN=1 接受差异）
cargo test -p autome-cli                       # CLI 渲染与命令
cargo run -p automed -- protocol eval protocol/  # 协议静态门，改了 protocol/ 必跑
cargo clippy --all-targets -- -D warnings      # 警告即错误
cargo fmt --all -- --check
npm run lint --prefix apps/web && npm run typecheck --prefix apps/web
sh scripts/ci-local.sh                         # 全量本地 CI（Rust + 桌面端 + Web），即本项目的 [gate] command
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记，只把模型换成替身；测试数据库由
`AUTOMED_DB_PATH` 各自隔离。命令输出超过 5 行的，全文重定向到 `.autome/output/`，只读头尾。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/` 下的任何内容；只在当前 worktree 内工作，不切分支、不操作 `.worktree/` 下的其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进
  `.autome/output/`，不要进任务文档目录。
- `.autome/rules/`：只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`（只读；worktree 里这份是分支基线的副本，生效的是主检出那份）：`[loop] parallel = 3`；
  `[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、`timeout_minutes = 30`；
  `[retry] attempts = 2`、`backoff_seconds = 60`。没有 `quick`——本任务自己的开发过程里，宿主只在合并前跑一次完整门禁。
