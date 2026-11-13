# 单个会话的时长上限（`[loop] session_minutes`）：超时终止并按 `[retry]` 重派

status: 设计中
design-round: 1/15
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01：在 autome-domain 的 LoopDefaults / LoopOverrides / resolve / validate 加 session_minutes（默认 120，≥ 5），config_io 四处读写与 config.set_loop 带上它，README 配置样例加一行

任务文件：`docs/单个会话的时长上限-loop-session/单个会话的时长上限-loop-session-task.md`

任务卡：`docs/单个会话的时长上限-loop-session/单个会话的时长上限-loop-session-card.md`

下文 R-n、N-n、C-n、A-n 指任务文件第 3～6 节的编号；「开放问题 n」指任务文件第 7 节第 n 条。代码位置以符号名为准，行号只是 2026-11-13 读代码时的近似值。

## 背景与现状

内核今天怎么发现一个会话结束（`scheduler.rs` 的 `tick_locked` 第 1 步 → `reap_session`）：

- 读退出标记 `.autome/output/sessions/<task>/<session>.exit`、看 wrapper 的 pid 是否还活着、看日志多久没动，交给纯函数
  `autome_domain::session::classify` 得出 `SessionLifecycle`（`Running / Exited / Killed / Vanished`）。
- 不是 `Running` 就 `finish_session` → 记用量 → `sweep_commit`（把会话没提交的改动提交掉）→ 范围守卫 → `read_outcome`
  （非 `Exited{0}` 一律 `SessionOutcome::Crashed { detail }`）→ `deliver_outcome`。
- `deliver_outcome` 对 `Crashed` 先调 `schedule_retry`：流日志里认出「模型不存在」就按配置失败；认出限流就进冷却、不计次；
  否则 `retry_attempts < [retry] attempts` 时计一次、按 `backoff_seconds` 翻倍退避，把 `carried_inject`（从该会话的
  `session.started` 事件读回的原注入）写进 `session_retries` 一行，记 `session.retry` 事件；用尽则 `advance(SessionEnded{Crashed})`，
  状态机给出 `Failed { reason: SessionCrashed { detail } }`。Bug Loop 的 `deliver_bug_outcome` 走同一个 `schedule_retry`。
- 任务处于 `Paused` / `Requeued` 时，会话照跑，结果作为 `session.outcome_held` 事件挂起，恢复时原样交给 `deliver_outcome`。
- 唯一的兜底是 `classify` 里「pid 没了且日志 600 秒没动」判 `Vanished`（`VANISHED_AFTER_SECS`）。一个 pid 活着、一直在输出的会话
  永远是 `Running`。

人叫停（`dispatch.rs` 的 `stop_running_session`）：`launcher::stop_session(pid)` 对 `-pid` 发 SIGTERM、**固定睡 3000 毫秒**、
组还在就 SIGKILL；然后直接 `finish_session(Killed)`，不经过 `reap_session`。wrapper（`.autome/skill/run_session.sh`）是进程组
组长（`run_in_terminal` 里 `process_group(0)`），trap 了 INT/TERM/HUP，收到后写退出标记 `exit_code=130` 再退出；headless 模式下
由一个线程 `wait()` 它，不留僵尸。现有 e2e `stopping_a_task_kills_the_cli_the_wrapper_started` 断言叫停后 CLI 进程不在。

会话只在一处登记：`scheduler.rs` 里启动会话的函数 `ctx.store.insert_session(&Session { … started_at: now_iso(), … })`，
同一个函数里已经拿着该项目的 `resolved` 配置（读 `resolved.loop_defaults.design_rounds`）。所有**账本里的**会话类型——任务 Loop
各角色、任务整理、Bug 三步、复盘、手动复盘——都从这里进来。Onboarding 是例外：`start_onboarding` 直接调 `launcher::launch`，只追加
一条 `session.started` 事件，不登记会话行（所以 `all_running_sessions()` 看不到它，`reap_session` 也要求真实任务）；见方案 §3 的范围裁定。

会话的轮号 `round` 也在这里定：`launch_session` 调 `store.next_round(task, kind)`，它对同任务同类型所有会话取 `MAX(round)+1`；
实现轮的预算行（`implementation-round` 写 `k/N`）、rebase 冲突后的预算判断（`scheduler.rs` 里 `next > limit` 得 `ImplBudget`）都读它。
门禁失败 / rebase 冲突的注入文案要求「implementation-round 照常加一」。

调度没有自己的定时器以外的依赖：`autome run` 按间隔循环 `scheduler::tick`，桌面端与 web 服务端定时调 `scheduler.tick`。
所以一个一声不吭的会话也会被按时检查。

配置：`LoopDefaults { parallel, design_rounds, budget_factor }` 与稀疏的 `LoopOverrides`；`resolve()` 逐字段覆盖；
`validate()` 产出 `ConfigViolation`（`DesignRoundsOutOfRange` 等）；`config_io.rs` 有全局读、项目读、全局写、项目写四处
`[loop]` 代码；`dispatch.rs` 的 `config_set_loop` 按参数逐字段改、写前 `validate`；`describe_violation` 给出中文句子。

e2e 已有「改账本里的时间而不是真等」的先例：门禁排队超时用例把 `w.ctx.store` 换成盘上 SQLite（`Store::open(&db)`），再用
`rusqlite` 直接 `UPDATE events SET at = …` 往前拨（注释：Backdate real job clocks, never wait a minute in the suite）。

## 目标与非目标

**目标**（R-1～R-5，A-1～A-6）

1. `[loop] session_minutes`：全局与项目两级，默认 120，`config.validate` 要求 ≥ 5（R-1、A-5）。
2. tick 发现在跑的会话从 `started_at` 起超过它启动时的上限，就用人叫停的同一组信号终止整个进程组，**确认进程组已不存在之后**
   才记会话行为超时、才往下收割；记 `session.timeout { session_id, minutes, limit }`（R-2、C-4、A-3）。确认不了就不收割、不重派。
3. 超时按崩溃的同一套 `[retry]` 处理：计一次、退避、原节点重派，用尽才失败，失败理由写明超时；不是协议失败，不动任何轮数
   ——账本里的轮号也不动：超时的会话不占轮号，重派的会话沿用它的轮号（R-3、C-5、A-1）。
