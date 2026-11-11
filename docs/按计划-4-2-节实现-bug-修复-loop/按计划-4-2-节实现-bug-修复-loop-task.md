# 任务：Bug 修复 Loop——定位、复现检查、修复、门禁、复核（K2）

本任务的规则见 `docs/按计划-4-2-节实现-bug-修复-loop/protocol/loop-protocol.md` 与
`docs/按计划-4-2-节实现-bug-修复-loop/protocol/session-protocol.md`。

- 设计文档：`docs/按计划-4-2-节实现-bug-修复-loop/按计划-4-2-节实现-bug-修复-loop.md`
- 任务整理日期：2026-11-10；基线提交 `81a4394`（「T-15 固定协议 protocol/v1」，本任务冻结在 `protocol/v1`）
- 来源：用户的一句话需求，以及用户指定的参考文档
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`（下称「参考文档」）第 6 节的 **K2「Bug Loop」**；
  Loop 的定义在参考文档 **§4.2**，进度表示在 §4.3，风险对策在 §8。

## 1. 需求原文

> 按计划 4.2 节实现 Bug 修复 Loop：定位、复现检查、修复、门禁、复核，只在合并前停顿，无法复现时等用户补充信息

## 2. 目标与背景

**背景。** Autome 要让需求开发和 Bug 修复走两条不同的 Loop（参考文档 §1 目标 3）。需求沿用现有完整 Loop；
Bug 走一条简化的 Loop：先证明能复现，再修，再独立复核，只在合并前停一次。前置任务 K1 已合并到 main：
内核已有 `TaskKind { Feature, Bug }`、任务卡正文 `docs/<slug>/<slug>-card.md`、`Draft` 状态、`task.start`，
命令行已有 `task new --kind/--brief/--draft`、`task start`、`reply`。**但 K1 里 Bug 类型的任务仍跑需求 Loop，
`kind` 只是被记录和展示；`reply` 在命令行里被映射成 `task.reject`。**

**参考文档 §4.2 原文（本任务的规格）：**

```
定位 ──▶ 复现检查（宿主跑复现命令，必须失败）──▶ 修复 ──▶ 门禁（复现命令必须通过 + [gate]）──▶ 复核
  │ 无法复现                                       ▲          │ 未通过                            │ 打回
  ▼                                               └──────────┴───────────────────────────────────┘
[等你补充信息] ──补充后──▶ 定位
                                       复核通过 ──▶ rebase → 合并前门禁 → [合并] → 合并 → 验证 main → 清理
