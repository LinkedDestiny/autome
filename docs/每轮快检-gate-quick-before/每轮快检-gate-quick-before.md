# 每轮快检（`[gate] quick`）：`before_merge` 下实现轮 / 修复轮之后由宿主跑一条静态检查

status: 实现中
design-round: 1/15
implementation-round: 1/25
current-milestone: M-02
current-milestone-reopens: 0
convergence-mode: normal
next-action: 推进 M-02 宿主快检执行、启动记录与结果沿用隔离

任务文件：`docs/每轮快检-gate-quick-before/每轮快检-gate-quick-before-task.md`

任务卡：`docs/每轮快检-gate-quick-before/每轮快检-gate-quick-before-card.md`

下文「目标 n」指任务文件 §2 的八件事，「验收 n」指 §3 的六条验收标准，「约束 n」指 §5 的硬性约束。代码位置以符号名为准
（行号是 `042fd06` 上的，rebase 后会漂移）。

## 背景与现状

T-61 给项目加了 `[gate] run = "before_merge"`：完整门禁每个任务只在 MergeGate 跑一次。格式、lint、类型检查这类几秒钟的问题
因此也要等到合并前才暴露（T-56 的 Web typecheck、T-61 自己的 `clippy::needless_borrow`）。本任务加一条宿主自己跑的「快检」命令。

本轮设计前核对过的代码现状（只列方案依赖的）：

- **配置。** `GateConfig { command, timeout_minutes }`；`[gate]` 的其它键都是 `ProjectConfig` / `ResolvedConfig` 上的旁挂字段
  （`gate_run`、`gate_concurrency` 等）。`ResolvedConfig { .. }` 的完整字面量只有 `config::resolve` 一处，其它构造都用 `..c`；
  `ProjectConfig { .. }` 的构造都带 `..Default::default()`。所以**新增旁挂字段不会让任何现有测试编译不过**，而往
  `GateConfig` 加字段会动 9 处字面量。`config_io::parse_project` 的 `[gate]` 段依次读 `reuse_for_main_check`、`concurrency`、
  `same_failure_limit`、`run`、`command`、`timeout_minutes`；`parse_global` 只读 `concurrency`。
- **需求状态机。** `task.rs` 里 `Node::Implement` 结束只有一处判断 `ctx.config.gates_each_round()`；`after_gate` 不看模式。
- **`run_gate`（`scheduler.rs`）。** 开头对「非 Bug、`Gate`、有门禁、`before_merge`」直接 `discard_node` + `gate.skipped` + 按通过继续；
  Bug 的 `ReproCheck | FixGate | MergeGate` 现场拼命令；作业文件 `<节点>-<head 前 12 位>.*`；**收结果时 `command` 取的是收结果
  那一刻配置算出的命令**（只有 FixGate 的 `.repro-only` 标记例外），这一点由现有测试
  `before_merge_unconfigured_fix_gate_keeps_legacy_current_command_accounting` 钉住。
- **沿用。** MergeGate：最后一条 `gate.result` 通过且 `sha == head`，需求任务不比命令不比节点就写 `gate.reused`。
  `main_check_reuse`：`merge_gate` 节点最近的 `gate.result` / `gate.reused`，后者按 `sha` 找回的原结果不限节点。
- **熔断。** `record_gate_result` 按节点找上一条 `gate.result` / `gate.reused` 计连续相同指纹。
- **超时。** `gate::poll` 超时在日志末尾写 `=== Autome：门禁运行超过 N 分钟，已终止 ===`，`gate_failure::kernel_timeout` 只认这一句。
  另有 `gate_queue.rs` 的巡检对所有在跑作业按**项目门禁的** `timeout_minutes` 调 `poll_as`。
- **作业枚举。** `gate_queue::jobs` 只认 `.started`；`gate::discard_except` 按后缀清单删文件（含 `.repro-only`）。
- **prompt。** `launcher::gate_line(cmd)` 由宿主填进 `{gate_line}`，`render_gate_parts` 先按占位符切开再拼接，命令文本不会被
  当成模板语法。`fix-review.md` 的 `before_merge` 段把「修复门禁只跑了复现命令」写死在正文；`bug-protocol.md` 也这么写。
  `Inject::GateFailed { head, command, tail }` 没有种类信息，`GateOutcome::Failed` / `Inject::GateFailed` 的构造共 23 处。
  `protocol/eval.rs` 有占位符白名单 `KNOWN_PLACEHOLDERS`（10 个），`eval_run.rs` 自己渲染模板跑行为用例。
- **读者。** `task.get.gate` 原样返回最后一条 `gate.result`；`autome run`（`render.rs`）与 `autome status`（`main.rs`）按节点名渲染；
  桌面 `task.js::hostTags` 显示「<节点>通过 · sha」；Web 没有读门禁结果的地方。

## 目标与非目标

目标（全部来自任务卡）：

- 目标 1：`[gate] quick` / `quick_timeout_minutes` 的解析、缺省、校验；全局配置不认。→ 方案 §1，M-01。
- 目标 2：`before_merge` + `quick` 下需求实现轮后 `Gate` 跑快检、Bug FixGate 跑 `( 复现 ) && ( quick )`、MergeGate 不变。→ §2–§5，M-01–M-03。
- 目标 3：`each_round` 下 `quick` 解析通过、不生效。→ §1、§3，M-01、M-02。
- 目标 4：共用队列、熔断、126/127、日志位置、环境变量，另导出 `AUTOME_GATE_KIND`。→ §3、§6，M-02、M-03。
- 目标 5：事件、`autome run` / `status`、`task.get.gate`、审计简报与复核 prompt 分清快检与完整门禁。→ §3、§8、§10，M-02–M-05。
- 目标 6：prompt 在配了 `quick` 时说明快检；协议改动发新版本。→ §8、§9，M-04。
- 目标 7：README 与技术设计 §8.1 写两级门禁的取舍。→ §11，M-05。
- 目标 8：不改本仓库 `.autome/config.toml`；实测四段快检命令的热缓存耗时写进证据。→ §12，M-05。

验收 1–6 的落点：验收 1 → M-01；验收 2 → M-02；验收 3 → M-03；验收 4 → M-01（解析、保存）+ M-02（`each_round` + `quick` 不跑快检）；
验收 5 → M-04（金样）+ M-05（渲染单测）；验收 6 → 每个改了 `.rs` 的实现轮在证据里记 `cargo fmt --all -- --check` 与
`cargo clippy --all-targets -- -D warnings` 的退出码（改了 `apps/web` 再加 lint / typecheck；本任务不改 `apps/web`），`sh scripts/ci-local.sh`
由宿主在合并前跑。

非目标（任务卡「不做什么」与任务文件 §4 调研补充的边界）：不改 `each_round` 下任何行为；不从完整门禁里拆快检、不解析
`ci-local.sh`；不按改动文件选命令；不做跨任务合并队列；不改 MergeGate / MainCheck 跑什么命令；不改排队、MainCheck 沿用、
同因熔断的规则本身（只加「快检结果不得顶替」与「种类不同不连成一串」两条限制）；不新增 JSON-RPC 方法、不改参数、不改账本
schema；不改 `.autome/` 下任何内容、不改任务目录里的协议副本、不动 `kernel-contract` 区域；不顺手重构。

