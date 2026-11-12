# 任务：宿主门禁按机器排队——`[gate] concurrency` 缺省 1，排队不计预算、超时从真正启动算

本任务的规则见 `docs/宿主门禁按机器排队-gate/protocol/loop-protocol.md` 与
`docs/宿主门禁按机器排队-gate/protocol/session-protocol.md`。

- 设计文档：`docs/宿主门禁按机器排队-gate/宿主门禁按机器排队-gate.md`
- 任务卡（用户写的，第一输入）：`docs/宿主门禁按机器排队-gate/宿主门禁按机器排队-gate-card.md`
- 任务整理日期：2026-11-12
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `ed2460d`

## 1. 需求原文

> 宿主门禁按机器排队：[gate] concurrency 缺省 1，排队不计预算、超时从真正启动算

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§4 照任务卡整理。

## 2. 目标与背景

**背景（任务卡原文的事实）。** 门禁作业（Gate / MergeGate / MainCheck / ReproCheck / FixGate / 准备命令）由宿主以
后台作业运行，互不排队。本仓库 `[loop] parallel = 3`，经常 2–3 个完整 `ci-local`（fmt + clippy + 全部 Rust 测试 +
e2e + desktop + web）同时跑，再叠加 3 个会话各自的 `cargo test`。2026-11-12 实测：T-28 合并后验证 main 时，T-36 的
合并前门禁早 43 秒启动、T-35 的门禁早 20 分钟启动；Rust `Test` 步骤安静时 51 秒，这次 1192 秒（23 倍），整条门禁撞到
`[gate] timeout_minutes = 30` 被杀（退出码 124）。同一时段 T-36 的合并前门禁也超时，退回修复轮，白白消耗一个修复轮次；
T-28 留下一条 `main_gate_failed` 警报。

证据（在**主仓库**的 `.autome/output/` 下，不在本 worktree 里；只读，不要改、不要复制进任务文档目录）：

- `/Users/dannie/project/autome-v2/.autome/output/gates/T-28/main_check-9b5750b07fe9.log`
- `/Users/dannie/project/autome-v2/.autome/output/gates/T-36/merge_gate-fe76a37af7a5.log`
- `/Users/dannie/project/autome-v2/.autome/output/gates/T-30/main_check-*.log`（安静基线）

**目标。** 宿主同一时刻最多运行 N 个门禁/准备作业（N = `[gate] concurrency`，缺省 1，跨项目计）；超出的任务停在原节点
排队，排队期间不启动作业、不计预算、不计超时。

任务卡「要做什么」五条：

1. 全局配置 `~/.autome/config.toml` 新增 `[gate] concurrency = 1`（可省，缺省 1；项目配置可覆盖但**不得超过全局**）。
   宿主同一时刻最多运行 N 个门禁/准备作业（跨项目计）。
2. 超过时任务停在原节点、状态显示「排队等门禁（前面还有 k 个）」；排队不启动作业、不写 `gate.started`、不计入任何预算；
   轮到时才启动，**超时时钟从作业真正启动时算**。顺序先到先得；MainCheck（合并后验证）与 MergeGate 可以优先于普通
   Gate，**由设计轮定**。
3. `autome status` / `run` 与 `dashboard.get`、`task.get` 体现排队状态（新事件 `gate.queued {position}`，或在 `task.get`
   里加 `gate_queue_position`）。
4. core 重启后排队顺序按任务进入排队的时间恢复（可由事件推导，**不加表**）。
5. 顺带：门禁超时（124）产生的失败在 `FailureReason` / 注入文案里明确写「门禁超时」，与测试失败区分（门禁熔断的失败
   指纹已把超时单列，沿用）。

## 3. 验收标准

取自任务卡「验收标准」，拆成可逐条核对的条目。设计文档的里程碑必须逐条覆盖。

1. e2e：`concurrency = 1` 下两个任务同时到门禁，第二个**只在第一个结束后**才有 `gate.started`。
2. e2e：第二个任务排队期间，`task.get` 显示排队，且**无预算变化**。
3. e2e：超时按各自**启动时间**计（排队时间不算进超时）。
4. 未配置 `concurrency` 时行为等同 `concurrency = 1`——这是缺省，所以要**同时断言旧用例不变**。
5. `cargo test --workspace` 全绿。
6. README 配置一节补 `concurrency`。

补充说明（从任务卡和调研得出，不是新范围）：

