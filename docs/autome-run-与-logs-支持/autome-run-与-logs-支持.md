# `autome run` 与 `autome logs` 支持 `--json` 逐行 JSON 输出

status: 已完成
design-round: 1/15
implementation-round: 3/10
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无

任务文件：`docs/autome-run-与-logs-支持/autome-run-与-logs-支持-task.md`

任务卡正文：`docs/autome-run-与-logs-支持/autome-run-与-logs-支持-card.md`

下文用「任务 §x-n」引用任务文件第 x 节第 n 项（如任务 §3-1 = `run --json`，任务 §6-7 = 管道关闭约束）。

## 背景与现状

基线 `9f2ba3b`，以下均已对照源码核实。

- **参数白名单。** `crates/autome-cli/src/main.rs` `run()` 里 `run` 只认 `--once / --until-idle / --interval / --verbose`，
  `logs` 只认 `--session / --follow`（`-f` 由 `args.rs` 映射为 `--follow`）。传 `--json` 以 `CliError::Usage` 退出码 2 被拒。
  白名单之后 `if !a.has("--json")` 才注册「等待驱动释放状态库…」提示——放开白名单后这条对 `run` / `logs` 自动生效。
- **其它命令的 `--json`。** 统一走 `print_json`，即 `serde_json::to_string_pretty`——**多行美化输出**，不能拿来做逐行 JSON。
  命令失败（`CliError`）时 stdout 不输出任何东西，`main()` 把 `autome：<消息>` 写到 stderr，用法错误退出码 2、内核拒绝退出码 1。
- **stdout 与管道关闭。** 文件内的 `print!` / `println!` 宏走 `print_stdout`：写入遇到 `BrokenPipe` 时 `exit(0)`，其它错误 panic。
  现有 `logs()` 没走这条路，而是 `std::io::stdout().write_all(..).map_err(CliError::Failed)`，管道关闭时会以退出码 1 加 stderr 文案结束。
- **`drive()`。** 先用 `events.since {after_seq: u64::MAX}` 取 `latest_seq` 作起点；每轮 `scheduler.tick` → `report.errors`（字符串数组）
  逐条 `eprintln!("  ! …")` → 分页 `events.since {after_seq, limit: 500}`，经 `render::event_line` 打印 `<本机时钟>  <文案>`，
  `event_line` 返回 `None` 的事件（如每个会话收尾都会写的 `task.head`）只在 `--verbose` 下打印。`--once` 跑一轮即停；
  `--until-idle` 在 `idle(c)` 为真时停；SIGINT 置位后停。stderr 上有三条人类提示：「驱动 Loop 中…」（非 `--once`）、
  「另一个驱动…正在推进」（`report.skipped`）、「已停止驱动…」（被 Ctrl-C 打断）。
- **事件与时间。** `events` 表有 `at TEXT NOT NULL`，由 `Store::append_event` 写 `now_iso()`，格式 `YYYY-MM-DDTHH:MM:SSZ`（UTC，秒精度）。
  `Store::events_since_with_subject` 只读 `seq, kind, subject_id, payload`，被 `dispatch::events_since` 与 `scheduler.rs`、
  `tests/end_to_end.rs` 共 8 处以四元组解构使用。`events.since` 返回 `{events: [{seq, kind, subject, payload}], latest_seq}`；
  RPC 的外部使用者只有 `apps/web/server/eventbus.ts` 与 `notifier.ts`，按字段读取、原样转发，多一个字段不影响它们。
- **事件主体。** 任务 id 形如 `T-<n>`（`Store::next_task_id`），项目 id 形如 `prj-<uuid>`；`rule.dismissed`、`project.*`、`config.changed`
  等事件的主体是项目或其它对象。任务不会被删除（store 里没有 `DELETE FROM tasks`）。
