# 任务：排队任务的依赖与顺序——`--after` 声明依赖、`queue move` 调整顺序、出队跳过依赖未完成的任务

本任务的规则见 `docs/排队任务可以调整顺序并声明依赖-task/protocol/loop-protocol.md` 与
`docs/排队任务可以调整顺序并声明依赖-task/protocol/session-protocol.md`。

- 设计文档：`docs/排队任务可以调整顺序并声明依赖-task/排队任务可以调整顺序并声明依赖-task.md`
- 任务卡（用户写的，第一输入）：`docs/排队任务可以调整顺序并声明依赖-task/排队任务可以调整顺序并声明依赖-task-card.md`
- 任务整理日期：2026-11-13
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `016f6a9`

## 1. 需求原文

> 排队任务可以调整顺序并声明依赖：task new --after、autome after、autome queue move，fill_slots 跳过依赖未完成的任务

任务卡正文比这句话更具体；两者不一致时以任务卡为准。下面 §2–§5 照任务卡整理，§6–§7 是整理轮从调研得出的约束与
待定问题，§8–§9 是代码现状与项目背景。

## 2. 背景与目标

**背景（任务卡原文的事实）。** 排队任务现在严格按进入排队的先后出队（`store.queued_tasks` 按 `queued_seq, created_at`
排序，`fill_slots` 从头取）。没有办法把一个排队任务提前或推后，也没有办法说「这个任务要等另一个任务合并之后再开始」。

两个真实场景：

- `docs/plans/2026-11-10-web-dashboard-via-loop.md` §7.3 要求 W6 在 W5 之后、W7 在 K2 之后、W10 在 W12 之前。现在只能
  靠人工晚一点创建，或者创建成待开始再手工 `task start`，等待的人要一直盯着。
- 2026-11-12 项目里排了 9 个任务（T-44 到 T-52），其中 T-51、T-47 是其它任务的减速带，应该先做；T-52 依赖 T-41 的
  改动。现在它们只能按编号顺序等。

**目标。** 队列的先后和依赖成为内核的控制面：任务可以声明「在哪些任务之后」，出队时跳过依赖未完成的任务；排队与
待开始任务可以被移动到队列里的指定位置；这两件事在 RPC、CLI 与只读输出里都可见，Web 网关放行两个新写方法。

## 3. 要做什么（任务卡「要做什么」五条，逐条照录）

1. **依赖**：任务可以声明「在哪些任务之后」。
   - `task.create` 新增 `after: ["T-41"]`（同项目的任务 id 列表，可空）；命令行 `autome task new … --after T-41[,T-47]`。
   - 新 RPC `task.set_after { task, after: [...] }`，可以改、可以清空；命令行 `autome after <任务> T-41,T-47` 和
     `autome after <任务> --none`。只允许对待开始、排队、等你决定（设计卡点之前）的任务修改；已经开始实现轮的任务
     拒绝并说明原因。
   - 依赖只能指向同项目的任务；指向自己、形成环、指向不存在的任务，都在创建或修改时拒绝。
   - store 迁移一个版本，旧任务没有依赖。
2. **出队规则**：`fill_slots` 跳过任何一个依赖还没有到已完成的排队任务，但不改它的 `queued_seq`，后面的任务可以先走。
   依赖到已完成（Done）就自然轮到它。
   - 依赖被取消、失败或归档：被依赖的任务**不**自动开始，也不失败；它留在排队，`ls` / `status` / `dashboard.get`
     写明「等 T-41（已取消）」，并写一条事件 `task.blocked { task, on, reason }`，通知器能据此提醒用户。用户用
     `autome after … --none` 解除后照常出队。
3. **顺序**：新 RPC `task.move { task, before?: id, after?: id, top?: bool }`，只对排队和待开始任务有效（待开始的任务
   `task start` 时按它新的位置插入）。命令行 `autome queue move <任务> --before T-44 | --after T-46 | --top`。写一条
   事件 `task.moved`。
