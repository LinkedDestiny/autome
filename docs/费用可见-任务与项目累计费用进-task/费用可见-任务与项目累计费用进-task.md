# 费用可见：任务与项目的累计费用，可选预算告警

status: 实现中
design-round: 1/15
implementation-round: 1/25
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01 待独立审计；通过后下一实现轮推进 M-02 项目与看板费用字段及 3 个 cost_rollup_ 单测

任务文件：`docs/费用可见-任务与项目累计费用进-task/费用可见-任务与项目累计费用进-task-task.md`

下文「验收 n」指任务文件 §3 的第 n 条验收标准，「约束 n」指 §5 的第 n 条硬性约束，「范围」指 §4。
行号是 2026-11-12 设计轮核对时的，实现前请重新定位。

## 背景与现状

每个会话的费用来自 CLI 自己报出的 `total_cost_usd`（`usage.rs`，只有 Claude 有；Codex 恒为 `None`），
会话被回收时由 `scheduler.rs:815` `record_usage → store.set_session_metrics` 写进 sessions 表的 `cost_usd` 列。
这是累计费用变化的唯一时点。读出来的地方只有三处：

- `task.get` 的 `sessions[].metrics.cost_usd`（逐条）与 `so_far.total_cost_usd`（`dispatch.rs:1736`
  `running_totals`，口径是「没有一个会话报价就是 `null`」）；
- CLI `autome status <任务>` 的「会话（最近 5 个）」逐条 `$x.xx`（`main.rs:723`）；
- 任务结束后的终态聚合 `task.metrics.total_cost_usd`（`task_metrics.rs`）。

没有任务累计（按任务卡口径）、没有项目累计、看板与列表上没有费用。

设计轮核对过、与任务文件 §6 不同或需要补充的事实：

1. **`ProjectConfig` 派生了 `Eq`**（`config.rs:238`），`f64` 字段放不进去。预算在类型里存成整数美分
   `u64`，读写时与 TOML 的美元数互转（见方案 §3）。
2. **警报种类不止一种。** 除 `main_gate_failed`（`scheduler.rs:3286`）外还有 `setup_failed`
   （`scheduler.rs:2959`、`3158`）。三处都是 `raise_alert` 后紧跟一条 `project.alert` 事件
   `{alert_id, kind, task_id}`；新警报照同一写法。
3. **`save_project` 有生产调用方。** `dispatch.rs:1152`、`1176`、`1236`（角色与 Loop 设置的保存）都是
   `load_project → 改一处 → save_project`。所以 `[budget]` 必须在 load 与 save 两边都认，否则用户在
   桌面端改一次角色设置就会把 `[budget]` 抹掉或写坏。`[gate]` / `[retry]` / `[setup]` 现在靠
   「save 不碰」保留；`[bug]` 走「Some 写、None 删」。
4. **`autome ls` 读的是 `project.get.tasks[]`**（`client.rs:141` `all_tasks`），即 `dispatch.rs:559`
   `task_json` 的形状；`dashboard.get` 的 `waiting[]/running[].task` 也是它。在 `task_json` 上加费用，
   `ls`、`status` 运行中行、看板与各自的 `--json` 一次都有。
5. **`so_far` 的读者只有桌面端渲染进程**（`apps/desktop/renderer/screens/task.js:517` 与测试夹具
   `apps/desktop/test/fixtures.js:696`）。CLI 与 Web 都不读它。
6. **终态任务只有 `Done` 与 `Cancelled`**（`task.rs:368` `is_terminal`）。失败、已停止、已暂停都可以
   恢复，不是终态。
7. **Onboarding 会话不进 sessions 表**（`scheduler.rs:3921` 只用 `ONBOARDING_SESSION_KEY` 拼路径），
   所以按任务聚合不会遇到无主会话。
8. **警报的其它读者：**
   - Web 通知器 `apps/web/server/notifier-rules.ts:31`：`project.alert` 规则对任何 kind 都会发
     「需要你」通知，标题对非 `main_gate_failed` 的种类是原始字符串 `项目告警：<kind>`；
     `task.budget_exceeded` 不匹配任何规则，不会多发一条。
   - 桌面端 `dashboard.js:114` 用 `ALERT_TITLES[kind] || kind`，新种类显示成原始 `budget_exceeded`，
     卡片、关闭按钮照常工作，不会被吞掉。约束 6 不许改，本任务不改（见非目标）。
   - CLI `render.rs:358` `project.alert` 行显示原始 kind；`autome alerts` 显示 kind 与 detail。