## 方案

### 1. 配置（目标 1、3；约束 2、3、12）

`autome-domain/src/config.rs`：

```rust
pub const DEFAULT_GATE_QUICK_TIMEOUT_MINUTES: u32 = 5;

/// `[gate] quick`：before_merge 下实现轮 / 修复轮之后由宿主跑的快检。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickGate { pub command: String, pub timeout_minutes: u32 }

// ProjectConfig 与 ResolvedConfig 各加一个旁挂字段（沿用 gate_run 的做法）：
#[serde(default, skip_serializing_if = "Option::is_none")]
pub gate_quick: Option<QuickGate>,

impl ResolvedConfig {
    /// 有门禁、before_merge、配了 quick 时才生效。
    pub fn quick_gate(&self) -> Option<&QuickGate>;
    /// 实现轮结束后要不要进 Gate：gates_each_round() || quick_gate().is_some()。
    pub fn gates_after_implement(&self) -> bool;
}
```

`resolve` 原样搬运 `project.gate_quick`（全局没有对应项）。没写 `quick` 时 `ResolvedConfig` 的 JSON 与今天逐字节相同，旧 JSON
反序列化得到 `None`（约束 12，样板 `before_merge_defaults_override_and_json_compatibility`）。`GateConfig` 不动。

`automed/src/config_io.rs::parse_project` 在读完 `run`、读 `command` 之前按下面顺序处理两个新键：

1. 有 `quick` 键：不是字符串 → `[gate] quick 必须是字符串`；取值去首尾空白。
2. 有 `quick_timeout_minutes` 键：表里没有 `quick` 键 → `[gate] quick_timeout_minutes 需要和 quick 一起配置`；否则必须是正整数
   （`as_integer().filter(|m| *m > 0)`，上限截到 `u32::MAX`，与 `timeout_minutes` 同一写法）→ `[gate] quick_timeout_minutes 必须是正整数`。
   只要写了就校验，不论 `quick` 是否为空。
3. `quick` 去空白后非空，而 `command` 缺失或去空白后为空 → `[gate] 配了 quick 就需要 command（完整门禁），例如 command = "make check"`。
   这一步放在现有 `ok_or("[gate] 需要 command…")` 之前，所以只写 `quick`（不论有没有 `concurrency`）报的都是这句。
4. `quick` 非空且有门禁 → `gate_quick = Some(QuickGate { command, timeout_minutes: 写了的值或 5 })`。

任务卡没说的边角，逐一写明：

| 写法 | 结果 |
|---|---|
| `command = "make check"`、`quick = "cargo fmt --check"` | 有快检，超时 5 分钟（`before_merge` 下才生效） |
| 只有 `quick`（没有 `command` 键，有无 `concurrency` 都一样） | 报第 3 条 |
| `command = ''`、`quick = "x"` | 报第 3 条（空 `command` 等于没有门禁，快检没有完整门禁可依附） |
| `command = ''`、`quick = ''` | 没有门禁、没有快检，不报错（同表其它键照旧校验） |
| `command` 非空、`quick = ''` 或只有空白 | 没有快检，不报错——与 `command = ''` 表示「没有」同一约定 |
| `quick = 1` / `true` / `["a"]` | 报第 1 条 |
| 只写 `quick_timeout_minutes` | 报第 2 条前半 |
| `quick_timeout_minutes = 0` / `-1` / `"5"` / `1.5`（含 `quick = ''` 时） | 报第 2 条后半 |
| `each_round` + `quick` | 解析通过，`config::validate` 的 `violations` 为空，不生效 |
| 全局配置的 `[gate]` 写这两个键 | 忽略，`load_global` 成功（`parse_global` 不新增读取） |

`save_project` 用 `toml_edit` 写回、不碰 `[gate]`，不改代码；用测试钉住「改角色保存后 `quick` / `quick_timeout_minutes` 连同注释
仍在、重载仍生效」。`each_round` 下的说明文字（任务卡「不强求」）不做，见 Backlog B-01。

### 2. 需求状态机（目标 2；约束 4、12）

`task.rs` 只改一处：`Node::Implement` 结束时 `if ctx.config.gates_each_round()` 改成 `if ctx.config.gates_after_implement()`。
`after_gate` 的 `(Gate, Passed)` / `(Gate, Failed)`（含 `impl_round >= budget` → `Fail(ImplBudget)`）、rebase 后进 MergeGate、
合并后进 MainCheck 都不动。没配 `quick` 时 `gates_after_implement() == gates_each_round()`，T-61 的 10 个 `before_merge` 单测原样通过。
Bug 状态机不改：`Node::Fix => core(Node::FixGate)` 保持，变的只是 FixGate 这一次跑什么命令（§4）。

### 3. 宿主运行：选命令、种类、超时与启动记录（目标 2、4、5；约束 6、7、8）

**种类。** 每次运行有一个种类 `kind`，取值三种：

| 运行 | 命令 | `kind` | 超时 |
|---|---|---|---|
| 需求 `Gate`，`each_round` | 完整门禁 | `full` | `timeout_minutes` |
| 需求 `Gate`，`before_merge` + `quick` | `quick` | `quick` | `quick_timeout_minutes` |
| 需求 `Gate`，`before_merge` 无 `quick` | 不跑，`gate.skipped`（T-61 原样） | — | — |
| MergeGate / MainCheck（需求与 Bug） | 完整门禁（Bug 的 MergeGate 是 `( 复现 ) && ( 完整 )`） | `full` | `timeout_minutes` |
| Bug FixGate，`each_round` | `( 复现 ) && ( 完整 )` | `full` | `timeout_minutes` |
| Bug FixGate，`before_merge` + `quick` | `( 复现 ) && ( quick )` | `quick` | `timeout_minutes`（理由见 §4） |
| Bug FixGate，`before_merge` 无 `quick`，或项目没有门禁 | 复现命令 | `repro` | `timeout_minutes`，没有门禁时 30 |
| Bug ReproCheck | 复现命令 | `repro` | 同上 |
| Bug MergeGate，项目没有门禁 | 复现命令 | `repro` | 30 |

任务卡只写了 `quick|full`。只跑复现命令的两种运行既不是快检也不是完整门禁，记成 `full` 会让读者把它当成完整门禁的证据，
记成 `quick` 不属实，所以加第三个值 `repro`。`AUTOME_GATE_KIND` 导出同一个值。

**落到事件。** `gate.started` 与 `gate.result` 各加一个 `kind` 字段（任务卡写的 `gate.finished` 在代码里不存在，按 `gate.result` 落实，
不新增事件种类，约束 6）。`gate.result.command` 的名字与含义不改。`gate.queued` 不带 `kind`（排队时还没决定跑什么，开跑那一刻
配置可能已变）；`gate.skipped` 不带（什么都没跑）；`gate.reused` 不带（它指向的那条 `gate.result` 自己带着 `kind`，且
`main_check` 的 `gate.reused` 载荷被现有 e2e 按整体相等断言）。

