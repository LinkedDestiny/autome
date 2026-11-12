# 任务：费用可见——任务与项目的累计费用，可选预算告警

本任务的规则见 `docs/费用可见-任务与项目累计费用进-task/protocol/loop-protocol.md` 与
`docs/费用可见-任务与项目累计费用进-task/protocol/session-protocol.md`。

- 设计文档：`docs/费用可见-任务与项目累计费用进-task/费用可见-任务与项目累计费用进-task.md`
- 任务卡（用户写的，第一输入）：`docs/费用可见-任务与项目累计费用进-task/费用可见-任务与项目累计费用进-task-card.md`
- 任务整理日期：2026-11-12
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）

## 1. 需求原文

> 费用可见：任务与项目累计费用进 task.get/project.get/status/ls，可选 [budget] 告警

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§4 照任务卡整理。

## 2. 目标与背景

**背景（任务卡原文）。** 每个会话的 `cost_usd` 已入库（`store.rs` sessions 表），但只在
`autome status <任务>` 的「最近 5 个会话」里逐条显示。T-27 跑了 25 个实现轮，没有任何地方能看到这个任务
总共花了多少钱；项目维度更没有。用户是按任务付费的，这是最该有的数字之一。

**目标。** 让任务与项目的累计费用在内核接口和 CLI 上直接可见，并提供一个可选的、只告警不停任务的
单任务预算。

任务卡规定的四件事：

1. **内核。**
   - `task.get` 返回 `cost: {usd_total, sessions, by_role: {...}}`；
   - `project.get` / `project.list` 返回 `cost_usd_total`（未完成 + 已完成分开）；
   - `dashboard.get` 的任务卡带 `cost_usd`；
   - 费用缺失（Codex 没有计费数据）的会话计为 0，但 `sessions_without_cost` 计数单列。
2. **CLI。**
   - `autome status <任务>` 顶部一行「费用 $x.xx（n 个会话，其中 m 个无计费数据）」；
   - `autome ls` 每行末尾加费用列；
   - `autome status`（不带任务）的「运行中」行带费用；
   - `--json` 同步。
3. **可选预算。** 项目 `.autome/config.toml` 里 `[budget] per_task_usd = 20`（可省）。任务累计超过时写一条
   `task.budget_exceeded` 事件并 `raise_alert("budget_exceeded")`，**不停任务**（停不停由用户决定，
   `stop_on_exceed` 可后续再加，不在本任务内）。
4. **网页网关。** read 白名单无需新增方法（走现有 `task.get` / `project.get`）。

## 3. 验收标准

以下五条逐字取自任务卡「验收标准」（原文是一句话，这里按分号拆开编号）。设计文档的里程碑必须逐条覆盖；
用户在设计定稿时会对照它们批准或驳回。

1. 单测：聚合函数对缺失费用的处理。
2. e2e：两个会话各记费用后 `task.get.cost.usd_total` 等于和。
3. e2e：`[budget]` 超限产生一条警报且任务继续。
4. CLI 快照测试。
5. `cargo test --workspace` 全绿。

补充说明（从任务卡和调研得出，不是新范围）：

- 第 2、3 条的 e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装
  脚本，只把模型换成替身）。只在 `dispatch.rs` 或 `scheduler.rs` 里写单元测试不能代替它。
- 第 3 条「一条警报」是字面意思：同一任务超限后继续跑出更多会话，警报与 `task.budget_exceeded` 事件
  不应每个会话再来一条。「任务继续」要用状态或账本里后续的会话记录断言，不靠时序猜测。
- 第 4 条的 CLI 测试落在 `crates/autome-cli/tests/cli.rs`（驱动真实二进制的沙箱，已有
  `status_shows_latest_five_sessions_in_chronological_order` 可参照）和/或 `render.rs` 的单元测试。
  仓库里目前没有快照测试库；「快照」是引入依赖还是对完整输出行做精确断言，由设计轮决定并写明理由。
- 第 5 条之外，本项目的宿主门禁是 `sh scripts/ci-local.sh`（见 §7），它还跑 `cargo fmt --check`、
  `cargo clippy -D warnings`、桌面端与 Web 的测试。每个实现轮之后宿主都会跑它，红了会退回实现。

## 4. 范围

在范围内：