4. 重派的会话收到新的注入：上一个会话因超过 N 分钟被终止、最后 40 行日志、先把进度写进文档并缩小本轮目标；原来携带的注入
   一起带上，**并保持它原有的全部语义**（用户决定的领取、门禁修复轮的守卫豁免）（R-4、A-4）。
5. `autome status <任务>` 显示「已运行 37 分钟 / 上限 120」；`task.get`、`session.log` 的会话条目带 `timed_out: true`；
   `dashboard.get` 等你处理的失败理由里有「超时」（R-5）。
6. 上限之前结束的会话行为不变，现有 e2e 一行不改（C-3、A-2）。

**非目标**

- 按角色分别设上限（N-1）、按费用设上限（N-2）、改心跳兜底 `Vanished` 那条路径（N-3）。
- Onboarding 向导会话的时长上限：它没有会话行、没有任务节点、没有可重派的位置（见方案 §3 范围裁定），本任务不覆盖；进 Backlog。
- 生产里任何测试专用开关（C-1）：上限只有分钟、最小 5；测试用改账本时间的办法缩短（见方案 §8）。
- 桌面端与 web 界面的新展示：`apps/desktop/renderer/screens/task.js` 对未知生命周期显示原始 `state`（会显示 `timed_out`），
  `labels.js` 对未知违规显示原始 `kind`——都不坏，不在 R-5 清单里，本任务不改界面代码。失败理由仍是 `session_crashed`，
  桌面端与 web 通知器显示「会话异常退出（…超时）」，两处按名映射无需改动。
- 不改协议源 `protocol/`，也不改任务目录下的 `protocol/` 副本：注入文案按任务卡放在 `launcher.rs`。
- 不改 `.autome/` 下任何东西（含本仓库自己的 `.autome/config.toml`）。

## 方案

### 1. 配置（R-1、A-5；开放问题 11）

`autome-domain/src/config.rs`：

- `pub const DEFAULT_SESSION_MINUTES: u32 = 120;`、`pub const SESSION_MINUTES_MIN: u32 = 5;`
- `LoopDefaults` 加 `#[serde(default = "default_session_minutes")] pub session_minutes: u32`（`Default` 给 120）；
  `LoopOverrides` 加 `pub session_minutes: Option<u32>`（同另外三个字段的 serde 写法）；`resolve()` 照 `budget_factor` 覆盖。
- `ConfigViolation::SessionMinutesOutOfRange { value: u32, min: u32 }`；`validate()` 在 `value < SESSION_MINUTES_MIN` 时报。
  没有上界：上限是安全线，用户愿意给 600 分钟就给。

`automed`：

- `config_io.rs` 四处都带上 `session_minutes`：全局读、项目读（同 `design_rounds` 的解析写法）、全局写（`loop_table["session_minutes"]`）、
  项目写（同 `budget_factor` 的 `Option` 写法）。写路径必须带，否则桌面端保存设置会把手写值抹掉。
- `config_set_loop` 接受可选参数 `session_minutes`，全局 / 项目两支都照 `budget_factor` 处理；写前的 `validate` 自然拒绝 < 5。
  界面不发这个参数，但 IPC 开放它，CLI 与以后的界面可用。
- `describe_violation`：`会话时长上限 {value} 分钟太短，至少 {min} 分钟`。
- README「Host gate, retries and cooldowns」的 TOML 样例加 `[loop]` 表与一行
  `session_minutes = 120     # the default, at least 5; a session running longer is stopped and retried like a crash`。

### 2. 上限取哪一刻的值（开放问题 9、12）

README 写着「配置修改对下一个启动的会话生效，在跑的会话不被打断」。所以上限在**会话启动时**定下来，跟着会话走：

- `Session` 加 `#[serde(default, skip_serializing_if = "Option::is_none")] pub limit_minutes: Option<u32>`；
  `sessions` 表照 `protocol_ref` 的迁移写法加列 `limit_minutes INTEGER`，`insert_session` 写、读出时为 NULL 则 `None`。
- 唯一的登记点 `insert_session(&Session { … })` 填 `Some(resolved.loop_defaults.session_minutes)`。
- `automed` 里一个函数 `effective_limit(ctx, s, project) -> u32`：`s.limit_minutes` 有值用它；`None`（升级前启动的会话）
  用该项目当前生效配置；项目配置读不出来时用 `DEFAULT_SESSION_MINUTES`。tick 的判定与 `task.get` 的显示都只调这一个函数，
  不会出现「判定用一个数、显示另一个数」。
- 事件里的 `limit` 就是这个数。

### 3. 判定与终止（R-2、C-4、A-3；开放问题 1、7、8）

纯函数放 `autome-domain/src/session.rs`：

```rust
/// Whether a session that has run `elapsed_secs` has reached its limit.
pub fn over_limit(elapsed_secs: u64, limit_minutes: u32) -> bool {
    elapsed_secs >= u64::from(limit_minutes) * 60
}
```

`SessionLifecycle` 加一个变体：

```rust
/// `automed` stopped it because it ran past `[loop] session_minutes`.
TimedOut { limit_minutes: u32 },
```

序列化为 `{"state":"timed_out","limit_minutes":120}`；`is_running()` 为假，`should_parse_document()` 为假。账本里的旧行没有这个
值，照旧解析。选新变体而不是 `Killed` 加标记：A-1 要求「会话行是超时」，生命周期本身就说清楚，`timed_out: true` 直接由它派生，
不需要第二个来源。

`reap_session` 的改动（`scheduler.rs`），顺序是关键：

1. 照旧 `classify`。**已经有退出标记、或已判 `Vanished` 的会话照旧收割**——超过上限但自己已经结束的会话不算超时（C-3）。
   唯一例外：该会话已有 `session.timeout` 事件（上一次终止到一半内核退出了，wrapper 留下的是 130 标记）。这时它**不能直接按
   `TimedOut` 收割**：先对它的进程组再做一次「终止并确认」（第 3 步的 `terminate_group`，组已不在时立即返回）。确认了才按
   `TimedOut` 收割；确认不了（组里有进程在 SIGKILL 之后仍在）就 `return Ok(false)`，会话保持 `Running`，下一次 tick 再试。
   这次查询只发生在会话已有退出标记的那一次 tick，不是每个 tick。