4. **可见**：`task.get`、`project.get`、`dashboard.get` 的任务都带 `after`（声明的依赖）和 `blocked_by`（其中还没完成的
   那些，以及每个的当前状态）。`autome ls` 的排队段按真正的出队顺序列出，每行末尾加「← 等 T-41」；
   `autome status <任务>` 多一行「依赖」。所有输出 `--json` 一致。
5. 网关 `apps/web/server/gates/write-gate.ts` 的两份白名单加上 `task.set_after`、`task.move`（照 `task.extend_budget`
   的输入校验写上限：id 列表最多 8 个）。

## 4. 验收标准

取自任务卡「验收标准」，编号与任务卡一致。设计文档的里程碑必须逐条覆盖。

1. e2e：`parallel = 2`，创建 A，再创建 B `--after A`，再创建 C。驱动跑到稳定后 A 和 C 在跑、B 仍在排队；把 A 一路
   推到已完成后 B 自动开始。全程没有手工步骤。
2. e2e：A 被取消后 B 留在排队并带 `blocked_by = [A(已取消)]`，事件里有 `task.blocked`；`task.set_after` 清空后 B 在
   下一个 tick 开始。
3. e2e：三个排队任务，`task.move` 把第三个提到最前，下一个 tick 先启动它。
4. 单测：自引用、环、跨项目、不存在的 id 都被拒绝，拒绝信息是中文的一句话。
5. 现有的需求和 Bug e2e 不做修改仍然通过；store 迁移测试覆盖新版本。
6. CLI 集成测试：`task new --after`、`after … --none`、`queue move --top`、`ls --json` 里的 `after` / `blocked_by`。
7. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

补充说明（整理轮的理解，不是新范围；设计轮可据证据修正并写明理由）：

- e2e 指 `crates/automed/tests/end_to_end.rs` 这一套（真实 Git 仓库、真实 worktree、真实包装脚本，只把模型换成替身）。
  只在 `scheduler.rs` 里写单元测试不能代替验收 1–3。
- 验收 1 的「全程没有手工步骤」指 B 的开始不需要任何针对 B 的操作（不 `task start`、不 `after`、不 `queue move`）；
  A 走到已完成所需的批准与合并是 Loop 原有的卡点，不算在内。
- 验收 1–3 的「先后」用账本事件的 `seq` 顺序或任务状态断言，不要靠 `sleep` 后看一眼。可参考
  `the_parallel_limit_queues_the_fourth_task_and_releases_it_on_completion`（约 945 行）让会话 `sleep` 占住槽位的写法。
- 验收 5 的「不做修改」针对现有的需求与 Bug e2e 用例本身。`store.rs` 里钉死版本号的两处断言
  （`assert_eq!(SCHEMA_VERSION, 5)` 约 2584 行、迁移测试里的 `assert_eq!(version, 5)` 约 3258 行）随迁移必然要改，
  不在此列；若还要改别的已有测试，设计文档逐条列出理由，且理由不能是「新行为与旧断言不符」。
- 验收 7 的 `sh scripts/ci-local.sh` 同时是本仓库的宿主门禁命令（`[gate] run = "before_merge"`，合并前由宿主跑一次，
  超时 30 分钟）。实现轮与审计轮跑里程碑验收和直接相关的测试即可，整套 CI 不必每轮都跑。

## 5. 范围

在范围内：

- `crates/automed`：`store.rs`（依赖的持久化、迁移到新 schema 版本、队列顺序读写）、`scheduler.rs`（`fill_slots` 的
  跳过规则、`task.blocked` 事件）、`dispatch.rs`（`task.create` 的 `after`、新写方法 `task.set_after` / `task.move`、
  `task.start` 对已移动的待开始任务的处理、`task.get` / `project.get` / `dashboard.get` 输出 `after` / `blocked_by`）；
  相应单元测试与 `tests/end_to_end.rs` 的新用例。
- `crates/autome-domain`：若依赖校验（自引用、环、跨项目）或「哪些状态允许改依赖」适合做成纯函数，放在这里
  （本 crate 不做 I/O）。是否新增模块由设计轮定。