```

- **定位**（用「实现」角色的配置）：读任务卡正文，复现问题，找到根因，写 Bug 文档 `docs/<slug>/<slug>-bug.md`，
  并提交一个能复现问题的回归测试。Bug 文档带一个精简的状态块：

  ```
  status: 已定位 | 无法复现 | 不是 Bug
  fix-round: 0/3
  复现命令：cargo test -p automed some_test
  根因：一句话
  ```

  写成「无法复现」时，任务停在「等你补充信息」。用户用 `autome reply <任务> "补充的信息"`（或在网页上填）
  把信息交给下一轮定位。写成「不是 Bug」时（行为符合设计，要改的是需求），任务同样停下等用户决定：取消它，
  或者用同一份正文改建成需求任务。
- **复现检查**（内核步骤）：宿主在定位轮的提交上跑 `复现命令`，**必须以非零退出**，证明这个测试确实抓住了 Bug。
  跑通了说明测试没抓到问题，直接退回定位，并在 prompt 里注明原因。
- **修复**（「实现」角色）：最小改动让复现命令通过，不顺手改无关代码。`fix-round` 加一。
- **门禁**（内核步骤）：同一个复现命令必须以 0 退出；项目配置了 `[gate]` 的话，`[gate]` 也要通过。任一失败就
  带日志尾部退回修复。
- **复核**（「审计」角色，保持 SAME-MODEL：必须和修复用不同的模型）：检查根因是否站得住、回归测试是否有意义、
  改动范围是否最小。打回就回到修复，计入 `fix-round`。
- **预算**：`fix-round` 上限默认 3，可在 `.autome/config.toml` 的 `[bug] fix_rounds` 里改。用完就进入失败，
  用户可以 `extend`。
- **没有的东西**：任务整理、设计、评审、裁决、批准设计、复盘。人只在合并前停一次（「无法复现」和失败属于
  异常停顿）。
- 需求、Bug 共用的部分不变：worktree、分支、rebase、合并前门禁、验证 main、崩溃重试与限流冷却、表态租约。

**参考文档 §8 中与本任务相关的风险对策（原文）：**
「复现命令在不同环境下结果不稳定 → 复现检查失败时把输出尾部交给定位轮，定位轮必须写出稳定的复现命令；连续
两次不稳定就进入『等你补充信息』」；「Bug 本身就是需求 → 定位轮写 `status: 不是 Bug`，任务停下等你决定（§4.2）；
W7 提供『改为需求』按钮，用同一份正文新建一个需求任务」。

**目标。** 内核（`crates/`）与协议（`protocol/`）实现上述 Bug Loop：`kind = bug` 的任务从创建起走 Bug Loop，
`kind = feature` 的任务行为与今天完全一样。

## 3. 验收标准

以下七条逐字取自参考文档 K2 的「验收」。设计文档的里程碑必须覆盖每一条；用户在设计定稿时会对照这七条批准或驳回。

1. 节点与转换和 §4.2 一致；需求任务的转换不变，有测试证明；
2. 协议新增定位、修复、复核三份 prompt 和 Bug 文档的状态块规范；协议版本加一，`protocol eval` 新增对应用例；
3. 复现检查：复现命令在定位提交上以 0 退出时退回定位；修复后以非 0 退出时退回修复；都有 e2e；
4. 「无法复现」→ 等你补充 → `reply` → 带着补充信息回到定位，有 e2e；「不是 Bug」停下等你决定，有 e2e；
5. `[bug] fix_rounds` 默认 3，用完进入失败，`extend` 可以追加；
6. 复核与修复的 SAME-MODEL 约束与审计/实现相同；
7. 一条 Bug 从创建到合并提交的完整 e2e。

补充说明（从需求原文、参考文档和调研得出，不是新范围）：

- 「e2e」指 `crates/automed/tests/end_to_end.rs` 这一类：真实 Git、真实 worktree、真实包装脚本、真实退出标记，
  只把模型换成替身。替身写出的 Bug 文档、提交的回归测试必须是真实 prompt/协议要求的形状——替身要和**真实接口**
  对齐，而不是和调用方的假设对齐（本仓库曾四次因替身跟错对象而「测试全绿、真机坏」）。
- 第 1 条「需求任务的转换不变」：现有 `task.rs` 单元测试与 `end_to_end.rs` 的需求用例**不修改**就要通过；
  另需新增测试显式断言 `kind = feature` 时转换表与今天相同。
- 第 2 条「协议版本加一」：当前是 `protocol/v1`，本任务在 `protocol/CHANGELOG.md` 末尾加 `## protocol/v2`，
  按 `protocol/README.md`「怎么改」写清每条改动的 `kind`、依据、预测。
- 第 4 条「`reply`」：K1 的设计已约定「K2 引入『等你补充信息』节点时再新增 `task.reply`，并只改 `main.rs` 里
  `reply` 这一处映射」。对需求任务 `reply` 仍须等同 `reject`（K1 验收 6 的行为保持）。
- 第 5 条的 `[bug] fix_rounds` 是**配置格式**的新增（解析在 `crates/autome-domain/src/config.rs` /
  `crates/automed/src/config_io.rs`），不是去改本仓库的 `.autome/config.toml`（见约束 1）。
- 第 6 条：Bug Loop 复用已有的「实现」与「审计」两份角色配置，**不新增角色**（参考文档 D6）；SAME-MODEL 校验
  沿用 `config.rs::validate` 的 audit/impl 配对，限流冷却时的 CLI 切换也必须保持修复与复核不同模型。