9. **e2e 已经能造费用。** `end_to_end.rs:318` `FAKE_USAGE` 让替身模型打印一条
   `total_cost_usd: 0.25` 的 result 行，`doc_step_with_usage` 用它；`World::configure`
   （`end_to_end.rs:445`）往主检出的 `.autome/config.toml` 追加并提交配置。默认角色配置下
   plan / adjudicate / impl 是 Claude，review / audit / retro 是 Codex（`config.rs:199–206`），
   Codex 会话因此天然是「无计费数据」。
10. **CLI 测试能直接造账本。** `cli.rs:151` 的测试用 `Store::open` + `insert_session` 造会话；
    `store.set_session_metrics`、`store.set_task_state`（`store.rs:1018`）都是公开方法。
    现有 CLI 测试对 `ls` / `status` 只做 `contains` 断言，加列不会打破它们。

## 目标与非目标

目标（任务卡四件事）：

- G1 `task.get` 返回 `cost`；`project.get` / `project.list` 返回未完成与已完成分开的 `cost_usd_total`；
  `dashboard.get` 的任务卡带 `cost_usd`；无计费数据的会话计 0 并单列计数。
- G2 CLI `status <任务>` 顶部费用行、`ls` 费用列、`status` 运行中行费用，`--json` 同步。
- G3 可选 `[budget] per_task_usd`；超限写一条 `task.budget_exceeded` 事件并 `raise_alert("budget_exceeded")`，
  不停任务。
- G4 read 白名单不动。

非目标（任务卡与任务文件 §4，加上本设计的取舍）：

- 不做图表、不按模型单价估算、不改 `apps/desktop/renderer/`、不改 Web 面板页面。
- 不停任务、不暂停、不改状态机；不实现 `stop_on_exceed`；不做全局或项目总预算。
- 不新增 JSON-RPC 方法，不动 `READ_METHODS` / `ALLOWED_READ_METHODS`。
- 不改 `so_far`、`task.metrics.total_cost_usd` 的字段与口径（理由见方案 §1.3）。
- `autome project ls` 的文本输出不加费用列（任务卡 CLI 一节没列它）；它的 `--json` 因内核字段而自动带上。
- 不把预算放进 `project.get.config`（解析后的配置）——任务卡没要求，进 Backlog。
- 不修 `setup_failed` 在通知器与 CLI 上显示原始 kind 的旧问题——不在本任务读者改动的范围内，进 Backlog。

## 方案

### 1. 费用聚合（纯函数，放 `autome-domain`）

#### 1.1 口径

在 `crates/autome-domain/src/metrics.rs` 新增：

```rust
pub struct CostSummary {
    pub usd_total: f64,
    pub sessions: u32,
    pub sessions_without_cost: u32,
    pub by_role: BTreeMap<String, RoleCost>,
}
pub struct RoleCost { pub usd_total: f64, pub sessions: u32, pub sessions_without_cost: u32 }

impl CostSummary {
    pub fn of<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> Self;
}
```

| 会话 | 计入 `sessions` | 计入 `sessions_without_cost` | 计入 `usd_total` |
|---|---|---|---|
| 已结束，`cost_usd = Some(c)` | 是 | 否 | `+ c` |
| 已结束，`cost_usd = None`（Codex，或流缺失） | 是 | 是 | `+ 0` |
| 运行中（`is_running()`） | 否 | 否 | 否 |

- **运行中的会话不算。** 它的费用要回收时才知道，算进「无计费数据」会把一个 Claude 会话说成 Codex 那样
  永远不会有价格；算进 `sessions` 又会让「n 个会话」与金额对不上。它们一结束就进账。
- **一个会话都没有时 `usd_total = 0.0`，不是 `null`。** 这是任务卡的口径（缺失计 0）。
- `usd_total` 是原始和，不在内核取整；两位小数是展示层的事（CLI 格式化）。
- `by_role` 的键是会话种类的稳定英文名：`Role` 会话用 `Role::as_str()`（`plan` / `review` /
  `adjudicate` / `impl` / `audit` / `retro`），Bug 会话用步骤名（`locate` / `fix` / `fix_review`），
  另有 `intake`、`onboarding`。只出现有已结束会话的键。键由 `SessionKind` 上新增的一个
  `cost_key() -> &'static str` 给出，避免 CLI 或内核各写一份映射。每个键下与顶层同形
  （`usd_total`、`sessions`、`sessions_without_cost`）——一个角色花了多少钱、跑了几次、其中几次没报价，
  缺一个就说不清那个角色贵在哪。

#### 1.2 JSON 形状（只增不改，约束 7）

- `task.get` 顶层新增
  `"cost": {"usd_total": 3.75, "sessions": 3, "sessions_without_cost": 1, "by_role": {"plan": {...}, ...}}`。
