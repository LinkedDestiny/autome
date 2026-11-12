# 任务：文档补齐——Bug Loop、任务类型与任务卡、待开始、reply、`[bug]`/`[setup]`/`same_failure_limit`

本任务的规则见 `docs/文档补齐-bug-loop-任务卡-待开始/protocol/loop-protocol.md` 与
`docs/文档补齐-bug-loop-任务卡-待开始/protocol/session-protocol.md`。

- 设计文档：`docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md`
- 任务卡正文（用户写的第一输入）：`docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始-card.md`
- 任务整理日期：2026-11-12；任务 id T-45；基线提交 `cf522fb`；分支 `autome/文档补齐-bug-loop-任务卡-待开始`
- 任务类型：需求（走设计 → 评审/裁决 → 实现/审计的 Loop），但**交付物只有文档**。

## 1. 需求原文

> 文档补齐 Bug Loop、任务卡、待开始、reply 与 [bug]/[setup]/same_failure_limit

任务卡正文与这句话不一致时，以任务卡正文为准。下面 §2–§5 已把任务卡正文的全部内容收进来，后续各轮不必
再读 card 文件；要核对原话时再去看。

## 2. 背景与目标

**背景（任务卡正文）。** K1（任务类型、任务卡正文、待开始）、K2（Bug 修复 Loop）、T-27（项目可配置准备命令
`[setup]`）、T-28（门禁连续同因失败熔断 `same_failure_limit`）都已合并进 main，但文档没有跟上：

- `docs/development/technical-design.md` §5.1 只列了需求 Loop 的节点；全文没有 Bug Loop、`kind`、`Draft`、
  `task start`、`task reply`、`[bug] fix_rounds`；
- `README.md` 只在 `[gate]` 那一条说明里提到一次 `fix_gate`；`[setup]` 与 `same_failure_limit` 是否已写全需要核对。

新用户和程序化建卡的 Agent（用 `autome task new --json` 的调用方）从文档里读不到这些。

**目标。** 让 `README.md` 与 `docs/development/technical-design.md` 如实描述已经在代码里的这些行为，且
**每一处陈述都能指到代码位置**，由审计轮逐条与代码核对。

## 3. 要做什么（逐条取自任务卡正文）

1. **README.md**
   - 「命令行」一节（`## The command line`）补：`task new --kind bug --brief - --draft --json`、`task start`、
     `reply`、`extend`、`rerun`；
   - 配置一节（`### Host gate, retries and cooldowns` 及其 TOML 示例）补：`[bug] fix_rounds`、`[setup]`、
     `[gate] same_failure_limit`——**先核对 T-27/T-28 是否已经写了，缺才补，已有且正确的不重写**。
2. **docs/development/technical-design.md**
   - §5 加 Bug Loop 的节点与转换，与 `crates/autome-domain/src/bug_loop.rs` 对照，至少包含三点：
     - 复现命令退出码 126/127 不算失败；
     - 连续两次复现检查失败进入「等你补充信息」；
     - 合并前门禁总带复现命令；
   - §8 加任务卡正文 / 附件 / `Draft`；
   - §14（IPC 接口）更新。
3. **每处陈述给出代码位置**；审计轮核对代码与文档一致。

## 4. 验收标准（取自任务卡正文）

1. `grep` 能在两份文档里找到上述关键词。设计轮要把「哪个关键词必须出现在哪份文档」列成一张可逐行执行的
   清单（README：`--kind`、`--brief`、`--draft`、`task start`、`reply`、`extend`、`rerun`、`fix_rounds`、
   `[setup]`、`same_failure_limit`；technical-design：Bug Loop、`kind`、`Draft`、`task start`/`task.start`、
   `task reply`/`task.reply`、`fix_rounds`，以及 §3 第 2 条的三点）。
2. 审计轮逐条核对文档陈述与代码一致。
3. `sh scripts/ci-local.sh` 全绿（文档改动不应影响它）。

## 5. 范围与硬性约束

**只允许改两份文件：`README.md` 与 `docs/development/technical-design.md`**，外加本任务文档目录
`docs/文档补齐-bug-loop-任务卡-待开始/` 下协议要求的文档（设计文档、评审/裁决/审计记录、`evidence/` 等）。