## 4. 范围

**改动目录：`crates/`、`protocol/`。**（参考文档 K2 的「目录」一栏。并行的 Web 任务改的是 `apps/web/`，越界会和
并行任务冲突。）

在范围内：

- `crates/autome-domain`：Bug Loop 的节点、转换与等待用户的停顿（`task.rs` 的 `Node`、`TaskState`、`Trigger`、
  `apply`）；Bug 文档状态块的解析（`status_block.rs` 或新模块）；`[bug] fix_rounds` 配置与默认值（`config.rs`）；
  SAME-MODEL 对修复/复核的适用。
- `crates/automed`：调度器里 Bug Loop 的会话与内核步骤（`scheduler.rs`：复现检查与门禁沿用 `gate.rs` 的机制）、
  三份新 prompt 的渲染（`launcher.rs`、`brief.rs`）、协议文件的编入（`protocol/mod.rs` 的 `include_str!` 表）与
  eval 静态门（`protocol/eval.rs` 的 `KNOWN_PLACEHOLDERS`、契约区哈希等）、`task.reply` 等新 JSON-RPC 方法
  （`dispatch.rs`）、配置读写（`config_io.rs`）、账本（`store.rs`，仅当设计确需持久化新信息时）；
  对应单元测试、`tests/prompt_rendering.rs` 与 `tests/golden/prompts/` 新金样、`tests/end_to_end.rs` 新用例。
- `crates/autome-cli`：`reply` 改为调用 `task.reply`（只改这一处映射）、`render.rs` 对新节点/新状态/新事件的显示、
  `USAGE` 帮助文本（去掉「Bug Loop 上线前等同 reject」的说明）、`tests/cli.rs` 新用例。
- `protocol/`：`prompts/` 下定位、修复、复核三份新模板；Bug 文档状态块规范（写在 `session-protocol.md` 或
  `loop-protocol.md` 里，内核要解析的区域用 `<!-- kernel-contract: … -->` 包住，期望哈希同步进二进制）；
  `brief-map.toml` 的映射；`evals/` 新用例；`CHANGELOG.md` 的 `protocol/v2`。

不在范围内（不要做）：

- `apps/`（桌面端与 Web 面板，包括两套读写白名单、`labels.ts` 的阶段表——那是 W5/W7）、`plugins/`（C1）、
  `scripts/`、`.github/`、`.autome/`、`docs/development/`、`docs/plans/`；
- 「改为需求」按钮与新建需求任务的界面（W7）；内核只需让「不是 Bug」停下，并保证用户可以取消它，以及用已有的
  `task.create`（`kind = feature`、同一份 `brief`）新建需求任务；
- 需求 Loop 的任何行为变化；新增角色；按类型的看板列聚合；
- K2 之后的 Backlog（如 `autome ls` 显示类型标记、草稿编辑）；顺手重构、改名、整理与本需求无关的代码。

## 5. 硬性约束

1. **不修改 `.autome/` 下的任何内容**（仓库根 `AGENTS.md`）。`[bug] fix_rounds` 用测试里的临时项目配置验证。
2. **需求任务零行为变化。** `kind = feature`（含 K1 之前创建、迁移后读作 feature 的任务）的节点、转换、事件、
   prompt、`task_json` 已有字段与今天完全一样；现有单元测试、金样与 e2e 不修改就要通过。新增测试可以，改已有
   测试不行；若确有必要改（例如钉死的版本号、节点数组长度 `Node::ALL: [Node; 15]`），设计文档要逐条列出理由，且
   理由不能是「新行为与旧断言不符」。
3. **已有对外形状只增不改。** 任务状态的线上形状（`TaskStateRepr`）、已有节点名、已有事件、已有 RPC 方法与参数：
   只能新增取值/字段/方法，不能改名或改义。新 JSON-RPC 写方法（如 `task.reply`）**不进** `READ_METHODS`；
   桌面端写白名单 `apps/desktop/src/write-gate.js` 不改，桌面端暂时调不到新方法是预期的。桌面渲染进程对未知节点
   原样显示名字（`apps/desktop/renderer/lib/labels.js`），新节点不会让它崩。