- `crates/automed`：
  - 费用聚合（按任务：总额、会话数、无计费数据会话数、按角色拆分；按项目：未完成与已完成分开）；
  - `dispatch.rs` 的 `task.get`、`project.get`、`project.list`、`dashboard.get` 带上新字段；
  - `config_io.rs` 读写 `[budget] per_task_usd`（含 `save_project` 的往返）与非法值的中文报错；
  - 超限检测、`task.budget_exceeded` 事件、`raise_alert("budget_exceeded")`；
  - 相应单元测试与 e2e。
- `crates/autome-domain`：`ProjectConfig`（及解析后的配置，若需要）新增预算字段；聚合若写成纯函数，
  放这里还是放 `automed` 由设计轮决定。
- `crates/autome-cli`：`status <任务>` 顶部费用行、`ls` 费用列、`status` 运行中行的费用、`--json` 同步、
  新事件与新警报种类在 `render.rs` 里的文案；相应测试。
- 新事件/新警报种类的其它**读者**，只限让它们不显示成原始字符串或被吞掉的最小文案改动（见 §6.5），
  是否需要由设计轮核实后决定。
- 文档：README 的项目配置示例加 `[budget]`；`docs/development/technical-design.md` 相关小节补一句。

不在范围内（任务卡「不做什么」，逐字）：

- 不做图表；
- 不做按模型单价估算；
- 不改桌面端渲染进程（W11 删除）。

同样不在范围内（从任务卡推出）：

- 超预算**不停任务**、不暂停、不改状态机转换；不实现 `stop_on_exceed`。
- 不给网页网关 read 白名单加方法；不新增 JSON-RPC 方法来承载费用（走现有四个方法的返回值）。
- 不做全局（`~/.autome/config.toml`）预算、不做项目总预算——任务卡只写了项目级的 `per_task_usd`。
- 不改 Web 面板（`apps/web/app`）的页面去展示费用——任务卡只列了内核与 CLI。

## 5. 硬性约束

1. **缺失费用的口径以任务卡为准。** 无计费数据的会话在总额里计 0，同时单列 `sessions_without_cost`。
   现有 `running_totals`（§6.1）的口径是「一个都没报就是 `null`」，与此不同；新字段按任务卡，旧字段
   `so_far` 是保留、改口径还是删除，设计轮必须明确写出并核对它的所有读者（CLI、桌面端、Web）。
2. **不估价。** 费用只来自 CLI 自己报出的 `total_cost_usd`；不得维护价目表、不得用 token 数推算。
3. **超预算只告警。** 任务的状态、调度、轮次预算一律不受 `[budget]` 影响。
4. **`[budget]` 可省。** 不写时行为与今天完全一致：不产生事件、不产生警报。未写 `[budget]` 的既有项目配置
   读写往返不得多出内容。
5. **read 白名单不动。** `dispatch.rs` 的 `READ_METHODS`、`apps/web/server/gates/read-gate.ts` 的
   `ALLOWED_READ_METHODS`、桌面端对白名单的交叉校验都不新增方法。
6. **不改 `apps/desktop/renderer/`。** 若设计认为那里必须改才能不坏，写进争议项交用户决定，不要直接改。
7. **既有字段不破坏。** `task.get` / `project.get` / `project.list` / `dashboard.get` 现有字段只增不改名、
   不改类型；`autome ... --json` 现有键同理。现有测试若需修改，设计文档逐条列理由。
8. **CLI 文案是中文**，金额格式按任务卡示例 `$x.xx`（两位小数）。
9. **不修改 `.autome/`、不修改 `protocol/`（仓库根的协议 seed）与本任务目录下的 `protocol/` 副本。**
   本任务不是协议改动。
10. 只在当前 worktree 内工作；不切分支、不推送、不操作 `.worktree/` 下其它目录；不自行启动下一个会话。
11. 超过 5 行的命令输出与临时验证产物写入 `.autome/output/`，不进任务文档目录。

## 6. 调研时观察到的事实（供设计参考，不是设计结论）

行号是 2026-11-12 整理时的，实现前请重新定位。

### 6.1 费用数据从哪来、现在怎么用