1. **不改代码。** `crates/`、`apps/`、`plugins/`、`protocol/`、`scripts/`、`.github/`、`.githooks/`、
   `Cargo.*` 一律不动——包括 CLI 的 usage 文本、注释、测试。核对中若发现代码与预期不符或代码本身有缺陷：
   文档按**代码的实际行为**写，把发现记进设计文档的 Backlog，不在本任务里修。
2. **不扩大范围。** 只补需求与任务卡点名的主题。不重写两份文档的其它章节，不顺手改措辞、排版、章节编号，
   不动 `docs/development/requirements.md`、`docs/plans/`、`docs/adr/`、其它任务的文档目录。
   technical-design 现有章节号（§5、§8、§14 等）被别处引用，新增内容以新增小节/段落的方式放进去，不重排已有编号。
3. **以代码为准，不以计划文档或记忆为准。** 每条陈述写之前先在基线代码里找到出处；`docs/plans/` 与以往
   任务的设计文档可以帮助定位，但不能当依据。读不到出处的行为不写。
4. **代码位置的写法。** 行号会随后续提交漂移，引用以「文件路径 + 符号名（函数、枚举变体、常量、测试名）」为主，
   行号可有可无；审计轮必须能按这个引用用 `grep` 找到。README 面向用户，引用怎么放（行内、脚注、
   只在设计文档的对照表里）由设计决定，但对照关系必须能逐条审计。
5. **语言跟随原文档。** `README.md` 是英文，补的内容用英文；`technical-design.md` 是中文，补的内容用中文。
   代码里的中文字面量（错误信息、usage 文本）引用时保持原样。
6. **不修改 `.autome/` 下任何内容**；不切分支、不建 worktree、不推送；只在当前 worktree 内工作。
7. 超过 5 行的命令输出、临时验证产物写进 `.autome/output/`，不要放进任务文档目录（逐轮证据按协议写进
   `docs/文档补齐-bug-loop-任务卡-待开始/evidence/`）。
8. **验证时不要碰真实账本。** 需要运行 `autome`/`automed` 确认行为（例如看 `autome help` 的输出）时，
   用隔离的 `AUTOME_HOME` / `AUTOMED_DB_PATH`（现有测试的做法），不要让本 worktree 构建出的二进制打开真实的
   `~/.autome`——正在驱动本仓库 Loop 的是已安装的 `autome`，账本被新二进制迁移会让它停摆。
   读 usage 文本优先直接读 `crates/autome-cli/src/main.rs` 的 `USAGE` 常量。

## 6. 项目背景摘要（2026-11-12，基线 `cf522fb`）

**单仓库。** 工作目录就是 autome-v2 这一个仓库的任务检出，没有多个独立仓库，状态块不写 `repos:`。

**是什么。** Autome 2.0：一个驱动「Agent Loop」开发的内核 + 命令行 + 面板。本仓库用自己的 Loop 开发自己。

**技术栈与布局。**

| 路径 | 内容 |
|---|---|
| `crates/autome-domain` | Rust，纯领域：状态机（`task.rs`）、Bug Loop（`bug_loop.rs`、`bug_doc.rs`）、配置解析结果（`config.rs`）、`setup.rs`、`gate_failure.rs`、状态块解析等 |
| `crates/automed` | Rust，I/O 与 JSON-RPC：`dispatch.rs`（方法派发表、`READ_METHODS`）、`scheduler.rs`、`store.rs`（SQLite 账本）、`config_io.rs`（TOML 读写与校验）、`gate.rs`、`setup.rs`、`launcher.rs`、`git.rs` 等 |
| `crates/autome-cli` | Rust，命令行 `autome`：`main.rs`（`USAGE` 与各子命令）、`args.rs`、`render.rs`、`tests/cli.rs` |
| `apps/desktop` | Electron 外壳（`node --test`） |
| `apps/web` | Node 网关 + React Router 面板（vitest、eslint、tsc） |
| `protocol/` | Loop 协议的唯一来源（编译进 `automed`）；本任务固定的副本在任务文档目录的 `protocol/` 下 |
| `plugins/` | Claude 插件（`autome`、`loop`） |
| `docs/development/` | `requirements.md`、`technical-design.md`（**本任务要改后者**） |
| `docs/plans/`、`docs/adr/` | 计划与决策记录（只读参考） |
| `docs/<任务 slug>/` | 各任务的文档目录（只读，本任务自己的除外） |
| `README.md` | 英文，面向用户（**本任务要改**） |