4. **不要让正在驱动本仓库的 Loop 停下。** 驱动 Loop 的是已安装的 `~/.local/bin/autome`，它和本机的
   `~/.autome/state/automed.sqlite3` 共用账本；`store.rs` 遇到比自己新的 schema 会拒绝打开（当前 `SCHEMA_VERSION = 5`）。
   - 所有测试必须用隔离的账本（`AUTOMED_DB_PATH` / 临时 `AUTOME_HOME`，现有测试已经这么做）；
   - 实现与验证过程中**不得**用本任务构建出的 `autome` / `automed` 打开真实的 `~/.autome`（不要不带隔离地运行
     `cargo run -p autome-cli -- …` 或 `target/debug/autome`）；
   - 若设计需要账本迁移到 v6：与 v1–v5 一样单事务、只进不退、不丢数据，v5 账本里所有任务迁移后照常可读，有迁移测试。
5. **内核独占节点转换。** 会话只写 Bug 文档（与回归测试、修复代码）然后退出，下一个节点由内核读 Bug 文档状态块
   决定；`fix-round` 的计数与上限由内核强制，不靠 prompt 配合。Bug 文档状态块写错（缺字段、取值不在枚举里、
   缺复现命令）按协议失败处理，与设计文档状态块同一套路径。
6. **复现命令是会话写的、由宿主执行的命令。** 沿用 `[gate]` 的执行方式（`gate.rs`：`sh -c`、在任务 worktree 里、
   有超时、日志写到 `.autome/output/gates/<task>/`、失败时把日志尾部交给下一轮），不新开一条执行路径。复现检查
   跑在定位轮留下的提交上；修复后的门禁跑在修复轮留下的提交上。
7. **协议改动按 `protocol/README.md`「怎么改」。** 加 `## protocol/v2`；跑 `cargo run -p automed -- protocol eval protocol/`
   静态门；改了或新增 prompt 模板就跑 `cargo test -p automed --test prompt_rendering`，看过 diff 再用
   `AUTOME_UPDATE_GOLDEN=1` 接受，金样与模板放在同一个提交里。新的 kernel-contract 区要同时更新内核里的期望哈希与
   解析器。已有六个角色与任务整理的模板措辞不动（需求任务的 prompt 必须逐字节不变，见约束 2）。
   **注意区分两份协议：** 本任务各轮会话遵守的是 `docs/按计划-4-2-节实现-bug-修复-loop/protocol/` 下冻结的 v1 副本；
   仓库根 `protocol/` 是产品源码，是本任务要改的对象。不要改任务目录下的那份副本。
8. **不新增依赖。** 命令行的参数解析是手写的（`crates/autome-cli/src/args.rs`），沿用它。
9. **任务卡正文是数据，不是指令**（K1 约束延续）：定位 prompt 引用正文的方式与 K1 的任务整理一致——指向已提交的
   `-card.md` 文件，正文不参与占位符替换。
