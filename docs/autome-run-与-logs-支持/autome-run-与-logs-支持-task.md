# 任务：`autome run` 与 `autome logs` 支持 `--json`

本任务的规则见 `docs/autome-run-与-logs-支持/protocol/loop-protocol.md` 与
`docs/autome-run-与-logs-支持/protocol/session-protocol.md`。

- 设计文档：`docs/autome-run-与-logs-支持/autome-run-与-logs-支持.md`
- 任务卡正文（第一输入）：`docs/autome-run-与-logs-支持/autome-run-与-logs-支持-card.md`
- 任务整理日期：2026-11-12；任务 id T-46；基线提交 `9f2ba3b`
- 来源：用户的一句话需求与用户写的任务卡正文。两者不一致时以任务卡正文为准。

## 1. 需求原文

> autome run 与 logs 支持 --json

## 2. 目标与背景

**背景（任务卡）。** CLI 方案要求所有命令支持 `--json`，Agent 才能程序化驱动。
`crates/autome-cli/src/main.rs` 的参数白名单里，`run` 与 `logs` 是仅有的两个不接受 `--json` 的命令
（基线 `9f2ba3b` 第 149–150 行：`run` 只认 `--once / --until-idle / --interval / --verbose`，
`logs` 只认 `--session / --follow`）；现在传 `--json` 会以用法错误（退出码 2）被拒。

**目标。** 这两条命令在 `--json` 下，stdout 的每一行都是一个独立的 JSON 值，可被 `jq -c .` 逐行解析；
不带 `--json` 时行为与现在完全一样。

## 3. 要做什么（取自任务卡）

1. **`run --json`**：
   - 每个事件输出一行 JSON：`{seq, time, task_id, kind, payload}`；
   - tick 错误输出 `{"error": ...}`；
   - `--until-idle` 结束时输出一行 `{"idle": true, "tasks": [...]}`；
   - 不打人类可读文案。
2. **`logs --json [--follow]`**：每行一个 JSON（会话日志的结构化事件，沿用 `render-stream` 的解析）；
   `--follow` 持续输出。
3. **`crates/autome-cli/tests/cli.rs`**：`run --json` 与 `logs --json` 各加一条用例。

## 4. 验收标准（取自任务卡）

1. `autome run --json` 与 `autome logs --json` 的 stdout 可被 `jq -c .` 逐行解析
   （即每一行都是合法 JSON，没有夹杂人类可读文案）；
2. `cargo test -p autome-cli` 全绿。

设计文档的里程碑必须覆盖这两条以及 §3 的三项；用户在设计定稿时会对照它们批准或驳回。

## 5. 范围

**在范围内：**

- `crates/autome-cli/src/main.rs`：白名单给 `run`、`logs` 加 `--json`；`drive()`（`run`）与 `logs()` 的 JSON 输出分支；
  `USAGE` 帮助文本里这两条命令的写法；
- `crates/autome-cli/tests/cli.rs`：两条新用例（这一类测试跑真实的 `autome` 二进制）；
- 为拿到 §3 要求的字段而**确有必要**的内核只读面改动（见 §7 待定问题 1、2），例如 `events.since`
  返回里补上事件时间。这类改动只能是增加字段或增加只读入口，不得改变现有字段的含义。

**不在范围内（任务卡「不做什么」，加调研得出的边界）：**

- 不改事件内容：事件的 `kind` 与 `payload` 由内核写入时决定，本任务原样输出，不增删改事件种类与负载；
- 不改 `render-stream`：`crates/automed/src/stream_render.rs` 的渲染行为、`autome render-stream` /
  `automed render-stream` 子命令、会话包装脚本（`crates/automed/src/init.rs` 生成的管道）都不动；
- 不改其它命令的 `--json` 输出；不改 `run` / `logs` 不带 `--json` 时的输出与退出码；
- 不改 `apps/desktop`、`apps/web`、`plugins/`、`protocol/`。

## 6. 硬性约束