- `crates/autome-cli`：`args.rs`（新的取值开关）、`main.rs`（`task new --after`、新命令 `after`、`queue move`、`ls` 与
  `status <任务>` 的显示、`USAGE` 文本）、`render.rs`（状态行与新事件 `task.blocked` / `task.moved` 在 `autome run` 里的
  一行文案）、`tests/cli.rs` 的新用例。
- `apps/web/server/gates/write-gate.ts` 与 `apps/web/test/write-gate.test.ts`：放行 `task.set_after`、`task.move` 及其
  输入校验。
- 随行文档：本任务使之失实的现有陈述就地更正——`docs/development/technical-design.md` §8（现写「排队按 created_at
  升序。槽位释放后立即启动队首」）、§13（表与迁移）、§14（命令与事件清单），以及 `README.md`「The command line」
  一节的命令说明。只改失实处和补新命令，不扩写。

不在范围内（不要做）：

- 面板上的拖动排序和依赖编辑界面（任务卡「不做什么」：留给 W6/W7 的 Backlog，本任务只做内核、CLI 和网关放行）。
  `apps/web/app/` 与 `apps/desktop/renderer/` 不加新界面。
- 跨项目依赖（任务卡「不做什么」）。
- 根据任务目录重叠自动推断依赖（任务卡「不做什么」）。
- 依赖完成时自动合并或自动批准——什么都不自动化，只控制「什么时候开始」（任务卡「不做什么」）。
- 桌面端的写白名单 `apps/desktop/src/write-gate.js` 与 `preload.js`：任务卡只点名 Web 网关。桌面端只做「不坏」的核对
  （`npm test --prefix apps/desktop` 仍然通过）。
- 通知器规则（`apps/web/server/notifier-rules.ts`）：任务卡要求的是内核写出 `task.blocked` 事件、其内容足以让通知器
  据此提醒；「要做什么」没有列通知器改动。缺省不改，见 §7 问题 11。
- `task.convert` 的依赖继承或改指、依赖完成后的任何联动。
- 改 `protocol/`（内置协议 seed，改它要走协议变更流程）、`plugins/`、`.autome/`（AGENTS.md 明令禁止）、
  `docs/排队任务可以调整顺序并声明依赖-task/protocol/`（本任务固定的协议副本）。
- 顺手重构 `scheduler.rs`、`dispatch.rs`、`store.rs` 中与本需求无关的代码。

## 6. 硬性约束

1. **跳过不改位置。** `fill_slots` 跳过依赖未完成的任务时不得改它的 `queued_seq`，也不得让它失败或改变状态；它后面
   的任务可以先走（任务卡第 2 条）。
2. **依赖只控制「什么时候开始」。** 依赖完成不触发批准、合并或任何别的动作；依赖被取消、失败时，被阻塞的任务既不
   自动开始也不失败，留在排队等用户处理（任务卡第 2 条与「不做什么」）。
3. **创建即带依赖，不留空窗。** `dispatch.rs` 的 `create`（约 1562 行）在 `store.create_task` 之后立刻调
   `scheduler::tick`，有空槽位就当场启动。所以 `task.create` 的 `after` 必须在这次 tick 之前已经落库（与任务本身同一
   事务），不能做成「先创建、再调 `task.set_after`」——那样 B 会在依赖写入之前就被启动，验收 1 会间歇性失败。
   CLI 的 `task new --after` 同理，必须是一次 `task.create` 调用。
4. **校验在写入之前，拒绝即无副作用。** 自引用、成环、跨项目、不存在的 id，在 `task.create` 与 `task.set_after` 里
   拒绝；被拒绝的 `task.create` 不留下任务，被拒绝的 `task.set_after` 不改动原有依赖。拒绝信息是中文的一句话
   （验收 4），沿用 `rejected(...)` / `bad_params(...)` 的写法。
5. **迁移只加不改。** `store.rs` 顶部注释的规矩：`SCHEMA_VERSION` 加一（现为 5），在 `migrate` 里加一个新的
   `if version < N` 块，块内是包含 `PRAGMA user_version` 的单个事务；**不得编辑已有的迁移块**。旧任务迁移后没有依赖，
   队列顺序与迁移前一致。迁移测试照 v4→v5 那条的写法（先把库降回上一版再跑 `migrate`）覆盖新版本（验收 5）。