- e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本、真实门禁后台作业，
  只把模型换成替身）。只在 `scheduler.rs` 里写单元测试不能代替它。
- 第 1 条要用账本事件的 `seq` 顺序（第一个任务的 `gate.result` 先于第二个任务的 `gate.started`）或等价的、不依赖
  时序猜测的证据断言，不要靠 `sleep` 后看一眼。
- 第 2 条的「预算」至少包括：实现轮次（`implementation-round` / `ImplBudget`）、Bug Loop 的修复轮次（`FixBudget`）、
  门禁同因失败的连续计数（`same_failure_limit`）、崩溃重试次数（`[retry] attempts`）。设计要列出自己核对过的全部
  计数，并说明断言的是哪些字段。
- 第 3 条：门禁超时最小单位是 1 分钟（`timeout_minutes` 为正整数），真跑一分钟的 e2e 会拖慢门禁本身。现有测试的
  做法可参考 `gate.rs` 的 `a_gate_that_runs_too_long_is_killed_and_reported_as_timed_out` 与
  `setup_job_past_its_timeout_is_killed_with_124_and_a_setup_note`（看它们怎样不等满一分钟）。设计要写明这条怎么测，
  既不能弱化成「只断言 `.started` 文件的时间」，也不能让测试套件慢上几分钟。
- 第 4 条：现有门禁、准备命令相关的单元测试与 e2e（见 §7）原则上**一行不改**就要通过。若确有必要改已有测试，设计文档
  要逐条列出理由，且理由不能是「新行为与旧断言不符」。

## 4. 范围

在范围内：

- `crates/autome-domain`：`config.rs` 的配置形状（全局 `[gate] concurrency`、项目覆盖、解析后的值、范围校验）；若超时
  要进 `FailureReason` / `Inject`，在 `task.rs` / `bug_loop.rs` 里做（见约束 9）；
- `crates/automed`：`config_io.rs` 读写全局与项目的 `concurrency` 及中文报错；`scheduler.rs`（`run_gate`、
  `run_core_step`、恢复路径）与 `setup.rs` 的排队判定；`gate.rs` 如需新的只读探测；`dispatch.rs` 的 `task.get` /
  `dashboard.get` 输出排队状态；`launcher.rs` 的超时注入文案；相应单元测试与 e2e；
- `crates/autome-cli`：`render.rs`（`state_line`、`event_line`、`failure`）与 `main.rs` 的 `status`，显示
  「排队等门禁（前面还有 k 个）」以及新事件；
- 文档：README「Host gate, retries and cooldowns」一节补 `concurrency`；`docs/development/technical-design.md` §8.1
  「宿主门禁」「宿主准备」补排队规则。

不在范围内（不要做）：

- 改 `scripts/ci-local.sh`（任务卡「不做什么」）；
- 改会话沙箱（任务卡「不做什么」）；
- 自动调 `timeout_minutes`（任务卡「不做什么」）；
- 限制会话自己在沙箱里跑的 `cargo test`——本任务只排宿主起的门禁/准备作业；
- 新增 SQLite 表（任务卡第 4 条：不加表）；
- 改 `protocol/`、`plugins/`、`.autome/`（后者 AGENTS.md 明令禁止；本仓库自己的 `concurrency` 由用户配）；
- 桌面端 / Web 面板的新展示——任务卡只点名 `autome status` / `run`、`dashboard.get`、`task.get`。界面读者只做
  「不坏」的核对（见约束 11），文案改进进 Backlog；
- 顺手重构 `gate.rs`、`scheduler.rs`、`setup.rs` 或与本需求无关的代码。

## 5. 硬性约束

1. **排队不留痕迹于预算与作业。** 排队中的任务：不调 `gate::start`、不产生 `.started` / `.pid` / `.log` 文件、不写
   `gate.started` / `setup.started`、不动任何轮次与计数（验收 2）。特别注意 `record_gate_result` 用
   `any_event_after(task, ["gate.started", "session.started"], seq)` 判断「上一次结果之后是否有新运行」——新增的排队
   事件不能被它误当成一次运行，`gate.started` 也不能在排队时提前写。
2. **超时从真正启动算。** `gate::poll_as` 用 `.started` 文件里的秒数算已运行时长；`.started` 由 `gate::start` 写。
   排队期间不得有任何东西让时钟开始走。准备命令（`setup.rs` 用 `gate::Job::at(repo, task, "setup", kind)` 与
   `poll_as(…, "准备命令")`）同理。
