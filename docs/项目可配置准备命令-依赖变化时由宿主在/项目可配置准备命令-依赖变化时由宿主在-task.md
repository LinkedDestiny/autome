# 任务：项目可配置准备命令，依赖变化时由宿主在 worktree 里安装

本任务的规则见 `docs/项目可配置准备命令-依赖变化时由宿主在/protocol/loop-protocol.md` 与
`docs/项目可配置准备命令-依赖变化时由宿主在/protocol/session-protocol.md`。

- 设计文档：`docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在.md`
- 任务卡（用户原文，第一输入）：`docs/项目可配置准备命令-依赖变化时由宿主在/项目可配置准备命令-依赖变化时由宿主在-card.md`
- 需求原文：项目可配置准备命令，依赖变化时由宿主在 worktree 里安装

## 1. 为什么做

实现会话（Codex）的沙箱没有网络。T-14 给 `apps/web` 加了新依赖 `ws`，`package.json`
与 lock 都写对了，但任务 worktree 里的 `node_modules` 没有它；会话装不上（ENOTFOUND /
ENOTCACHED），宿主门禁（`sh scripts/ci-local.sh`）每轮都报「Cannot find module 'ws'」，
T-14 因此连续白跑 17 个实现轮，最后靠人在宿主上手动 `npm install` 才解开。合并之后，
用户主检出的 `node_modules` 同样不会跟着更新。

根因：依赖安装需要网络，只有宿主（automed 自己）有网络；而今天宿主从不替任务安装依赖。

## 2. 目标

在项目配置里增加一条「准备命令」，由宿主（内核）而不是会话，在需要的时机于 worktree
（以及合并后的主检出）里执行；只在依赖声明文件变化时重跑。

## 3. 范围（要做）

1. **配置**：项目 `.autome/config.toml` 新增 `[setup]` 表：
   - `command`：字符串，用 `sh -c` 在 worktree 根目录执行，例如 `npm ci --prefix apps/web`；
   - `watch`：文件路径列表（相对 worktree 根），例如 `["apps/web/package-lock.json"]`，
     其中任一文件的内容哈希变化即触发重跑。
   - 与 `[gate]` 一样是**项目级**配置（解析位置：`crates/automed/src/config_io.rs`，类型：
     `crates/autome-domain/src/config.rs` 的 `ProjectConfig` / 生效配置）。缺省、校验、
     「没写 command 怎么办」「空 command 等于没配」等语义参照 `[gate]` 现有处理并在设计中写明。
2. **三个运行时机**：
   - (a) 建 worktree 之后、任务第一个会话之前（`scheduler.rs` 中懒建 worktree 的那段，
     约 `git::worktree_add` 调用处）；
   - (b) 每次门禁之前：若 `watch` 中任一文件的内容哈希与「该检出上次成功准备时」不同，
     先跑准备再跑门禁；相同则不跑；
   - (c) 合并到默认分支之后，在**主检出**（项目根，`project.path`）里按同样的 `watch`
     哈希规则判断一次，变了才跑。
3. **失败语义**：准备命令失败（非零退出 / 超时）→ 任务进入失败（`TaskState::Failed`），
   失败原因写明命令、退出码与日志路径；**不**当作门禁失败退回实现轮（会话无法修复环境）。
   合并后主检出那一次的失败如何呈现（任务已合并、不能撤回）由设计决定，需与 MainCheck
   失败只写项目报警的现有做法对齐并说明理由。
4. **执行机制**：复用门禁的后台作业机制（`crates/automed/src/gate.rs`：wrapper、独立进程组、
   pid / exit 标记 / 日志文件、超时、core 重启后能接上），tick 只轮询、不阻塞 IPC。
   写 `setup.started` / `setup.result` 事件；`autome run` 打印它们（`crates/autome-cli/src/render.rs`
   中与 `gate.started` / `gate.result` 并列）。
5. **测试**：见第 6 节验收标准。