6. **不得用本 worktree 构建出的二进制打开真实账本。** 本仓库正由已安装的 `autome` / `automed`（schema 5）驱动自己的
   Loop。`refuse_a_database_that_is_not_ours`（`store.rs` 约 519 行）会让旧二进制拒绝打开版本更高的库；一旦新二进制
   对 `~/.autome` 跑过迁移，正在驱动本任务和其它任务的内核就再也打不开账本。任何手工验证都必须像
   `crates/autome-cli/tests/cli.rs` 那样把 `AUTOME_HOME` 指到临时目录，不得省略、不得指向 `~/.autome`。
7. **没有依赖、没有移动时行为与现在逐字节一致。** 出队仍按 `queued_seq, created_at`；`queue_position` 的取值、现有
   事件的种类与顺序不变。现有的需求与 Bug e2e 一行不改就要通过（验收 5）。
8. **新方法是写方法。** `task.set_after`、`task.move` 不进 `READ_METHODS`（现为 17 项，`dispatch.rs` 约 2708 行）——
   不在其中的方法自动持有驱动锁。`READ_METHODS` 的内容与常量名不要动：`apps/desktop/test/ipc-gate.test.js`、
   `apps/web/test/read-gate.test.ts` 解析 `dispatch.rs` 核对它，`ci-local.sh` 的 `Contract` 步骤 grep 这个常量名。
9. **`task.blocked` 不得每个 tick 重写。** `fill_slots` 每个 tick 都跑；任务卡写的是「写一条事件」。同一个被阻塞任务
   对同一个依赖的同一原因只写一次，否则账本被刷屏、通知器重复提醒。去重方式由设计轮定，并用测试钉住
   （连续多个 tick 后事件数不变）。
10. **输出只增不改。** `after` / `blocked_by` 是新增字段；`task.get`、`project.get`、`dashboard.get` 现有字段的名字、
    类型与含义不变，`autome ls --json`（现为任务对象数组）、`autome status --json` 的外形不变。文本输出与 `--json`
    表达同一份数据（任务卡第 4 条）。
11. **文案语言沿用现状。** 报错、界面与 CLI 文案用中文；代码注释与 `README.md` 用英文。
12. **会话边界。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改 `.autome/`；
    超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成本轮后结束会话，不自行启动
    下一个会话（AGENTS.md）。

## 7. 待设计轮回答的问题

任务卡留白或与代码现状有出入的地方。设计文档「方案」一节须逐条给出结论与依据；没有依据支持改动任务卡字面的，
按任务卡字面做。

1. **参数名。** 任务卡写 `task.set_after { task, after }`、`task.move { task, before?, after?, top? }`；现有全部任务
   方法用 `task_id`（`str_param(p, "task_id")`），Web 网关对 `task.start` / `task.reply` / `task.convert` 校验的也是
   `task_id`。用哪个名字，写明并让 CLI、网关校验与测试一致。
2. **依赖存在哪。** `tasks` 表加一列（JSON 数组，照 `attachments` / `doc_refs`）还是新表；`TaskRecord` 与
   `TASK_SELECT_BASE`（约 2038 行）怎样带上它。任务卡只要求「迁移一个版本」。
3. **哪些状态允许 `task.set_after`。** 任务卡：「待开始、排队、等你决定（设计卡点之前）」可改，「已经开始实现轮」
   拒绝。对照 `TaskState`：`Draft`、`Queued` 明确可改；需要逐一定下的是 `Requeued`、`Active{Intake/Design/Review/
   Adjudicate}`、`Active{AwaitDesignApproval}`、设计阶段的 `Paused` / `Stopped` / `Failed`、以及 Bug 任务（`Locate` 等
   节点没有「实现轮」的说法）。同时说明：对一个已经离开排队的任务改依赖意味着什么——依赖只在出队时起作用。
4. **`Requeued` 是否受依赖约束。** `queued_tasks` 同时返回 `Queued` 与 `Requeued{resume}`（暂停后恢复、等槽位的任务，
   对外仍显示排队）。任务卡说跳过「依赖还没有到已完成的排队任务」。恢复中的任务若带着未完成的依赖，是跳过还是放行。