2. `classify` 为 `Running` 时：`limit = effective_limit(…)`，`elapsed = secs_since(&s.started_at)`；读不出时间（`None`）或
   `!over_limit(elapsed, limit)` 就照旧 `return Ok(false)`。
3. 超限，三步，**只有第三步确认之后才算超时**：
   1. **取 pid。** `pid = s.pid`；为 `None` 时（终端启动慢于 `read_pid` 的 2 秒窗口，登记时没读到）再读一次 pid 文件
      （只读一次，不等），读到就用 `store.set_session_pid` 补写会话行。仍为 `None`：没有任何办法对进程组发信号，**不能假定它不存在**——
      记一次 `session.timeout_unconfirmed { session_id, reason: "no_pid" }`（已有就不重记），`return Ok(false)`，会话保持 `Running`；
      下一次 tick 重新尝试（pid 文件之后出现就会进入下一步）。wrapper 始终没起来的会话由既有的 `Vanished` 心跳兜底结束，N-3 不动。
   2. **记事件、发信号。** 记 `session.timeout { session_id, minutes: elapsed / 60, limit }`（已有就不重记），再
      `launcher::terminate_group(pid)`（见下「终止方式」）。事件先于信号：内核在两步之间退出时，下一次 tick 凭事件认得出它（第 1 步）。
   3. **确认。** `terminate_group` 返回 `false`（TERM、宽限、KILL 之后组仍存在）或出错：`return Ok(false)`，会话保持 `Running`，
      不 `finish_session`、不 `sweep_commit`、不重派——此刻还可能有进程在写工作树；下一次 tick 再发一轮信号。返回 `true` 才继续：
      生命周期定为 `TimedOut { limit_minutes: limit }`，`ended_at = now_iso()`。
4. 然后走**现有收割路径的剩余部分**，一行不改：`finish_session`、记用量、预算告警、`sweep_commit`、范围守卫、`read_outcome`、
   挂起或 `deliver_outcome`。`sweep_commit` 与 `Running` 之后的一切都建立在「组已确认不在」之上（第 1 步的恢复路径同理）。

`read_outcome` 与 `read_bug_outcome` 的 `match lifecycle` 加一支：
`TimedOut { limit_minutes } => format!("会话运行超过 {limit_minutes} 分钟，已被 Autome 终止（超时）")`。

**终止方式（C-4，开放问题 1）。** `launcher.rs` 把 `stop_session` 的内部抽成
`pub fn terminate_group(pid: i32) -> Result<bool>`，`stop_session` 变成它的薄包装（签名与对 `pid <= 1` 的拒绝都不变，人叫停的调用方
不动）。`terminate_group` 与今天同一组信号：对 `-pid` 发 SIGTERM；每 50 毫秒查一次 `kill(-pid, 0)`，组没了就返回 `true`；3 秒
到了还在才 SIGKILL，再每 50 毫秒查、至多再等 2 秒；仍在则返回 `false`。信号顺序与 3 秒宽限不变，把「固定睡 3 秒」换成「轮询」，
人叫停也因此变快。wrapper 收到 TERM 后几十毫秒内写完 130 标记退出，所以一次超时通常只让持锁的 tick 多花不到 0.1 秒；最坏
（进程忽略 TERM）5 秒。超时每个会话最多一次成功；`false` 的会话每个 tick 重试一轮信号，不会无声积累。
`stop_running_session` 里的门禁清理（`gate::stop_all`、`discard_node`）**不**用于超时：任务没有停，只是这一轮重来。

**覆盖哪些会话（开放问题 7）。** `all_running_sessions()` 里的会话一视同仁地判定与终止——也就是账本里登记过的所有会话；终止之后，
每种会话走它**崩溃时本来就走**的那条路，不新增分支：

| 会话 | 超时之后 |
|---|---|
| 任务 Loop 各角色、任务整理（`Active` 的会话节点） | `deliver_outcome` → `schedule_retry`，原节点重派 |
| Bug Loop 三步 | `deliver_bug_outcome` → 同一个 `schedule_retry` |
| 任务处于 `Paused` / `Requeued` | 结果挂起为 `session.outcome_held`；恢复时交给 `deliver_outcome`，再走重试 |
| 失败面板手动发起的复盘、终态任务上的复盘 | 照现有 `detached_retro` 分支：只记录，任务留在原终态 |
| Onboarding | **不覆盖**，见下 |

**Onboarding 的范围裁定。** `start_onboarding` 不调用 `insert_session`，向导会话不在账本里，`all_running_sessions()` 与 `reap_session`
都够不着它；`reap_session` 里现存的 `SessionKind::Onboarding` 分支对真实启动路径是不可达的，本设计不依赖它。任务文件开放问题 7 要求
给 Onboarding「确定行为」，这里的确定行为是：**本任务不给 Onboarding 加时长上限**。理由：它是用户在项目页主动发起、在可见终端里
盯着的交互式向导，运行在仓库根而不是任务 worktree，不占任务 Loop 的并行产能，也没有「原节点」可重派（R-3 的重试对它没有意义）；
R-2 的「每个在跑的会话」按账本里的会话理解。要补它需要给向导建会话登记与项目级归属，是另一件事，进 Backlog。

**起点（开放问题 8）。** 从 `started_at` 算。它比 wrapper 真正启动早一个终端拉起的空窗（毫秒到几秒），与 ≥ 5 分钟的上限相比
可以忽略，而且偏向「早停」而不是「漏停」。暂停 / 重新排队的任务若还有在跑的会话，照样计时——进程仍在占产能和花钱。
墙钟语义：电脑睡眠后醒来，睡眠时间也算进去；这是任务卡写明的「墙钟时长上限」，记入风险。

### 4. 接到崩溃重试上（R-3、C-5；开放问题 3、5、6）

超时会话在 `read_outcome` 里就是一个 `Crashed { detail }`，`deliver_outcome` 原样调 `schedule_retry`，所以「计一次重试、
退避、原节点重派、用尽才失败」都是现成的。`schedule_retry` 只多一件事：认出超时。