- **会话日志。** 会话包装脚本（`init.rs`）把 CLI 的 stdout+stderr 合流后 `tee -a <会话>.jsonl`，再经 `<当前二进制> render-stream <runtime>`
  渲染后 `tee -a <会话>.log`；`.log` 末尾另有一行 `=== autome session … finished, exit N ===`。`.jsonl` 与 `log_path` 同目录同名、
  只是扩展名不同（`SessionPaths::dir(task)/<session>.jsonl`，`scheduler.rs` 读用量时就是这样拼的）。渲染器不可执行时走另一支，
  不产生 `.jsonl`。
- **`render-stream` 的解析。** `automed::stream_render`（`pub mod`）的 `render_line`（Claude）与 `render_codex_line`（Codex）都是 `pub`，
  把一行 JSONL 变成 `Option<String>`：空行 → `Some("")`；非 JSON 行原样透传；`None` 只给「对人没有内容」的事件
  （Claude 的 `system/hook_started`、Codex 的 `turn.started` 等）。会话记录的 `runtime` 字段序列化为 `"claude"` / `"codex"`。
  默认角色：plan / adjudicate / impl 为 Claude，review / audit / retro 为 Codex（`autome-domain/src/config.rs`）。
- **测试沙箱。** `tests/cli.rs` 的 `Sandbox` 用 `fake-cli` 顶替两个 CLI，第 n 个会话执行 `.autome/fake/<n>.sh`；渲染器是被测的
  `autome` 二进制本身。本轮实测（保留沙箱后查看）：每个会话都有 `<session>.jsonl`，因为 fake 步骤不往 stdout 写东西，所以是空文件。
  步骤脚本往 stdout/stderr 写的内容会原样进入 `.jsonl`。
- **测试基线**（本轮实测）：`cargo test -p autome-cli` = 单元 25 个 + 集成（`tests/cli.rs`）14 个，全绿；
  `cargo test -p automed --lib events_since` = 3 个，全绿。

## 目标与非目标

**目标**

- G1（任务 §3-1、§4-1）：`autome run --json` 的 stdout 每行一个紧凑 JSON 值：事件行、tick 错误行、`--until-idle` 收尾行，没有人类文案。
- G2（任务 §3-2、§4-1）：`autome logs <任务> --json [--follow]` 的 stdout 每行一个紧凑 JSON 值，来自会话原始流的结构化事件，
  解析沿用 `render-stream`；`--follow` 持续输出且不输出半行。
- G3（任务 §3-3、§4-2）：`tests/cli.rs` 两条新用例跑真实二进制；`cargo test -p autome-cli` 全绿；宿主门禁 `sh scripts/ci-local.sh` 通过。
- G4（任务 §6-5）：不带 `--json` 时两条命令的输出与退出码逐字节不变，现有用例不改即通过。

**非目标**

- 不增删改事件种类与负载（任务 §5）；`payload` 原样输出。
- 不改 `stream_render.rs`、`render-stream` 子命令与会话包装脚本（任务 §5）；只调用其 `pub` 函数。
- 不改其它命令的 `--json`（仍是美化多行）；不改 `apps/`、`plugins/`、`protocol/`。
- 不给 `run --json` 增加任务卡没要求的行种类（如驱动被轮流跳过的提示），见 Backlog B-01。

## 方案

### 1. 共同规则

- 白名单：`"run"` 增加 `"--json"`；`"logs"` 增加 `"--json"`。`USAGE` 改为
  `run [--once | --until-idle] [--interval 秒] [--json]` 与 `logs <任务> [--session 会话] [-f] [--json]`
  （保留 `run [--once | --until-idle]` 前缀，`help_and_version_need_no_ledger` 断言的就是它）。
- **一行的写法**：`println!("{}", serde_json::to_string(&v)?)`——用文件内的 `println!` 宏（即 `print_stdout`），**不得**用 `print_json`
  （多行），也**不得**直接 `std::io::stdout().write_all`。`to_string` 输出的字符串值会把换行转义成 `\n`，所以一个值一定只占一行；
  管道关闭时沿用 `print_stdout` 的 `exit(0)`（任务 §6-7）。