**启动记录（`kind` 来自启动时的事实）。** `gate.rs` 的 `Job` 加一个与 `.repro-only` 同样性质的落盘标记 `<节点>-<key>.kind`，
内容是一行 JSON：`{"kind":"quick","command":"…","timeout_minutes":5}`，`full` / `repro` 只写 `{"kind":"full"}` / `{"kind":"repro"}`。
`Job::set_launch(&Launch)` / `Job::launch() -> Option<Launch>`；`run_gate` 在**每一次** `gate::start` 之前写它（所有节点，含 MainCheck）；
`gate::start` / `Job::clear` 不删它（每次启动前覆盖），`discard_except` 的两张后缀清单加 `.kind`。写失败 → `无法记录门禁种类：…`，
与 `set_repro_only` 同样处理。

**收结果。** 先读 `job.launch()`：

- 标记是 `quick`：`poll` 用标记里的超时，`gate.result` 的 `command` 用标记里的命令，`kind` 记 `quick`——不论现在的配置是什么。
- 标记是 `full` / `repro`：`poll` 超时与 `command` **按今天的逻辑**取（当前配置算出的完整门禁命令、`repro-only` 时取复现命令），
  `kind` 取标记里的值。这里的「今天的逻辑」指**不经过快检选择的原函数**（需求取 `resolved.gate.command`；Bug 按节点与模式拼），所以
  `command` 永远不会被记成快检命令（不会出现 `kind: full, command: <quick>`）；唯一残留的边角是 Bug 的 FixGate 以 `each_round`
  启动、运行中切到 `before_merge`，今天的函数会把 `command` 记成复现命令——这是 T-61 的既有记账，约束 1 要求保持（Backlog B-03）。
  这样没配 `quick` 的项目连「运行中改了 `command` 文本」这种边角也与今天逐事件相同（约束 1）。
- 没有标记（升级前启动的作业）：按今天的逻辑取命令，`kind` 按旧规则推出：`ReproCheck` 或 `repro-only` 或 Bug 节点没拼项目门禁
  → `repro`，否则 `full`。

**`run_gate` 的改动点**（不做其它移动）：

1. 开头的跳过条件加 `&& resolved.quick_gate().is_none()`。
2. 选命令：需求 `Gate` 且 `quick_gate()` 有值 → 快检；Bug 的 `match` 加 `(Some(_), FixGate)` 且 `quick_gate()` 有值 → `( {repro} ) && ( {quick} )`；
   其余不变。选出的结构除命令与超时外带上 `kind`。
3. FixGate 的 `set_repro_only(...)` 条件改成「有门禁、`before_merge`、没有 `quick`」。
4. 启动前写 `.kind` 标记；`env` 加 `("AUTOME_GATE_KIND", kind)`；`gate.started` 载荷加 `kind`。
5. 收结果按上面的规则；`gate.result` 载荷加 `kind`。
6. 沿用与熔断见 §5、§6。

超时说明：日志末尾那句 `=== Autome：门禁运行超过 N 分钟，已终止 ===` **不改**——`kernel_timeout` 与失败指纹都靠它；面向人和会话
的文字按 `kind` 说「快检超时（运行超过 N 分钟，被内核终止）」（§8、§10），N 就是那次运行实际用的超时，说得通。

`gate_queue.rs` 的巡检对在跑作业调 `poll_as` 时，超时改为：作业的 `.kind` 标记是 `quick` 时用标记里的超时，否则照旧。这是共用机制
对快检的必要适配，不改排队规则。126 / 127 的现有处理只在 ReproCheck 一侧，快检退出 126 / 127 就是一次普通失败，按 `Gate` /
FixGate 的失败路径退回，带命令与日志尾部。

### 4. Bug Loop 的 FixGate（目标 2；约束 4、8）

`before_merge` + `quick` 下 FixGate 跑 `( 复现 ) && ( quick )`，`kind = quick`，不写 `.repro-only`，`gate.result.command` 就是这条组合命令
（取自 `.kind` 标记）。MergeGate 仍是 `( 复现 ) && ( 完整 )`，`kind = full`。

超时取 `[gate] timeout_minutes`（缺省 30），不取 `quick_timeout_minutes`。理由：复现命令是定位轮写的回归测试，可能不快；今天
FixGate 在三种模式下都用 `timeout_minutes`，换成 5 分钟会让一条正常的慢复现被杀掉，而且杀掉后读者会把它读成「快检超时」。
代价是快检本身卡死时 FixGate 要等到 `timeout_minutes`——与今天只跑复现命令的 FixGate 相同，可以接受。

### 5. 合并前的完整门禁不能被快检顶替（约束 5）

- **MergeGate 沿用**（`run_gate` 内）：条件加一条 `last["kind"] != "quick"`，需求与 Bug 一样。没有 `kind` 的旧结果照旧可沿用。
  这同时堵住 Bug 的边角：项目把 `quick` 写成与 `command` 相同的文本时，`( 复现 ) && ( quick )` 与 `( 复现 ) && ( 完整 )` 字面相等，
  只比命令挡不住。
- **MainCheck 沿用**（`main_check_reuse`）：解析出的那条结果 `kind == "quick"` → 不沿用。正常情况下第一条已经保证 `merge_gate`
  的 `gate.reused` 不会指向快检结果，但 `gate.reused` 按 `sha` 找回原结果时不限节点，这里再挡一次，用决策表测试钉住。

### 6. 同因熔断与作业文件（约束 7）

- **熔断仍按节点计数**，快检失败计入它所在的节点（需求 `gate`、Bug `fix_gate`）。新增一条，且**只涉及 `quick`**：
  `record_gate_result` 找到的上一条结果与本次都带 `kind`、两者不同、并且其中至少一个是 `quick` → 视为没有上一条（连续计数从 1
  开始）。`full ↔ repro` 的切换**不隔断**，任一方没有 `kind`（升级前的结果）也**不隔断**，都按今天的规则累计。
  理由：模式切换前后，同一节点上一次是快检、一次是完整门禁（或复现命令），它们跑的不是同一条命令，指纹碰巧相同也不能叫「同因」；
  而 `full ↔ repro`（Bug 的 FixGate 在 `each_round` 与 `before_merge` 间切换，复现命令先失败时两种运行的日志与退出码相同）是没配
  `quick` 的项目也会遇到的路径，约束 1 要求熔断计数与今天逐项相同，所以不动（评审 R1-01）。这条只会让涉及 `quick` 的切换后熔断更晚
  触发，不会更早。正常运行中同一节点只有一种 `kind`，不受影响。`kind` 由 `run_gate` 在组装结果时写入，
  `record_gate_result` 只读不加字段——现有熔断单测（`same_failure_*`）直接传入不带 `kind` 的载荷并断言返回值与传入相等，保持原样通过。
- **作业文件名不变**（`<节点>-<head 前 12 位>.*`），快检与完整门禁在同一节点、同一 head 上共用一个作业槽。一个槽同一时刻只有
  一次运行；收结果按 `.kind` 标记认它是哪一种，所以不会把另一种的在途作业当成自己的；`gate::start` 启动前清掉旧 `.log`，
  `discard_stale` 清掉别的 head，旧日志不会被误读。模式切换的具体走向见 §7。