## 4. 不做什么（硬性范围边界）

- 不给会话开网络，不改会话沙箱、不改会话启动参数。
- 不自动猜测包管理器，不内置任何 npm/cargo 特判；没配 `[setup]` 时行为与今天**完全一样**
  （不产生新事件、不新增节点停留、不改变任何现有测试的结果）。
- **不修改本仓库自己的 `.autome/config.toml`**（由用户配置）；`.autome/` 下任何内容都不改。
- 不改 Loop 协议文本（`protocol/`）——这是内核行为，不需要会话配合；若设计认为必须改协议，
  须在设计文档里单独论证并提交裁决。
- 界面（`apps/desktop`、`apps/web`）展示准备事件不在必做范围；若某处对事件类型做穷举校验
  导致新事件破坏现有界面测试，只做让其不坏的最小改动。

## 5. 硬性约束

- 没配 `[setup]` 时，所有现有测试**不改**照常通过（`cargo test --workspace --all-targets`、
  `cargo test --test end_to_end`、`npm test --prefix apps/desktop`、`apps/web` 的 lint/typecheck/test/build）。
- 「上次成功准备时的哈希」必须绑定到具体检出：任务 worktree 一份、主检出一份；失败的准备不更新它。
  持久化位置（事件、SQLite、`.autome/output/` 下文件）由设计决定，但 core 重启后必须仍然有效，
  且不能写进会被提交的路径。
- 准备作业的文件与门禁作业同在 `.autome/output/` 下（gitignored），日志路径要出现在事件与失败原因里。
- 超时：设计须给出准备命令的超时来源（新字段或沿用某默认值），超时按失败处理（退出码 124，同门禁）。
- 分层：纯类型与状态迁移放 `autome-domain`（无 I/O、无 async、无 SQLite）；文件、进程、git、
  SQLite 放 `automed`；`autome-cli` 只解析参数与打印。
- 测试替身必须对齐真实接口：e2e 用真实 git worktree、真实 wrapper、真实 `sh -c`，只替换模型 CLI
  （沿用 `crates/automed/tests/end_to_end.rs` 现有 World / 替身 CLI / `write_gate_script` 的做法）。