Rust workspace：edition 2024，rust-version 1.93.1。

**测试 / 门禁命令。** `sh scripts/ci-local.sh`（项目 `.autome/config.toml` 的 `[gate] command`，每个实现轮之后、
合并前、合并后由 Autome 宿主自己跑，超时 30 分钟）。它依次跑：`cargo fmt --all -- --check`、
`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace --all-targets -- --nocapture`、
`npm test --prefix apps/desktop`、契约守卫（`dispatch.rs` 里有 `pub const READ_METHODS`）、
`apps/web` 的 lint / typecheck / test / build。输出很长，手动跑时重定向到 `.autome/output/` 只看尾部。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/`；只在当前 worktree 工作，不切分支、不动 `.worktree/` 下其它目录、
  不推送；本轮完成即结束会话，不自行启动下一个会话；超过 5 行的输出与临时产物写 `.autome/output/`。
- `.autome/rules/`：目前只有占位的 `README.md`（说明规则应如何写），没有实质性的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/文档补齐-bug-loop-任务卡-待开始/protocol/`（见文首指路）。

## 7. 调研时观察到的事实（供设计参考，不是设计结论；行号为基线 `cf522fb` 上的位置，设计轮需自行复核）

**README.md 现状（399 行）。**

- `### Host gate, retries and cooldowns`（约第 107 行起）的 TOML 示例里**已有** `[gate] same_failure_limit = 3`
  与 `[setup]`（`command` / `watch` / `timeout_minutes`）；其后的说明条目已有熔断（提到 `gate`、`merge_gate`、
  Bug Loop `fix_gate`，以及 `autome rerun <id> <node>` / `autome extend <id> 1`）与「Preparation」（`[setup]`）。
  ——这两项大概率是「核对」而不是「补」。
- **没有** `fix_rounds`、`[bug]`；`## The command line`（约第 168–209 行）的示例里没有 `--kind`、`--brief`、
  `--draft`、`task start`、`reply`；`rerun`、`extend` 只在末尾一句「`autome help` lists the rest」里以散文提到。
- `## The Loop`（约第 79 行）只讲需求 Loop。是否要在 README 里用一两句交代 Bug Loop 的存在，任务卡没有要求，
  由设计按「不扩大范围」判断。

**technical-design.md 现状（360 行）。**

- §5.1 状态（第 130 行起）只列 `Queued · Intake · … · Done` 与 `Paused/Stopped/Failed/Cancelled`，没有 `Draft`，
  没有 Bug Loop 节点；§5.2 角色与 prompt、§5.3 转换表、§5.5 计数与预算都只覆盖需求 Loop。
- §8 调度器（第 242 行）、§8.1（第 250 行）已有「宿主准备」（`[setup]`，约第 252 行）与「宿主门禁」（约第 256 行）
  两段；§9 配置解析（约第 264 行）已有 `[setup]` 的校验规则。全文 `grep -i` 找不到 `bug`、`kind`、`draft`、
  `task start`、`reply`、`fix_rounds`、`same_failure_limit`、任务卡（card）。
  ——任务卡没有点名让 technical-design 写 `same_failure_limit`，但它是需求原文里的词；是否在 §8.1/§9 补一句，
  由设计决定并说明理由。
- §3.3 配置文件示例（第 72 行起）是配置字段的另一处落点（`[bug]`）；§14（第 302 行）的命令清单是
  `task.create / list / get / approve / reject / merge / pause / resume / stop / cancel / decide / extend_budget /
  rerun_from / archive / restore`，缺 `task.start`、`task.reply`（以及代码里已有的 `task.retro`、`task.changes`
  ——后两者不在本任务点名的主题里，是否一并补由设计按范围判断）。

**代码出处的入口（去哪里找，不是结论）。**

- Bug Loop 状态机：`crates/autome-domain/src/bug_loop.rs`（1465 行）——`uses_bug_loop`、`apply_for`、
  `BugSessionOutcome`；复现检查对退出码的判定在约第 251 行（`![126, 127].contains(exit_code)`），对应测试在
  约第 1045–1064 行（`"not_runnable"`）。Bug 文档解析：`crates/autome-domain/src/bug_doc.rs`。