### 7. 运行中改配置（约束 11）

配置每个 tick 重新解析。下表是任务停在 `Gate` / FixGate 时的走向；原则是「已经启动的作业按启动时的种类收结果；还没启动的按
当前配置决定」。

| 情形 | 走向 |
|---|---|
| `Gate` 上快检在跑 / 排队 / 已暂停 / 已停止，拿掉 `quick`（仍 `before_merge`） | 走 T-61 的跳过分支：`discard_node` 杀掉进程组、写 `gate.skipped{node: gate, reason: 门禁只在合并前运行}`，按通过继续 |
| `Gate` 上快检在跑，改成 `each_round` | 按 `.kind` 收成 `kind: quick` 的结果，`command` 是快检命令；通过 → 审计（简报写快检，末句按当前配置说「已改为每轮运行完整门禁，这条结果是切换前的快检」，§8），之后 MergeGate 照跑完整门禁（快检结果不可沿用） |
| `Gate` 上快检在跑，改了 `quick` 文本或超时 | 按标记里的命令与超时收结果；下一轮用新值 |
| `Gate` 上完整门禁在跑（`each_round`），改成 `before_merge` + `quick` | 不再跳过；按 `.kind` 收成 `kind: full` 的结果，`command` 按今天逻辑；下一轮起跑快检 |
| `Gate` 上排队中（作业未启动），任意切换 | 开跑那一刻按当前配置选种类；`before_merge` 无 `quick` 时走跳过分支 |
| FixGate 上 `( 复现 ) && ( quick )` 在跑，拿掉 `quick` 或改成 `each_round` | 按 `.kind` 收成 `kind: quick`、`command` 为组合命令；复核的说明按该结果的 `kind` 生成（§8：`before_merge` 段填快检说明，`each_round` 段追加更正句） |
| FixGate 上 `( 复现 ) && ( 完整 )` 在跑（`each_round`），改成 `before_merge` + `quick` | 按 `.kind` 收成 `kind: full`；复核走 `before_merge` 段，说明写「切换前已跑过复现命令和完整门禁」而不是「只跑复现命令」（§8） |
| FixGate 上只跑复现命令（`repro-only`），加上 `quick` | 按 `.repro-only` 与 `.kind` 收成 `kind: repro`、`command` 为复现命令（T-61 测试照旧） |
| head 在作业期间变了 | 现有逻辑：作废、下一 tick 按当前配置重跑 |
| 任务停在审计等别的节点时加上 `quick` | 下一次实现轮结束时生效 |

不卡死：每一行都会在有限个 tick 内得到一个结果或一次跳过；不报协议失败：这里没有一条路径读写设计文档；`kind` 不记错：
来自启动记录；不漏跑合并前的完整门禁：快检结果在 §5 两处都不可沿用。

### 8. 会话看到的文字（目标 5、6；约束 9、10）

`launcher::PromptSpec` 加两个字段：`gate_quick: Option<&'a str>`（`resolved.quick_gate()` 的命令）与
`last_gate: Option<&'a serde_json::Value>`（本任务最后一条 `gate.result`，调度器启动会话时取）。`gate_line(cmd)` 本身不改。

**`{gate_line}` 的快检句。** 只在 `gate_quick` 有值时、只对两种会在结束后触发快检的会话，在 `gate_line(cmd)` 后面接一句：

- 实现轮（`Role::Impl`）：`宿主会在本轮结束后跑快检 \`<quick>\`，失败会退回。`
- 修复轮（`BugStep::Fix`）：`宿主会在本轮结束后跑复现命令和快检 \`<quick>\`，失败会退回。`

设计、审计、复核的 `{gate_line}` 不变。没配 `quick` 时所有 prompt 逐字节不变。这句由宿主拼在 `{gate_line}` 的位置上，所以
固定在 v3 / v4 的旧任务也能读到，不需要新占位符。

**审计简报的「宿主门禁」段。** `brief::gate_line_of_result` 按结果的 `kind` 分两种文字；`kind` 是 `full` / `repro` / 没有时逐字节不变。
`kind == "quick"` 时：

> 本轮宿主只跑了快检：\`<命令>\`，在提交 \`<sha 前 10 位>\` 上<通过 | 未通过 | 快检超时（运行超过 N 分钟，被内核终止）>，退出码 N。完整输出：\`<日志>\`。
> 快检不是全量门禁：它只说明这条命令里的检查是否通过，不能当作全量回归的证据；全量门禁只在合并前运行。复验时可以引用它，但它不替代你自己的负向对照。

最后一句里的「全量门禁只在合并前运行」是对**当前配置**的陈述，只在当前是 `before_merge` 时成立。当前配置已是 `each_round`（快检在途时才切换）时，
这一分句换成 `当前配置已改为每轮运行完整门禁，这条结果是切换前的快检。`，其余文字不变（评审 R1-02 第 3 条路径）。

`gate_evidence` 的分支不变（T-61 规则保留：`before_merge` 下结果的 `sha` 不等于本轮 head 不引用），只有「`before_merge` 且没有
当前 head 的结果」那句在快检生效时换成 `本项目全量门禁只在合并前运行；本轮的提交上还没有宿主快检结果。`，未生效时仍是
`GATE_ONLY_BEFORE_MERGE`。各情形：

| 情形 | 「宿主门禁」段 |
|---|---|
| `before_merge` + `quick`，最后一条是本轮 head 上的快检（常态，含 MergeGate 失败退回后修复轮之后那一轮） | 快检文字 |
| `before_merge` + `quick`，最后一条不在本轮 head 上 | `本项目全量门禁只在合并前运行；本轮的提交上还没有宿主快检结果。` |
| `before_merge` 无 `quick`，最后一条是本轮 head 上的完整门禁（MergeGate 失败后修复轮没提交） | T-61 原文 |
| `before_merge` 无 `quick`，最后一条是本轮 head 上的快检（快检跑完后才拿掉 `quick`） | 快检文字 |
| `each_round` | 最后一条按 `kind` 渲染；完整门禁时与今天相同 |

**失败注入（`Inject::GateFailed`）。** `Inject` 与 `GateOutcome` 不加字段（23 处构造），launcher 看 `last_gate` 的 `kind`——注入
GateFailed 的那次转换紧跟在那条 `gate.result` 之后，会话启动（含崩溃重试）前不会有别的 `gate.result` 插进来。

- 需求，`kind == "quick"`：
  > Autome 在提交 \`<sha>\` 上运行了快检 \`<命令>\`，<没有通过 | 快检超时（运行超过 N 分钟，被内核终止），不是检查失败的结论。请先看日志里卡在哪一步>。输出末尾（原文）：<tail 代码块>
  > 本轮先从输出里找出失败的检查，修好它，用同一条快检命令确认通过后提交，然后照常结束会话。<`before_merge` 时：gate_line + 快检句>本轮是修复轮，不推进新的里程碑：已是 \`待审\` 的里程碑保持 \`待审\`，implementation-round 照常加一，照常写本轮的证据文件。<结语>

  结语：快检生效 → `快检结果以 Autome 在你提交后重跑为准，不以你在证据里写的结论为准。`；`before_merge` 但已拿掉 `quick` → T-61 原句
  `门禁结果以 Autome 在合并前重跑为准，不以你在证据里写的结论为准。`；`each_round` → 今天 `each_round` 那句。
- 需求，`kind` 不是 `quick`（MergeGate 失败、`each_round` 的 Gate 失败）：今天的文字。唯一差别是快检生效时 `before_merge` 分支里
  的 `gate_line` 后接快检句、结语换成 `快检结果以 Autome 在你提交后重跑为准；全量门禁结果以 Autome 在合并前重跑为准，不以你在证据里写的结论为准。`
  没配 `quick` 时逐字节不变。
- Bug，`kind == "quick"`（FixGate 快检失败）：
  > Autome 在提交 \`<head>\` 上运行修复门禁（复现命令 + 快检）\`<命令>\`，<判定>。输出末尾（原文）：<tail 代码块>
  > 修复后用复现命令和快检命令确认通过并提交；<gate_line + 修复轮快检句>不要修改回归测试。
- Bug，其它 `kind`：今天的文字；快检生效时 `gate_line` 后接修复轮快检句。

**Bug 复核。** 协议 v5 把 `fix-review.md` `before_merge` 段写死的那句换成占位符 `{fix_gate_note}`（§9），宿主填：

说明按「最后一次 FixGate 实际跑了什么」（`last_gate` 的 `kind` / `command` / `exit_code`）决定，不按当前配置；当前配置只决定它落在哪里
（`before_merge` 段的占位符，或下面的补充句）：

- `kind == "quick"` →
  `本项目全量门禁只在合并前运行，本轮没有全量门禁结果；修复门禁跑的是复现命令和快检（\`<该结果的 command>\`，退出码 N），它通过不等于项目门禁通过。`
