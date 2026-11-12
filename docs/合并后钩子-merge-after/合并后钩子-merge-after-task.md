# 任务：合并后钩子 `[merge] after`——宿主在主检出上执行，失败只报警

本任务的规则见 `docs/合并后钩子-merge-after/protocol/loop-protocol.md` 与
`docs/合并后钩子-merge-after/protocol/session-protocol.md`。

- 设计文档：`docs/合并后钩子-merge-after/合并后钩子-merge-after.md`
- 任务卡（用户原文，第一输入；与本文件不一致时以任务卡为准）：
  `docs/合并后钩子-merge-after/合并后钩子-merge-after-card.md`
- 需求原文：合并后钩子 [merge] after：宿主在主检出上执行，失败只报警

## 1. 为什么做

Autome 用自己的 Loop 开发自己：每次含 `crates/` 的任务合并后，都要有人手动
`sh scripts/install-cli.sh` 并重启驱动，否则驱动继续跑旧二进制（2026-11-11 漏装一次，
导致 T-16 被旧 bug 判协议失败）。一般用户也有同样需求：合并后自动部署、重建索引、发通知。
今天的项目配置里没有任何钩子（`crates/autome-domain/src/config.rs`、
`crates/automed/src/config_io.rs` 中无 hook / after_merge / `[merge]`）。

## 2. 目标

项目可以在 `.autome/config.toml` 里配置一条「合并后命令」。任务合并成功之后，由宿主
（automed 内核，不是会话）在**主检出**上执行它；失败不影响任务结果，只报一条项目警报。

## 3. 范围（要做）

1. **配置**：项目 `.autome/config.toml` 新增 `[merge]` 表：

   ```toml
   [merge]
   after = "sh scripts/install-cli.sh"   # 可省；用 sh -c 跑
   after_timeout_minutes = 10            # 可省，缺省 10
   ```

   解析规则与 `[gate]` / `[setup]` 一致：`[merge]` 不是表 → 报错；`after` 去空白后为空 →
   等于没配；`after_timeout_minutes` 须为正整数，缺省 10。
   解析位置 `crates/automed/src/config_io.rs::parse_project`，类型放
   `crates/autome-domain/src/config.rs`（`ProjectConfig` / `ResolvedConfig`）。
2. **时机**：任务合并成功，且（配了 `[gate]` 时）验证 main（`MainCheck`）通过之后、
   清理（`Cleanup`）之前。
3. **执行位置与机制**：在主检出（`project.path`）上，以独立进程组执行；复用 `gate::Job`
   后台作业机制，作业文件在 `.autome/output/gates/<task>/after_merge-<sha>.*`；tick 只轮询、
   不阻塞。给命令的环境变量：`AUTOME_TASK`、`AUTOME_MERGE_COMMIT`。
4. **结果**：
   - 写事件 `merge.after {ok, exit_code, log, tail}`；
   - 失败（非零退出 / 超时）只 `raise_alert("after_merge_failed")`——任务已合并、不能撤回，
     任务照常进入 Done；
   - `autome run` 打印开始、结果两行（`crates/autome-cli/src/render.rs`，与
     `gate.*` / `setup.*` 并列）。
5. **工作区项目**（一个目录下多个独立仓库）：按成员仓库各跑一次，或跳过并写事件
   `merge.after_skipped`——二选一，由设计轮决定并写明确定行为。
6. **文档**：`README.md` 中 `[gate]` 配置说明旁补 `[merge]` 说明（配置示例块与下方的条目）。
7. **测试**：见第 6 节。

## 4. 不做什么（硬性范围边界）

- 不做合并前钩子。
- 不改 `[gate]` / `[setup]` 的语义。
- **不修改本仓库自己的 `.autome/config.toml`**（由用户合并后自行配置）；`.autome/` 下任何
  内容都不改。
- 钩子失败不撤回合并、不让任务失败、不退回任何轮次。
- 不扩大范围：不做其它时机的钩子、不做钩子列表 / 多条命令、不内置「安装 CLI 并重启驱动」
  之类特判。

## 5. 硬性约束

- **未配 `[merge] after` 时零差异**：不产生任何新事件、不新增节点停留、现有测试不改照常通过。
- 钩子在宿主上运行、在主检出上运行；不在任务 worktree 里、不由会话执行。
- 钩子作业不得阻塞 tick / IPC；core 重启后能接上正在跑的作业（`gate::Job` 已有的能力）。
- 超时按失败处理（门禁的做法是退出码 124，`gate::TIMED_OUT`）。
- 事件顺序：`merge.after` 的序号在 `main_check` 之后、`cleanup` 之前。
- 分层：纯类型与校验放 `autome-domain`（无 I/O、无 async、无 SQLite）；文件、进程、git、
  SQLite 放 `automed`；`autome-cli` 只解析参数与打印。