3. **「按机器」的计数范围是一个 Autome 账本（一个 `~/.autome`、一个 core），不是整台操作系统。** 「跨项目计」指同一
   账本里的所有项目。计数不得靠扫描全机进程或固定的全局路径实现：本仓库的门禁命令就是 `ci-local.sh`，它跑
   `cargo test`，其中的 e2e 会在各自的临时 home 里再起门禁作业；若这些内层门禁与外层真实门禁共用一个名额，
   `concurrency = 1` 下门禁会等自己，永远跑不完。同理，`cargo test` 默认并行跑多个测试，每个测试有自己的账本，互不
   排队。设计须写明计数范围及其依据，并保证这一点可测。
4. **计入名额的作业种类要逐个写清。** 任务卡列的是 Gate / MergeGate / MainCheck / ReproCheck / FixGate / 准备命令。
   设计须逐个说明是否占名额、何时占、何时放，并覆盖这些不启动作业的路径（它们**不应**排队）：`[gate]` 已从配置移除
   （直接 `Passed`）；MergeGate 的 head 与上次通过相同（`gate.reused`）；准备指纹未变（`Ready::Ready`）；工作区项目
   跳过准备（`setup.skipped`）；准备失败后跳过 MainCheck（`gate.skipped`）。还须说明「门禁前先跑准备」时，准备与
   随后的门禁是各排一次还是连着占同一个名额，并给出理由（各排一次会让一个任务在两个作业之间被插队）。
5. **名额必须能收回。** 作业结束（通过、失败、超时）、`Vanished`、head 已变被作废（`job.clear()`）、任务被暂停 /
   停止 / 取消 / `rerun` / 失败、`discard_node`、`stop_all`，都不能让名额泄漏，也不能让已离开门禁节点的任务继续占着
   队列位置挡住后面的任务。暂停的任务其作业仍在跑（现有行为：恢复后接着收同一作业，见
   `paused_gate_resumes_at_gate_without_restarting_job`）——设计须说明这种作业是否仍占名额，以及排队中被暂停的任务
   怎样处理。
