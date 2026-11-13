# 任务：单个会话的时长上限 `[loop] session_minutes`——超时终止并按 `[retry]` 重派

本任务的规则见 `docs/单个会话的时长上限-loop-session/protocol/loop-protocol.md` 与
`docs/单个会话的时长上限-loop-session/protocol/session-protocol.md`。

- 设计文档：`docs/单个会话的时长上限-loop-session/单个会话的时长上限-loop-session.md`
- 任务卡（用户原文，第一输入）：`docs/单个会话的时长上限-loop-session/单个会话的时长上限-loop-session-card.md`
- 需求原文：单个会话的时长上限 [loop] session_minutes：超时就终止会话并按 [retry] 重派，下一轮注入超时说明

任务卡与本文件不一致时，以任务卡正文为准。本文件第 3～6 节是对任务卡的逐条整理（未扩大范围）；
标注「整理轮推断」的条目不是任务卡原文，设计轮可以凭证据推翻。

## 1. 为什么做

内核对会话只有一种兜底：日志长时间不再更新就判定会话消失（心跳兜底）。一个**一直有输出**的
会话可以无限跑下去——实现轮反复跑测试、反复改同一处、或者 CLI 在工具调用里打转。预算
（`budget_factor`、`fix_rounds`）只限制轮数，不限制一轮有多长。宿主门禁有
`[gate] timeout_minutes`，会话没有对应的东西。

现场：2026-11-12 的 T-35（W5）实现第 7 轮从 16:12 跑到 17:14 才结束。并行 3 个任务时，
一个失控会话占掉三分之一产能，费用还一直在涨。靠 Loop 自主推进项目的产品需要这条安全线。

## 2. 目标

给每个会话一个墙钟时长上限 `[loop] session_minutes`。在跑的会话超过上限时，内核终止它
（整个进程组），按会话崩溃的同一套 `[retry]` 策略在原节点重派；重派的会话在 prompt 里被告知
上一个会话是因为超时被终止的。

## 3. 范围（要做）

编号供评审、裁决、审计追溯。

- **R-1 配置。** `[loop] session_minutes`，全局默认值与项目覆盖项都要有，照 `parallel` /
  `design_rounds` 在 `LoopDefaults` / `LoopOverrides` 里的写法。默认 120。`config.validate`
  要求 ≥ 5。
- **R-2 检查与终止。** tick 里检查每个在跑的会话：`started_at` 到现在超过上限，就用和人叫停
  一样的方式结束它（整个进程组），记事件 `session.timeout { session_id, minutes, limit }`。
- **R-3 结果处理。** 和会话崩溃一样走 `[retry]`：计一次重试，退避后在原节点重新派一个会话；
  重试用尽才进入失败。**不**算作协议失败，也**不**消耗轮数预算。
- **R-4 注入。** 下一个会话的 prompt 里注入一段说明——新的 `Inject` 分支，文案放
  `launcher.rs`，加金样测试：上一个会话因为超过 N 分钟被终止，附最后 40 行日志，要求这一轮
  先把进度写进文档、缩小本轮目标。
- **R-5 可见。**
  - `autome status <任务>` 的会话行显示「已运行 37 分钟 / 上限 120」；
  - `session.log`、`task.get` 的会话条目带 `timed_out: true`；
  - `dashboard.get` 等你处理的「失败」理由里能看出是超时。

## 4. 不做什么（任务卡明确排除）

- **N-1** 按角色（设计 / 实现 / 审计）分别设上限。
- **N-2** 按费用设上限（等 T-44 把费用汇总做出来之后另提）。
- **N-3** 改变心跳兜底的行为（`log_idle_secs` / `VANISHED_AFTER_SECS` 那条路径保持原样）。

整理轮推断的范围边界（非任务卡原文）：

- `apps/desktop`、`apps/web` 的界面不在 R-5 的可见清单里，不是必做项。若新的事件、会话字段或
  失败理由破坏了它们现有的测试（两处对失败理由做了按名映射：
  `apps/desktop/renderer/lib/labels.js`、`apps/web/server/notifier-rules.ts`），只做让其不坏的
  最小改动。
- 注入文案按任务卡放在 `launcher.rs`，预计不需要改协议源 `protocol/`；若设计认为必须改，
  须在设计文档里单独论证。任务目录下的 `protocol/` 副本任何情况下都不改。