- **命令失败**（参数错、找不到任务、RPC 报错）：与其它命令的 `--json` 一致——stdout 不写，`main()` 把 `autome：<消息>` 写到 stderr，
  退出码 2 / 1。已经写出的 JSON 行保持完整（每行写完即 flush），所以中途失败也不会留下半行。
- **stderr 上的人类提示**：`--json` 下三条（「驱动 Loop 中…」「另一个驱动…」「已停止驱动…」）一律不打，与 `main.rs` 不注册
  「等待驱动释放状态库…」的先例一致。理由：`--json` 的使用者是程序，stderr 留给 `CliError` 的失败消息，便于调用方「stderr 非空即出错」。
- 不带 `--json` 的代码路径一字不改（分支在最外层判断）。

### 2. 内核只读面：事件时间（M-01）

- `Store` 新增只读方法 `events_since_with_at(after_seq, limit) -> Vec<StoredEvent>`（`StoredEvent { seq, kind, subject, payload, at }`），
  SQL 在现有查询上多选 `at`。**`events_since_with_subject` 与 `events_since` 不动**，8 处四元组调用方不受影响。
- `dispatch::events_since` 改用新方法，每个事件对象**增加** `"at"` 字段，其余字段与 `latest_seq` 含义不变。这是任务 §5 允许的「增加字段」。
- `run --json` 的 `time` 取事件的 `at` 原值（`YYYY-MM-DDTHH:MM:SSZ`）。理由：`time` 应当是事件发生的时间，人类模式用的 `clock()` 是收到事件时的本机时钟，
  补跑历史事件或驱动落后时两者会差很多；UTC 原值无时区歧义，`jq` 可直接 `fromdateiso8601`。

### 3. `run --json`（M-01）

- **事件行**：`{"seq": <u64>, "time": <at>, "task_id": <string|null>, "kind": <string>, "subject": <string>, "payload": <原样>}`。
  - `task_id`：`subject` 是一个存在的任务 id（`c.ctx.store.get_task(subject).is_ok()`，可按 id 缓存结果）时等于 `subject`，否则 `null`。
    任务不会被删除，所以历史事件也判得准；不靠 `T-` 前缀猜。
  - 额外保留 `subject`：主体不是任务的事件（项目、规则等）若只给 `task_id: null` 会丢掉主体，加一个字段不违反任务卡给的字段集合。
  - 输出**全部**事件，不经 `render::event_line` 过滤：过滤是为了人读，程序要的是完整事件流。`--verbose` 在 `--json` 下接受但无作用。
- **tick 错误行**：`report.errors` 每个字符串一行 `{"error": "<原文>"}`，位置与人类模式相同（本轮事件之前）。
- **收尾行**：只在循环**因 `idle(c)` 为真而结束**时输出一行
  `{"idle": true, "tasks": [{"id", "slug", "state"}, …]}`：`tasks` 是 `store.list_unfinished()` 里的全部任务（与 `idle()` 判定用的是同一份），
  按其顺序；每项的 `id`、`slug`、`state` 从 `c.all_tasks()` 的任务投影里原样取（即与 `autome ls --json` 同一形状，`state` 与
  `autome status <任务> --json` 的 `.task.state` 相等）。调用方据此就能知道「停下来等谁」。
  `--once` 结束、被 Ctrl-C 打断、命令失败时**没有**收尾行——它们都不是「已空闲」，调用方以进程退出为结束信号。
- 每批事件写完照旧 flush；`seq` 推进逻辑不变。

### 4. `logs --json [--follow]`（M-02）

- **数据源**：会话原始流 `<log_path 去掉扩展名>.jsonl`（`Path::with_extension("jsonl")`），不读 `.log`。理由：`.log` 是渲染后的
  人类文本，再解析一遍只能拿到文字；`.jsonl` 是完整记录，内核读用量也读它。