- `kind == "full"` 且快检当前生效（走 `before_merge` 段，即启动时是 `each_round`、运行中才切换的路径）→
  `本项目全量门禁只在合并前运行；本轮修复门禁在切换配置前已跑过复现命令和完整门禁（\`<该结果的 command>\`，退出码 N），不是只跑复现命令。`
- 其它（没有 `last_gate`、`repro`、或没配 `quick` 的 `full`）→ `BUG_REVIEW_GATE_NOTE` 原文（金样 `fix-review-before-merge.md` 逐字节不变；
  没配 `quick` 的项目模式切换时的既有说法也不动，约束 1）。

**当前是 `each_round`、最后一次 FixGate 却是快检**（快检在途时切到 `each_round`）：`each_round` 段没有占位符，渲染末尾追加一次
`更正：本轮修复门禁跑的是复现命令和快检（\`<命令>\`，退出码 N），不是完整门禁；它通过不等于项目门禁通过。`。这和下面旧模板的补充句同属一个
机制：**最后一次 FixGate 是快检、而渲染出的正文里没有 `{fix_gate_note}` 的位置**（`each_round` 段、v3 / v4 的 `before_merge` 段、没有
`before_merge` 段的旧模板）时，末尾追加且只追加一次。没配 `quick`、也没有快检结果的项目不触发，渲染逐字节不变。

**旧模板（约束 10）。** 固定在旧协议版本的任务遇到 `quick`：

- `impl.md` / `fix.md`（v3、v4 有 `{gate_line}`）：快检句随 `{gate_line}` 进来。v3 之前没有 `before_merge` 段的模板读不到快检句，
  快检照跑，失败时注入会说清楚。
- `fix-review.md` v3 / v4（有 `before_merge` 段、没有 `{fix_gate_note}`）且最后一次 FixGate 是快检时：渲染末尾追加一次
  `更正：上面「修复门禁只跑了复现命令」不属实——修复门禁跑的是复现命令和快检（\`<命令>\`，退出码 N），它通过仍不等于项目门禁通过。`
  最后一次 FixGate 是 `full` 而快检当前生效时，更正句改为 `更正：上面「修复门禁只跑了复现命令」不属实——本轮修复门禁在切换配置前已跑过复现命令和完整门禁（\`<命令>\`，退出码 N）。`
- 没有 `before_merge` 段的旧 `fix-review.md`：现有 `legacy_review` 追加 `BUG_REVIEW_GATE_NOTE`，应填快检说明时改为追加快检说明。
- 任何情形都不渲染失败、不留未填占位符。

**命令是数据。** `{fix_gate_note}` 与 `{gate_line}` 一样先切开模板再拼接数据（`render_gate_parts` 推广到两个占位符），`quick` 里的
`{slug}` 原样出现；快检句与注入里的命令同样不经过 `.replace`。

### 9. 协议 v5（目标 6；约束 10）

改 `protocol/`（源）下三处，在 `CHANGELOG.md` 末尾加 `## protocol/v5`，两条改动各一个块：

- `prompts/fix-review.md` 的 `before_merge` 段：`本项目门禁只在合并前运行，本轮没有宿主门禁结果；修复门禁只跑了复现命令，它通过不等于项目门禁通过。`
  → `{fix_gate_note}`。没配 `quick` 时渲染结果逐字节不变。
- `bug-protocol.md` 第 40 行那句改为：`项目 \`[gate] run = "before_merge"\` 时修复门禁只跑复现命令，同时配了 \`[gate] quick\` 时跑复现命令和快检；项目完整门禁留到合并前门禁。`
- 两个块都是 `kind: clarify`（没配 `quick` 时行为与文字不变，配了时让文字与事实一致），`evidence` 写 T-61（L-07）与 T-56 / T-65，
  `predicted_impact` 写 `{metric: total_turns, direction: down, scope: task, horizon: 3}`，`eval: null`。若 `protocol eval` 判它们必须是
  `behavioral`，实现轮按要求补「改前红、改后绿」的 `evals/` 用例并在证据里说明。

代码侧配套：`protocol/eval.rs` 的 `KNOWN_PLACEHOLDERS` 加 `fix_gate_note`，并像 `gate_line` 一样只允许出现在 `before_merge` 段内；
`protocol/eval_run.rs` 渲染行为用例时在 `before_merge` 下用 `BUG_REVIEW_GATE_NOTE` 填它。`cargo run -p automed -- protocol eval protocol/`
必须通过。`kernel-contract` 区域、任务目录里的 v4 副本都不动。

### 10. 人读的文字与 `task.get.gate` 的读者（目标 5；约束 9、15）