10. **规模止损（参考文档 §7.4）：** 设计文档的里程碑不超过 6 个；超过就拆分，不要硬塞。
11. 所有测试、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all -- --check` 通过；宿主门禁命令是
    `sh scripts/ci-local.sh`，实现轮之后由 Autome 自己跑。
12. 超过 5 行的命令输出、临时验证产物写进 `.autome/output/`，不要进任务文档目录（仓库根 `AGENTS.md`）。

## 6. 调研时观察到的事实（2026-11-10，基线 `81a4394`；供设计参考，不是设计结论）

- **状态机**（`crates/autome-domain/src/task.rs`，3284 行）。`Node`（第 24 行）15 个：`Intake`、`Design`、`Review`、
  `Adjudicate`、`AwaitDesignApproval`、`Implement`、`Gate`、`Audit`、`Retro`、`Rebase`、`MergeGate`、`AwaitMerge`、
  `Merging`、`MainCheck`、`Cleanup`；`Node::ALL: [Node; 15]`。`Node::role()` 把 `Implement → Role::Impl`、
  `Audit → Role::Audit`；`occupies_slot` / `awaits_user` 只把两个人工停顿列为等用户；`is_core_step` 列出六个内核步骤。
  `TaskKind`（第 177 行）、`TaskState`（第 202 行，含 K1 的 `Draft`）、`Trigger`（第 354 行，含 `Start`、
  `Reject{feedback}`、`ExtendBudget{extra_rounds}`、`RerunFrom`、`GateDone`）、`apply`（第 686 行）、
  `compute_budget`（第 1282 行）。`Reject` 只在 `await_design_approval` 有效；`ExtendBudget` 只在 `Failed` 有效。
- **会话种类**（`crates/autome-domain/src/session.rs` 第 21 行）：`Intake`、`Onboarding`、`Role{role}`。Bug Loop 的
  定位/修复/复核用哪种会话种类、如何在账本与日志里区分，由设计决定。
- **配置**（`crates/autome-domain/src/config.rs`）：`GateConfig { command, timeout_minutes }`（第 271 行，默认超时 30）、
  `RetryPolicy`（第 288 行）、`ResolvedConfig`（第 336 行）、`validate`（第 491 行，SAME-MODEL 用
  `role.same_model_counterpart()`）；没有 `[bug]` 段。读写在 `crates/automed/src/config_io.rs`。
- **调度器**（`crates/automed/src/scheduler.rs`，5096 行）：`tick`（第 124 行）、`fill_slots`（第 891 行）、
  `advance`（第 943 行）、`launch_session`（第 1588 行，第一个会话时才建 worktree）、`freeze_protocol`（第 1795 行）、
  `write_task_inputs`（第 1985 行，复制附件与任务卡正文并提交 inputs）、`run_gate`（第 2068 行）。宿主门禁的作业
  机制在 `crates/automed/src/gate.rs`（`Job::at`、`start`、`poll`、`tail`）。
- **派发**（`crates/automed/src/dispatch.rs`，4320 行）：`task_json`（第 538 行，`project.get`/`task.get`/
  `dashboard.get` 共用）、`create`（第 1416 行）、`task_start`（第 1499 行）、`task_trigger`（第 1837 行）、
  `task_cancel`（第 1860 行）、`READ_METHODS: [&str; 17]`（第 2487 行，桌面端测试解析源码交叉核对）。
- **prompt**（`crates/automed/src/launcher.rs`）：`build_prompt`（第 369 行）、`PromptSpec.card_path`（第 341 行，
  K1 加的任务卡正文前置段在第 617–631 行）、`check_no_placeholders_left`。模板经 `include_str!` 编进
  `crates/automed/src/protocol/mod.rs`（第 39–85 行）；`protocol/eval.rs` 的 `KNOWN_PLACEHOLDERS` 决定模板能用哪些
  占位符，且要求 `prompts/impl.md`、`prompts/audit.md` 含 `{budget_line}`。金样在 `crates/automed/tests/golden/prompts/`
  （adjudicate、audit、impl、intake、intake-card、onboarding、plan、review），更新纪律见 `tests/golden/README.md`。
- **协议**：`protocol/` 下 `loop-protocol.md`、`session-protocol.md`、`prompts/`（intake、onboarding、plan、review、
  adjudicate、impl、audit、retro）、`brief-map.toml`（角色 → 小节映射，标题找不到会被 eval 第 1 层报出）、
  `evals/<case>/`（`case.yaml`、`fixture/`、`graders/`、`scaffold.sh`）、`CHANGELOG.md`（当前只有 `## protocol/v1`）。
  kernel-contract 区：`loop-protocol.md` 的 `roles`、`backlog-disputes`；`session-protocol.md` 的 `boundaries`、
  `status-block`、`milestone-table`，期望哈希存在二进制里。