- **每行的转换**（纯函数，放 `render.rs` 或 `main.rs`，带单元测试）：对 `.jsonl` 第 `n` 行（1 起计，空行也计数）：
  - 去掉行尾空白后为空 → 不输出；
  - 否则输出 `{"line": n, "type": <事件的 "type" 字段，非字符串或不是 JSON 对象时为 null>, "text": <渲染结果|null>, "event": <解析出的 JSON 值|null>}`，
    其中 `text` = 按会话 `runtime` 调 `automed::stream_render::render_line`（`"claude"`）或 `render_codex_line`（`"codex"`）的返回值，
    `None` 记为 `null`；`event` = 该行能解析为 JSON 时的值，否则 `null`。
  - 这样「沿用 `render-stream` 的解析」落在 `text` 上：与 `.log` 里那一行逐字相同；非 JSON 行（合流进来的 stderr 警告）
    得到 `event: null, type: null, text: <原文>`，同样是一行合法 JSON。两种运行时形状不同由 `render_*` 各自处理，`event` 保留原始对象不做归一。
  - 不合法 UTF-8 用 `String::from_utf8_lossy`。
- **行边界**（纯函数，带单元测试）：维护已消费的字节偏移；每次读到的新字节里只处理到**最后一个 `\n`** 为止，剩余半行留到下次。
  会话已不在运行（与现有 `--follow` 判定相同：`list_sessions` 里该会话 `is_running()` 为假）时，读尽文件后把末尾没有换行的那一段
  当作最后一行处理。所以：不带 `--follow` 且会话还在跑时，只输出完整行；带 `--follow` 时持续输出、跟到会话结束且字节读尽才退出；
  任何时候都不输出半行。Ctrl-C 照旧停止跟随。
- **会话选择**沿用现有 `--session` 前缀；JSON 模式默认取最新会话（列表按 `started_at DESC` 返回，取 `first()`）。人类模式保留原有 `last()` 行为。`runtime` 取会话记录的 `runtime` 字段，`Runtime::parse` 不认识时 `CliError::Failed`。
- **`.jsonl` 不存在**：会话仍在运行时视为空文件（包装脚本启动时才建，可能还没来得及建）；会话已结束仍不存在（渲染器不可用的老会话）→
  `CliError::Failed("会话 <id> 没有原始流 <路径>；不带 --json 的 `autome logs` 可以看渲染日志")`，退出码 1，stdout 为空。
- 写出一律走 `println!`，管道关闭即 `exit(0)`（任务 §6-7）。

## 测试与验收

- 两条集成用例只**新增**，现有 14 条一字不改（任务 §6-5）。逐行用 `serde_json::from_str` 解析 stdout 的每一行，等价于 `jq -c .` 逐行解析（任务 §4-1）。
- `logs --json` 用例喂给会话的流，逐字取自 `crates/automed/src/stream_render.rs` 测试里的真实 CLI 样例行（任务 §6-8），不自造形状：
  - Claude（第 4 个会话 = adjudicate，默认 Claude）：`{"type":"system","subtype":"init","model":"claude-opus-5","permissionMode":"acceptEdits"}`、
    `{"type":"assistant","message":{"content":[{"type":"text","text":"hi there"}]}}`、`{"type":"system","subtype":"hook_started"}`、
    `{"type":"result","subtype":"success","num_turns":7,"duration_ms":91500}`，外加 stderr 一行非 JSON 文本；
  - Codex（第 3 个会话 = review，默认 Codex）：`{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Hello!"}}`、
    `{"type":"turn.completed","usage":{"input_tokens":36111,"cached_input_tokens":12928,"cache_write_input_tokens":0,"output_tokens":6,"reasoning_output_tokens":128}}`。
  - 步骤脚本在原有内容之外用 `printf` 把这些行写到 stdout（stderr 那行写到 `>&2`），经包装脚本进 `.jsonl`。
- 宿主门禁 `sh scripts/ci-local.sh`（fmt、clippy `-D warnings`、全量测试、web）由 Autome 在每个实现轮后跑，实现轮自查时也跑一遍并在证据里记结果。
- 长输出写 `.autome/output/`，证据文件只写命令、用例数与判据。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | events.since 补事件时间，run --json 逐行输出事件、错误与收尾行 | 0 | cli-run |
| M-02 | 已完成 | logs --json [--follow] 逐行输出会话原始流的结构化事件 | 0 | cli-logs |