| 读者 | 快检结果下显示 | 改不改 |
|---|---|---|
| `autome run`（`render.rs` 的 `gate.started` / `gate.result`） | `<任务> 快检开始：<命令> @ <sha>`；`<任务> 快检通过（退出码 0）  日志：…`；`快检未通过（退出码 N）`；超时 `快检超时（运行超过 N 分钟，被内核终止）（退出码 124）`。FixGate 快检的标签是 `修复门禁（复现 + 快检）` | 改；`kind` 不是 `quick` 的行逐字节不变 |
| `autome status <任务>`（`main.rs` 的门禁行） | `门禁      快检 在 <sha> 上通过（<命令>）  日志 …`，超时写 `超时（运行超过 N 分钟，被内核终止）` | 改；把这一行抽成 `render::gate_status_line(&Value) -> String` 以便单测，`main.rs` 调它 |
| `task.get.gate`（`dispatch.rs`） | 原样返回最后一条 `gate.result`，现在多一个 `kind` | 不改代码 |
| 桌面 `task.js::hostTags` | `快检通过 · <sha>` / `快检未通过 · <sha>`；FixGate 快检为 `修复门禁（复现 + 快检）通过 · <sha>` | 改：标签文字移到 `renderer/lib/labels.js` 的纯函数 `gateTagLabel(gate)`，`hostTags` 调它 |
| 桌面 `routing.js` | 用 `resolved.gate.command` 画固定门禁节点，不读结果 | 不改（Backlog B-02） |
| Web `apps/web` | 只有节点名映射，没有读门禁结果的地方 | 不改 |
| `autome run --json` / `logs --json`（T-46，在 main 上） | 事件载荷原样输出，`gate.started` / `gate.result` 多一个 `kind` 键，纯增加 | 不改；rebase 后跑 `cargo test -p autome-cli` 确认没有按整份载荷相等比较 |
| `gate.queued` / `gate.reused` / `gate.skipped` 的行 | 不带 `kind`，与今天相同 | 不改 |

### 11. 文档（目标 7；约束 13）

- `README.md`「Host gate, retries and cooldowns」：`[gate]` 示例加 `quick = "…"` 与 `quick_timeout_minutes = 5` 两行（带注释）；新增
  一段 **Two-level gate**：快检在 `before_merge` 下每个实现轮 / 修复轮之后跑、完整门禁在合并前跑；推荐快检只放格式 / lint / 类型
  检查并控制在 1 分钟内，测试留给完整门禁；`each_round` 下 `quick` 不生效；完整门禁是否包含快检的内容由项目自己决定，Autome
  不拼接；`AUTOME_GATE_KIND`（`quick` / `full` / `repro`）；快检结果永不顶替合并前门禁。原句 "Bug fix rounds run only the repro command;"
  改为写明配了 `quick` 时修复门禁跑复现命令和快检。
- `docs/development/technical-design.md` §8.1：「合并前门禁模式」段里「需求实现轮跳过 Gate」「Bug 修复轮只跑复现命令，FixGate 节点保留」
  两处改准；新增「两级门禁。」一段，内容同上（中文）；第 256 行「合并前门禁总带复现命令」一段补 FixGate 在快检下的命令。

### 12. 本仓库快检命令实测（目标 8）

M-05 的实现轮在本 worktree 跑任务卡给的四段命令：先完整跑一遍预热，再用 `/usr/bin/time -p` 分别量四段和整条命令的热缓存耗时，
把总耗时与各段耗时写进该轮证据。`apps/web` 依赖没装时先 `npm ci --prefix apps/web`（不改受版本控制的文件）；装不上或沙箱拒绝
就照实写「未取证」与原因，不阻塞里程碑。不改 `.autome/config.toml`。

### 13. 现有测试与现有代码的改动清单（约束 1）

- `crates/automed/tests/prompt_rendering.rs` 的 `PromptSpec` 构造函数、`crates/automed/src/launcher.rs` 测试模块的 `PromptSpec` 构造函数：
  各加 `gate_quick: None, last_gate: None` 两行。理由是结构体多了字段导致编译不过；不改任何断言。
- `scheduler.rs` 的 `start_onboarding` 里的 `PromptSpec`（产品代码）同样补两个 `None`。
- 其余现有单测、金样、e2e 一行不改。给 `gate.started` / `gate.result` 加 `kind` 是任务卡要求的可见变化，已核对：现有测试对这两种事件
  只断言单个字段或计数，没有按整份载荷相等比较的（按整份比较的是 `main_check` 的 `gate.reused`，它不加 `kind`）；读者见 §10。
- 若实现中发现某条现有测试必须改，实现轮在证据里逐条列出并说明理由，理由不能是「新行为与旧断言不符」。

### 14. 与任务卡字面的出入

- `gate.finished` 不存在，`kind` 加在 `gate.started` 与 `gate.result` 上。
- `kind` / `AUTOME_GATE_KIND` 有第三个值 `repro`（只跑复现命令的运行）。
- Bug FixGate 快检的超时取 `timeout_minutes` 而不是 `quick_timeout_minutes`（§4）。
- `quick = ''` 视为没有快检而不是报错（与 `command = ''` 同一约定）。

### 15. 否决的做法

- 往 `GateConfig` 加字段：9 处字面量要改，且 `GateConfig` 会同时被 Bug 现场拼装的路径使用，语义混乱。
- 给 `GateOutcome::Failed` / `Inject::GateFailed` 加 `kind`：23 处构造，其中大半在现有测试里。
- 收结果时从 `gate.started` 事件读 `kind`：崩溃窗口里这条事件可能缺失或过期（T-61 为此给 FixGate 用了落盘标记，测试
  `before_merge_fix_gate_crash_window_ignores_missing_and_stale_start_events`）。
- 给快检单设一个节点：要改两台状态机，违反约束 4。
- 在 `impl.md` / `fix.md` 加新占位符说快检：旧版本任务读不到；宿主在 `{gate_line}` 位置接句子对 v3 / v4 都有效。
- 把超时日志那句改成「快检运行超过」：`kernel_timeout` 与失败指纹都解析那一句，改了会让超时识别与熔断变化。

## 风险与验证安排

- **沿用漏洞（约束 5）是最大的正确性风险。** e2e 里审计、复盘轮会提交，head 通常会动，自然路径很难走到「快检通过后 head 不变就到
  MergeGate」。所以用调度器单测直接构造这个状态（M-02、M-03 各一条），e2e 再从计数上兜底（完整门禁脚本恰好 1 次）。
- **启动记录与崩溃窗口。** 标记在 `gate::start` 之前写；写完标记、启动前崩溃 → 没有 `.started`，下一 tick 重写标记重跑；启动后、
  写 `gate.started` 前崩溃 → 收结果靠标记，与 `gate.started` 无关。M-02 的标记单测覆盖「survive start/clear、discard 删除」。
- **计数不靠时序。** e2e 的快检脚本与完整门禁脚本是两个不同的脚本，各自把 `$AUTOME_GATE_NODE $AUTOME_GATE_KIND` 追加进自己的计数文件；
  完整门禁的计数照 T-61 的办法区分 `merge_gate` 与 `main_check`，不关 MainCheck 凑数（`reuse_for_main_check = false`）。
- **与 T-46 rebase 相遇。** `render.rs`、`main.rs`、`dispatch.rs` 会冲突；改动只限必要的行。rebase 后以 main 上的代码为准，并跑
  `cargo test -p autome-cli` 确认 `--json` 输出没有按整份载荷比较门禁事件。
- **沙箱。** 桌面端只新增纯 `node --test` 的标签测试（不起 Electron）；Web 不改。`ci-local.sh` 里在会话沙箱跑不通的步骤（T-61 记录过
  Desktop SIGABRT、Web listen EPERM）由宿主合并前门禁承担，实现轮照实记录，不写成「由宿主门禁负责」。
- **队列占位。** 快检占 `[gate] concurrency` 的名额，`parallel = 3`、`concurrency = 1` 时快检可能排在别的任务的完整门禁后面。
  这是任务卡要求的共用，README 里提一句；不改排队规则。