- 判据是该会话有没有 `session.timeout` 事件（`ctx.store.session_event(task, s.id, "session.timeout")`）。用事件而不是传参，是因为
  挂起后恢复的那条路（`outcome_held` → `deliver_outcome`）拿到的只有会话与结果；事件在账本里，内核重启后仍在。
- 超时时**跳过**「模型不存在」与「限流」两个流日志判定，直接走计次重试（开放问题 6）。理由：上限是失控会话的安全线；若一个在
  CLI 里反复撞限流的会话超时后被当成限流处理（不计次、立即重派），它可以无限循环，R-3「重试用尽才进入失败」就不成立。
  真正的限流会让 CLI 自己退出，那时会话没有超时，照旧进冷却。
- 计次、退避、`session.retry` 事件、`reason = detail` 都不变。用尽时 `schedule_retry` 返回 `false`，`advance` 给出
  `Failed { reason: SessionCrashed { detail: "会话运行超过 120 分钟，已被 Autome 终止（超时）" } }`。

**失败理由复用 `SessionCrashed`（开放问题 5）。** 不新增 `FailureReason` 变体：超时与崩溃在状态机里是同一个事件
（`SessionEnded { Crashed }`），新变体要改 `task.rs` / `bug_loop.rs` 两张迁移表、`render.rs`、`labels.js`、`notifier-rules.ts`，
换来的只是一个机器可读的名字；而 R-5 要的是「能看出是超时」，`detail` 里的「（超时）」在 CLI（`会话崩溃：…（超时）`）、
桌面端与 web 通知器（`会话异常退出（…（超时））`）、`dashboard.get` 的 `task.state.reason.detail` 里都原样可见。

**不是协议失败，不动轮数（C-5）。** 超时结果是 `Crashed`，不解析文档、不进守卫的协议判定，不会产生 `task.protocol_failure`；
`schedule_retry` 不碰任何预算字段；`fix_rounds` 也只在 Bug 复核结果上计，不在崩溃上计。范围守卫（审计 / 复盘越界改文件）照旧
先于超时生效——越界是另一个事实，今天崩溃时也是这样。

**账本里的轮号：物理会话次数 ≠ 逻辑轮次。** 上一段只说了内核没有显式「加轮数」的代码；但 `launch_session` 的
`store.next_round` 对同角色所有会话取 `MAX(round)+1`，重派的会话会因此占下一个轮号，实现预算行（`k/N`）和 rebase 冲突后的
`next > limit → ImplBudget` 判断都读它——例如 N = 3，第 1 轮实现超时、重派登记为第 2 轮，之后连续两次 rebase 冲突就会在本该还有
一轮时耗尽预算。所以规则定为：**超时终止的会话不占轮号，重派的会话沿用它的轮号。** 落点只有一处：`next_round` 的查询排除
生命周期为 `timed_out` 的会话行（`WHERE … AND json_extract(lifecycle, '$.state') IS NOT 'timed_out'`，或等价写法）。于是被超时的
那一轮是第 k 轮，重派是同一个第 k 轮，预算行写同样的 `k/N`，rebase 预算判断不受影响；`sweep_commit` 的提交信息里的 `#k` 也自然相同。
`next_round` 的所有调用点（`launch_session` 的轮号与预算行、rebase 冲突预算判断、其余读者）一并受益，不逐点打补丁。
崩溃（非超时）重派的轮号行为不变，C-3。Bug Loop 不依赖 `next_round` 记预算（`fix_round` 由 `bug.fix_round_started` 事件按 requested /
started 配对计数，重派不会再产生 requested），不受影响。

文档侧的状态块轮数是会话自己写的，内核只能管账本；两侧的一致靠两件事：实现轮的 `implementation-round` 以预算行给的 `k/N` 为准
（重派时 `k` 与被中断的那一轮相同），以及 §5 的注入文案把原注入里「implementation-round 照常加一」改写为「以开头给出的数为准」。
设计轮的 `design-round` 没有账本侧的数，靠文案「已加过则保持」，由 H-01 人工验收兜底。

**没提交的改动（开放问题 3）。** 照走 `sweep_commit`：进程组已经没了，没有并发写者，工作区里的半成品被提交进任务分支。下一轮
的注入因此可以说「改动已提交，从那里接着做」。

### 5. 注入（R-4、A-4；开放问题 4、10）

`autome-domain/src/task.rs` 的 `Inject` 加一个变体：

```rust
/// The previous session ran past `[loop] session_minutes` and was stopped.
SessionTimedOut {
    minutes: u32,
    /// The last lines of its readable log, verbatim.
    tail: String,
    /// What that session itself was carrying — a gate failure, a rebase
    /// conflict, the user's decisions — so the retry does not lose it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    carried: Option<Box<Inject>>,
},
```

并配一个构造函数 `Inject::timed_out(minutes, tail, carried: Option<Inject>) -> Inject`：`carried` 本身是 `SessionTimedOut` 时取
它里面的 `carried`，**不嵌套**——连续两次超时，第二次的注入仍只有一段超时说明加最初的那份注入。`session_retries.inject` 仍是一个
`Option<Inject>`，表结构不变，旧行照旧解析（开放问题 4）。

**包装之后原注入的语义不能丢。** `Inject` 有按**顶层变体**判定行为的消费者，包装会让它们看不见里面的原注入。核对过全部读者
（`rg 'Inject::|spec\.inject'`，生产代码里共三处，其余是测试与状态机产出注入的位置）：

| 读者 | 位置 | 按顶层变体做什么 | 处理 |
|---|---|---|---|
| 决定领取 | `scheduler.rs` `start_session` 的 `matches!(inject, Some(Decisions \| DesignFeedback{..}))` → `carry_decisions` | 只有带用户决定的轮才领取并交给 prompt | 改读 `inject.and_then(Inject::effective)` |
| 修复轮守卫豁免 | `scheduler.rs` 里 `repairing: carried_inject(…).is_some_and(GateFailed)`，决定 `guards.rs` 的 `impl_had_no_target` 是否豁免 | 顶层是 `GateFailed` 才豁免 | 改读 `.and_then(|i| i.effective()).is_some_and(GateFailed)` |
| prompt 渲染 | `launcher.rs` `build_prompt` 的 `match spec.inject`，含 `render_decisions(spec.decisions)` | 每种变体一段文案 | `render_inject` 对 `SessionTimedOut` 先写超时说明、再对 `effective` 的内层递归渲染 |