- `crates/autome-domain/src/metrics.rs`：`SessionMetrics.cost_usd: Option<f64>`，注释写明 Codex 恒为 `None`；
  `TaskMetrics.total_cost_usd: Option<f64>` 是任务**结束后**的终态聚合（`a_cost_that_was_never_measured_reads_as_absent_not_zero`
  这条测试固定了「没测到是缺失不是 0」的旧口径）。
- `crates/automed/src/usage.rs`：从 Claude 的 stream 里读 `total_cost_usd`（约 251 行）；Codex 分支不填。
- `crates/automed/src/store.rs`：sessions 表有 `cost_usd REAL` 列（约 334 行迁移）；
  `set_session_metrics`（约 1379 行）在会话被回收时写入；`list_sessions(task_id)`（约 1408 行）取一个任务的全部会话。
- `crates/automed/src/scheduler.rs` 约 815 行：会话回收后调用 `set_session_metrics`——累计费用发生变化的唯一时点，
  预算检查大概率挂在这附近。
- `crates/automed/src/dispatch.rs`：
  - `task_get`（约 1585 行）已返回 `sessions`（每条带 `metrics`）与 `"so_far": running_totals(&sessions)`；
  - `running_totals`（约 1736 行）返回 `{sessions_measured, total_tokens, total_turns, total_cost_usd}`，
    其中 `total_cost_usd` 在没有任何会话报价时为 `null`，注释解释了为什么不是 0；
  - `project_list`（约 522 行）、`project_get`（约 536 行）、`task_json`（约 559 行，任务卡片的公共形状，
    `project.get.tasks[]` 与 `dashboard.get` 的 `task` 都用它）、`dashboard_get`（约 2186 行，跳过终态任务）；
  - `READ_METHODS`（约 2510 行）已含这四个方法。
- `crates/automed/src/task_metrics.rs`：终态聚合 `total_cost_usd` 的来源（约 116、163 行）。

### 6.2 CLI

- `crates/autome-cli/src/main.rs`：`task.get` 调用在约 523、874、941 行；`dashboard.get` 在约 532、974 行；
  `project.list` 在约 327 行；「运行中」小节约 576 行；「最近 5 个会话」逐条费用在约 723–739 行
  （任务卡写的 `main.rs:720`）。
- `crates/autome-cli/src/render.rs`：`state_line`、`next_step`、`event_line`（约 196 行，约 358 行有
  `project.alert` 的文案「报警（…）：<kind> → autome alerts」）。
- `crates/autome-cli/tests/cli.rs`：`Sandbox` 驱动真实 `autome` 二进制；`json()` 辅助函数取 `--json` 输出。

### 6.3 配置

- `crates/autome-domain/src/config.rs` 约 239 行 `ProjectConfig`：字段全是 `Option` + `skip_serializing_if`，
  已有 `gate`、`setup`、`retry`、`bug`、`gate_same_failure_limit`。
- `crates/automed/src/config_io.rs`：`load_project`（约 109 行）手工从 `toml::Value` 解析，逐字段给中文报错
  （例：`[gate] same_failure_limit 必须是 1 到 10 的整数`）；`save_project`（约 464 行）写回。
- 本仓库自己的 `.autome/config.toml` 现有 `[loop]`、`[gate]`、`[retry]` 三节（只读参考，**不要改它**）。

### 6.4 警报与事件

- `store.rs` 约 1805 行：`raise_alert(project_id, kind, task_id: Option<&str>, detail) -> Result<String>`。
- 目前唯一的警报种类是 `main_gate_failed`（`scheduler.rs` 约 3286 行），随后写一条带
  `{alert_id, kind, task_id}` 的事件；另两处 `raise_alert` 调用在约 2959、3158 行，可作写法参照。
- 事件由 `store.append_event(kind, subject_id, payload)`（约 531 行）写入；现有 `task.*` 事件名见 `scheduler.rs`。
- e2e 里对警报的断言写法：`end_to_end.rs` 约 2690 行 `assert_eq!(alerts[0].kind, "main_gate_failed")`。

### 6.5 新警报种类的其它读者

- `apps/desktop/renderer/screens/dashboard.js` 约 143 行 `ALERT_TITLES = { main_gate_failed: … }`——
  渲染进程，约束 6 不许改；未知种类在那里显示成什么需设计轮核实。
- `apps/web/server/notifier-rules.ts` 约 31 行按 `payload.kind` 渲染通知文案——新种类/新事件是否会被通知器
  处理、处理成什么，需设计轮核实。