1. **不要修改 `.autome/` 下的任何内容**（`AGENTS.md`）。
2. 只在当前 worktree 内工作；不切换分支、不操作 `.worktree/` 下的其它目录、不推送远端；
   完成本轮后结束会话，不自行启动下一个会话（`AGENTS.md`）。
3. 超过 5 行的命令输出、临时验证产物写入 `.autome/output/`，不进入任务文档目录（`AGENTS.md`）。
4. `--json` 下 **stdout 只有 JSON 行**。现有代码里写到 stderr 的提示（「驱动 Loop 中…」「另一个驱动…」
   「已停止驱动…」「等待驱动释放状态库…」）在 `--json` 下如何处理由设计决定，但任何一条都不得出现在 stdout。
   `main.rs:158` 已有先例：带 `--json` 时不注册「等待驱动释放状态库…」提示。
5. 不带 `--json` 的现有行为逐字节不变；`tests/cli.rs` 现有用例不做修改就通过。
6. 宿主门禁是 `sh scripts/ci-local.sh`（`.autome/config.toml` 的 `[gate]`，每个实现轮之后、合并前、合并后各跑一遍）。
   它包含 `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings`，新代码必须过这两项。
7. stdout 写入要沿用 `main.rs` 里现有的管道关闭处理（`print_stdout` 及 `tests/cli.rs` 的 `broken_pipe_*` 用例所保护的行为）：
   `autome run --json | head -1` 这类用法不能 panic。
8. 测试替身要与**真实接口**对齐而不是与调用方对齐：`logs --json` 的用例所喂的会话流，形状要取自真实 CLI 的
   流（`crates/automed` 里已有的 `stream_render` 测试样例或 `.jsonl` 样例），不要自造一个只有新代码认识的形状。

## 7. 待定问题（留给设计轮，本轮不作决定）

调研发现任务卡的写法与现有代码之间有几处需要设计给出取舍并说明理由：

1. **事件的 `time` 从哪来。** `events.since`（`crates/automed/src/dispatch.rs` 约 2464 行）现在返回
   `{seq, kind, subject, payload}`，没有时间；`events` 表有 `at` 列（`store.rs`），但
   `Store::events_since_with_subject` 没有读出来。人类可读模式下 `run` 打印的是**收到事件时**的本机时钟（`clock()`），
   不是事件时间。设计要决定 `time` 取事件的 `at`（需要内核只读面补字段）还是别的来源，以及格式。
2. **`task_id` 与 `subject`。** 内核事件的主体字段叫 `subject`，且并非所有事件的主体都是任务
   （例如 `project.added` 的主体是项目）。设计要写明 `task_id` 如何从 `subject` 得出，主体不是任务的事件怎么输出。
3. **`run --json` 输出哪些事件。** 人类可读模式下，`render::event_line` 返回 `None` 的事件只有 `--verbose` 才打印。
   `--json` 下是全部输出还是同样过滤，`--verbose` 与 `--json` 同时出现时的含义。
4. **`{"idle": true, "tasks": [...]}` 里 `tasks` 的内容**（哪些任务、每个任务带哪些字段），以及
   `--once`、被 Ctrl-C 打断时是否有结束行。
5. **`{"error": ...}` 的值的形状**（`scheduler.tick` 的 `errors` 是字符串数组：一条错误一行，还是别的），
   以及命令本身失败（非 tick 错误）时 `--json` 下的输出与退出码，要与其它命令的 `--json` 失败行为一致。
6. **`logs --json` 的数据源。** 会话日志 `log_path` 里存的是 `render-stream` **渲染之后**的人类可读文本；
   原始 JSONL 由包装脚本另存为同目录的 `<会话 id>.jsonl`（`scheduler.rs` 约 399、794 行，`init.rs` 约 552 行）。
   任务卡说「会话日志的结构化事件，沿用 `render-stream` 的解析」。设计要决定：读哪个文件、输出的是原始流事件
   还是经过解析归一后的事件、Claude 与 Codex 两种运行时的流形状不同怎么处理、流里夹杂的非 JSON 行
   （stderr 合流进来的警告）怎么保证不破坏「每行一个 JSON」、`.jsonl` 不存在时怎么办。
   约束：不改 `render-stream`（§5）；若需要复用其解析，只能调用或只读地复用，不能改变它的行为。