## 里程碑范围

- **M-01**（前置：无）覆盖任务 §3-1、§3-3（`run` 那条）、§4-1（`run`）、§4-2、§6-4、§6-5、§6-7，待定问题 1–5。
  改动：`store.rs` 新增 `events_since_with_at` 及其单元测试；`dispatch.rs` `events_since` 加 `at` 及一条单元测试；
  `main.rs` 白名单、`USAGE`、`drive()` 的 JSON 分支；事件行组装做成纯函数并带单元测试；`tests/cli.rs` 新增 `run_json_prints_every_event_as_one_json_line`。
  可观察结果：沙箱里 `autome run --json --until-idle` 的 stdout 逐行是 JSON，最后一行是 idle 收尾行。
- **M-02**（前置：M-01，共用白名单与 `USAGE` 改动、写行方式）覆盖任务 §3-2、§3-3（`logs` 那条）、§4-1（`logs`）、§4-2、§6-4、§6-5、§6-7、§6-8，待定问题 6–7。
  改动：`main.rs` `logs()` 的 JSON 分支；行转换与行边界两个纯函数及单元测试；`tests/cli.rs` 新增 `logs_json_prints_the_session_stream_one_json_value_per_line`。
  可观察结果：沙箱里 `autome logs <任务> --json` 与 `--session <review 会话> --json` 逐行输出两种运行时的结构化事件。

## 验收契约

### M-01 验收契约

最新证据：docs/autome-run-与-logs-支持/evidence/M-01-r1-audit.md · 通过 · 审计 #1

- 验收命令：`cargo test -p automed --lib events_since && cargo test -p autome-cli`
- 预期：`automed` 过滤后 4 个用例通过（基线 3 + `events_since_carries_when_each_event_happened`：断言 `events.since` 每个事件有 `at` 且等于表里的值）；`autome-cli` 单元 28 个通过（基线 25 + 3：主体是任务时 `task_id` 等于主体、主体是项目时 `task_id` 为 `null` 且 `subject` 保留、`time` 取 `at`），集成 17 个通过（基线 14 + 断管/驱动占用、tick 错误/SIGINT 两条异常路径用例 + `run_json_prints_every_event_as_one_json_line`：在任务建好、**尚未被驱动推进**时先用 `autome rpc events.since` 记起点 seq，再由受测的 `run --json --until-idle --interval 0.05` 把它推进到等设计批准（起点必须在受测驱动之前取，否则驱动从最新 seq 起步、事件列表为空）；断言 stdout 事件行非空且含 `task.head`，每行可解析为 JSON；除最后一行外每行的 `seq` 列表与事后 `rpc events.since {after_seq: 起点}` 返回的 `seq` 列表完全相等、`time` 等于对应事件的 `at`、主体为该任务的事件 `task_id` 等于任务 id；最后一行 `idle` 为 `true` 且 `tasks` 里该任务的 `state` 等于 `status <任务> --json` 的 `.task.state`、`node` 为 `await_design_approval`；stderr 不含「驱动 Loop 中」）。
- 负向对照：JSON 分支用 `print_json`（美化多行）时第一行 `{` 解析失败，集成用例红；沿用 `render::event_line` 过滤时每个会话收尾的 `task.head` 被丢掉，`seq` 列表相等断言红；`task_id` 直接取 `subject` 时项目主体那条单元用例红；`time` 用 `clock()` 时 `time == at` 断言红（`clock()` 输出本地 `HH:MM`，与 `at` 的 `YYYY-MM-DDTHH:MM:SSZ` 不同）。

### M-02 验收契约

最新证据：docs/autome-run-与-logs-支持/evidence/M-02-r3-impl.md · rebase 后 CLI 通过，宿主门禁环境受限 · 实现 #3