- 测试替身对齐真实接口：e2e 用真实 git、真实 wrapper、真实 `sh -c`，只替换模型 CLI（沿用
  `crates/automed/tests/end_to_end.rs` 的 World / 替身 CLI / `write_gate_script` /
  `setup_fixture` 的做法）。
- `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings` 必须干净。
- 仓库规则（`AGENTS.md`）：只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下其它目录、
  不推送；超过 5 行的命令输出与临时验证产物写入 `.autome/output/`，不进任务文档目录。

## 6. 验收标准（来自任务卡，逐条可测）

1. e2e：配了 `[merge] after` 的任务合并后，主检出上出现钩子产物，且 `merge.after` 事件序号在
   `main_check` 之后、`cleanup` 之前。
2. e2e：钩子失败 → 恰好一条 `after_merge_failed` 警报，任务 Done。
3. e2e：未配时零差异（无 `merge.after` 相关事件）。
4. `cargo test --workspace` 全绿。
5. `README.md` 的 `[gate]` 旁补了 `[merge]` 说明。
6. 本仓库的 `.autome/config.toml` 未被修改。

## 7. 设计需要回答的开放问题（不预设答案）

- **工作区项目**：各成员仓库各跑一次，还是跳过并写 `merge.after_skipped`（任务卡第 4 条
  明确留给设计轮）。若各跑一次：`<sha>` 与 `AUTOME_MERGE_COMMIT` 取哪个仓库的合并提交、
  未改动而被跳过合并的成员是否跑、作业文件放哪个仓库的 `.autome/output/` 下。
  参照：`[setup]` 在工作区里是跳过并每任务写一次 `setup.skipped`（`setup::skip_workspace`）。
- **在状态机里怎么表示**：新节点、还是 `Cleanup` 步骤内的前置子阶段。现状是
  `Merging → MainCheck → Cleanup` 仅在配了 `[gate]` 时经过 MainCheck，否则 `Merging → Cleanup`；
  `[setup]` 的主检出准备就是挂在 `run_core_step(Cleanup)` 开头、未完成时返回等待下一个 tick。
- **与主检出 `[setup]` 的先后**：两者都在合并后、清理前、主检出上运行。谁先谁后，
  准备失败时钩子还跑不跑。
- **MainCheck 失败时钩子跑不跑**：任务卡写「验证 main 通过之后」；今天 MainCheck 失败只报
  `main_gate_failed` 警报、任务照常进 Cleanup。需明确失败时是跳过钩子（是否写事件）还是照跑。
- **主检出的并发**：并行任务（`[loop] parallel`）可能先后合并，两个钩子是否允许同时在
  主检出上跑（`[setup]` 已有「另一任务的主检出准备在跑时等待」的处理，见 scheduler 测试
  `setup_in_the_main_checkout_waits_while_another_tasks_main_setup_is_running`）。
- **事件形状**：任务卡只给了结果事件 `merge.after {ok, exit_code, log, tail}`，又要求
  `autome run` 打印「开始 / 结果两行」。开始那一行由什么事件驱动（是否需要
  `merge.after_started` 之类）、事件里是否带 `command` / `commit`。
- **作业生命周期**：钩子在跑时任务被暂停 / 停止 / 取消、core 重启、作业消失（`Vanished`）
  各自怎么处理（参照 `gate::stop_all`、`discard_node` 与 `[setup]` 的现有处理）。
- **Bug Loop 任务**（`kind = bug`）合并后是否同样执行钩子（任务卡说「任务合并成功」，未区分类型）。
- **界面**：`apps/desktop` / `apps/web` 对未知事件类型与警报 `kind` 是否有回退显示；若有穷举
  校验被新事件 / 新警报破坏，只做让其不坏的最小改动。
- **权威文档同步**：`docs/development/technical-design.md`（§3.3 配置、§8.1 门禁、§9 配置解析）
  是否需要随之更新。

## 8. 项目背景摘要（生成时，2026-11-12，基线提交 `410d7c2`）