- **验收 6 的自查。** 每个改了 `.rs` 的实现轮在证据里写 `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings` 的退出码；
  M-05 改了 `apps/desktop` 的 JS，但它不在 lint / typecheck 范围内，不需要另跑。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | quick 配置解析与需求状态机：实现轮结束进 Gate | 0 | |
| M-02 | 开放 | 宿主在需求 Gate 上跑快检：种类、启动记录、沿用与熔断、e2e | 0 | |
| M-03 | 开放 | Bug FixGate 跑复现命令加快检，结果不顶替 MergeGate | 0 | |
| M-04 | 开放 | 会话文字与协议 v5：简报、快检句、失败注入、复核说明、金样 | 0 | |
| M-05 | 开放 | autome run/status 与桌面标签、README 与技术设计、快检耗时实测 | 0 | |

## 里程碑说明

最新证据：docs/每轮快检-gate-quick-before/evidence/M-01-r1-impl.md · 待审 · 实现 #1

- **M-01**（覆盖：目标 1、3；验收 1、4（解析与保存）；约束 2、3、4、12。前置：无）。方案 §1、§2。新增用例：
  - autome-domain 6 个（名字以 `gate_quick_` 开头）：`gate_quick_implement_with_audit_enters_gate`（转换与 `each_round` 下逐字段相等）、
    `gate_quick_implement_without_audit_enters_gate`、`gate_quick_gate_pass_and_fail_equal_each_round`（通过、失败、预算耗尽三种）、
    `gate_quick_inactive_under_each_round_or_without_gate`、`gate_quick_resolved_json_is_unchanged_without_quick`（含旧 JSON 反序列化与往返）、
    `gate_quick_merge_and_main_check_nodes_unchanged`。
  - automed lib `config_io` 8 个（`gate_quick_` 开头）：默认与显式超时、只有 `quick` 报错（有无 `concurrency`、`command = ''`）、超时越界、
    非字符串与空白、只有超时、全局忽略、`save_project` 保留文本与注释并重载生效、`each_round` + `quick` 的 `violations` 为空。
- **M-02**（覆盖：目标 2、3、4、5（事件）；验收 2、4（`each_round` 不跑快检）；约束 1、5、6、7、8、11。前置：M-01）。方案 §3、§5、§6、§7
  的需求部分，以及 `gate.rs` 标记、`gate_queue.rs` 巡检超时。新增用例：
  - automed lib 9 个（`gate_quick_host_` 开头）：`.kind` 标记 survive start/clear、discard 删除；`Gate` 跑快检（命令、`gate.started` /
    `gate.result` 的 `kind`、脚本读到的 `AUTOME_GATE_KIND=quick`、标记超时 = `quick_timeout_minutes`）；完整门禁的运行带 `kind: full`
    且无标记的旧作业收成 `full`；MergeGate 不沿用同 head 上通过的快检（对照：完整结果仍沿用）；`main_check_reuse` 不沿用快检结果；
    熔断不跨 `quick` 与别的种类（对照：同种类照常累加、旧结果无 `kind` 照常累加；**没配 `quick` 的对照**：Bug `fix_gate` 上 `full` 失败两次后
    切到 `repro` 再以相同指纹失败，第三次仍熔断，`repro → full` 同理，约束 1）；模式切换按启动种类收结果（§7 第 2、3、4 行）；在跑时拿掉
    `quick` 走跳过分支；巡检按标记超时终止快检。
  - end_to_end 3 个：`gate_quick_feature_two_milestones_run_quick_each_round_and_full_once`（到 `await_merge` 时快检计数文件两行 `gate quick`、
    完整门禁计数文件一行 `merge_gate full`；合并后完整门禁计数为 `merge_gate`、`main_check` 两行；`gate.started` 的 `kind` 依次为
    `quick, quick, full`）、`gate_quick_feature_failing_quick_returns_to_implement_with_its_tail`（一个里程碑；快检第一次失败并打印一个标记串
    → 回 `implement`，该轮 prompt 含标记串与快检命令（只断言这两样，措辞归 M-04）→ 再次快检通过 → 合并前完整门禁 1 次；快检计数 2）、
    `gate_quick_each_round_ignores_quick_event_for_event`（同一剧本跑两遍，一遍多写 `quick`：快检脚本 0 次，门禁事件的
    `(事件种类, node, kind)` 序列两遍相同）。
- **M-03**（覆盖：目标 2（Bug）、4；验收 3；约束 4、5、6、8、11。前置：M-02）。方案 §3 的 Bug 部分、§4、§5 的 Bug 边角。新增用例：
  - automed lib 4 个（`gate_quick_bug_` 开头）：FixGate 跑 `( 复现 ) && ( quick )`、`kind: quick`、没有 `.repro-only`、标记超时 =
    `timeout_minutes`；FixGate 的快检结果在 `quick` 与 `command` 文本相同时也不被 MergeGate 沿用；FixGate 中途切换按启动种类收结果
    （§7 第 6、7 行）；ReproCheck 与只跑复现的 FixGate 是 `kind: repro`、导出 `AUTOME_GATE_KIND=repro`。
  - end_to_end 2 个：`gate_quick_bug_fix_gate_runs_repro_and_quick_and_merge_gate_runs_repro_and_full`（FixGate 的 `gate.started.command`
    含复现命令与快检脚本、不含完整门禁脚本，`kind: quick`；MergeGate 的含复现命令与完整门禁脚本，`kind: full`；快检脚本记录 1 次
    `fix_gate quick`）、`gate_quick_bug_failing_quick_returns_to_fix_with_its_tail`。
- **M-04**（覆盖：目标 5（简报、复核）、6；验收 5（金样）；约束 9、10。前置：M-03）。方案 §8、§9、§13 的 `PromptSpec` 补字段。新增用例：
  - automed lib 10 个：`gate_quick_prompt_` 开头 8 个（实现 / 修复轮快检句且其它角色不变、需求快检失败注入、Bug 快检失败注入、
    复核说明随最后一次 FixGate（含退出码；含两条在途切换路径：`quick` 启动后切 `each_round` 得到更正句恰好一次，`each_round` 的 `full` 启动后
    切 `before_merge` + `quick` 得到「切换前已跑过复现命令和完整门禁」而非「只跑了复现命令」；没配 `quick` 时渲染逐字节不变）、v3/v4 旧模板渲染无占位符且更正句只出现一次、命令里的 `{slug}` 原样、`eval_run` 填 `{fix_gate_note}`、
    `eval` 只允许 `{fix_gate_note}` 在 `before_merge` 段内）；`gate_quick_brief_` 开头 2 个（快检文字含「本轮宿主只跑了快检」与退出码、
    不含「这是实现轮之外的一次独立运行」，超时变体，当前 `each_round` 时末句换成「已改为每轮运行完整门禁」；`full` / 无 `kind` 结果与今天逐字节相同、快检生效且无当前结果时的新常量）。
  - prompt_rendering 3 个（`gate_quick_` 开头）与金样 4 份：`golden/prompts/impl-before-merge-quick.md`、`fix-before-merge-quick.md`、
    `fix-review-before-merge-quick.md`、`golden/briefs/audit-before-merge-quick.md`。现有金样一份不改。
  - 协议 v5：§9 三处文本、`CHANGELOG.md` 两个块。