5. **「归档」与「已完成」的冲突。** 任务卡第 2 条把「依赖被取消、失败或归档」并列为阻塞；但 `task_archive`
   （`dispatch.rs` 约 2170 行）只允许归档 `Done` 的任务（「只有已完成的任务可以归档」），归档只是 `archived_at` 标记。
   按字面，一个依赖先完成、后归档，被依赖方就从「可出队」变成「被阻塞」，与同一条里「依赖到已完成就自然轮到它」
   相抵。设计轮给出一致的规则并说明依据。
6. **「失败」不是终态。** `is_terminal` 只含 `Done` / `Cancelled`；`Failed` 可以 `rerun` / `extend` 后继续走到 `Done`，
   `Stopped` / `Paused` 也能恢复。`blocked_by` 里每个依赖的「当前状态」取什么值域；哪些状态算「需要提醒用户」并写
   `task.blocked`；依赖从失败恢复并最终完成后是否自然解除（按任务卡出队规则应当是）；`reason` 的取值。
7. **`dashboard.get` 里没有排队任务。** 现在它只输出 `waiting`（`awaits_user`）与 `running`（`occupies_slot`）两组，
   `Queued` / `Draft` 的任务不在任何一组（`dispatch.rs` 约 2377–2445 行）。任务卡要求 `dashboard.get` 的任务带
   `after` / `blocked_by` 并写明「等 T-41（已取消）」。被阻塞的排队任务在 `dashboard.get` 里放哪——新增一组、还是只给
   已有两组加字段——以及 `autome status`（不带任务，读的就是 `dashboard.get`）怎样显示。新增只能是加法（约束 10），
   并核对现有读者不坏：`crates/autome-cli/src/main.rs` 的 `status`、`apps/desktop/renderer/screens/dashboard.js`、
   `apps/web/app/` 下读 `dashboard.get` 的代码。
8. **`autome ls` 没有「排队段」。** 现在 `ls` 是把每个项目 `project.get` 的 `tasks`（`list_tasks` 的顺序）平铺成一张
   表，`--json` 是任务对象数组（`main.rs` 约 530 行，`client.rs` 的 `all_tasks`）。任务卡要「排队段按真正的出队顺序
   列出，每行末尾加 ← 等 T-41」。定下：文本输出怎样分出排队段而不破坏现有 `ls` 测试
   （`cost_cli_ls_appends_cost_and_a_footnote` 等）；「真正的出队顺序」对被阻塞任务怎么排（留在 `queued_seq` 的原位并
   标注，还是排到可出队任务之后）；`queue_position` 对被阻塞任务给什么值。
9. **`task.move` 的语义。** `before` / `after` / `top` 是否恰好给一个；参照任务须同项目且处于排队或待开始；参照任务
   就是自己时怎样。位置怎样落库：`queued_seq` 现在是**跨项目的全局递增**（`mark_queued`：
   `COALESCE(MAX(queued_seq), 0) + 1 FROM tasks`），且迁移前的旧任务为 `NULL`、排在最前
   （`ORDER BY queued_seq IS NOT NULL, queued_seq, created_at`）——移动一个项目的任务不得打乱别的项目的相对顺序。
   待开始任务：`task_start`（约 1604 行）现在无条件调 `mark_queued` 把它排到队尾，任务卡要求「按它新的位置插入」。
   把任务移到它自己的依赖之前是否允许（出队时依赖规则仍然优先）。`task.moved` 的 payload。
10. **CLI 形状。** `--after` 在任务卡里有两个意思：`task new --after`（依赖）与 `queue move --after`（位置）。
    `args.rs` 的 `VALUED` 表是全局的（哪些开关带值），每个命令允许哪些开关在 `main.rs` 的 `run` 里按动词列出。新增
    动词 `after` 与 `queue`；`autome after <任务> --none` 与给了列表同时出现时怎样；依赖列表里的名字是否也接受
    `resolve_task` 的 slug 前缀写法；`USAGE` 补哪几行（`main.rs` 有断言 `USAGE` 内容的测试）。