（`hands_decisions_to_a_session` 匹配的是状态机产出的 `Action::StartRole`，重派不经过它，不受影响。）统一入口是 `autome-domain` 的
`impl Inject { pub fn effective(&self) -> Option<&Inject> }`：非包装变体返回自身；`SessionTimedOut` 返回 `carried.as_deref()`
（`timed_out` 构造保证不嵌套，所以一层即可）。今后任何读 `Inject` 变体语义的新代码都经过它，不直接 `match` 顶层。决定的往返：
超时的会话带着 `Decisions`，崩溃路径本来就把它领的决定放回（`release_decisions`），重派时 `effective` 认出 `Decisions` →
再次 `carry_decisions` → prompt 里是决定正文，不是空列表。

`schedule_retry` 在超时时：`tail = gate::tail(&repo.join(SessionPaths::log(task, session)), 40)`（可读日志，不是 `.jsonl`
流日志；`gate::tail` 已按 6000 字节封顶），`inject = Inject::timed_out(limit, tail, carried_inject(…))`，写进
`session_retries`——随重试持久化，内核重启后仍在（开放问题 10）。

`launcher.rs`：把 `match spec.inject { … }` 抽成 `fn render_inject(p: &mut String, spec: &PromptSpec, inject: &Inject)`，
现有分支逐字不变；新分支先写超时说明，再对 `carried`（即 `effective`）渲染一次，且渲染内层时把输出片段里的
「implementation-round 照常加一」替换为「implementation-round 以本 prompt 开头给出的数为准（本轮是被中断那一轮的续做，不再加一）」
（与现有快检措辞替换同一写法：渲染进临时 `String` 后 `replace`）。这样超时说明和原注入不会对轮数给出相反的要求。

**Intake 也要走这里。** `build_prompt` 里 `SessionKind::Intake` 和 `Onboarding` 现在是提前 `return`，绕过 `match spec.inject`，任务整理
重派时超时说明会静默丢失。改为：`Intake` 取 `intake_prompt(spec)` 的结果作为 `p`，然后只执行 `render_inject`（不插「必须使用的技能」，
与今天 `Intake` 不带技能行一致），再返回；`inject` 为 `None` 时输出与今天逐字节相同（C-3）。`Onboarding` 保持提前返回——它不会被重派
（见 §3 范围裁定）。`Bug` 与 `Role` 本来就走到注入段，不改。文案（中文，`{minutes}`、`{tail}` 为实参）：

```text
上一个会话运行超过 {minutes} 分钟，被 Autome 终止（超时）。它留在工作区里的改动已由 Autome 提交到本分支。日志最后 40 行（原文）：

（围栏包住的 {tail}）

本轮先把已有进度写进文档（设计文档、证据文件、Bug 文档或任务整理本该写的那份文档里本轮该写的地方）并提交，再缩小本轮目标：
只做能在上限内完成的部分，其余写进 next-action 留给下一轮。本轮是被中断那一轮的续做，不是新的一轮：状态块里的轮数——
实现轮以本 prompt 开头给出的 `implementation-round` 为准，设计轮的 design-round 如果上一个会话已经加过一，保持现值——不要再加。
```

最后一句是 C-5 在文档侧的那一半：实现轮的数由 Autome 给定（上一节账本里的轮号），设计轮的数写在会话自己维护的状态块里，被中断的会话
可能已经加过。

金样：`crates/automed/tests/golden/prompts/impl-timeout.md`（实现轮、无携带注入）、`impl-timeout-gate.md`（携带 `GateFailed`，
并钉住「照常加一」已被替换）与 `intake-timeout.md`（任务整理，钉住 Intake 不再绕过注入段且任务卡 / 附件内容仍在），由
`prompt_rendering.rs` 新增的三个用例逐字节对比，`tests/golden/README.md` 补一句说明。

### 6. 可见（R-5；开放问题 12）

- `dispatch.rs` 的 `session_json` 加 `"timed_out": matches!(lifecycle, TimedOut{..})` 与 `"limit_minutes"`。后者与运行中会话
  的 `"elapsed_minutes"`（`secs_since(started_at) / 60`）需要账本与时钟，所以 `session_json` 改为接收 `ctx` / 项目，或由调用方
  在 `session_json(s)` 的结果上补字段——实现轮二选一，但三处调用（`task.get`、`session.log`、`dashboard.get` 的 running 条目）
  必须都带上，且 `limit_minutes` 只经 `effective_limit` 得出。
- `autome status <任务>`（`autome-cli/src/main.rs` 的 `task_detail`「会话（最近 5 个）」）：运行中的行把「运行中」换成
  `运行中 · 已运行 {elapsed_minutes} 分钟 / 上限 {limit_minutes}`；`timed_out` 为真的行加 `超时（上限 {limit_minutes} 分钟）`。
  CLI 只读字段、拼字符串，不读配置、不算时间（C-6）。
- `autome-cli/src/render.rs` 的事件行加 `session.timeout`：`{subject} 会话运行 {minutes} 分钟，超过上限 {limit}，已终止`。
- `dashboard.get`：失败任务在 `waiting` 里，`task.state.reason` 为 `{kind: session_crashed, detail: "…（超时）"}`，无需改代码，
  只加测试钉住。
- `docs/development/technical-design.md`：§7.3 的生命周期列表加 `TimedOut`，§17 的兜底一节加一段「时长上限」，各几行，不是验收项。

### 7. 分层（C-6）

| 层 | 本任务放什么 |
|---|---|
| `autome-domain` | `session_minutes` 字段、默认值、校验与违规；`SessionLifecycle::TimedOut`；`over_limit`；`Inject::SessionTimedOut`、`Inject::timed_out` 与 `Inject::effective` |
| `automed` | 配置读写、`limit_minutes` 列与 `set_session_pid`、`effective_limit`、`next_round` 排除超时会话、tick 里的判定与 `terminate_group`、事件、`schedule_retry` 的超时分支、决定领取与修复守卫改读 `effective`、日志尾、注入文案（含 Intake）、JSON 字段 |
| `autome-cli` | 读字段、打印 |

### 8. 测试怎么把上限缩短（C-1、C-2、C-7）