- `.autome/` 下任何内容都不改，包括本仓库自己的 `.autome/config.toml`。

## 5. 硬性约束

- **C-1 生产里没有测试专用后门。** e2e 需要把上限缩到很短，但不能为此给生产留一个测试专用的
  开关（环境变量、隐藏字段之类）。任务卡认可的一种做法：内部单位做成秒，再由
  `session_minutes` 换算。具体怎么缩短由设计决定。
- **C-2 e2e 用例 30 秒内跑完**（A-1 整条用例，含终止、退避、重派、重试用尽）。
- **C-3 上限之前正常结束的会话，行为与今天完全相同**；现有 e2e **不改**就过。
- **C-4 终止方式与人叫停一致**：对整个进程组发信号，wrapper 与它的子进程都不留。
- **C-5 超时不是协议失败，不消耗轮数预算**（设计轮数、实现轮数、`fix_rounds` 都不因超时而变）。
- **C-6 分层。** 纯类型与状态迁移放 `autome-domain`（无 I/O、无 async、无 SQLite）；文件、
  进程、git、SQLite 放 `automed`；`autome-cli` 只解析参数与打印。
- **C-7 测试替身对齐真实接口。** e2e 用真实 git worktree、真实 wrapper、真实进程组，只替换
  模型 CLI（沿用 `crates/automed/tests/end_to_end.rs` 现有 `World` 与替身 CLI 的做法）。
- **C-8 质量门。** `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
  `sh scripts/ci-local.sh` 全绿。
- **C-9 仓库规则。** 见第 8 节「权威规则文件」。

## 6. 验收标准（任务卡原文，逐条可测）

- **A-1** e2e：假 CLI 每秒写一行日志永不退出，上限设得很短，驱动在上限到了之后终止会话，
  任务留在原节点、会话行是超时、事件里有 `session.timeout`，随后按 `[retry]` 重新派一个会话；
  重试用尽后任务失败，失败原因写明超时。整条用例 30 秒内跑完。
- **A-2** e2e：会话在上限之前正常结束，行为与今天完全相同，现有 e2e 不改就过。
- **A-3** 单测：终止后 wrapper 和它的子进程都不在了（照 stop 的现有测试）。
- **A-4** 金样测试覆盖注入文案。
- **A-5** `config.validate` 对 `session_minutes = 1` 报错，对缺省值通过；`README.md` 配置一节
  加一行。
- **A-6** `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

## 7. 设计需要回答的开放问题（不预设答案）

以下是整理轮读代码时看到的、任务卡没有直接回答的地方。

1. **终止会阻塞 tick。** `launcher::stop_session` 发 SIGTERM 后同步 `sleep` 3000 毫秒再补
   SIGKILL。今天它只在用户按停止时被调用；放进 tick 之后，每次超时都会让持有驱动锁的那次 tick
   停 3 秒。这对并行任务与 C-2 的 30 秒预算（终止次数 = 重试次数 + 1）意味着什么。
2. **被终止的会话记成什么。** `SessionLifecycle` 现有 `Running / Exited / Killed / Vanished`
   四种，序列化后落在 SQLite 的会话表里并被桌面端读取。人叫停走的是
   `finish_session(Killed)` 且**不**经过 `reap_session`。超时用新的生命周期值，还是 `Killed`
   加一个标记；`timed_out: true`（R-5）从哪里来；旧账本里的会话行怎么读。
3. **怎么接到崩溃重试上。** 崩溃重试的入口是 `deliver_outcome` 里
   `SessionOutcome::Crashed → schedule_retry`，它在 `reap_session` 之后才发生（先
   `sweep_commit`、范围守卫、读文档）。超时会话是直接构造一个崩溃结果，还是走完整的收割路径；
   被终止时工作区里没提交的改动怎么处置（R-4 的文案要求下一轮「先把进度写进文档」，暗示
   上一轮的改动还在）。
4. **与已携带的注入冲突。** `schedule_retry` 用 `carried_inject` 把崩溃会话原本带着的注入
   （门禁失败、rebase 冲突、设计驳回意见……）原样带给重派的会话，而 `session_retries` 一行只存
   一个 `Option<Inject>`。超时说明与原注入同时存在时怎么办，原注入不能丢。