11. **网关。** (a) 任务卡说「两份白名单」：`write-gate.ts` 里能对上的是 `ALLOWED_WRITE_OPS`（放行哪些 op）与
    `ARRAY_FIELDS`（哪些 op 的哪些字段允许数组——不在表里的数组一律被 `validateShape` 以「不允许数组」拒绝）。确认
    是不是这两份。(b) 任务卡说「照 `task.extend_budget` 的输入校验写上限」，但网关里没有针对 `task.extend_budget`
    的专门校验（它只过通用的 `validateShape`）；可照的写法是 `validateOpSpecific` 里 `task.start` / `task.convert` 的
    分支。id 列表上限 8 个是任务卡定的，通用的 `MAX_ARRAY_ENTRIES` 是 256。(c) `task.create` 经网关带 `after` 是否
    放行——任务卡第 5 条只点名两个新方法，但不把 `after` 加进 `task.create` 的 `ARRAY_FIELDS`，面板将来就无法创建
    带依赖的任务。(d) 内核自己是否也限制列表长度。(e) `apps/web/test/write-gate.test.ts` 的
    「长度上限钉死，边界两侧和全部 op 正路径可达」对 `ALLOWED_WRITE_OPS` 里每个 op 用 `specific[op] ?? {}` 走一遍正
    路径——新 op 若有必填参数，要在那张表里补上；「写门禁全部拒绝消息为中文」要求新拒绝文案是中文。
    (f) 通知器：见 §5，缺省不改规则；若设计轮认为任务卡要求本任务就让通知器提醒，写明依据再纳入。
12. **依赖指向的任务处于什么状态才接受。** 任务卡只列了四种拒绝（自己、环、跨项目、不存在）。指向已完成的任务
    （依赖天然满足）、已取消的任务（一创建就被阻塞）是否接受；重复的 id 是否去重。任务 id 是全局的 `T-N`
    （`next_task_id` 不分项目），所以「跨项目」与「不存在」是两种不同的拒绝，文案要分开。
13. **`task.convert` 的旁路。** 它把「不是 Bug」的任务改建成新需求任务并取消原任务（`store.convert_task`）。依赖着
    原任务的排队任务会看到「等 T-x（已取消）」。本任务不做改指（§5），设计里写明这一行为即可。

## 8. 代码现状（整理轮调研，行号为约数，以实际代码为准）

**队列与出队。**

- `crates/automed/src/store.rs`：`SCHEMA_VERSION = 5`（70 行）；`migrate`（231 行起，v5 块在 467–480 行加了
  `tasks.kind`、`tasks.queued_seq` 与 `task_briefs` 表）；`create_task`（1012 行，事务内 `insert_task`，状态为 `Queued`
  时 `mark_queued`）；`convert_task`（1028 行）；`mark_queued`（1078 行）；`queued_tasks`（1429 行，按
  `queued_seq IS NOT NULL, queued_seq, created_at` 取项目全部任务后过滤出 `Queued` / `Requeued`）；`slots_in_use`；
  `TaskRecord`（76 行）；`TASK_SELECT_BASE`（2038 行）；`task_counts`（2252 行）。
- `crates/automed/src/scheduler.rs`：`tick_locked` 的第 3 步调 `fill_slots_reporting_errors`（232 行）；
  `fill_slots_reporting_errors`（1706 行）逐项目取 `parallel` 上限与 `slots_in_use`，按 `queued_tasks` 的顺序对每个任务
  `advance(ctx, id, &Trigger::SlotAvailable)`，槽位满即 `break`；启动失败的任务不挡后面的。`fill_slots`（1740 行）是
  仅测试用的包装。
- `crates/autome-domain/src/task.rs`：`TaskState`（270 行：`Draft`、`Queued`、`Requeued{resume}`、`Active{node}`、
  `Paused{resume}`、`Stopped{at}`、`Failed{at, reason}`、`Done`、`Cancelled`）；`is_terminal`（368 行）；
  `occupies_slot` / `awaits_user`；`Trigger::Start`（待开始 → 排队）与 `Trigger::SlotAvailable`（约 960–975 行）；
  `Node::ALL`（需求 Loop 15 个节点）与 `Node::BUG`（7 个）。