- `task_json`（`project.get.tasks[]`、`dashboard.get` 的 `waiting[]/running[].task`、以及其它返回任务卡的方法）
  新增 `"cost_usd": f64` 与 `"cost_sessions_without_cost": u32`。后者让 `ls` 能标出「这个数偏低」，
  否则一个全跑在 Codex 上的任务和一个没花钱的任务都显示 `$0.00`。
- `project.get` 顶层、`project.list` 的每个 `projects[]` 条目新增
  `"cost_usd_total": {"unfinished": f64, "finished": f64}` 与 `"cost_sessions_without_cost": u32`。
  **已完成 = `is_terminal()`，即 `Done` 与 `Cancelled`**；失败、已停止、已暂停、草稿、排队、进行中都算未完成——
  它们都还可能再花钱（恢复、重跑），这正是用户想单独看的那部分。已归档的任务仍在 `list_tasks` 里，按其状态归边。

#### 1.3 `so_far` 保留原样

`so_far` 不改名、不改口径、不删。它的唯一读者是桌面端渲染进程（事实 5），改它就要改渲染进程（约束 6），删它会
打破约束 7。新旧两个口径并存但字段名不同（`so_far.total_cost_usd` 可为 `null`；`cost.usd_total` 恒为数），
`running_totals` 的文档注释加一句指向 `cost`，说明前者是「只看测到的」的旧视图、后者是任务卡口径。
`TaskMetrics.total_cost_usd` 与它的测试 `a_cost_that_was_never_measured_reads_as_absent_not_zero` 同理不动。

#### 1.4 取数

- `task.get`：已有 `list_sessions(task_id)`，直接 `CostSummary::of(&sessions)`。
- `project.get` / `project.list` / `dashboard.get`：在 `store.rs` 新增
  `sessions_in_project(project_id) -> Vec<Session>`——**一条** `SESSION_SELECT_BASE` + `WHERE task_id IN (SELECT id FROM tasks
  WHERE project_id = ?1)` 的查询，在 Rust 里按 `task_id` 分组后对每组 `CostSummary::of`。
  **不用 JOIN**：`SESSION_SELECT_BASE` 的 `id`、`kind`、`protocol_ref`、`rules_hash` 没有表名前缀，与 `tasks`
  的同名列拼 JOIN 会在 prepare 阶段报 `ambiguous column name`（评审 R1-P03，已在 SQLite 内存表上复现）；
  子查询里的 `id` 属于 `tasks`，外层列都属于 `sessions`，没有歧义。不逐任务调 `list_sessions`，也不在 SQL 里重写一遍口径（口径只在 `CostSummary::of` 一处）。
- `task_json` 增加一个参数，接收该任务的 `&CostSummary`。只返回单个任务的调用方（`dispatch.rs:1513`、`1534`、
  `1605`）用 `list_sessions` 现算；列表调用方（`project.get`、`dashboard.get`）用上面的分组结果，缺组即空汇总。

### 2. CLI（`crates/autome-cli`）

快照的落法：**不引入快照库，对完整输出行做精确相等断言。** 理由：仓库没有快照依赖，`insta` 一类工具的
审阅流程（`cargo insta review`）会话里跑不了；要固定的只是三四行文本，逐行 `assert_eq!` 的鉴别力与快照相同，
失败信息还更直接。

在 `render.rs` 新增两个纯函数（单测覆盖）：

- `usd(v: f64) -> String`：`format!("${v:.2}")`。
- `cost_cell(task: &Value) -> String`：`usd(task.cost_usd)`，`cost_sessions_without_cost > 0` 时末尾加 `*`；
  字段缺失（旧内核）时为空串。

| 位置 | 输出（精确） |
|---|---|
| `status <任务>`，紧接「状态」行之后 | `  费用      $3.75（3 个会话，其中 1 个无计费数据）` |
| 同上，没有会话 | `  费用      $0.00（0 个会话，其中 0 个无计费数据）` |
| `ls` 每行 | 现有内容后接两个空格与 `cost_cell`，如 `…  <状态>  $3.75*` / `…  $0.00` |
| `ls` 末尾，至少一行带 `*` 时 | `* 有会话没有计费数据（如 Codex），实际花费不低于所示金额` |
| `status` 运行中行 | 紧接状态之后插入 `  · $3.75*`，再接原有的会话与重试片段 |

- 费用行的格式按任务卡原文「费用 $x.xx（n 个会话，其中 m 个无计费数据）」，`m = 0` 也照写，不另造变体；
  标签与同块的「状态」「项目」等对齐。`task.get` 没有 `cost` 字段时不打印这一行。
- `--json`：四条命令都是把内核结果原样打印，内核字段到了就同步了；CLI 测试对 `status <任务> --json` 的
  `cost`、`ls --json` 的 `[].cost_usd`、`status --json` 的 `running[].task.cost_usd`、`project ls --json`
  的 `projects[].cost_usd_total` 各断言一次。