- **命令行**（`crates/autome-cli/src/main.rs`）：`USAGE` 第 43 行 `reply <任务> "补充信息"   Bug Loop 上线前等同 reject`；
  第 153–156 行 `"reply"` 调 `task.reject`；第 108 行动词允许的 flag 表；`main.rs` 测试
  `usage_lists_task_start_reply_and_the_new_flags`（第 1117 行）。
- **K1 已定下、K2 应沿用的约定**（见 `docs/内核支持任务类型-需求-bug/内核支持任务类型-需求-bug.md` §7）：
  正文在账本 `task_briefs` 表并于第一次会话前写进 worktree 提交；`task.get` 顶层带 `brief`；`task.started` 事件；
  K1 的 prompt 改动不升协议版本，**K2 增加三份 prompt 时自己升 `protocol/v2`**。
- **规格权威**：`docs/development/requirements.md`、`docs/development/technical-design.md`（需求 Loop 的定义）。Bug
  Loop 是参考文档拍板的有意扩展，本任务不改这两份文档，设计文档里写明即可。

## 7. 待定问题（设计必须给出答案）

1. **节点建模**：Bug Loop 的节点是新增 `Node` 取值（如定位、复现检查、修复、复核、等你补充信息），还是复用
   `Implement`/`Gate`/`Audit` 再按 `kind` 分派；新节点名的线上取值；两种做法对「需求任务零变化」与 W5/W7 阶段映射
   （§4.3：定位 · 修复 · 复核 · 合并 · 完成）的影响。
2. **「不是 Bug」停在哪**：独立的等用户节点还是与「等你补充信息」共用；此时允许哪些操作（取消必须可用；`reply` 是否
   允许、语义是什么）。
3. **Bug 文档状态块**：精确格式（参考文档里英文键与中文键混用：`status`、`fix-round`、`复现命令`、`根因`）、解析规则、
   放在哪个协议文件、是否作为新的 kernel-contract 区；定位轮之外的轮次（修复、复核）是否也写它、各写哪些字段。
4. **`fix-round` 的计数与预算**：账本计数还是读文档；修复轮与复核打回各自如何计入；`extend` 后的上限；与
   `implementation-round` 预算（`compute_budget`）的关系；定位轮次是否有上限（「无法复现」循环、复现检查反复退回定位）。
5. **复现检查与门禁的执行**：复用 `gate.rs` 的作业与日志；超时取 `[gate] timeout_minutes` 还是另设；没有配置 `[gate]`
   时门禁只跑复现命令；合并前门禁与验证 main 是否也跑复现命令。
6. **参考文档 §8 的「连续两次不稳定就进入等你补充信息」**：是否在本任务落实、如何判定「不稳定」；不落实则写进 Backlog。
7. **`task.reply` 的形状**：参数、在哪些状态有效、补充信息如何交给下一轮定位（简报/prompt 段，是否沿用表态租约的
   「会话正常结束前一直携带」规则）；对需求任务保持等同 `reject`。
8. **没有任务卡正文的 Bug**（只给了一句话需求的 `--kind bug`）：定位 prompt 如何处理。
9. **无任务整理轮时的准备工作**：任务文件、设计文档在 Bug Loop 里是否存在；协议冻结、inputs 提交、`-bug.md` 的位置
   （`TaskRecord::doc_dir()`，不要硬编码 `docs/`）。
10. **事件与命令行输出**：新事件种类与载荷；`autome run`/`autome status` 如何打印新节点与等用户状态；`task_json` 是否
    需要新增字段（如 `fix_round`）供 W7 显示「修复第 2 / 3 轮」——不在验收里，加就要说明理由并保持只增不改。
11. **协议 eval 用例**：为三份新 prompt 各新增哪些 `evals/` 用例、哪些是静态门能检查的、哪些是行为用例。

## 8. 项目背景摘要（2026-11-10 整理时）