**RPC。**

- `crates/automed/src/dispatch.rs`：方法表在 380–450 行；`task.extend_budget`（429 行）是「取 `task_id` + 一个数值参数
  → `task_trigger`」的最小写法；参数助手 `str_param` / `opt_str_param` / `bool_param` / `u32_param`（492 行起）；
  `rejected` / `bad_params` 产生拒绝；`task_create`（1392 行）解析参数，`create`（1562 行）落库、写 `task.created`、
  立刻 `tick`；`task_start`（1604 行）；`queue_position`（1636 行，`queued_tasks` 里的 1 起始下标）；`task_json`
  （611 行，`task.get` / `project.get` / `dashboard.get` 三处共用的任务对象）；`project_get`（584 行）；`task_get`
  （1668 行）；`dashboard_get`（2377 行）；`task_archive`（2170 行）；`task_cancel`（2074 行）；`READ_METHODS`（2708 行）。
- 事件用 `ctx.store.append_event(kind, subject_id, payload)` 写入账本，并在返回值里带 `event(seq, kind, id, …)`。

**CLI。**

- `crates/autome-cli/src/args.rs`：手写解析器，`VALUED`（现 10 项）列出带值的开关，`--flag=value` 与 `--flag value`
  都认，`values()` 取重复开关。
- `crates/autome-cli/src/main.rs`：`USAGE`（44 行）；`run`（113 行）按动词列允许的开关并分发；`task`（413 行，`new` /
  `start` / `convert`）；`list`（530 行）；`status`（562 行）与 `task_detail`（670 行，逐行打印「状态」「费用」「项目」
  「门禁排队」「需求」…）；`trigger`（1066 行）是「解析任务名 → 调一个写方法 → 打印结果」的通用路径。
- `crates/autome-cli/src/client.rs`：`all_tasks`（141 行）、`resolve_task`（154 行，id 或 slug 的唯一前缀）。
- `crates/autome-cli/src/render.rs`：`state_line`（118 行：待开始 / 排队 / 排队（会从 X 继续）/ 运行中 · X / …）、
  `event_line`（约 350 行，`autome run` 每个事件一行）。

**Web 网关与其它读者。**

- `apps/web/server/gates/write-gate.ts`（293 行）：`ALLOWED_WRITE_OPS`（1 行起）、`PROSE_FIELDS`、`ARRAY_FIELDS`
  （66 行）、`MAX_ARRAY_ENTRIES = 256`、`validateShape`（98 行）、`validateOpSpecific`（166 行）。
  `apps/web/server/gates/read-gate.ts`：`ALLOWED_READ_METHODS`（本任务不新增读方法）。
- `apps/web/server/notifier-rules.ts`：通知规则表，现在只匹配 `task.updated` 与 `project.alert`。
- `apps/desktop/src/write-gate.js`：桌面端独立的一份写白名单（不在本任务范围）。读 `queue_position` 的界面只有
  `apps/desktop/renderer/screens/task.js`。

**现有测试（改动时最可能碰到的）。**

- `store.rs`：`queued_tasks_come_back_in_submission_order`、`requeued_counts_and_queue_order_do_not_consume_slots`、
  v4→v5 迁移测试（约 3235–3275 行）、版本号断言、拒绝更高版本库的测试。
- `scheduler.rs`：`fill_slots_respects_the_projects_parallel_limit`（9015 行）、`fill_slots_never_starts_a_draft`
  （9285 行）、`tick_isolates_invalid_project_config_and_still_fills_other_projects`（8932 行）。
- `dispatch.rs`：`queue_position` 相关断言（约 4306、4827、4969、4987、5040 行）、`READ_METHODS` 相关断言、
  `a_draft_is_created_without_starting_or_a_queue_position_and_is_not_on_the_dashboard`。
- `tests/end_to_end.rs`（166 个用例）：`World`（183 行；`call`、`settle`、`configure`、`doc_step_for`、`state`、`node`）、
  `create_task`（624 行）、`the_parallel_limit_queues_the_fourth_task_and_releases_it_on_completion`（945 行）、
  `task_card_a_started_draft_waits_behind_the_parallel_limit`（4729 行）、`the_dashboard_lists_running_and_waiting_tasks`
  （1242 行）、`cancelling_a_task_archives_its_documents_and_removes_the_worktree`（1099 行）。