### 3. 预算配置（`[budget] per_task_usd`）

- 类型：`autome-domain/src/config.rs` 新增 `BudgetConfig { per_task_cents: u64 }`（`Eq` 可派生，事实 1），
  `ProjectConfig` 加 `#[serde(default, skip_serializing_if = "Option::is_none")] pub budget: Option<BudgetConfig>`。
- 读（`config_io::parse_project`，照 `[retry]` 的写法）：
  - 没有 `[budget]`，或 `[budget]` 里没有 `per_task_usd` → `budget = None`。
  - `per_task_usd` 接受 TOML 整数或浮点；必须有限、`0.01 ≤ v ≤ 1000000`、且最多两位小数
    （`(v*100).round()` 与 `v*100` 相差小于 `1e-6`）。否则报错，文案固定为
    `[budget] per_task_usd 必须是 0.01 到 1000000 之间、最多两位小数的美元金额`；`[budget]` 不是表时报
    `[budget] 不是一个表`。
  - 字符串 `"20"`、布尔、`0`、负数、`0.001` 都拒绝。
- 写（`save_project`）：
  - `budget = None`：文件里有 `[budget].per_task_usd` 就删掉这个键，`[budget]` 因此变空就删掉整节；没有就什么都不动。
    未写 `[budget]` 的配置 load→save 后不多出任何内容（约束 4）。
  - `budget = Some(b)`：文件里已有的 `per_task_usd` 解析出来等于 `b` 时**原样不动**（保留用户的写法与行尾注释）；
    不等或没有时写入——整美元写整数（`20`），否则写浮点（`12.5`）。
- 文档：README 项目配置示例（`README.md:114` 一带）加 `[budget]` 两行与一句说明；
  `docs/development/technical-design.md` §3.3「配置文件」补一句 `[budget]` 的含义与「只告警不停任务」。

### 4. 超限告警

#### 4.1 判定（纯函数，放 `autome-domain/src/metrics.rs`）

```rust
pub fn budget_crossed(usd_total: f64, limit_cents: u64, last_alerted_cents: Option<u64>) -> bool {
    let limit = limit_cents as f64;
    // 美元金额累加、乘 100 都带二进制浮点尾差（0.07*100.0 = 7.000000000000001，
    // 0.10+0.20 再乘 100 = 30.000000000000004）。比较时给上限加一个吸收尾差的容差，
    // 真正的子美分超额（0.071 美元对 7 美分）仍然越线。
    let tolerance = 1e-6 + limit * 1e-12;
    usd_total * 100.0 > limit + tolerance && last_alerted_cents.is_none_or(|last| limit_cents > last)
}
```

容差只用在这一处比较里：`CostSummary.usd_total` 仍是原始和（§1.1），不在聚合处取整，也不改会话费用。
容差 1e-6 美分（1e-8 美元）远小于 1 美分，也比 `f64` 在常见金额上的累加误差（约 1e-14 美分量级）大得多；
1e-12 的相对项让它在上限取最大值 1e8 美分时仍盖得住累加误差。

| 情形 | 结果 |
|---|---|
| 从未告警，累计 > 上限 | 告警 |
| 累计 = 上限（含浮点尾差：0.07 对 7 美分，0.10 + 0.20 对 30 美分） | 不告警（「超过」是严格大于） |
| 累计只比上限多出子美分（0.071 对 7 美分） | 告警（真实超额） |
| 已对同一上限告警过，继续花钱 | 不告警（「一条警报」是字面意思） |
| 用户把上限调低（已告警 2000，现 1500） | 不告警：这个任务已经被提醒过超预算 |
| 用户把上限调高（已告警 2000，现 3000）且累计 > 3000 | 再告警一次：用户改了预期，新预期又被越过了 |
| 用户把上限调高且累计 ≤ 新上限 | 不告警 |
| 用户关掉了警报（dismiss） | 不影响判定：去重看事件历史，不看未处理警报，否则关一次就又冒一次 |

去重依据是**事件历史**：`store.last_event(task_id, "task.budget_exceeded")` 的 payload 里的 `limit_cents`。
不新增列、不新增表、不依赖警报是否已关。

#### 4.2 挂载点

`scheduler.rs` `reap_session` 里 `record_usage` 之后、Onboarding 早退之后，调用新函数
`check_budget(ctx, &project, &task)`：