- 节点：`crates/autome-domain/src/task.rs` 的 `Node` 枚举（第 24 行起）含 `Locate`、`ReproCheck`、`Fix`、
  `FixGate`、`FixReview`、`AwaitInfo`、`NotABug`，线上名如 `locate`、`repro_check`、`fix_gate`（约第 174 行起）；
  角色映射约第 105 行（`Locate`/`Fix` → 实现角色，`FixReview` → 审计角色）；`TaskKind`（第 245 行）、
  `TaskState`（第 270 行，含 `Draft`）、`Trigger`（第 422 行）。
- JSON-RPC：`crates/automed/src/dispatch.rs` 的派发表（约第 394–440 行）——`task.create`、`task.start`、
  `task.reply`、`task.extend_budget`、`task.rerun_from` 等；`task_create` 里 `kind` / `brief` / `draft` 的参数与
  校验；`READ_METHODS`。
- 命令行：`crates/autome-cli/src/main.rs` 的 `USAGE`（约第 52–71 行：
  `task new [需求] [--kind feature|bug] [--brief <文件|->] [--draft] [-p 项目] [--attach 文件]... [--doc 路径]...`、
  `task start <任务>`、`reply <任务> "补充信息"`、`rerun <任务> <节点>`、`extend <任务> <轮数>`）与各子命令的实现
  （`reply` → `task.reply`，`rerun` → `task.rerun_from`，`extend` → `task.extend_budget`；`--draft` 成功后的提示
  「已存为待开始。用 `autome task start …` 开始。」）。
- 配置：`crates/automed/src/config_io.rs`——`[bug] fix_rounds`（约第 262 行，「必须是 1 到 20 的整数」）、
  `[gate] same_failure_limit`（约第 314 行，「必须是 1 到 10 的整数」）、`[setup]`；解析结果与默认值在
  `crates/autome-domain/src/config.rs`；熔断指纹在 `crates/autome-domain/src/gate_failure.rs`；
  准备命令在 `crates/autome-domain/src/setup.rs` 与 `crates/automed/src/setup.rs`。
- 任务卡正文与附件落盘、首个会话前提交：`crates/automed/src/scheduler.rs`（`write_task_inputs` 一带）与
  `crates/automed/src/store.rs`（`TaskRecord`）；Bug 任务的 prompt 模板在 `protocol/prompts/locate.md`、
  `fix.md`、`fix-review.md`，Bug 协议在 `protocol/bug-protocol.md`。
- 可帮助定位（不作依据）的既往任务文档：`docs/内核支持任务类型-需求-bug/`（K1）、
  `docs/按计划-4-2-节实现-bug-修复-loop/`（K2）、`docs/项目可配置准备命令-依赖变化时由宿主在/`（T-27）、
  `docs/门禁连续因同一原因失败时熔断-停下等人/`（T-28）、`docs/审计角色关闭时-bug-loop-静默跳过复核/`（T-42）、
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`。

**对门禁的影响。** 有若干测试与源码文件里出现 `README.md` / `technical-design` 字样（`grep -rl` 命中
`crates/automed/tests/*.rs`、`crates/automed/src/{dispatch,init,skills,scheduler,git,lib}.rs`、
`crates/autome-cli/tests/cli.rs`）。整理轮没有逐个确认它们是否读取仓库根 `README.md` 或 technical-design 的内容
（多数看起来是测试夹具里的同名文件）；设计轮应确认「改这两份文档不会让任何测试变红」，实现后以
`sh scripts/ci-local.sh` 全绿为证。

## 8. 留给设计轮的待定问题

1. 关键词清单的最终形态：每个关键词落在哪份文档的哪一节，`grep` 命令怎么写（验收 1）。
2. 「代码位置」在 README（面向用户、英文）里怎么呈现，才既可审计又不把 README 变成源码索引（约束 4）。
3. technical-design §5 的 Bug Loop 放成新小节（如 §5.6）还是并入 §5.1/§5.3；§8 的任务卡正文/附件/`Draft`
   同理——前提是不重排已有编号（约束 2）。
4. README 配置一节里 `[setup]`、`same_failure_limit` 已有内容是否与代码完全一致（默认值、取值范围、熔断适用的
   节点、`extend`/`rerun` 之后计数是否重置）；不一致处按代码改。
5. `reply` 对需求任务与 Bug 任务的不同语义、可用的状态（`AwaitInfo`、`NotABug` 等）以代码为准怎么表述。
6. 里程碑怎么切（例如 README 一个、technical-design §5 一个、§8+§14 一个），使审计轮能逐条对照代码。