5. **重试用尽后的失败理由。** 今天是 `FailureReason::SessionCrashed { detail }`，CLI 打印
   「会话崩溃：…」。A-1 要求「失败原因写明超时」、R-5 要求 `dashboard.get` 里能看出是超时：
   新增 `FailureReason` 变体还是复用 `SessionCrashed` 并在 `detail` 里写明——前者会波及
   `task.rs` / `bug_loop.rs` 的迁移表、`render.rs`、`labels.js`、`notifier-rules.ts`。
6. **限流分支。** `schedule_retry` 先看流日志判断是不是限流（进冷却、不计重试）或模型不存在
   （按配置失败）。一个超时会话的流日志里若恰好有限流记录，按哪条走。
7. **覆盖哪些会话。** 任务卡说「每个在跑的会话」。`all_running_sessions()` 里还有任务整理
   （intake）、Bug Loop 三步、复盘、从失败面板手动发起的复盘，以及不属于任务 Loop 的
   Onboarding 会话。后几种没有「原节点重派」的位置，需要给出确定行为。
8. **`started_at` 的起点。** 会话记录的 `started_at` 写于内核登记会话时，早于终端真正拉起
   wrapper（`session_idle_secs` 的注释记录过这段空窗）。上限从哪一刻算起、暂停 / 重新排队中的
   任务是否仍有在跑的会话需要计时。
9. **上限读的是哪一刻的配置。** README 写明「配置修改对下一个启动的会话生效，在跑的会话不被
   打断」。上限按会话启动时的值，还是每次 tick 读当前生效配置；事件里的 `limit` 取哪个。
10. **「最后 40 行日志」取哪份。** 会话有可读日志（`SessionPaths::log`）与流日志（`.jsonl`）
    两份；注入内容需要随重试持久化（core 重启后仍在）。
11. **`config.validate` 之外的读写路径。** `[loop]` 在 `config_io.rs` 有四处：全局读、项目读、
    全局写、项目写。写路径若不带上新字段，桌面端保存设置时会把用户手写的 `session_minutes`
    抹掉。`config.set_*` 是否接受这个字段（界面不做，但 IPC 参数是否开放）需要明确。
12. **`autome status` 的「已运行 / 上限」数据从哪来。** `task.get` 的会话条目今天没有上限
    字段；由内核下发还是 CLI 自己读配置（C-6：CLI 没有自己的逻辑）。
13. **文档同步范围。** 任务卡只要求 README 配置一节加一行。`docs/development/technical-design.md`
    的 §3.3、§7.3、§8.1、§9、§14、§17 描述了相关行为，是否同步由设计决定（非验收项）。

## 8. 项目背景摘要（生成时，2026-11-13）

**仓库**：单仓库 `autome-v2`（当前 worktree 即其检出，默认分支 `main`；有远端 `origin`，
会话不得推送）。macOS 上的
Autome 2.0：把一句话需求跑过 Loop（intake → 设计 → 评审 ⇄ 裁决 → [批准设计] → 实现 ⇄ 审计 →
复盘 → rebase → [按合并] → 合并 → 清理），会话是 Claude Code / Codex CLI。本仓库用自己的
Loop 开发自己。

**技术栈与布局**