**仓库**：单仓库 `autome-v2`（当前 worktree 即其检出，默认分支 `main`；工作目录下没有其它
独立仓库）。Autome 2.0：把一句话需求跑过 Loop（intake → 设计 → 评审 ⇄ 裁决 → [批准设计] →
实现 ⇄ 审计 → 复盘 → rebase → [按合并] → 合并 → 验证 main → 清理），会话为 Claude Code / Codex；
另有 Bug Loop。本仓库自己由 Autome 驱动开发（launchd 代理跑已安装的 `autome` 二进制，
`scripts/install-cli.sh` 负责构建并安装到 `~/.local/bin`）。

**技术栈与布局**：

- Rust workspace（toolchain 1.93.1，edition 2024），成员：
  - `crates/autome-domain` — 纯类型与迁移，无 I/O。`config.rs`（`ProjectConfig`、
    `ResolvedConfig`、`GateConfig`、`SetupConfig`、`DEFAULT_GATE_TIMEOUT_MINUTES`、
    `DEFAULT_SETUP_TIMEOUT_MINUTES`）、`task.rs`（`Node`、`TaskState`、`FailureReason`）、
    `setup.rs`、`project.rs`（`Project::is_workspace`）。
  - `crates/automed` — 一切外部交互：
    - `config_io.rs`：`parse_project` 解析 `[setup]`（约 271 行）与 `[gate]`（约 312 行）；
      `save_project` 用 toml_edit，不认识的表原样保留。
    - `gate.rs`：后台作业 `Job`（`.autome/output/gates/<task>/<node>-<key>.*` 的
      log / pid / exit / started 文件、独立进程组 wrapper、`start` / `poll` / `stop_all` /
      `discard_node` / `tail`、超时 124）。
    - `setup.rs`（T-27，同一套作业机制的最近参照）：`ensure` 返回 `Ready / Pending / Failed`，
      `skip_workspace`。
    - `scheduler.rs`：`run_core_step`（约 2923 行）——`Cleanup` 分支开头先做主检出 `[setup]`，
      失败 `raise_alert("setup_failed")` 并写 `project.alert` 事件，然后 `cleanup(&layout)`；
      `run_gate`（MainCheck 失败处 `raise_alert("main_gate_failed")`，约 3286 行）；`merge_all`。
    - `store.rs`（SQLite、`append_event`、`raise_alert`、`count_events`）、`git.rs`、
      `layout.rs`（`TaskLayout`，区分单仓库与工作区）、`dispatch.rs`（JSON-RPC）。
  - `crates/autome-cli` — `autome` 命令；事件打印在 `src/render.rs` 的 `event_line`
    （`setup.started` / `setup.result` / `gate.started` / `gate.result` 等）。
- `apps/desktop` — Electron 外壳（`node --test`）；`apps/web` — React Router + Vite 面板
  （vitest、eslint、tsc）。
- `protocol/` — Loop 协议源（编译进 automed）；本任务是内核行为，不需要改。
- `plugins/`、`scripts/`（`ci-local.sh`、`install-cli.sh`、打包脚本）。
- `README.md` 约 108–150 行：`[gate]` / `[retry]` / `[setup]` 的配置示例与逐条说明——
  `[merge]` 说明补在这里。
- 权威文档：`docs/development/requirements.md`、`docs/development/technical-design.md`、`docs/adr/`。

**测试命令**：

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --test end_to_end -- --nocapture       # 真实 git、真实 wrapper，只替换模型 CLI
npm test --prefix apps/desktop
npm run lint|typecheck|test|build --prefix apps/web
sh scripts/ci-local.sh                            # 以上全部，即本项目宿主门禁命令（[gate]，超时 30 分钟）
```

**权威规则文件**：

- 仓库根 `AGENTS.md`：不改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/`
  其它目录、不推送；完成本轮即结束会话，不自行启动下一会话；超过 5 行的命令输出与临时验证产物
  写入 `.autome/output/`。
- `.autome/rules/`：只有模板 `README.md`，无额外项目规则。
- `docs/agent-project-profile.md`：不存在。

**参考现场**：

- `[setup]` 任务（T-27）的全部文档：`docs/项目可配置准备命令-依赖变化时由宿主在/`。
- 现有 e2e：`crates/automed/tests/end_to_end.rs` 中 `write_gate_script`（约 2566 行）、
  `setup_fixture` / `setup_events`（约 5394 行起）、
  `setup_runs_in_the_merge_commit_checkout_before_the_main_check`、
  `setup_events_are_absent_when_no_setup_is_configured`。
- 配置解析测试：`config_io.rs` 的 `a_gate_and_a_retry_policy_are_read_from_the_project_file`（约 960 行）。