1. `config_io::load_project(project.path)` 取 `budget`；`None` 直接返回——不读会话、不写任何东西（约束 4）。
2. `CostSummary::of(&store.list_sessions(task.id))`，`budget_crossed(...)` 为假则返回。
3. 调用 `store` 上新增的 `raise_budget_alert(project_id, task_id, detail, usd_total, limit_cents)`，它在**同一个
   SQLite 事务**（`self.conn.unchecked_transaction()`，`store.rs` 已有同样用法）里依次写：
   `raise_alert(project.id, "budget_exceeded", Some(task.id), detail)`；
   `append_event("task.budget_exceeded", task.id, {alert_id, usd_total, limit_usd, limit_cents})`；
   `append_event("project.alert", project.id, {alert_id, kind: "budget_exceeded", task_id})`（与事实 2 的写法一致，
   通知器与 `autome run` 的事件流因此能看到）。三条要么都在、要么都不在：去重靠 `task.budget_exceeded` 事件，
   若警报先提交、事件写失败，下一次回收会看不到去重标记而再发一条警报（评审 R1-P02）；反过来则出现
   「已去重却没有警报」。事务任一步失败即整体回滚，调用方只记日志，下次回收重新判定。
   不新增表、不做重试或补偿。

`detail` 固定为：
`任务 <id> 累计费用 $<x.xx>，超过本项目单任务预算 $<y.yy>（.autome/config.toml [budget] per_task_usd）。任务照常继续；要停下用 autome stop <id>。`

**尽力而为，不影响回收。** `check_budget` 的任何错误（配置读不出、事务写库失败）只 `tracing::warn!`，不向上传——
和 `record_usage` 同理：丢一条警报不能变成丢一个任务。它不读写任务状态、不碰调度与轮次（约束 3）。
配置从项目主检出读（与门禁、重试等其它项目配置一致），改了立即对下一次回收生效。

#### 4.3 读者的最小文案（范围「其它读者」）

- CLI `render.rs` `event_line`：新增 `task.budget_exceeded` →
  `<任务> 累计费用 $<x.xx> 超过预算 $<y.yy>，任务照常继续  → autome alerts`；
  `project.alert` 在 `kind == "budget_exceeded"` 时显示 `报警（<项目>）：<task_id> 费用超出预算  → autome alerts`，
  其它 kind 的输出逐字不变。
- Web 通知器 `notifier-rules.ts` `renderTitle`：`payload.kind === 'budget_exceeded'` 时标题为
  `<task_id> 费用超出预算`；规则表不加行（仍是七行，现有「恰七行」测试不动）。
- 桌面端渲染进程不改（约束 6）：它会把标题显示成 `budget_exceeded`，功能不坏。这一条写进人工验收清单让用户看一眼，
  不作为争议项——它不会「坏」，只是不好看，而渲染进程按任务卡将由 W11 删除。

### 5. 现有测试

预期不需要修改任何现有测试。若某个现有测试对 `task_json` / `project.get` / `project.list` / `dashboard.get`
的整对象做相等断言而因新增字段失败，只允许给期望值补上新字段，并在该轮证据文件逐条写明测试名与理由（约束 7）。
改动任何现有断言的值、或删除断言，都不在允许之列。

## 风险与验证安排

| 风险 | 落在哪 | 怎么验证 |
|---|---|---|
| 新旧口径混用：把 `so_far` 改成 0 口径，或让 `cost` 沿用 `null` 口径 | M-01 | `cost_summary_of_no_sessions_is_zero_not_absent`；`so_far` 现有测试与桌面端测试不改仍绿 |
| 浮点和的末位误差让「等于和」不稳 | M-01、M-04 | 用例只用二进制可精确表示的金额（0.25、1.25、2.5、0.4 的比较在美分上做） |
| `save_project` 抹掉或改写用户的 `[budget]` | M-03 | `budget_config_save_*` 两条：原样保留与 None 不多写 |
| 每个会话都告警一次 | M-04 | e2e 断言警报与事件都恰为 1，且之后还有带费用的会话 |
| 浮点尾差让等额费用误报超限 | M-04 | `budget_alert_requires_strictly_more_than_the_limit` 含 0.07/7 美分、0.10+0.20/30 美分（假）与 0.071/7 美分（真） |
| 警报与去重事件分开提交，写失败后重复报警 | M-04 | `budget_alert_record_is_all_or_nothing`：事务内某一步失败后三条记录都不在 |
| 项目级会话查询列名歧义 | M-02 | `cost_rollup_` 三个用例都经过 `sessions_in_project`，歧义会在 prepare 阶段让它们全部失败 |
| 告警逻辑意外停任务或抛错中断回收 | M-04 | e2e 断言任务走到 `await_merge`；`check_budget` 错误只记日志 |
| 项目汇总串项目 / 终态边界错 | M-02 | 两项目 + 六种状态的 `cost_rollup_` 单测 |
| 全量门禁红（fmt / clippy / 桌面 / Web） | 每个里程碑 | 宿主每个实现轮后跑 `sh scripts/ci-local.sh`；M-05 把它写进验收 |