- `crates/autome-cli/tests/cli.rs`（24 个用例）：`Sandbox`（35 行；`autome`、`ok`、`json`、`step`、`task_state`，
  `AUTOME_HOME` 指向沙箱）、`task_card_*`、`task_convert_*`、`cost_cli_ls_appends_cost_and_a_footnote`。
- `apps/web/test/write-gate.test.ts`：见 §7 问题 11(e)。

## 9. 项目背景摘要（生成于 2026-11-13）

**这是什么。** Autome 2.0：一个驱动「设计 → 评审/裁决 → 实现 → 门禁 → 审计 → 合并」Loop 的本地工具。内核 `automed`
持有全部状态（SQLite 账本 + 事件流），通过 stdio JSON-RPC 服务 Electron 桌面端与 Web 面板；命令行 `autome` 直接链接
内核库、打开同一个账本。本仓库用 Autome 自己开发自己（`.autome/config.toml`：`[loop] parallel = 3`，
`[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、`timeout_minutes = 30`，`[retry] attempts = 2`）。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：
  - `crates/autome-domain`——纯域：状态机（`task.rs`、`bug_loop.rs`）、配置形状（`config.rs`）、状态块解析等。不做 I/O。
  - `crates/automed`——内核：调度（`scheduler.rs`）、JSON-RPC 分发（`dispatch.rs`）、账本（`store.rs`）、门禁
    （`gate.rs`、`gate_queue.rs`）、会话启动（`launcher.rs`）、Git（`git.rs`）。测试：`tests/end_to_end.rs`、
    `tests/prompt_rendering.rs`、`tests/real_cli.rs`。
  - `crates/autome-cli`——`autome` 命令行（`main.rs`、`render.rs`、`client.rs`、`args.rs`），测试 `tests/cli.rs`。
- `apps/web`——Node 网关（`server/`，含 `gates/`、通知器）+ React Router 面板（`app/`）；vitest、eslint、tsc。
- `apps/desktop`——Electron 外壳，纯 `node --test`（注意：`node --test` 会把 `test/` 下任意 `.js` 当测试跑）。
- `protocol/`——内置协议 seed（`include_str!` 进内核）；`plugins/`——Claude 插件；`scripts/`——`ci-local.sh`、打包脚本。
- 文档：`README.md`（英文）、`docs/development/technical-design.md`（§8 调度器、§13 持久化、§14 IPC）、
  `docs/plans/2026-11-10-web-dashboard-via-loop.md`（§7.3 是本任务的动机之一）、`docs/adr/`。每个任务的文档在
  `docs/<slug>/`。

**测试与检查命令（在 worktree 根目录运行）。**

- `cargo test --workspace`；与门禁同款：`cargo test --workspace --all-targets -- --nocapture`
- 单个 e2e：`cargo test -p automed --test end_to_end <用例名>`；CLI 集成：`cargo test -p autome-cli --test cli <用例名>`
- 单元测试：`cargo test -p automed --lib <名字>`、`cargo test -p autome-domain <名字>`
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`
- `npm run lint|typecheck|test|build --prefix apps/web`；`npm test --prefix apps/desktop`
- 整套本地 CI（即宿主门禁命令）：`sh scripts/ci-local.sh`——每一步都跑完，任一步失败则非零退出。
- 输出长的命令重定向到 `.autome/output/` 只读头尾，例如
  `cargo test -p automed --test end_to_end > .autome/output/e2e.txt 2>&1; tail -20 .autome/output/e2e.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它
  目录、不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入
  `.autome/output/`，不进任务文档目录。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/排队任务可以调整顺序并声明依赖-task/protocol/`（见文首指路）。

**环境注意。** macOS（Darwin），系统 `sh` 是 bash 3.2；包装脚本与测试脚本按 POSIX `sh` 写。e2e 需要本机有 `git`
（用例开头的 `needs_git!()`）。