- Rust workspace（toolchain 1.93.1，edition 2024），成员：
  - `crates/autome-domain` — 纯类型与迁移，无 I/O。与本任务相关：
    - `src/config.rs`：`LoopDefaults`、`LoopOverrides`、`resolve()`、`validate()` 与
      `ConfigViolation`（现有 `ParallelOutOfRange`、`DesignRoundsOutOfRange`、
      `BudgetFactorOutOfRange`）、`RetryPolicy`（`attempts` 默认 2、`backoff_seconds` 默认 60、
      `delay_secs` 每次翻倍）、`GateConfig.timeout_minutes`。
    - `src/session.rs`：`SessionLifecycle`、`classify()`、`VANISHED_AFTER_SECS = 600`。
    - `src/task.rs`：`Inject`（约 599 行）、`FailureReason`（约 195 行）、状态机迁移；
      `src/bug_loop.rs` 是 Bug Loop 的对应迁移。
  - `crates/automed` — 一切外部交互：
    - `src/scheduler.rs`：`tick` / `tick_locked`（第 1 步遍历 `all_running_sessions()` 调
      `reap_session`，1b 步 `start_due_retries`）、`reap_session`、`deliver_outcome`、
      `schedule_retry`（写 `session.retry` 事件）、`carried_inject`、`log_idle_secs`、
      `session_idle_secs`。
    - `src/dispatch.rs`：JSON-RPC。`stop_running_session`（约 2094 行）、`task_stop`、
      `session_json`（约 1790 行，`task.get` / `dashboard.get` 的会话条目）、`session_log`、
      `dashboard_get`、`config_validate`、`READ_METHODS`。
    - `src/launcher.rs`：会话启动与 prompt 组装。`stop_session`（对 `-pid` 发 SIGTERM，等
      3 秒，仍在则 SIGKILL）、`pid_alive`、各 `Inject` 分支的文案（`match spec.inject`，
      约 488 行起）与文件内的注入测试。
    - `src/gate.rs`：宿主门禁后台作业；`poll_as` 超时后杀进程组、在日志末尾追加
      「=== Autome：…运行超过 N 分钟，已终止 ===」，退出码 `TIMED_OUT = 124`。
    - `src/config_io.rs`：`[loop]` / `[retry]` / `[gate]` 的 TOML 读写与解析测试。
    - `src/store.rs`：SQLite（`sessions`、`events`、`session_retries` 表；`finish_session`、
      `schedule_retry`、`pending_retry`、`retry_attempts`、`retry_started`）。
    - `tests/end_to_end.rs`：真实 git、真实 wrapper，只替换模型 CLI（`World`、`FAKE_CLI`、
      `.autome/fake/<n>.sh` 步骤脚本、`w.configure(toml)`、`wait_for_retry`、`scheduling_wait`）。
    - `tests/prompt_rendering.rs` 与 `tests/golden/prompts/*.md`：prompt 金样（说明见
      `tests/golden/README.md`）。
  - `crates/autome-cli` — `autome` 命令。`src/main.rs` 的 `status` / `task_detail`
    （「会话（最近 5 个）」约 770 行）；`src/render.rs` 的失败理由文案（`session_crashed →
    会话崩溃：…`，约 209 行）；`tests/cli.rs`。
- `apps/desktop` — Electron 外壳（`node --test`）；`apps/web` — React Router + Vite 面板
  （vitest、eslint、tsc）。
- `protocol/` — Loop 协议源，经 `include_str!` 编译进 `automed`。
- `plugins/` — 1.x 的 loop 插件与 autome 插件，与本任务无关。

行号是生成时的近似位置，以符号名为准。

**任务卡参考一节与代码现状的出入**（已核对）

- 任务卡把 `stop_running_session` 列在 `scheduler.rs` 下；它实际在
  `crates/automed/src/dispatch.rs`，底层的进程组终止是 `launcher.rs` 的 `stop_session`。
- A-3 说「照 stop 的现有测试」。现有与会话停止相关的测试是 e2e
  `stopping_a_task_leaves_it_resumable`（不断言进程组）和 `launcher.rs` 的
  `stopping_an_implausible_pid_is_refused_rather_than_signalling_everything`；断言「进程组已
  不存在」的现成写法在门禁的 e2e 里（`libc::kill(-pid, 0) != 0`，「gate process group
  survived」）。
- README 配置一节（「Host gate, retries and cooldowns」）的 TOML 样例目前没有 `[loop]` 表。

**测试命令**

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --test end_to_end -- --nocapture       # 真实 git、真实 wrapper，只替换模型 CLI
npm test --prefix apps/desktop
npm run lint --prefix apps/web                    # 另有 typecheck / test / build
sh scripts/ci-local.sh                            # 以上全部；本项目的宿主门禁命令
```

本项目的宿主门禁配置为 `run = "before_merge"`：每个任务只在合并前跑一次完整门禁，实现轮与
审计轮只跑里程碑验收与直接相关的测试。

**权威规则文件**

- 仓库根 `AGENTS.md`：不改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/`
  下其它目录、不推送远端；完成本轮即结束会话，不自行启动下一个会话；超过 5 行的命令输出与
  临时验证产物写入 `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有模板 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 规格与决策：`docs/development/requirements.md`、`docs/development/technical-design.md`、
  `docs/adr/0001`～`0003`。