没有需要在设计期提前做原型的重大风险：费用数据、事件、警报、配置读写、e2e 替身都已在仓库里有现成写法（事实 2、9、10）。
第一个里程碑先打通「会话费用 → 纯聚合 → `task.get`」这条最短端到端链路，并用 e2e 固定口径。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 待审 | 任务费用纯聚合与 task.get.cost | 0 | |
| M-02 | 开放 | 项目与看板费用字段 | 0 | |
| M-03 | 开放 | [budget] 配置读写 | 0 | |
| M-04 | 开放 | 超限告警与读者文案 | 0 | |
| M-05 | 开放 | CLI 费用展示与全量门禁 | 0 | |

## 里程碑说明

- **M-01** 覆盖验收 1、2，G1 的 `task.get` 部分。前置：无。结果：`CostSummary` 与 `SessionKind::cost_key`；
  `task.get.cost`；`running_totals` 注释。新增测试见契约。
最新证据：docs/费用可见-任务与项目累计费用进-task/evidence/M-01-r1-impl.md · 验收通过，待审 · 实现 #1

- **M-02** 覆盖 G1 其余部分。前置 M-01。结果：`store.sessions_in_project`（子查询，非 JOIN）；`task_json` 的 `cost_usd` /
  `cost_sessions_without_cost`；`project.get` / `project.list` 的 `cost_usd_total` 与 `cost_sessions_without_cost`；
  `dashboard.get` 任务卡因 `task_json` 带上费用。
- **M-03** 覆盖 G3 的配置部分、约束 4。前置：无（可与 M-01/M-02 独立，按编号顺序做）。结果：`BudgetConfig`、
  `parse_project` / `save_project` 支持 `[budget]`；README 与 technical-design 各补一处。
- **M-04** 覆盖验收 3、G3 告警部分、约束 3。前置 M-01、M-03。结果：`budget_crossed`（带浮点容差）；`check_budget` 挂在回收后；
  `store.raise_budget_alert`（一次事务写警报与两条事件）；CLI `event_line` 两处文案；Web 通知器标题。
- **M-05** 覆盖验收 4、5，G2。前置 M-02、M-04。结果：`render::usd` / `cost_cell`；`status <任务>` 费用行、
  `ls` 费用列与脚注、`status` 运行中行费用；`USAGE` 末尾说明配置节的那句加上 `[budget]`。

## 验收契约

所有过滤名前缀（`cost_summary_`、`cost_rollup_`、`budget_config_`、`budget_alert_`、`cost_cli_`、`预算告警`）
在 2026-11-12 的代码里没有任何命中，所以各条命令的基线都是 0 个用例，预期数即本里程碑新增数。

### M-01 验收契约

- 验收命令：`cargo test -p autome-domain --lib cost_summary_ && cargo test -p automed --test end_to_end cost_rollup_`
- 预期：共 5 个用例通过（基线 0，新增 5）。autome-domain 4 个：
  `cost_summary_counts_a_session_without_cost_as_zero_and_lists_it_separately`（已结束 1.25 / 2.5 / None 三个会话 →
  `usd_total = 3.75`、`sessions = 3`、`sessions_without_cost = 1`）、
  `cost_summary_skips_running_sessions`（再加一个运行中会话，三个数不变）、
  `cost_summary_splits_by_role_with_bug_steps_and_intake_as_their_own_keys`（`plan`、`review`、`fix_review`、
  `intake` 各自成键，每键三数正确，未出现的角色无键）、
  `cost_summary_of_no_sessions_is_zero_not_absent`（空集合 → `0.0 / 0 / 0`，`by_role` 为空）；
  end_to_end 1 个：`cost_rollup_task_get_sums_two_sessions_and_counts_missing_cost`（plan 会话报 1.25、
  review 会话（Codex）、adjudicate 会话报 2.5，任务停在 `await_design_approval` 时 `task.get.cost.usd_total == 3.75`、
  `sessions == 3`、`sessions_without_cost == 1`、`by_role.plan.usd_total == 1.25`，且 `usd_total` 等于
  `list_sessions` 各会话 `cost_usd` 之和；需要在 e2e 里新增一个可指定金额的 `doc_step_with_cost` 辅助）。
- 负向对照：把 `cost_usd = None` 的会话从 `sessions` 里跳过时，第 1 个单测与 e2e 的 `sessions == 3` 失败；
  沿用 `running_totals` 的「全缺失即 null」口径时第 4 个单测失败；把运行中会话计入时第 2 个单测失败。

### M-02 验收契约