- `apps/web/server/gates/read-gate.ts` 约 15 行 `ALLOWED_READ_METHODS` 已含 `project.list`、`project.get`、
  `task.get`、`dashboard.get`。

### 6.6 留给设计轮的开放点（任务卡没有写死）

- `cost.sessions` 是计全部会话还是只计已结束的；运行中的会话尚无费用，算不算「无计费数据」。
- `by_role` 的键用什么（角色名）、每个角色下是只有金额还是也带会话数。
- `project.*` 的「未完成 + 已完成分开」的字段形状与「已完成」的边界（`Done` 之外的终态——失败、已停止、
  已归档——归哪一边）。
- `project.list` 对每个项目聚合全部任务的全部会话，是否需要一条 SQL 聚合而不是逐任务 `list_sessions`。
- 「超过」是严格大于；同一任务只告警一次靠什么去重（已有未处理警报、事件历史或新列），用户处理掉警报后
  是否再次告警；调高 `per_task_usd` 之后的行为。
- `per_task_usd` 的合法取值（正数、是否允许小数与整数两种 TOML 写法）与报错文案。
- `autome ls` 费用列在没有任何会话、或全部无计费数据时显示什么。

## 7. 项目背景摘要（2026-11-12 整理时）

**是什么。** Autome 2.0：把一句需求变成一个合并好的分支。内核 `automed`（Rust，持有全部状态）通过
JSON-RPC 被 CLI `autome`、Electron 桌面端与 Web 面板调用；会话是后台运行的 `claude` / `codex` CLI。
本仓库用自己的 Loop 开发自己。

**技术栈。** Rust 1.93.1（edition 2024，`rust-toolchain.toml` 固定，含 rustfmt、clippy）；
SQLite（`rusqlite` bundled）；`serde` / `serde_json` / `toml` / `toml_edit`。
`apps/desktop` 是 Electron（纯 `node --test`）；`apps/web` 是 Node 网关 + React Router 8（vitest、eslint、tsc）。

**布局。**

| 路径 | 负责什么 |
|---|---|
| `crates/autome-domain` | 纯域逻辑，无 I/O：状态机（`task.rs`）、配置类型（`config.rs`）、状态块解析、指标类型（`metrics.rs`、`session.rs`） |
| `crates/automed` | 内核：`store.rs`（SQLite 账本）、`dispatch.rs`（JSON-RPC 方法）、`scheduler.rs`（调度与会话回收）、`config_io.rs`、`usage.rs`、`task_metrics.rs`；`tests/end_to_end.rs` 是 e2e |
| `crates/autome-cli` | `autome` 命令行：`main.rs`（命令）、`render.rs`（文案）、`args.rs`、`client.rs`；`tests/cli.rs` |
| `apps/desktop` | Electron 外壳（本任务不改渲染进程） |
| `apps/web` | Web 面板与网关（`server/gates/read-gate.ts` 是 read 白名单） |
| `protocol/` | Loop 协议 seed，编译进 `automed`（本任务不改） |
| `plugins/` | Claude 插件（本任务不涉及） |
| `scripts/ci-local.sh` | 本地 CI，也是本项目的宿主门禁 |
| `docs/development/` | `requirements.md`、`technical-design.md`；`docs/adr/` 三份 ADR |

**测试命令。**

- 全量门禁：`sh scripts/ci-local.sh`（依次跑 `cargo fmt --all -- --check`、
  `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets -- --nocapture`、
  `npm test --prefix apps/desktop`、Contract 检查、`apps/web` 的 lint / typecheck / test / build；
  每步都跑完，任一失败则非零退出）。
- 单 crate：`cargo test -p automed`、`cargo test -p autome-cli`、`cargo test -p autome-domain`；
  只跑 e2e：`cargo test -p automed --test end_to_end`。
- 输出很长，按会话协议重定向到 `.autome/output/` 后只读头尾。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不改 `.autome/`；只在当前 worktree 工作，不切分支、不动 `.worktree/` 其它目录、不推送；
  完成后结束会话，不自行启动下一个；超过 5 行的命令输出与临时产物写 `.autome/output/`。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`、
  `timeout_minutes = 30`；`[retry] attempts = 2`、`backoff_seconds = 60`。