不给生产留任何开关：上限只有 `session_minutes`，最小 5。e2e 把**账本里会话的 `started_at` 往前拨**：照门禁排队超时用例，
`w.ctx.store = Store::open(&db)` 换成盘上账本，会话起来后用 `rusqlite` 执行
`UPDATE sessions SET started_at = strftime('%Y-%m-%dT%H:%M:%SZ','now','-6 minutes') WHERE id = ?1`，然后 tick。内核看到的就是一个
真实跑了 6 分钟的会话，判定、终止、重试走的全是生产代码。任务卡认可的「秒做内部单位」不需要了——那仍要在测试里配出一个低于
5 分钟的值，等于另开一个口子。

真实的 git worktree、真实 wrapper、真实进程组，只替换模型 CLI（`World` + `FAKE_CLI` + `.autome/fake/<n>.sh`），满足 C-7。
永不退出的假 CLI 步骤：`echo $$ > cli.pid; sleep 1000 & echo $! > child.pid; i=0; while :; do i=$((i+1)); echo "tick $i"; sleep 1; done`。

A-1 整条（两次超时、一次重试、失败）的耗时：两次 TERM 后各几十毫秒、`backoff_seconds = 0`、每次 tick 毫秒级，预计 5 秒以内；
用例自己用 `Instant` 断言 < 30 秒（C-2）。

## 风险与验证安排

- **设计轮数在文档侧被加两次（C-5 的文档一半）。** 实现轮的数由账本给定、可测（§4）；`design-round` 没有账本侧的数，被中断的会话
  是否已改状态块、重派的会话是否照注入不再加，取决于模型。缓解：注入文案明说；验证：人工验收 H-01。
- **终止未确认。** 进程组在 SIGKILL 后 2 秒仍在（几乎只会是尚未被收的僵尸组长，或不可中断睡眠的进程）：会话保持 `Running`，不收割、不
  重派，每个 tick 再发一轮信号，`session.timeout` 事件已在；组一旦消失就按超时收割。代价是这类会话在被终止之前一直占着产能，比「假定它
  没了」安全。无法取到 pid 的会话同理（`session.timeout_unconfirmed`），由 `Vanished` 心跳兜底。
- **`stop_session` 改为轮询后的行为差异。** 风险在「组判空」：已退出但未被收的进程会让 `kill(-pid, 0)` 仍成功，于是等满 3 秒再
  SIGKILL，再等至多 2 秒——人叫停与今天一样慢（不会更糟），超时路径则可能走到「终止未确认」。headless 下 wrapper 有专门的 `wait()`
  线程、其余进程由 launchd 收养回收。验证：M-02 的 launcher 单测分别覆盖「TERM 后立即返回」与「忽略 TERM 时升级 SIGKILL」；现有
  `stopping_a_task_*` 两个 e2e 回归。
- **tick 持锁时间。** 每次超时最多 5 秒（进程忽略 TERM 时）；未确认的会话每个 tick 再付一次。接受。
- **墙钟与睡眠。** 笔记本睡眠两小时醒来，在跑的会话会在下一次 tick 被判超时并重派一次。这是「墙钟上限」的直接含义，重派计一次
  重试；接受，在技术设计 §17 写一句。
- **升级时正在跑的旧会话。** `limit_minutes` 为 `None`，按当前配置（默认 120）判定；一个升级前已跑了三小时的会话会在升级后第一次
  tick 被终止并重派。接受：这正是要拦的会话。
- **超时说明与原注入同时出现时 prompt 变长。** `tail` 6000 字节封顶，`carried` 不嵌套，最多一段超时说明加一份原注入。
- **桌面端显示原始 `timed_out` 字样。** 不坏，不在范围，见非目标。

各里程碑的验收只跑能区分对错的子集；现有 e2e 是否「一行不改」用 `git diff` 的删除行数核对（C-3）；全量回归由宿主在合并前跑
`sh scripts/ci-local.sh`（含 `cargo fmt --check`、`clippy -D warnings`，A-6）。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | session_minutes 配置：默认、覆盖、校验、读写与 README | 0 | |
| M-02 | 开放 | 超时判定与终止：TimedOut 生命周期、启动时上限、进程组终止 | 0 | |
| M-03 | 开放 | 超时走重试：跳过限流判定、超时注入与金样、用尽后失败 | 0 | |
| M-04 | 开放 | 可见：会话条目、autome status、事件行与技术设计 | 0 | |

## 里程碑说明

- **M-01**（覆盖：R-1、A-5、开放问题 11；C-6。前置：无）。方案 §1。新增用例名都含 `session_minutes`：
  - autome-domain 3 个：① 默认 120、项目覆盖胜出、未覆盖时继承全局；② `1` 与 `4` 各产生 `SessionMinutesOutOfRange { value, min: 5 }`；
    ③ `5` 与缺省值校验无违规。
  - automed lib 5 个：config_io ④ 全局与项目都能读出；⑤ `save_global` 保留手写的 `session_minutes`；⑥ `save_project` 保留项目覆盖；
    dispatch ⑦ `config.validate` 对项目 `session_minutes = 1` 返回 `ok: false` 且违规 kind 为 `session_minutes_out_of_range`，
    缺省配置 `ok: true`；⑧ `config.set_loop` 写入 `session_minutes = 30` 后 `config.get` 读回 30，写 `4` 被拒绝且文件不变。