- 验收命令：`cargo test -p autome-cli`
- 预期：单元 32 个通过（M-01 后的 28 + 4：Claude 样例行的 `type`/`text`/`event`、Codex 样例行（`turn.started` 的 `text` 为 `null`）、非 JSON 行得到 `event: null, type: null, text: 原文` 且空行不输出、行边界函数对「两行半」的输入只吐两行且会话结束后补出第三行），集成 18 个通过（M-01 后的 17 + `logs_json_prints_the_session_stream_one_json_value_per_line`：第 3 个会话写 Codex 样例、第 4 个写 Claude 样例与一行 stderr 文本，跑到等设计批准后：`logs <任务> --json` 与 `logs <任务> --json -f` 的 stdout 每行可解析为 JSON，行数等于最后一个会话 `.jsonl` 的非空行数，`type` 依次为 `system`、`assistant`、`system`、`result`、`null`，`assistant` 行 `text` 含 `hi there`、`hook_started` 行 `text` 为 `null`；`logs <任务> --session <review 会话 id> --json` 的 `text` 含 `Hello!` 与 `in 36111`；把 stdout 接到读端已关闭的管道再跑 `logs <任务> --json`，退出码为 0 或 SIGPIPE 且 stderr 为空）。
- 负向对照：读 `.log` 而不是 `.jsonl` 时行数多出收尾行 `=== autome session … ===`、`event` 全为 `null`，集成用例红；按运行时选错渲染函数（一律 `render_line`）时 Codex 会话的 `text` 不含 `in 36111`，集成用例红；行边界函数不留半行时「两行半」单元用例红；JSON 分支沿用 `write_all(..).map_err(CliError::Failed)` 时管道关闭那一步退出码 1 且 stderr 有 `Broken pipe`，集成用例红。

## 人工验收清单

- 在一个真实运行中的任务上执行 `autome run --json --until-idle | jq -c .`，jq 全程不报错，结束时最后一行是 `{"idle":true,…}`。
- 在一个真实 Claude 会话运行期间执行 `autome logs <任务> --json -f | jq -c .`，输出随会话持续增长、jq 不报错，会话结束后命令自行退出。
- 对一个真实 Codex 会话执行 `autome logs <任务> --session <会话> --json | jq -c 'select(.type=="turn.completed") | .text'`，能看到用量文字。

## 风险与验证安排

- **R1 新增 `at` 字段影响 RPC 使用者。** 只加字段，`apps/web` 按字段读取；宿主门禁里的 `apps/web` 测试在 M-01 的实现轮后即验证。
- **R2 `.jsonl` 在沙箱里是否产生、步骤脚本输出是否进入它。** 本轮已实测：沙箱每个会话都有 `.jsonl`（空文件），所以 M-02 用例的前提成立，
  无需提前验证。
- **R3 合流的 stderr 打断一行 JSON。** 两个 CLI 的 stderr 与 stdout 合流时理论上可能在行中插入；转换函数对任何一行都输出合法 JSON
  （解析失败即 `event: null` 的文本行），不会破坏「每行一个 JSON」，最多让那一条事件以文本形式出现。
- **R4 `get_task` 每事件一次查询。** 本地 SQLite 主键查询，按 id 缓存后开销可忽略。
- **R5 秒精度时间。** 同一秒内多个事件 `time` 相同；顺序以 `seq` 为准，文档里已说明 `seq` 是排序键。

## Backlog

- B-01 `run --json` 在 `scheduler.tick` 报告 `skipped`（另一个驱动在推进）时输出一行结构化提示，例如 `{"skipped": true}`；任务卡没要求，本任务不做。
- B-02 `logs --json` 在会话结束后补一行 `{"ended": true, "exit": N}`，取自退出标记；任务卡没要求，本任务不做。
- B-03 `logs --json --follow` 从上次偏移按 `seek` 增量读文件，而不是每 500ms 整读一次；当前文件规模下不影响正确性。
- B-04 为运行中的 `logs --json --follow` 增加自动化端到端取证（有界握手让会话分段追加原始流，检查换行前无输出、补齐后输出、会话结束后退出）；行边界单元测试与人工验收清单已覆盖意图，来源评审 R1-N02，本任务不做。

## 争议项