- **M-05**（覆盖：目标 5（人读）、7、8；验收 5（渲染单测）；约束 9、13、15。前置：M-02、M-03）。方案 §10、§11、§12。新增用例：
  autome-cli 2 个（`render::tests::gate_quick_run_lines_name_the_quick_run`、`render::tests::gate_quick_status_line_names_the_kind`，
  均含「`kind` 不是 `quick` 时与今天逐字节相同」的对照）；桌面 `apps/desktop/test/labels-gate-kind.test.js` 3 个（需求快检、FixGate 快检、
  完整 / 无 `kind` 不变）。证据另记 §12 的实测耗时。

## 验收契约

### M-01 验收契约

- 验收命令：`cargo test -p autome-domain -- gate_quick before_merge` 与 `cargo test -p automed --lib -- config_io::tests::gate_quick config_io::tests::before_merge`
- 预期：第一条 16 个用例通过（新增 6 + T-61 现有 `before_merge` 10）；第二条 13 个通过（新增 8 + 现有 `config_io` 的 `before_merge` 5）；现有用例一行不改。
- 负向对照：实现轮结束仍只判 `gates_each_round()` 时，`gate_quick_implement_with_audit_enters_gate` 与 `gate_quick_implement_without_audit_enters_gate` 失败（得到 `Audit` / 下一步而不是 `Gate`）；第 3 条校验漏掉「有 `concurrency` 无 `command`」时，`config_io` 的「只有 quick 报错」用例失败（解析成功且没有门禁）。

### M-02 验收契约

- 验收命令：`cargo test -p automed --lib -- gate_quick_host before_merge_fix_gate before_merge_unconfigured before_merge_repro_marker main_check_reuse` 与 `cargo test -p automed --test end_to_end -- gate_quick_feature gate_quick_each_round feature_before_merge`
- 预期：第一条 18 个通过（新增 9 + 现有 9：`before_merge_fix_gate` 3、`before_merge_unconfigured` 1、`before_merge_repro_marker` 1、`main_check_reuse` 4）；第二条 7 个通过（新增 3 + T-61 的 `feature_before_merge` 4）。
- 负向对照：MergeGate 沿用不看 `kind` 时，`gate_quick_host_merge_gate_never_reuses_a_quick_result` 失败（出现 `gate.reused`、没有 `merge_gate` 的 `gate.started`）；收结果按当前配置而不按 `.kind` 标记时，`gate_quick_host_mode_switch_collects_by_launch_kind` 失败（`kind` / `command` 记成切换后的）；开头的跳过条件没加 `quick_gate().is_none()` 时，两个 `gate_quick_feature_` e2e 的快检计数为 0 而失败。

### M-03 验收契约

- 验收命令：`cargo test -p automed --lib -- gate_quick_bug before_merge_fix_gate before_merge_unconfigured` 与 `cargo test -p automed --test end_to_end -- gate_quick_bug bug_loop_before_merge`
- 预期：第一条 8 个通过（新增 4 + 现有 4）；第二条 4 个通过（新增 2 + T-61 的 `bug_loop_before_merge` 2）。
- 负向对照：FixGate 在 `before_merge` + `quick` 下仍只跑复现命令时，`gate_quick_bug_fix_gate_runs_repro_and_quick_under_the_full_timeout` 与对应 e2e 失败（命令不含 `&& (`、快检计数 0）；沿用只比命令时，`gate_quick_bug_fix_gate_result_never_stands_in_for_the_merge_gate` 失败（`quick` 与 `command` 同文本时出现 `gate.reused`）。

### M-04 验收契约

- 验收命令：`cargo test -p automed --lib -- gate_quick_prompt gate_quick_brief before_merge_prompt brief::tests::before_merge`、`cargo test -p automed --test prompt_rendering`、`cargo run -p automed -- protocol eval protocol/`
- 预期：第一条 25 个通过（新增 10 + 现有 `before_merge_prompt` 12 + `brief::tests::before_merge` 3）；第二条 31 个通过（现有 28，含全部旧金样逐字节不变 + 新增 3）；第三条退出码 0。
- 负向对照：`gate_line_of_result` 不看 `kind` 时，`gate_quick_brief_` 的快检用例与 `audit-before-merge-quick.md` 金样失败；`{fix_gate_note}` 固定填 `BUG_REVIEW_GATE_NOTE` 时，复核说明用例与 `fix-review-before-merge-quick.md` 金样失败；用 `.replace("{fix_gate_note}", …)` 在占位符检查之前塞数据时，`quick = "echo {slug}"` 的用例失败（`{slug}` 被替换或被判未知占位符）。

### M-05 验收契约

- 验收命令：`cargo test -p autome-cli -- render::tests::gate_`、`node --test apps/desktop/test/labels-gate-kind.test.js`、`sh -c 'grep -q AUTOME_GATE_KIND README.md && grep -q AUTOME_GATE_KIND docs/development/technical-design.md && grep -q quick_timeout_minutes README.md && ! grep -q "Bug fix rounds run only the repro command;" README.md && ! grep -q "Bug 修复轮只跑复现命令，FixGate" docs/development/technical-design.md'`
- 预期：第一条 11 个通过（现有 `render::tests::gate_` 9 + 新增 2）；第二条 3 个通过；第三条退出码 0。证据另记四段快检命令热缓存下的总耗时与各段耗时（或「未取证」与原因）。
- 负向对照：`render.rs` 不看 `kind` 时，`gate_quick_run_lines_name_the_quick_run` 失败（输出「门禁开始」而不是「快检开始」）；`gateTagLabel` 不看 `kind` 时桌面第 1 个用例失败（得到「门禁通过」）；README 保留原句时第三条退出码 1。

## 人工验收清单

- H-01 合并后在主检出的 `.autome/config.toml` `[gate]` 加上任务卡那条 `quick`，跑一个任务：`autome run` 在每个实现轮后打印「快检开始 / 快检通过」，合并前只出现一次完整门禁
- H-02 桌面端打开该任务的卡片，宿主标签显示「快检通过 · <sha>」，悬停看到快检命令与日志路径

## Backlog

- B-01 `each_round` 下写了 `quick` 时，在 `project.get` / `config.get` 的 `violations` 之外给一句「快检只在 before_merge 下生效」的说明（任务卡「不强求」，本任务不做）。
- B-02 桌面路由图（`routing.js`）在 `before_merge` + `quick` 下把实现轮后的门禁节点标成「快检」。
- B-03 Bug FixGate 以 `each_round` 启动、运行中切到 `before_merge` 时，T-61 的既有记账会把 `gate.result.command` 记成复现命令（`kind` 仍是 `full`）；约束 1 要求保持，本任务不改（评审 N1）。
- B-04 §12 的快检耗时实测若记为「未取证」，M-05 的实现轮要在证据里写明原因，审计与复盘把它列为未关闭项，由用户在合并前或合并后补测；不据此阻塞里程碑（评审 N2）。

## 争议项