6. **重启后正确，且不加表。** core 会在门禁运行中途重启（`gate.rs` 模块注释）；名额占用与排队顺序不能只存在内存里。
   「谁在跑」可由作业文件（`.started` / `.pid` / `.exit`）与账本推出，「谁先排的」由事件推出（任务卡第 4 条）。
   账本 schema 当前是 v5（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`），**不改 schema**。升级当下正在跑的
   多个门禁作业（旧版本并行启动的）必须能被正常收完，不能被杀或被判 `Vanished`；设计须说明此时运行数超过 N 的
   过渡行为。
7. **顺序。** 先到先得；MainCheck 与 MergeGate 是否优先于普通 Gate 由设计轮决定并给出理由（任务卡明确留给设计轮）。
   无论怎样定，都要说明不会饿死：任何排队任务在有限个作业之后一定轮到。「前面还有 k 个」的 k 要与实际启动顺序
   一致。
8. **配置。**
   - 全局 `~/.autome/config.toml` 的 `[gate] concurrency`：可省，缺省 1。现状：全局配置**没有** `[gate]` 段
     （`parse_global` 只读 `loop` / `ui` / `roles`），`GlobalConfig` 也没有对应字段；`save_global` 重写文件时不得丢掉
     用户手写的这一项。
   - 项目 `.autome/config.toml` 的 `[gate] concurrency`：可覆盖，**不得超过全局**。设计须定：超过时是报中文错误还是
     按全局取小，并说明理由；还须说明「跨项目计」下项目级的值是什么语义（例如：该项目的任务在全机运行数达到它
     自己的值时就不再启动）。现状：项目 `[gate]` 缺 `command` 会报「[gate] 需要 command…」，`command` 为空白等于
     没配——只写 `concurrency` 不写 `command` 的项目 `[gate]`（例如只有 Bug Loop 复现命令、或只有 `[setup]` 的项目）
     要有确定行为。
   - 取值范围与越界的中文报错沿用 `config_io.rs` 现有写法（例如 `[gate] same_failure_limit 必须是 1 到 10 的整数`）；
     0、负数、非整数都要拒绝。`save_project` 从路由界面保存时不得丢掉这一项（参照测试
     `a_gate_and_a_retry_policy_are_read_from_the_project_file`）。
   - 缺省行为就是 `concurrency = 1`（验收 4）：没写这一项的旧配置文件不需要迁移。
9. **超时与测试失败要分得开（任务卡第 5 条）。** 现状：`gate::TIMED_OUT = 124`；`gate_failure::fingerprint` 对超时
   返回摘要「门禁运行超时，被内核终止」（熔断路径已区分）；但 `Inject::GateFailed{head, command, tail}` 与
   `launcher.rs` 里两段注入文案（普通 Loop 与 Bug Loop）只说「没有通过」，`main_gate_failed` 警报文案只写
   「退出码 124」。设计须列出门禁超时会走到的每一条用户/会话可见文案并改到明确写「门禁超时」。`FailureReason` 与
   `Inject`、`Trigger` 是对外的 serde 形状：**可以新增字段或种类，不能改已有种类的名字与字段含义**；新增字段要让旧
   账本里的旧事件照常可读（缺省值）。新增 `FailureReason` 种类时同步读者：`crates/autome-cli/src/render.rs` 的
   `failure()`、`apps/desktop/renderer/lib/labels.js` 的 `failureReason()`。退出码恰好是 124 但不是内核杀的命令
   （门禁命令自己用了 `timeout(1)`）是否算「门禁超时」，设计须说明。
10. **对外形状只增不改。** `task.get` 现有字段（含 `queue_position`——它是**任务排队等项目并行槽位**的名次，语义
    不同，不得复用或改义；`gate` 是最后一条 `gate.result`）、`dashboard.get` 现有字段、`gate.started` /
    `gate.result` / `gate.reused` / `setup.*` 事件现有字段，名字与含义都不变（审计简报的「宿主门禁」段在读
    `gate.result`）。排队状态以新增字段或新增事件表达。任务卡给了两种做法（`gate.queued {position}` 事件，或
    `gate_queue_position` 字段），设计可取其一或都做，但要保证事件里的 `position` 不会因为后来的插队 / 离队变成
    误导（事件是追加的，名次会变）。**不新增 JSON-RPC 方法、不改已有方法的参数**：桌面端两份白名单与
    `dispatch.rs` 的 `READ_METHODS` 由测试交叉核对。
11. **读者不坏。** `TaskState` / `Node` 若要新增取值来表达「排队等门禁」，须逐个列出读者并同步或确认其回退：
    `render.rs` 的 `state_line` / `node_label`、`apps/desktop/renderer/lib/labels.js`、
    `apps/desktop/renderer/screens/task.js`、`apps/web/` 的任务卡展示。优先考虑不新增状态、任务仍停在原门禁节点
    （任务卡原文：「停在原节点」），排队只作为附加信息。新事件种类进入 `autome run` 的事件流时，`event_line` 要有
    对应文案，未识别的读者不能崩。
12. **`autome-domain` 保持纯净。** 不做 I/O、不读文件、不碰 SQLite。名额判定若放进 domain，就是接收「正在跑的作业 /
    排队者列表」作参数的纯函数；探测文件与读账本在 `automed`。
13. **排队中的任务不能卡住调度的其它部分。** 一次 tick 的顺序是：收已结束的会话 → 启动到点的重试 → 跑内核步骤
    （rebase、门禁、合并、清理）→ 填空槽位。排队中的任务仍占着项目的并行槽位（`slots_in_use`）；设计须确认这是
    想要的，并确认一个任务排队不会让同一 tick 里别的任务的内核步骤被跳过、也不会让 `tick` 报错。主检出准备命令
    已有的互斥（`setup.rs` 里按其它任务 `setup-main` 作业存活状态返回 `Pending`）与新名额之间不得死锁。
14. **测试替身对齐真实接口。** e2e 的假门禁用真实的包装脚本与真实后台作业（参照 `write_gate_script` /
    `run_gated_task`）；不要为了好测给 `gate.rs` 加只在测试里走的旁路。「两个任务同时到门禁」要用真实的两个任务与
    `[loop] parallel ≥ 2` 构造。
15. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过；宿主门禁命令
    `sh scripts/ci-local.sh` 由 Autome 在实现轮之后自己跑。本任务改的正是门禁启动路径，而本任务自己的门禁在实现期间
    仍按旧逻辑并行跑、仍可能被别的任务挤到超时：遇到时按协议如实记录，不要为了让门禁变绿去改 `ci-local.sh`、
    调 `timeout_minutes` 或跳过步骤。新增的 e2e 不要明显拖慢 `cargo test`（不要真等一分钟超时，见 §3 补充说明）。
16. **并行任务。** 本仓库 `[loop] parallel = 3`，其它任务也在改 `scheduler.rs`、`task.rs`、`dispatch.rs`、
    `end_to_end.rs`。改动保持在必要范围内，不做无关的移动或重排，降低 rebase 冲突。

## 6. 留给设计轮的开放问题

任务卡明确留白或调研中发现需要定的地方（设计文档须逐条给出结论与理由）：

1. MainCheck / MergeGate 是否优先于普通 Gate；优先时怎样保证不饿死（约束 7）。
2. 排队状态用事件、用 `task.get` 字段，还是两者（约束 10）；`dashboard.get` 里放在哪。
3. 项目 `concurrency` 超过全局：报错还是取小；项目级值在「跨项目计」下的确切语义；只写 `concurrency` 不写
   `command` 的项目 `[gate]`（约束 8）。
4. `concurrency` 的上限。
5. 准备命令与其后的门禁是否连占一个名额；`setup-main`（合并后主检出准备）与 `setup-verify` 怎样计（约束 4）。
6. 被暂停任务的在途作业、排队中被暂停 / 停止的任务（约束 5）。
7. 升级瞬间已有多个作业在跑的过渡行为（约束 6）。
8. 超时文案改哪些处、是否新增字段 / 种类；非内核产生的 124 怎样算（约束 9）。
9. 验收 3 的测法（§3 补充说明）。
10. 排队顺序由哪类事件推导；任务离队后再回来（例如 head 变了重跑、`rerun`）算新排还是保留原位。

## 7. 调研时观察到的事实（供设计参考，不是设计结论；行号以基线 `ed2460d` 为准）

**门禁作业（`crates/automed/src/gate.rs`，645 行）。**

- 每次运行是一个后台作业：`Job::at(repo, task, node, key)`，文件在主仓库
  `.autome/output/gates/<task>/<node>-<key>.{log,pid,exit,started}`；门禁的 `key` 是 head 前 12 位，准备命令的
  `node` 是 `"setup"`、`key` 是检出种类（`worktree` / `verify` / `main`）。
- `start(job, cwd, command, env)`：先 `job.clear()`，写 `.started`（当前秒数），再在独立进程组里起包装脚本；
  包装脚本第一行写 `.pid`，结束时写 `.exit` 并删 `.pid`。
- `poll` / `poll_as(job, timeout_minutes, what)` 返回 `Running` / `Done{exit_code}` / `Vanished`；已运行时长 =
  现在 − `.started`；超时则杀进程组、在日志末尾追加「=== Autome：{what}运行超过 N 分钟，已终止 ===」、写退出码
  `TIMED_OUT = 124`。没有 `.pid` 且启动不足 10 秒算 `Running`（启动空窗）。
- `running(job)`：只读的存活探测（含启动空窗），目前只被主检出准备的互斥使用。
- `stop_all(repo, task)` 杀该任务目录下所有存活 pid；`discard_node(repo, task, node)` 按 `<node>-` 前缀杀并删文件。

**调度器（`crates/automed/src/scheduler.rs`，7430 行）。**

- `run_core_step`（约 2923 行）：`n if n.is_gate() => run_gate(…)`；之前对任务检出调 `setup::ensure`。
- `run_gate`（约 3009 行）：统一处理 Gate / MergeGate / MainCheck / ReproCheck / FixGate。Bug Loop 时为
  `repro_check` / `fix_gate` / `merge_gate` 现场拼一个 `GateConfig`（复现命令 + 项目门禁命令，`timeout_minutes` 取项目
  门禁的或缺省 30）。`[gate]` 被移除 → 直接 `finish_gate(Passed)`。
- `if !job.exists()` 分支（约 3091–3201 行）是启动路径：MergeGate 同 sha 沿用（`gate.reused`）→ MainCheck 建
  `.worktree/_verify-<sha12>` 检出并先跑 verify 准备 → `gate::start` → 写 `gate.started {node, sha, command, log}`。
  这是插入排队判定最自然的位置，但设计自行决定。
- 之后是收结果：`Vanished` → `job.clear()` 下个 tick 重跑；非 MainCheck 且 head 已变 → `job.clear()` 重跑；否则
  `tail = gate::tail(&job.log, 60)`，`record_gate_result` 写 `gate.result`（含指纹与连续计数），熔断则发
  `Trigger::GateSameFailure`；`ReproCheck` 发 `Trigger::ReproDone{…, timed_out: exit_code == TIMED_OUT, …}`；
  `MainCheck` 失败写 `main_gate_failed` 警报（文案只含「退出码 {exit_code}」）；其余经 `finish_gate` 发
  `Trigger::GateDone`，失败为 `GateOutcome::Failed{head, command, tail}`。
- `fill_slots_reporting_errors`（约 1445 行）：项目并行槽位按 `config.loop_defaults.parallel` 与
  `store.slots_in_use(project)` 填，是**按项目**的；本任务的名额是**跨项目**的，两者是不同的东西。
- `recover`（约 3726 行）是 core 重启后的恢复入口。

**准备命令（`crates/automed/src/setup.rs`，162 行）。** `ensure(…)` 先收在途作业（`poll_as`），再按指纹判断是否要跑；
要跑时写 `.running` 标记、`gate::start`、写 `setup.started`，返回 `Ready::Pending`。主检出准备已有一处互斥：别的任务的
`setup-main` 作业存活时返回 `Pending`。

**配置（`crates/autome-domain/src/config.rs`、`crates/automed/src/config_io.rs`）。**

- `GlobalConfig { loop_defaults, roles, ui }`——没有门禁相关字段；`parse_global` 对未知顶层表静默忽略。
- `ProjectConfig { bug, loop_overrides, roles, doc_root, gate: Option<GateConfig>, setup, gate_same_failure_limit, retry }`；
  `GateConfig { command, timeout_minutes }`。`same_failure_limit` 写在 TOML 的 `[gate]` 表里，但在结构体上是
  `ProjectConfig` 的独立字段（不在 `GateConfig` 里）——所以即便 `command` 为空它也被读入，可作 `concurrency` 的参照。
- `parse_project` 的 `[gate]`（约 312–340 行）：不是表报错、缺 `command` 报错、空白 `command` 等于没配、
  `timeout_minutes` 须为正整数。
- `resolve(global, project) -> ResolvedConfig` 是把全局与项目叠加的唯一入口；`validate` 做范围校验（参照 `parallel`）。
- `save_global` / `save_project` 用 `toml_edit` 改写文件。

**状态机与文案。**

- `FailureReason`（`task.rs` 约 195 行，`#[serde(tag = "kind")]`）：`Setup`、`GateSameFailure{count, summary, log}`、
  `DesignBudget`、`ImplBudget`、`FixBudget`、`Infeasible`、`Protocol`、`SessionCrashed`、`RebaseConflict`、`Config`、
  `CoreStep`……没有单独的「门禁超时」。