- **M-02**（覆盖：R-2、A-3、C-1、C-3、C-4、C-7、开放问题 1、2、7、8、9。前置：M-01）。方案 §2、§3。`stop_session` 抽出 `terminate_group` 并改轮询；
  `read_outcome` / `read_bug_outcome` 加 `TimedOut` 文案；`set_session_pid`、终止确认与恢复路径；本里程碑结束时超时会话已经能经由
  现有 `schedule_retry` 被重派（带原注入、尚无超时说明；轮号处理在 M-03）。Onboarding 不在范围（§3）。新增用例名都含
  `session_timeout`：
  - autome-domain 2 个：① `over_limit` 在 `limit*60` 为真、`limit*60 - 1` 为假；② `TimedOut` 序列化为
    `{"state":"timed_out","limit_minutes":5}` 且能读回，`is_running` 与 `should_parse_document` 均为假。
  - automed lib 4 个：③ store：`limit_minutes` 写入读回，迁移前的旧行（列为 NULL）读成 `None`，`set_session_pid` 补写后读回；
    ④ launcher：一个自成进程组、带后台子进程的 `sh`，`terminate_group` 在 1 秒内返回 `true`，组长与子进程都不在；⑤ launcher：组长
    `trap '' TERM` 时，`terminate_group` 返回 `true` 且组已不在（升级 SIGKILL）；对已不存在的组返回 `true`。④⑤ 照 `run_in_terminal`
    用线程 `wait()` 组长，免得僵尸让断言失真。⑨ scheduler（恢复路径）：会话已有 `session.timeout` 事件与 130 退出标记，但 `s.pid`
    指向一个自成进程组、`trap '' TERM` 的残留进程；`reap_session` 返回后该进程组已不在，生命周期为 `timed_out`（对照：残留进程杀不掉时
    不收割——不单测，靠 `terminate_group` 返回值分支的代码审查）。
  - e2e 4 个（盘上账本 + 拨 `started_at`）：⑥ `session_minutes = 5`、`[retry] attempts = 1, backoff_seconds = 3600`，永不退出的步骤，
    拨到 6 分钟前后 tick 一次：`session.timeout` 事件 `{session_id, minutes: 6, limit: 5}`、会话生命周期 `timed_out`、任务仍是同一
    节点、`pending_retry` 存在、wrapper / CLI / 后台子进程三者在 10 秒内都不在且 `kill(-wrapper, 0) != 0`（A-3，照
    `stopping_a_task_kills_the_cli_the_wrapper_started`）；⑦ 步骤立即 `exit 0` 写好文档，等退出标记出现后再拨时间、再 tick：
    照常收割，没有 `session.timeout` 事件，生命周期 `exited`；⑧ 以 `session_minutes = 5` 启动后把配置改成 120 并提交，拨到 6 分钟
    前：仍被终止，事件 `limit` 为 5；⑩ 取不到 pid：账本里把会话的 `pid` 置 NULL 并临时挪走 pid 文件，拨到 6 分钟前 tick：会话仍
    `running`，有 `session.timeout_unconfirmed`、没有 `session.timeout`、进程仍在；把 pid 文件放回再 tick：补写 pid、被终止、生命周期
    `timed_out`、`pending_retry` 存在。
- **M-03**（覆盖：R-3、R-4、A-1、A-4、C-2、C-5、开放问题 3、4、5、6、10。前置：M-02）。方案 §4、§5。`next_round` 排除超时会话；
  `Inject::SessionTimedOut` / `timed_out` / `effective`；决定领取与修复守卫改读 `effective`；`render_inject`（含 Intake）。新增用例名
  都含 `timeout_retry`：
  - autome-domain 2 个：① `Inject::timed_out` 携带 `GateFailed` 时原样保留；携带一个 `SessionTimedOut` 时取其内层，不嵌套；JSON 读回相等；
    ② `effective()`：`SessionTimedOut{carried: Decisions}` 得 `Decisions`，`carried: None` 得 `None`，非包装变体得自身。
  - automed lib 6 个：③ scheduler：超时会话的 `.jsonl` 里有一条限流记录，`schedule_retry` 仍计次（`session.retry` 的
    `rate_limited: false`、`attempt: 1`），不写冷却；④ scheduler：重试已用尽的超时会话让任务 `Failed`，理由 `session_crashed`，
    `detail` 含「超时」与上限分钟数，且没有 `task.protocol_failure` 事件；⑤ launcher：带 `carried: GateFailed` 的注入先渲染超时
    说明、后渲染门禁失败段，两段各出现一次，且输出里不再有「implementation-round 照常加一」；⑥ scheduler（轮号）：实现预算 N = 3，
    实现轮第 1 轮超时后重派，新会话行 `round == 1`、prompt 含「实现轮第 1 轮」，`next_round` 仍为 2；超时之后的 rebase 冲突预算判断按
    未消耗的预算计（第 1 次冲突后的修复轮仍是第 2 轮，未触发 `ImplBudget`）；⑦ scheduler（决定）：带 `Decisions` 的会话超时后重派，
    新会话 `carry_decisions` 非空、prompt 含决定正文；⑧ scheduler（修复守卫）：`session.started` 事件的注入为
    `SessionTimedOut{carried: GateFailed}` 的会话，`repairing` 判定为真（抽成可测的小函数后断言；对照：顶层 `GateFailed` 为真、
    `SessionTimedOut{carried: None}` 为假）。
  - prompt_rendering 3 个：⑨ `impl-timeout.md`、⑩ `impl-timeout-gate.md`、⑪ `intake-timeout.md` 金样逐字节相等；Intake 且无注入的既有
    金样不变。
  - e2e 2 个：⑫ A-1 全程：`session_minutes = 5`、`attempts = 1`、`backoff_seconds = 0`，步骤 1、2 都永不退出（每秒一行
    `tick N`）；每有一个在跑的会话就拨时间再 tick；结束时 `sessions_run == ["1","2"]`、两条 `session.timeout`、一条 `session.retry`、
    任务 `failed` 且 `reason.detail` 含「超时」、第 2 个 prompt 含「超过 5 分钟」与 `tick` 行、没有 `task.protocol_failure` 事件、
    整条用时 < 30 秒；⑬ 会话在跑时 `task.pause`，拨时间后 tick：进程被终止、结果挂起；`task.resume` 后重派，第 2 个 prompt 含超时说明。
- **M-04**（覆盖：R-5、开放问题 12、13；C-6。前置：M-03）。方案 §6。新增用例名都含 `timeout_visible`：
  - automed lib 3 个：① `task.get` 与 `session.log` 的超时会话条目 `timed_out: true`、`limit_minutes: 5`，正常结束的会话
    `timed_out: false`；② 运行中会话（`started_at` 为 37 分钟前、`limit_minutes` 120）的条目 `elapsed_minutes: 37`、`limit_minutes: 120`，
    `limit_minutes` 为 `None` 的旧会话按项目当前配置给出；③ `dashboard.get` 的 `waiting` 里失败任务的 `reason.detail` 含「超时」。
  - autome-cli 3 个：④ render：`session.timeout` 事件行；⑤ cli 集成：`autome status <任务>` 输出含「已运行 37 分钟 / 上限 120」；
    ⑥ cli 集成：超时会话的行含「超时（上限 5 分钟）」。
  - `technical-design.md` §7.3、§17 各补几行（不计入验收）。