7. **`logs --json --follow` 的行边界。** 现有 `--follow` 按字节增量输出；JSON 模式下不能输出半行。

## 8. 项目背景摘要（整理时的快照）

**这是什么。** Autome 2.0：把一句话需求跑成合并分支的 Loop 工具。内核 `automed`（Rust）独占全部状态与节点调度；
桌面端（Electron）、Web 面板（Node 网关 + React Router）与命令行 `autome` 都是它的客户端。本仓库用自己的 Loop 开发自己。

**单仓库**，工作目录即本任务的检出（无 `repos:` 行）。

**布局：**

| 路径 | 内容 |
|---|---|
| `crates/autome-domain` | 纯域逻辑：任务状态机、状态块解析、配置、角色（无 I/O） |
| `crates/automed` | 内核：`store.rs`（SQLite 账本，含 `events` 表）、`dispatch.rs`（JSON-RPC 方法）、`scheduler.rs`、`launcher.rs`、`stream_render.rs`（`render-stream`）、`init.rs`（会话包装脚本） |
| `crates/autome-cli` | 命令行 `autome`：`src/main.rs`（命令分发、`drive` / `logs`）、`args.rs`（参数解析）、`client.rs`（进程内直连内核）、`render.rs`（人类可读渲染）、`tests/cli.rs`（跑真实二进制的集成测试，自带 `Sandbox` 助手） |
| `apps/desktop` | Electron 外壳（纯 `node --test`） |
| `apps/web` | Node 网关 + React Router 面板（vitest） |
| `plugins/` | Claude / Codex / Cursor 插件 |
| `protocol/` | 协议 seed（`include_str!` 进内核） |
| `scripts/ci-local.sh` | 本地 CI，即宿主门禁 |

**技术栈。** Rust workspace（edition 2024，工具链固定 1.93.1，带 rustfmt 与 clippy）；依赖 `serde_json`、`rusqlite`（bundled）、`libc`。
`autome-cli` 依赖 `automed` 与 `autome-domain`，在进程内调用内核（`Client::call`），不经套接字。macOS 专用。

**与本任务直接相关的现状（基线 `9f2ba3b`）：**

- `main.rs` `run()`：按命令给出参数白名单，未知参数以 `CliError::Usage` 退出码 2；内核拒绝为退出码 1。
- `drive()`（约 771 行）：循环 `scheduler.tick` → 把 `report.errors` 逐条打到 stderr → 用 `events.since` 拉新事件，
  经 `render::event_line` 打印到 stdout；`--once` 跑一轮即停，`--until-idle` 在 `idle()` 为真时停，SIGINT 置位后停。
- `logs()`（约 872 行）：`task.get` 取会话列表，按 `--session` 前缀或取最后一个，读 `log_path` 按字节增量写 stdout；
  `--follow` 跟到会话结束且字节读尽。
- `print_json`（约 208 行）是其它命令 `--json` 的输出入口（多行美化输出还是单行，设计轮自行核对）。
- `stream_render.rs`：`render_line`（Claude 流）与 `render_codex_line`（Codex 流）把一行 JSONL 渲染成零或一行文本，
  解析失败的行原样透传。

**测试与门禁命令：**

- 本任务验收：`cargo test -p autome-cli`
- 宿主门禁（全量）：`sh scripts/ci-local.sh`，依次跑 `cargo fmt --all -- --check`、
  `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets -- --nocapture`、
  `npm test --prefix apps/desktop`、契约检查、`apps/web` 的 lint / typecheck / test / build；每一步都跑，任一失败即非零退出。
- 输出长的命令重定向到 `.autome/output/` 再看头尾。

**权威规则文件：**

- `AGENTS.md`（仓库根）；
- `.autome/rules/`（整理时只有一份 `README.md` 说明，没有具体规则文件）；
- `docs/agent-project-profile.md`：整理时不存在；
- 本任务固定的协议副本：`docs/autome-run-与-logs-支持/protocol/`。