- `Inject::GateFailed{head, command, tail}`（约 619 行）没有超时标记；`launcher.rs` 约 426 与 447 行是它的两段文案。
- `bug_loop.rs` 约 247–260 行：`ReproDone` 的 `timed_out` 已单独处理（原因串 `"timed_out"`）。
- `gate_failure.rs` 143 行：`fingerprint(log, exit_code, timed_out)`，超时的摘要是「门禁运行超时，被内核终止」。

**读者。**

- `dispatch.rs`：`task_get`（约 1589 行）输出 `queue_position`（项目并行槽位的排队名次，只对 `Queued` / `Requeued`
  有值）与 `gate`（最后一条 `gate.result`）；`dashboard_get`（约 2190 行）输出 `waiting` 等；`READ_METHODS` 17 个。
- `crates/autome-cli/src/render.rs`：`state_line`（`"排队"` / `"排队（会从 X 继续）"` 指并行槽位排队）、`failure()`、
  `event_line`（已有 `gate.started`、`setup.started` 等）；`main.rs` 的 `status`（约 520 行）。
- 桌面端读 `gate` 相关数据的只有 `apps/desktop/renderer/screens/task.js` 与 `renderer/lib/labels.js`；`apps/web/app`
  下没有搜到对 `gate.*` 事件或 `queue_position` 的读取。