- `cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 必须干净。

## 6. 验收标准（来自任务卡，逐条可测）

1. 没配 `[setup]` 时所有现有测试不改照常通过。
2. e2e：配置一个会在 worktree 写标记文件的准备命令，任务第一个会话启动之前标记文件已存在。
3. e2e：会话修改 `watch` 中的文件后，下一次门禁之前准备命令重跑一次；未修改时不重跑。
4. e2e：准备命令失败时任务失败，原因含命令与退出码，且没有启动门禁（无 `gate.started`）。
5. 合并后主检出的准备同样按 `watch` 判断（变了跑、没变不跑），有测试。
6. 本仓库的 `.autome/config.toml` 未被修改。

## 7. 设计需要回答的开放问题（不预设答案）

- 门禁节点很多：功能 Loop 的 Gate / MergeGate / MainCheck，Bug Loop 的 ReproCheck / FixGate /
  MergeGate。「每次门禁之前」逐一覆盖到哪些，各自在哪个检出上判断。
- MainCheck 今天跑在 detached 的 `.worktree/_verify-<sha>` 检出里，不是用户的主检出；
  那里同样没有依赖。准备命令要不要在 `_verify` 检出里也跑、与「主检出里判断一次」的关系如何。
  任务卡只要求主检出，若设计扩到 `_verify` 须说明这是让 MainCheck 不误报的必要条件。
- 工作区项目（一个目录下多个独立仓库）：「worktree 根目录」指 `TaskLayout::cwd` 还是每个成员仓库；
  `watch` 路径相对谁；主检出有多个时怎么办。可以明确只支持单仓库并在工作区里给出确定行为
  （跳过并记事件 / 拒绝配置），但必须写清楚。
- 准备运行在 Loop 状态机里怎么表示：新节点、门禁节点内的前置子阶段，还是调度器里的旁路步骤；
  对 `domain::task` 状态机与 `FailureReason`（是否新增变体，如 `Setup { command, exit_code, log }`）的影响。
- 任务暂停 / 停止 / 取消 / 从某节点重跑时，正在跑的准备作业怎么处理（参照 `gate::stop_all`、
  `discard_node`）。
- 首次准备（时机 a）在 worktree 已存在时（core 重启、任务恢复）是否重跑：建议以哈希记录为准。

## 8. 项目背景摘要（生成时，2026-11-11）

**仓库**：单仓库 `autome-v2`（当前 worktree 即其检出，默认分支 `main`）。macOS 桌面应用
Autome 2.0：把一句话需求跑过 Loop（intake → 设计 → 评审 ⇄ 裁决 → [批准设计] → 实现 ⇄ 审计 →
复盘 → rebase → [按合并] → 合并 → 清理），会话为 Claude Code / Codex。

**技术栈与布局**：
- Rust workspace（toolchain 1.93.1，edition 2024），`Cargo.toml` 成员：
  - `crates/autome-domain` — 纯类型与迁移：配置覆盖与校验（`config.rs`）、任务状态机
    （`task.rs`：`Node`、`TaskState`、`FailureReason`、`Trigger`）、状态块解析等。无 I/O。
  - `crates/automed` — 一切外部交互：SQLite（`store.rs`，事件 `append_event`/`last_event`/`count_events`）、
    git（`git.rs`：`worktree_add`、`worktree_add_detached`、`head_sha`）、调度器（`scheduler.rs`：
    `run_gate`、`finish_gate`、`gate_head`、`verify_dir`、`merge_all`、`cleanup`、懒建 worktree）、
    宿主门禁后台作业（`gate.rs`：`Job`、`start`、`poll`、`stop_all`、`discard_node`、`tail`、`TIMED_OUT=124`）、
    项目配置读写（`config_io.rs`，`[gate]`/`[retry]` 解析与测试）、布局（`layout.rs`：`TaskLayout`）、
    JSON-RPC（`dispatch.rs`）。
  - `crates/autome-cli` — `autome` 命令；事件打印在 `src/render.rs`。
- `apps/desktop` — Electron 外壳，`node --test`；`apps/web` — React Router + Vite 面板（vitest、eslint、tsc）。
- `protocol/` — Loop 协议源（编译进 automed）；本任务不改。
- 权威文档：`docs/development/requirements.md`、`docs/development/technical-design.md`
  （§3.3 配置文件、§8.1 驱动锁/门禁/重试、§9 配置解析、§16 测试与质量门）、`docs/adr/`。
  若本任务改变了 §8.1 / §9 描述的行为，应同步更新技术设计与 `README.md` 中 `[gate]` 那段的配置说明。

**测试命令**：
```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --test end_to_end -- --nocapture       # 真实 git、真实 wrapper，只替换模型 CLI
npm test --prefix apps/desktop
npm run lint|typecheck|test|build --prefix apps/web
sh scripts/ci-local.sh                            # 以上全部，即本项目宿主门禁命令
```

**权威规则文件**：
- 仓库根 `AGENTS.md`：不改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 其它目录、
  不推送；完成本轮即结束会话，不自行启动下一会话；超过 5 行的命令输出与临时验证产物写入
  `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有模板 `README.md`，无额外项目规则。
- `docs/agent-project-profile.md`：不存在。

**参考现场**：
- T-14 门禁日志：主检出 `.autome/output/gates/T-14/`（「Cannot find module 'ws'」）。
- 现有门禁 e2e：`crates/automed/tests/end_to_end.rs` 的「The host-run gate」一节
  （`write_gate_script`、`run_gated_task`、`a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`）。
- 门禁配置解析测试：`config_io.rs` 的 `a_gate_and_a_retry_policy_are_read_from_the_project_file`。