- 验收命令：`cargo test -p automed --lib cost_rollup_`
- 预期：3 个用例通过（基线 0，新增 3）：
  `cost_rollup_project_get_splits_unfinished_and_finished`（同一项目六个任务，状态分别为 Done、Cancelled、
  Failed、Stopped、Draft、Active，各有一个已结束会话报不同金额，另一个会话无费用；`finished` 只等于 Done + Cancelled
  之和，`unfinished` 等于其余四个之和，`cost_sessions_without_cost` 等于无费用会话数，`tasks[].cost_usd` 逐个正确）、
  `cost_rollup_project_list_keeps_projects_apart`（两个项目各有会话，`project.list` 每项的 `cost_usd_total` 只含本项目，
  且与各自 `project.get` 的值相等）、
  `cost_rollup_dashboard_task_cards_carry_cost`（一个等待中、一个运行中的任务，`waiting[0].task.cost_usd` 与
  `running[0].task.cost_usd` 等于各自任务的和，`cost_sessions_without_cost` 正确）。
- 负向对照：只把 `Done` 当已完成（漏掉 `Cancelled`）时第 1 个用例 `finished` 断言失败；`sessions_in_project` 忘了
  `WHERE tasks.project_id = ?1` 时第 2 个用例失败；`task_json` 在列表调用方传空汇总时第 1、3 个用例的 `cost_usd` 断言失败。

### M-03 验收契约

- 验收命令：`cargo test -p automed --lib budget_config_`
- 预期：4 个用例通过（基线 0，新增 4）：
  `budget_config_reads_integer_and_decimal_dollars`（`20` → 2000 美分，`12.5` → 1250，`0.01` → 1，`1000000` → 1e8）、
  `budget_config_rejects_out_of_range_sub_cent_and_non_numbers`（`0`、`-1`、`0.001`、`1000000.01`、`"20"`、`true`、
  `budget = 1` 各自报错，前六个的报错文案逐字等于方案 §3 的那句，最后一个是 `[budget] 不是一个表`）、
  `budget_config_absent_or_empty_is_none_and_save_adds_nothing`（无 `[budget]` 与空 `[budget]` 都解析为 `None`；
  一份带 `[gate]` 与 `[retry]`、不带 `[budget]` 的文件 load→save 后字节不变）、
  `budget_config_save_keeps_the_users_line_and_writes_or_removes_on_change`（`per_task_usd = 20  # 注释` 在改角色后
  load→save 原样保留；改成 1250 美分写出 `per_task_usd = 12.5`；改成 `None` 后 `[budget]` 整节消失）。
- 负向对照：`save_project` 不认 `budget`（照 `[gate]` 那样不碰）时第 4 个用例「改成 12.5」与「改成 None」两处断言失败；
  `save_project` 无条件重写为浮点时「原样保留」断言失败（变成 `20.0` 且注释丢失）；接受 `0` 或 `0.001` 时第 2 个用例失败。

### M-04 验收契约

- 验收命令：`cargo test -p autome-domain --lib budget_alert_ && cargo test -p automed --lib budget_alert_ && cargo test -p automed --test end_to_end budget_alert_ && cargo test -p autome-cli budget_alert_ && npm test --prefix apps/web -- test/notifier-rules.test.ts -t 预算告警`
- 预期：共 8 个用例通过（基线 0，新增 8）。autome-domain 3 个：
  `budget_alert_requires_strictly_more_than_the_limit`（2000 美分，20.0 → 假，20.25 → 真；7 美分对 0.07 → 假、0.071 → 真；30 美分对 0.10 + 0.20 → 假）、
  `budget_alert_does_not_repeat_for_the_same_or_a_lower_limit`、
  `budget_alert_fires_again_only_after_a_raised_limit_is_crossed`（覆盖方案 §4.1 表的后五行）；
  automed lib（`store.rs` 测试模块）1 个：`budget_alert_record_is_all_or_nothing`（正常调用 `raise_budget_alert` 后
  `open_alerts()` 有 1 条、`task.budget_exceeded` 与 `project.alert` 事件各 1 条；另开一个账本把 `events` 表删掉，
  调用返回错误，`open_alerts()` 为空）；
  end_to_end 2 个：`budget_alert_fires_once_and_the_task_keeps_going`（`configure("[budget]\nper_task_usd = 0.4")`，
  每个 Claude 会话报 0.25；第二个 Claude 会话后累计 0.5 越线，之后至少还有一个带费用的 Claude 会话；任务最终到
  `await_merge`；`open_alerts()` 中 `kind == "budget_exceeded"` 的恰 1 条且 `task_id` 正确，
  `count_events(task, "task.budget_exceeded") == 1`，越线之后的会话数 ≥ 1）、
  `budget_alert_without_a_budget_section_raises_nothing`（同样的会话与费用、不写 `[budget]`，警报与事件都为 0）；
  autome-cli 1 个：`budget_alert_event_and_alert_lines`（`event_line` 对 `task.budget_exceeded` 与
  `project.alert`/`budget_exceeded` 的整行精确相等，`project.alert`/`main_gate_failed` 的旧输出逐字不变）；
  apps/web 1 个：`预算告警标题`（`renderTitle` 对 `{kind: 'budget_exceeded', task_id: 'T-9'}` 得 `T-9 费用超出预算`）。