**现有测试（验收 4 的「旧用例」）。**

- `gate.rs` 单测：`a_gate_that_runs_too_long_is_killed_and_reported_as_timed_out`、
  `setup_job_past_its_timeout_is_killed_with_124_and_a_setup_note`、
  `setup_jobs_are_discarded_by_their_own_prefix_and_leave_gate_jobs_alone` 等。
- `scheduler.rs` 单测：直接调 `run_gate` 的若干用例（约 4058–4870 行）、
  `fill_slots_respects_the_projects_parallel_limit`、`recovery_*`。
- `end_to_end.rs`：`a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`、
  `main_failing_its_gate_after_a_merge_raises_an_alert_the_user_can_dismiss`、
  `same_failure_rerun_gate_after_environment_repair_uses_the_same_head`、
  `stopped_gate_resumes_at_gate_and_discards_killed_result`、`paused_gate_resumes_at_gate_without_restarting_job`、
  `two_tasks_waiting_to_merge_both_land_and_the_second_rebases_through_the_gate_by_itself`（两个任务都过门禁——缺省变成
  1 之后它最可能受影响，要专门核对）、`a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again`、
  `bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate`、全部 `setup_*` 用例。辅助函数：
  `write_gate_script`、`run_gated_task`、`setup_fixture`、`setup_gate_task`。