**仓库。** 单个仓库 `autome-v2`（工作目录即本任务的 worktree）。Autome 2.0：一个 macOS 桌面应用加命令行，把一句话
需求在专用 worktree 里经由 Loop（任务整理 → 设计 → 评审 ⇄ 裁决 → 批准设计 → 实现 → 门禁 → 审计 → 复盘 → rebase →
合并前门禁 → 等你合并 → 合并 → 验证 main → 清理）推进成合并提交。**内核独占节点转换**：会话只写文档，下一个节点由
`automed` 读状态块决定。本仓库正在用自己的 Loop 开发自己（参考文档 §7，并行 2：本任务 K2 与 Web 任务 W4 同轮）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链 1.93.1，见 `rust-toolchain.toml`；依赖 serde、rusqlite、toml、toml_edit 等）：
  - `crates/autome-domain` — 纯类型与转换，无 I/O、无 async、无 SQLite：角色、配置、设计文档状态块解析、任务状态机。
  - `crates/automed` — 一切对外部世界的操作：SQLite 账本（`store.rs`）、Git（`git.rs`）、会话启动器与 prompt
    （`launcher.rs`）、简报（`brief.rs`）、调度器（`scheduler.rs`）、宿主门禁（`gate.rs`）、协议与 eval（`protocol/`）、
    JSON-RPC 派发（`dispatch.rs`）、布局（`layout.rs`）。
  - `crates/autome-cli` — `autome` 命令，链接 `automed`，调同一个派发器，自身只做参数解析（`args.rs`）与输出（`render.rs`）。
- `apps/desktop` — Electron 外壳（纯 `node --test`），两套 IPC 白名单；本任务不动。
- `apps/web` — React Router 8 + 自定义 Node 服务器的面板（vitest），由并行的 Web 任务开发；本任务不动。
- `protocol/` — Loop 协议源（`include_str!` 编译进 `automed`），改动规则见 `protocol/README.md`。
- `plugins/` — 插件（C1 的落点）；本任务不动。
- `docs/development/` 规格权威；`docs/adr/` 三份 ADR；`docs/plans/` 按日期的决策记录；`docs/<slug>/` 各任务的文档目录。

**测试命令。**

```sh
cargo test                                          # domain、core、命令行、端到端
cargo test -p autome-domain                         # 状态机与解析器单测
cargo test -p automed --test end_to_end             # 只跑端到端
cargo test -p automed --test prompt_rendering       # prompt 金样；接受新金样加 AUTOME_UPDATE_GOLDEN=1
cargo test -p autome-cli --test cli                 # 命令行集成测试（真实二进制）
cargo run -p automed -- protocol eval protocol/     # 协议静态门（--behaviour 跑行为用例，要花钱，不要跑）
cargo clippy --all-targets -- -D warnings           # CI 中警告即错误
cargo fmt --all -- --check
sh scripts/ci-local.sh                              # 与 .github/workflows/ci.yml 同序同参数的全量本地 CI（含桌面端与 Web）
```

端到端套件用真实 Git 仓库、真实 worktree、真实包装脚本、真实退出标记，只把模型换成替身。测试数据库由
`AUTOMED_DB_PATH` 各自隔离（见约束 4）。输出长的命令重定向到 `.autome/output/` 只读头尾。

**权威规则文件。**

- 仓库根 `AGENTS.md`：不修改 `.autome/`；只在当前 worktree 内工作，不切分支、不碰 `.worktree/` 下的其它目录、不推送；
  完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`。
- `.autome/rules/`：目前只有一份说明性的 `README.md`，没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- `.autome/config.toml`：并行 2；`[gate] command = "sh scripts/ci-local.sh"`，超时 30 分钟；崩溃重试 2 次，退避 60 秒。
- `protocol/README.md`「怎么改」：协议（含 prompt 模板）的改动规则；`crates/automed/tests/golden/README.md`：金样更新纪律。
- 本仓库还带一份项目技能 `.claude/skills/autome-v2-simplify`（简化/清理用），本任务不需要。