## 验收契约

### M-01 验收契约

- 验收命令：`cargo test -p autome-domain session_minutes && cargo test -p automed --lib session_minutes && grep -c 'session_minutes' README.md`
- 预期：autome-domain 过滤 `session_minutes` 共 3 个通过（基线 0，新增 ①②③）；automed lib 过滤 `session_minutes` 共 5 个通过（基线 0，新增 ④～⑧）；README 计数 ≥ 1。用例总数 8。
- 负向对照：`validate` 照 `design_rounds` 写成 `== 0` 时 ② 失败（`1`、`4` 没有违规）；`save_global` 不写 `session_minutes` 时 ⑤ 失败（手写的 30 被写回 120）；`resolve` 忽略项目覆盖时 ① 失败。

### M-02 验收契约

- 验收命令：`cargo test -p autome-domain session_timeout && cargo test -p automed --lib session_timeout && cargo test -p automed --test end_to_end session_timeout && cargo test -p automed --test end_to_end -- stopping_a_task a_non_zero_exit model_not_found a_rate_limited_round && test "$(git diff "$(git merge-base HEAD main)" -- crates/automed/tests/end_to_end.rs | grep -c '^-[^-]')" = 0`
- 预期：autome-domain 过滤 `session_timeout` 2 个通过；automed lib 过滤 `session_timeout` 4 个通过；e2e 过滤 `session_timeout` 4 个通过（三者基线均 0，新增 ①～⑩ 共 10 个）；现有 e2e 回归子集 5 个通过（`stopping_a_task_*` 2、`a_non_zero_exit_*` 1、`model_not_found_*` 1、`a_rate_limited_round_*` 1）；最后一条 `test` 成立（相对任务分支与 main 的分叉点，`end_to_end.rs` 删改行数为 0，即现有 e2e 一行未改）。用例总数 15。
- 负向对照：判定放在 `classify` 之前（先看时间再看退出标记）时 ⑦ 失败（已退出的会话被记成超时）；每个 tick 读当前配置而不是 `limit_minutes` 时 ⑧ 失败（不被终止）；只 `kill(pid)` 不 `kill(-pid)` 时 ⑥ 失败（后台子进程还在）；`stop_session` 保留固定 3 秒睡眠时 ④ 失败（返回超过 1 秒）；`pid` 为 `None` 时跳过终止、照样收割时 ⑩ 失败（进程仍在却已记超时）；恢复路径凭 `session.timeout` 事件直接按 `TimedOut` 收割、不再终止进程组时 ⑨ 失败（残留进程还在）。

### M-03 验收契约

- 验收命令：`cargo test -p autome-domain timeout_retry && cargo test -p automed --lib timeout_retry && cargo test -p automed --test prompt_rendering timeout_retry && cargo test -p automed --test end_to_end timeout_retry && cargo test -p automed --test end_to_end -- paused_running_session paused_then_stopped a_rate_limited_round a_non_zero_exit model_not_found && test "$(git diff "$(git merge-base HEAD main)" -- crates/automed/tests/end_to_end.rs | grep -c '^-[^-]')" = 0`
- 预期：autome-domain 2 个、automed lib 6 个、prompt_rendering 3 个、e2e 2 个过滤 `timeout_retry` 通过（基线均 0，新增 ①～⑬ 共 13 个）；现有 e2e 回归子集 5 个通过；最后一条 `test` 成立（`end_to_end.rs` 删改行数为 0）。用例总数 18。⑫ 的用例内断言整条用时 < 30 秒。
- 负向对照：`schedule_retry` 不认超时、照旧先做限流判定时 ③ 失败（`rate_limited: true`、写了冷却）；注入只放超时说明、丢掉 `carried` 时 ⑤ 与 ⑩ 失败；超时判据用传参而不是 `session.timeout` 事件时 ⑬ 失败（恢复后挂起结果走重试，第 2 个 prompt 没有超时说明）；`Inject::timed_out` 不展开内层时 ① 失败；`next_round` 不排除超时会话时 ⑥ 失败（重派占了第 2 轮）；决定领取或修复守卫仍读顶层变体时 ⑦、⑧ 失败；`build_prompt` 的 Intake 仍提前返回时 ⑪ 失败；渲染不替换「照常加一」时 ⑤、⑩ 失败。

### M-04 验收契约

- 验收命令：`cargo test -p automed --lib timeout_visible && cargo test -p autome-cli timeout_visible`
- 预期：automed lib 过滤 `timeout_visible` 3 个通过；autome-cli 过滤 `timeout_visible` 3 个通过（render 单测 1、cli 集成 2）；基线均 0。用例总数 6。
- 负向对照：`session_json` 只给 `task.get` 补字段、漏了 `session.log` 时 ① 失败；`limit_minutes` 不经 `effective_limit`、旧会话给出 `null` 时 ② 失败；CLI 自己读配置算上限（或不读 `elapsed_minutes`）时 ⑤ 的「已运行 37 分钟 / 上限 120」对不上。

## 人工验收清单

- H-01 合并前，在一个测试项目的 `.autome/config.toml` 写 `[loop] session_minutes = 5`，用真实模型跑一个实现轮会超过 5 分钟的任务（要花钱）：会话在约 5 分钟时被终止，`autome status <任务>` 运行中显示「已运行 N 分钟 / 上限 5」、结束后该行显示「超时」；重派的会话 prompt 里有超时说明与日志尾；重派会话结束后设计文档的 implementation-round 只比超时前多一。
- H-02 在桌面端打开 H-01 那个任务：会话列表里超时那一行显示 `timed_out`（原始字样，本任务不加中文标签），没有报错或空白卡片。

## Backlog

- D1-B01 Onboarding 向导会话没有时长上限：它不在会话账本里（`start_onboarding` 不登记会话行），要补需要给向导建登记与项目级归属、定义超时后「停下并提示用户」而非重派。本任务不做（见方案 §3 范围裁定）。

## 争议项