- `config_io.rs`：`a_gate_and_a_retry_policy_are_read_from_the_project_file`、
  `a_gate_without_a_command_is_refused_and_an_empty_one_means_none`、`same_failure_limit` 的几条。
- `crates/autome-cli/src/render.rs`：`setup_events_are_printed_by_autome_run`、`gate_same_failure_*`、
  `states_read_the_way_the_desktop_names_them` 等。

## 8. 项目背景摘要（生成于 2026-11-12）

**这是什么。** Autome 2.0：一个驱动「设计 → 评审/裁决 → 实现 → 门禁 → 审计 → 合并」Loop 的本地工具。内核 `automed`
持有全部状态（SQLite 账本 + 事件流），通过 stdio JSON-RPC 服务 CLI、Electron 桌面端与 Web 面板。本仓库用 Autome 自己
开发自己（`.autome/config.toml`：`[loop] parallel = 3`，`[gate] command = "sh scripts/ci-local.sh"`、
`timeout_minutes = 30`，`[retry] attempts = 2`）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：
  - `crates/autome-domain`——纯域：状态机（`task.rs`、`bug_loop.rs`）、配置形状与叠加（`config.rs`）、门禁失败指纹
    （`gate_failure.rs`）、准备指纹（`setup.rs`）、状态块解析等。不做 I/O。
  - `crates/automed`——内核：调度（`scheduler.rs`）、门禁作业（`gate.rs`）、准备（`setup.rs`）、配置读写
    （`config_io.rs`）、JSON-RPC 分发（`dispatch.rs`）、会话启动（`launcher.rs`）、账本（`store.rs`）、Git（`git.rs`）。
    测试：`tests/end_to_end.rs`、`tests/prompt_rendering.rs`、`tests/real_cli.rs`。
  - `crates/autome-cli`——`autome` 命令行（`main.rs`、`render.rs`、`client.rs`、`args.rs`），测试 `tests/cli.rs`。
- `apps/desktop`——Electron 外壳，纯 `node --test`（注意：`node --test` 会把 `test/` 下任意 `.js` 当测试跑）。
- `apps/web`——Node 网关 + React Router 面板（vitest、eslint、tsc）。
- `protocol/`——内置协议 seed（`include_str!` 进内核）；`plugins/`——Claude 插件；`scripts/`——`ci-local.sh`、打包脚本。
- 文档：`README.md`（英文，配置示例在「Host gate, retries and cooldowns」一节）、
  `docs/development/technical-design.md`（§8 调度、§8.1 驱动锁/门禁/重试）、`docs/development/requirements.md`、
  `docs/adr/`。每个任务的文档在 `docs/<slug>/`。

**测试与检查命令（在 worktree 根目录运行）。**

- `cargo test --workspace`（验收 5）；全量同门禁：`cargo test --workspace --all-targets -- --nocapture`
- 单个 e2e：`cargo test -p automed --test end_to_end <用例名>`
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`
- `npm test --prefix apps/desktop`
- `npm run lint|typecheck|test|build --prefix apps/web`
- 整套本地 CI（即宿主门禁命令）：`sh scripts/ci-local.sh`——每一步都跑完，任一步失败则非零退出。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入
  `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/宿主门禁按机器排队-gate/protocol/`（见文首指路）。

**环境注意。** macOS（Darwin），系统 `sh` 是 bash 3.2；包装脚本与测试脚本按 POSIX `sh` 写。报错与界面文案用中文，
代码注释与 README 用英文（沿用现状）。