- 负向对照：判定写成 `>=` 时第 1 个单测失败；不去重时 e2e 第 1 条的「恰 1 条」失败（第三个 Claude 会话后变 2）；
  去重只比较上限是否相等（不比大小）时第 2 个单测「调低上限不再告警」失败；超限后把任务停下或置失败时
  e2e 第 1 条走不到 `await_merge`；无 `[budget]` 时仍按某默认值告警则 e2e 第 2 条失败；比较不带容差时第 1 个单测的 0.07 / 7 美分与 0.10 + 0.20 / 30 美分两处失败；
警报与事件不在同一事务（先 `raise_alert` 再逐条 `append_event`）时 `budget_alert_record_is_all_or_nothing` 的回滚断言失败（警报残留）。

### M-05 验收契约

- 验收命令：`cargo test -p autome-cli cost_cli_ && sh scripts/ci-local.sh > .autome/output/m05-ci.txt 2>&1; echo "ci exit $?"; tail -20 .autome/output/m05-ci.txt`
- 预期：`cost_cli_` 共 5 个用例通过（基线 0，新增 5）：render 单测 2 个
  `cost_cli_usd_has_two_decimals`（3.75 → `$3.75`，0 → `$0.00`，1234.5 → `$1234.50`）、
  `cost_cli_cell_marks_tasks_with_sessions_without_cost`（有无计费会话加 `*`、没有不加、字段缺失为空串）；
  tests/cli.rs 3 个
  `cost_cli_status_task_prints_the_cost_line_under_state`（造 1.25 / 2.5 / None 三个已结束会话与一个运行中会话，
  「状态」行的下一行逐字等于 `  费用      $3.75（3 个会话，其中 1 个无计费数据）`；`--json` 的 `cost` 三数正确）、
  `cost_cli_ls_appends_cost_and_a_footnote`（两个任务：一个以 `  $3.75*` 结尾，一个以 `  $0.00` 结尾；末行逐字等于
  方案 §2 的脚注；`ls --json` 与 `project ls --json` 的新字段正确）、
  `cost_cli_running_row_shows_cost`（`set_task_state` 置为占槽的进行中节点，「运行中」小节该行包含 `  · $3.75*`；
  `status --json` 的 `running[0].task.cost_usd == 3.75`）。随后 `ci exit 0`（验收 5：`cargo test --workspace` 在其中）。
- 负向对照：金额用 `{}` 而非 `{:.2}` 时第 1 个单测（`$0`、`$1234.5`）与 `ls` 的 `$0.00` 断言失败；「其中 m 个」误用
  `sessions` 时 `status` 那行失败；没有任何 `*` 却仍打印脚注（或反之）时 `ls` 用例失败。

## 人工验收清单

- H-01 在一个真实项目里跑完一个任务，`autome status <任务>` 的费用行金额与该任务各会话 Claude 账单之和一致（Codex 会话计入「无计费数据」）
- H-02 给真实项目写 `[budget] per_task_usd` 一个很小的值，任务越线后桌面端看板出现一张报警卡（标题显示 `budget_exceeded`，已知不美化），能关闭，任务照常推进
- H-03 同上情形，Web 面板的通知铃收到一条「<任务> 费用超出预算」

## Backlog

- B-01 `project.get.config`（解析后的配置）里带上 `budget`，让面板能显示当前预算。
- B-02 `setup_failed` 在 Web 通知标题与 CLI `project.alert` 行仍显示原始 kind，可与 `budget_exceeded` 一起做成统一的种类文案表。
- B-03 `autome project ls` 文本输出加项目累计费用列。
- B-04 `stop_on_exceed`（任务卡点名的后续项）。
- B-05 `autome status` 运行中行的 `$3.75*` 没有星号脚注（只有 `ls` 末尾有），可复用同一句释义（评审 N-01）。
- B-06 `autome alerts`（`main.rs:983–991`）直接打印原始 `kind`，可与 B-02 的统一种类文案表一起做；`budget_exceeded` 的 detail 已是完整中文，含义可读（评审 N-02）。

## 争议项

（无）
