# 任务：W7 任务详情——阶段条、逐轮执行时间线、文档标签页、改动与合并、全部控制操作、Backlog 表态，随事件实时更新

本任务的规则见 `docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/protocol/loop-protocol.md` 与
`docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/protocol/session-protocol.md`。

- 设计文档：`docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/w7-任务详情-阶段进度-逐轮执行时间线-文档.md`
- 任务卡（第一输入，用户所写）：`docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/w7-任务详情-阶段进度-逐轮执行时间线-文档-card.md`
- 用户指定的参考文档：`docs/plans/2026-11-10-web-dashboard-via-loop.md`
- 任务文件生成日期：2026-11-13；任务 T-87；基线提交 `a858ae7a`（分支 `autome/w7-任务详情-阶段进度-逐轮执行时间线-文档`，
  与 main 的分叉点 `f5a963ad`，即 T-59 W8 的合并提交，生成时 main 也在这里）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支；状态块不写 `repos:`）
- 证据、简报、运行记录一律写在 `docs/w7-任务详情-阶段进度-逐轮执行时间线-文档/` 下

## 1. 需求原文

> W7 任务详情：阶段进度、逐轮执行时间线、文档、门禁结果与全部控制操作

这句话对应计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 **W7 任务详情：执行过程与进度**。
任务卡正文把它展开成了具体行为、排除项与验收标准；**两者不一致时以任务卡正文为准**，任务卡与计划不一致时同样以任务卡为准。
计划写成之后内核多出的信息（费用、门禁排队、快检与合并前门禁、门禁同因熔断、「改建成需求」、冷却 / 重试等待、
范围守卫与协议失败的原因）任务卡已点名要呈现，**字段以内核实际返回为准**，不以计划或本文件的转述为准。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-10 取自任务卡「要做什么」，N-01..N-05 取自「不做什么」，A-01..A-12 取自「验收标准」，原意不增不减。
编号供评审、审计追溯；措辞有出入时以任务卡原文为准。

- **R-01 页头。** 类型标签（需求 / Bug）、id、标题、所属项目（链接回项目页）、分支名、累计费用（含「有会话没有计费数据」的
  标注，与 CLI 口径一致）、当前状态的一句话（复用 W6 `task-card-model.ts` 的动态文案，不另写一份）。
- **R-02 阶段条（计划 §4.3）。** 需求与 Bug 各用 `app/lib/labels.ts` 的阶段表；当前阶段高亮，走过的阶段标完成；
  需求显示里程碑进度（M-01… 各自状态）与设计 / 实现轮次（已用 / 预算），Bug 显示修复轮次（已用 / 预算）与复现检查结果。
  任务失败 / 已停止 / 已取消时阶段条停在出事的阶段并标出状态。
- **R-03 执行时间线。** 每个会话一条，按时间排：阶段（中文）、第几轮、CLI（claude / codex）、起止时间、用时、花费、
  结果摘要（例如「M-02 待审 → 已完成」「复现命令失败（符合预期）」「协议失败：…」）。内核步骤也进时间线，作为较轻的条目：
  快检 / 合并前门禁（命令、提交、通过 / 失败 / 排队第 N 位 / 运行中、日志路径）、rebase（有冲突时列文件）、合并、验证 main。
  点开会话条目就是 W8 的 `SessionLog`（默认折叠；正在运行的那一个默认展开）。数据来自 `task.get` 与该任务的事件，
  **不新增 RPC**；现有只读方法确实拿不到的某一项，写进 Backlog，不在本任务里改内核。
- **R-04 文档标签页。** 任务卡正文、设计文档（需求）或 Bug 文档（Bug）、评审 / 裁决 / 审计 / 复盘文档（有就列），
  Markdown 渲染（服务端渲染，禁用原始 HTML，链接 `rel="noreferrer"`）；设计文档的里程碑表高亮当前里程碑。
  任务卡正文用 `task.get` 的 `brief`；其余文档由服务端 loader 读文件：**路径只能取自内核这一次 `task.get` 返回的
  `documents[].absolute`**（与 W8「日志路径只能来自 `session.log`」同一口径），浏览器只传文档的 `name`，不传路径；
  单个文档有大小上限（超过只显示前一段并注明）。`documents` 清单里没有的文档（例如 Bug 文档若未列出）写进 Backlog，
  不自己拼路径。
- **R-05 改动与合并。** 合并卡点上展示 `task.changes`（文件清单、增删行数、删除文件数——删除文件数异常多时醒目提示）；
  只落后默认分支时（`behind` / `rebase_first`）提示「按合并后会先自动 rebase、跑合并前门禁再合并」；按下合并后页面
  不说「任务完成」，而是跟随状态显示「合并中 → 验证 main → 已完成」，自动路径失败时显示内核给的原因。
- **R-06 操作区。** 按任务当前状态显示可用的操作，其余不显示或置灰并写明原因：
  - 卡点：批准 / 驳回（带意见）、Bug 的补充信息（reply）、合并、「改建成需求」（`task.convert`，Bug 判定为不是 Bug 时）；
  - 运行控制：开始（待开始）、暂停、恢复、停止、取消（二次确认）；
  - 恢复类：重跑（从哪个节点：只列内核允许的节点，需求与 Bug 各不相同）、追加预算（轮数）；
  - 内核拒绝时原样显示内核给出的那句话；输入长度上限与 write-gate 一致；内核离线时全部置灰（沿用外壳状态）。

  可用操作的判定写成一张**表驱动的纯函数**（状态 → 操作集合），有测试与内核的状态机交叉核对
  （参照 W5 `labels.ts` 与内核节点列表的交叉核对方式）。
- **R-07 待你决定。** Backlog 与争议项逐条列出，可表态（`task.decide`：include / ignore / ruled / none，ruled 带裁定文字）；
  显示每条当前的表态与「下一轮生效」的说明。
- **R-08 失败与等待看得懂。** 失败 / 已停止时在页头下方显示原因全文与内核建议的下一步（rerun / extend / cancel），对应按钮
  就在旁边；等待类状态（冷却到几点、重试第几次、门禁排队第几位、排队等槽位、等你合并但门禁结果已过期——T-70 合并后的字段，
  有就显示，没有不报错）有各自的一句话。
- **R-09 实时。** 以上各区在任务被另一个进程推进时不刷新页面自动更新（W3 的 `useLiveEvents`，按 `task.` / `session.` /
  `gate.` 事件与任务 id 重验）；时间线新增条目时不打断正在看的日志（滚动位置与展开状态保持）。
- **R-10 入口。** 项目页任务卡、首页总览卡、铃铛通知、⌘K 都能进到这一页（核对已有链接指向 `/tasks/<id>`）；
  页面内操作成功后的提示与跳转不离开本页。

不做什么（任务卡原文）：

- **N-01** 不新增 JSON-RPC 方法、不改 `crates/`；缺的数据进 Backlog。
- **N-02** 不做角色 / 路由配置（计划 D6）。
- **N-03** 不做网页上给进行中的任务留备注（T-85 合并后另提）、不做拖动排序（T-53）。
- **N-04** 不重写 W8 的 `SessionLog` 与日志订阅，只组合它。
- **N-05** 不改 `apps/desktop/`。

验收标准（任务卡原文）：

- **A-01** 阶段条：需求与 Bug 各有测试，覆盖进行中、卡点、失败、已完成；里程碑进度与修复轮次与 `task.get` 一致。
- **A-02** 时间线：对真实 automed 与假 CLI 跑一个需求任务到合并卡点，页面上逐轮出现任务整理、设计、评审、裁决、实现、审计、
  复盘，以及快检 / rebase / 合并前门禁条目，各条的阶段、CLI、用时、结果摘要正确（集成测试）；Bug 任务同样覆盖定位、
  复现检查、修复、复核。
- **A-03** 点开时间线条目能看到该会话日志（历史会话）与正在运行会话的实时增量（复用 W8，集成测试只断言接上了）。
- **A-04** 文档标签页：任务卡正文、设计文档 / Bug 文档渲染正确；原始 HTML 与 `javascript:` 链接不被渲染（测试）；
  里程碑表当前项高亮。
- **A-05** 合并：`task.changes` 展示；落后默认分支时出现 rebase 提示；按下合并后页面依次显示合并中、验证 main、已完成，
  不提前说完成（集成测试）。
- **A-06** 操作表：状态 → 可用操作的纯函数对需求、Bug 的每个状态有测试，并与内核状态机交叉核对
  （内核拒绝的操作页面不提供；内核允许的不缺）。
- **A-07** 每个操作各有一个对真实 automed 的集成测试：批准、驳回、reply、合并、开始、暂停、恢复、停止、取消、重跑、
  追加预算、改建成需求、表态。内核拒绝时页面显示内核原文。
- **A-08** 失败任务显示原因与下一步；冷却、重试、门禁排队、排队等槽位各有一句话（测试）。
- **A-09** 用命令行推进任务，页面在一个 tick 周期内更新，展开的日志不被折叠、滚动位置不丢（测试）。
- **A-10** 计划 §5.5：本页所有写操作走已有写入口，Host / Origin 检查有测试覆盖到这些 action。
- **A-11** 1512×944 下 `scrollHeight ≤ clientHeight`（时间线、文档、日志是内部滚动区域）。
- **A-12** `npm --prefix apps/web` 的 lint、typecheck、test、build 全绿；`sh scripts/ci-local.sh` 全绿。本任务新增的集成测试
  用到假 CLI / 子进程时，测试结束后不留存活进程（参照 T-86 的现象：`logtail-live` 测试收尾时目录先删、假 CLI 等不到
  停止信号而永不退出）。

**任务卡没有定死、留给设计轮回答并写进设计的问题**（只列问题与生成时读到的事实，不预设答案）：

1. **「该任务的事件」从哪条路拿。** 生成时的事实：`task.get` 回包里没有事件列表，只有 `gate`（最近一条 `gate.result`）
   与 `retry`；快检 / 合并前门禁 / rebase / 合并 / 验证 main 这些内核步骤只留在账本事件里（`gate.queued`、`gate.started`、
   `gate.result`、`gate.reused`、`gate.skipped`、`rebase.conflicts`、`merge.rebased`、`merge.auto`、`merge.repo`、`merge.after`、
   `bug.located`、`bug.reproduced`、`bug.repro_missed`、`bug.reviewed`、`session.retry`、`runtime.cooldown`、
   `task.protocol_failure`、`guard.warning` 等，subject 多为任务 id）。内核的 `events.since` 只收 `after_seq` 与
   `limit`（上限 1000），**不能按任务过滤**；它在内核 `READ_METHODS` 里，但 `server/gates/read-gate.ts` 把它列在
   `WITHHELD_READ_METHODS`（「浏览器经 WebSocket 收事件，账本拉取留给服务端」），而交给 loader 的
   `KernelGateway` 只有 `read` / `write` 两个经白名单的入口。设计轮要用真实内核实测后定：服务端经哪条路拿到一个任务的
   历史事件、翻页与上限怎么定、拿不到的条目哪些进 Backlog（R-03 原文允许）。
2. **会话的「结果摘要」来源。** `session_json` 只有 `kind`、`label`、`runtime`、`model`、`round`、`started_at`、`ended_at`、
   `lifecycle`、`running`、`metrics` 等，没有现成的摘要字段。「M-02 待审 → 已完成」「复现命令失败（符合预期）」「协议失败：…」
   各自由哪些事件或字段拼出来，CLI 的 `render::event_line`（`crates/autome-cli/src/render.rs`）是文字版参照。
3. **单个会话的花费与「无计费数据」标注。** `session.metrics` 为空时整个键缺省；`task.get` 的 `cost`
   （`usd_total`、`sessions`、`sessions_without_cost`）与 `task.cost_usd` / `cost_sessions_without_cost` 的关系，
   与 CLI `task_detail` 的「费用 $x（N 个会话，其中 M 个无计费数据）」口径对齐。
4. **Bug 任务的进度数据。** Bug 任务没有设计文档，`status_block` 为空；修复轮次的「已用」在 `task.fix_round`，「预算」在
   `task.budget_n` 还是项目配置 `[bug] fix_rounds`（W6 看板从 `project.get` 的 `config.bug.fix_rounds` 取）；
   复现检查结果从哪个事件取。
5. **Bug 文档与 A-04 的关系。** 生成时内核 `documents()` 只列六个固定名（`<slug>.md` 设计文档、`<slug>-task.md` 任务文件、
   `-review.md` 评审、`-adjudication.md` 裁决、`-audit.md` 审计、`retro.md` 运行记录）加 `evidence/*.md`；
   **不含** `<slug>-card.md`（任务卡正文走 `brief`）、`<slug>-bug.md`（Bug 文档）与 `lessons.md`。R-04 说清单里没有的文档进
   Backlog、不拼路径，A-04 又要求「设计文档 / Bug 文档渲染正确」。设计轮先用真实内核跑一个 Bug 任务核实清单，
   再写明 A-04 对 Bug 文档怎么落（R-04 原文是允许进 Backlog 的依据；若据此收窄验收，必须逐字引用它，并在设计里显著标出，
   让用户在批准设计时看得到）。
6. **Markdown 渲染用什么。** `apps/web/package.json` 生成时没有任何 Markdown 依赖。手写还是引入依赖由设计轮定；
   引入依赖要写明为什么不手写、`npm view <包> engines` 实测值、与 Node ≥ 22.22 的兼容性，以及「禁用原始 HTML、
   拒绝 `javascript:` 链接」由谁保证。大小上限取多少、「只显示前一段」怎么截、文档内相对链接与图片怎么处理。
7. **文档读取的接口形状。** 「浏览器只传 `name`」是 loader 的查询参数、子路由还是别的；`name` 与 `documents[].name`
   的匹配规则（`evidence/` 下的文件名与六个固定名不会相撞，是内核注释里的说法，需核实）；读文件失败、文件在两次
   `task.get` 之间消失时的呈现。
8. **「删除文件数」怎么得到。** `task.changes` 的 `files[]` 每项只有 `path`、`added`、`deleted`（行数），顶层有
   `total_added`、`total_deleted`、`commits`、`subjects`、`mergeable`、`rebase_first`、`blocked_by`、`pending_decisions`、
   `repos[]`（各含 `behind`、`merged`、`unchanged`），**没有现成的「删除文件数」字段**。能否由现有字段可靠得出、
   「异常多」的阈值取多少；得不出就按 N-01 进 Backlog。
9. **操作表与内核状态机怎么交叉核对。** 用户触发的转换散在 `crates/autome-domain/src/task.rs`（`apply_with_merge_request`）
   与 `bug_loop.rs`（`apply_for`）的 `match` 里，没有一张现成的「状态 → 可用操作」导出表。`labels.test.ts` 的做法是读 Rust
   源码文本解析枚举与常量；操作表的核对是照此解析源码，还是对真实内核逐状态逐操作实测，或两者结合，以及「内核允许的不缺、
   内核拒绝的不提供」各自怎么判定（含角色关闭、`await_merge` 不能停止 / 重跑这类条件性拒绝）。
10. **重跑节点清单。** 生成时读到：需求任务在失败 / 已停止时可从 `intake`、`design`、`review`、`adjudicate`、`implement`、
    `audit`、`retro`、`rebase` 重跑；Bug 任务可从 `locate`、`fix`、`fix_review`、`rebase` 重跑；失败原因是
    `gate_same_failure` 时另可从出事的那个门禁节点（需求 `gate` / `merge_gate`，Bug `fix_gate` / `merge_gate`）重跑；
    角色被关闭时内核拒绝。追加预算只对 `design_budget`、`impl_budget`、`fix_budget`、`gate_same_failure` 四种失败有效，
    `extra_rounds` 必须大于 0。以设计轮核对源码与实测为准。
11. **输入上限。** write-gate：`task.reject.feedback`、`task.reply.info`、`task.decide.ruling` 属 `PROSE_FIELDS`，
    上限 `MAX_PROSE_LENGTH = 8000`（按 `string.length` 计）；一般写请求 JSON 上限 32KB。`task.extend_budget.extra_rounds`
    与 `task.rerun_from.node` 只过通用校验。页面的输入限制从 write-gate 的导出常量取，不另写数字。
12. **实时重验的范围。** 占位页现在是 `liveShouldRevalidate(bySubjectParam(['task.', 'session.'], 'id'))`；R-09 要加 `gate.`。
    `bug.*`、`merge.*`、`rebase.*`、`runtime.cooldown`、`guard.*` 这些 subject 同为任务 id 的事件要不要触发重验，
    各类事件的 subject 实际是什么，由设计轮对真实内核核实后列表写明。
13. **展开状态与滚动位置怎么保住。** `SessionLog` 的展开状态与日志内容在组件自己的 state 里（`useState` + controller），
    loader 重验后列表重渲染时靠什么保持（稳定 key、状态上提等），「正在运行的默认展开」在会话结束、下一个会话开始时怎么变，
    同时又不改写 W8 的组件内部（N-04）。W8 的限制：每个连接最多同时订阅 4 个日志。
14. **等待类文案的数据。** 冷却：`retry.not_before` 与 `dashboard.get` 的 `cooldowns[]`（W6 `board-data.ts` 的取法）；
    重试：`retry.attempts`；门禁排队：`gate_queue_position`；排队等槽位：`queue_position`；「门禁结果已过期」的字段名
    生成时在源码里没找到（T-70 未合并），按 R-08「有就显示，没有不报错」。
15. **A-11 的可执行断言。** 现有做法是 `test/styles.test.ts` 对 `app.css` 规则做文本断言（外壳定高、列表内部滚动）；
    真实浏览器量 `scrollHeight` 的端到端在 W12。本任务用哪种、哪些留给 `## 人工验收清单`。
16. **A-02 / A-05 / A-07 集成测试怎么搭。** 「页面上逐轮出现」在没有浏览器的 vitest 里断言到哪一层（loader 数据 →
    视图模型 → `renderToString`）；假 CLI 怎样把需求任务一路推到合并卡点（要写出合法的状态块、里程碑表、证据文件，
    并经过快检与合并前门禁）、Bug 任务怎样经过复现检查；测试项目的 `[gate]` 配置怎么给；用例耗时与 60 秒超时的关系。
17. **A-12 的进程清理怎么断言。** 假 CLI / 内核 / 服务子进程在 `afterAll` 里按什么顺序停、临时目录何时删，
    用什么可执行的办法证明测试结束后没有存活进程。

## 3. 范围

**可以改动的目录与文件：**

- `apps/web/app/routes/tasks.$id*`——占位页 `tasks.$id.tsx`（24 行）**整页重写**；可在同一前缀下新增路由文件
  （例如文档子路由、action），新增路由要登记到 `apps/web/app/routes.ts`；
- `apps/web/app/components/`——新增任务详情用到的组件与纯逻辑模块（阶段条、时间线、文档、改动、操作表、表态等）；
  已有组件按下面的限定动：
  - `session-log.tsx`、`session-log-model.ts`、`session-log-controller.ts`——**只组合，不重写**（N-04）。确需为
    「默认展开」「重验后保持展开」加入口时，改动限于新增可选参数，W8 的行为与测试不变，设计里写明缺什么；
  - `task-card-model.ts`、`task-card.tsx`、`board-data.ts`、`board-live.ts`、`task-columns.ts`——只为让页头复用同一份
    动态文案（R-01）与取数写法而做抽取或导出，看板与卡片的行为、测试不变；
  - `nav-links.ts`、`bell-targets.ts`、`palette-model.ts`、`overview-card.tsx`——只在 R-10 核对出入口没指向 `/tasks/<id>` 时改；
- `apps/web/app/lib/labels.ts`——新增本页需要的中文文案与短语（`test/labels.test.ts` 第 7 条禁止在 `.tsx` 里重复状态、
  列、阶段的中文字面量，新文案要落在这里或 `.ts` 模块里）；`app/lib/live-routes.ts`、`live.tsx`——只在现有
  `bySubjectParam` / `byKind` / `useWriteGate` 不够用时加，不改已有导出的行为；
- `apps/web/app/load-context.ts`、`apps/web/server/http.ts`、`apps/web/server/main.ts`、`apps/web/server/gateway.ts`——
  **只为**把问题 1（任务事件）、问题 7（按 `documents[].absolute` 读文档）所需的服务端能力交给 loader 而做的最小接线；
  可新增 `apps/web/server/` 下的小模块承载「读文档」「取任务事件」的服务端逻辑；
- `apps/web/server/gates/{read-gate,write-gate}.ts`——预期**不改放行集合**。本页的写操作（`task.start`、`approve`、`reject`、
  `reply`、`merge`、`pause`、`resume`、`stop`、`cancel`、`convert`、`decide`、`extend_budget`、`rerun_from`）都已在
  `ALLOWED_WRITE_OPS` 里；只允许为页面复用而新增导出常量。确需改校验时设计里写明原因并连同 `test/{read,write}-gate.test.ts` 一起改；
- `apps/web/app/styles/app.css`——只加本页用到的样式，用 `tokens.css` 已有的 token；
- `apps/web/test/`——新测试与 `test/fixtures/` 下的替身。**`test/task-page.test.ts` 是 W8 为占位页写的**（其中一条断言
  「无阶段条和写按钮」），整页重写后必须改写或替换，设计里逐条说明每个原用例的去向；其余已有测试只在接线引起的
  必要变化时改，逐条说明理由；
- `apps/web/package.json`、`package-lock.json`——只在问题 6 决定引入 Markdown 依赖时改；
- `.github/workflows/ci.yml`（只 `web` job）、`scripts/ci-local.sh`、`apps/web/test/ci-config.test.ts`——
  **只在**新集成测试需要现有步骤给不了的东西时改，先实测并写明证据；不需要就不改。

**明确不做（出现即越界）：**

- N-01..N-05 全部；
- 不改 `crates/`、`protocol/`、`plugins/`、`apps/desktop/`、`.autome/`。内核给不了的数据写进设计文档 `## Backlog`
  （写明缺哪个字段、哪个方法、页面上因此少了什么），不绕过去改内核，也不在服务端直接读内核的 SQLite 账本；
- 不新开让浏览器按路径读文件的 HTTP 接口；浏览器发来的任何路径形状的参数一律拒绝；
- 不加令牌、Cookie、登录会话（计划 §5.5）；不改 W2 的 Host / Origin 检查语义；
- 不做归档 / 恢复归档（`task.archive` / `task.restore`）与手动复盘（`task.retro`）的按钮——任务卡 R-06 没有列；
- 不做项目页（W6）、日志组件内部（W8）、项目管理与环境（W9）、启动与安装（W10）、Electron 薄壳（W11）、
  Playwright 端到端（W12）的内容。

## 4. 硬性约束

1. **路径只来自内核。** 服务端打开的每一个文档路径都必须是**这一次请求内** `task.get` 回包 `documents[].absolute` 里的值，
   按浏览器给的 `name` 在清单里查出来；浏览器不传路径，`name` 按不可信输入处理（`read-gate.ts` 对路径形状的判定：
   含 `/`、`\`、`..`、`~` 开头、NUL）。不拿 `doc_dir`、`worktree`、`slug` 自己拼路径。会话日志仍只走 W8 的 `session.log` 订阅。
2. **不新增 RPC、不改内核。** 只用内核已有的方法（生成时 `READ_METHODS` 17 项、上面列出的 13 个 `task.*` 写操作）。
3. **写操作走已有写入口。** 页面的每个操作都经路由 action → `gateway.write`（write-gate 校验）→ 内核；`securityMiddleware`
   对非 GET / HEAD 请求检查 Origin、对所有请求检查 Host。不另开写通道，A-10 的测试要覆盖到本页的 action。
4. **内核拒绝原样显示。** `gateway.write` / `read` 失败时 `body.error` 就是内核给的那句话（内核拒绝是 409 带 `code`，
   网关校验拒绝是 400 且带「已拒绝：」前缀，离线 503、超时 504）；页面显示原文，不改写、不吞掉。
5. **前端不自己推断流程。** 阶段由 `labels.ts` 的阶段表与内核节点映射而来（计划 §4.3）；可用操作由表驱动的纯函数给出，
   并与内核状态机交叉核对；前端不维护任务状态副本，数据只来自 loader，变化靠 `useLiveEvents` 触发重验。
6. **Markdown 安全。** 服务端渲染；原始 HTML 不渲染；`javascript:` 等非安全协议的链接不渲染成可点链接；
   链接带 `rel="noreferrer"`；不用 `dangerouslySetInnerHTML` 注入未经上述处理的内容。
7. **测试不碰真实 `~/.autome`，不启动真实 Agent。** 起内核、服务器或命令行的测试用 `mkdtemp` 的临时 `AUTOME_HOME` / `HOME`；
   会话用假 CLI（`AUTOMED_CLAUDE_BINARY` / `AUTOMED_CODEX_BINARY` 指向替身脚本，现成写法见
   `apps/web/test/{gateway-task-lifecycle,notifier-live,logtail-live,board-detail-real,task-new-real}.test.ts` 与
   `test/fixtures/{w4-ledger.sh,w5-states.sh,w6-project.sh,wait-for-transition.ts}`，源头是
   `crates/automed/tests/end_to_end.rs` 的 `FAKE_CLI`）。**替身对齐真实接口**（真实内核的 `task.get` / `task.changes` /
   事件回包形状），不是对齐调用方的想象：本项目多次「测试全绿、真机坏」都出在替身跟调用方对齐上。手写的 JSON 夹具要注明
   是从哪次真实内核回包抓的。
8. **测试收尾不留进程。** 每个起子进程的测试文件在结束时停掉假 CLI、内核与服务，**先停进程再删临时目录**（A-12）。
9. **服务端 TypeScript 走 Node 原生类型剥离**（ESM、`.ts` 后缀导入、`erasableSyntaxOnly`，无 tsx）；测试用 vitest
   （`fileParallelism: false`，超时 60 秒）；`npm run lint`（`--max-warnings 0`）、`npm run typecheck`、`npm run build` 无错误。
10. **UI 硬规则。** 用户可见文字全中文，专有名词、方法名、字段名保留原文；卡片优先、低密度；1512×944 下页面本身不出现
    纵向滚动（时间线、文档、日志在内部滚动）；沿用现有外壳与 token（`app/root.tsx`、`components/shell.tsx`、
    `styles/tokens.css`），不引入新的 UI 库。`test/styles.test.ts` 的全局守卫对新文件同样生效：TSX 不写内联样式、
    样式与 TSX 不散写颜色、圆角只用 token、3D 阴影只给主按钮、焦点环黄色、源码没有 emoji。
11. **设计文档的里程碑不超过 6 个**（计划 §7.4 的止损线：超过就会被驳回并要求拆分）。本任务面很宽，里程碑按
    「可独立观察的结果」切，不按任务卡条目一条一个。
12. **宿主门禁只在合并前跑一次**（`.autome/config.toml`：`[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、
    超时 30 分钟）；实现轮 / 修复轮结束后宿主另跑一次快检（`cargo fmt --check`、`cargo clippy`、Web lint、Web typecheck）。
    A-12 的 `sh scripts/ci-local.sh` 全绿由合并前门禁给出，不要每轮都跑它；但 **Web 全量 `cd apps/web && npx vitest run`
    实现轮报待审之前要跑一次**（见 §5 前车之鉴）。
13. 不切分支、不推送、不操作 `.worktree/` 下的其它目录、不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写进
    `.autome/output/`（已 gitignore），不进任务目录。

## 5. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（任务整理 → 设计 → 评审 / 裁决 → 实现 / 审计 → 合并）驱动 Agent 会话开发软件的本地工具，
仅支持 macOS，用自己的 Loop 开发自己。计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 把 Electron 渲染进程换成浏览器
面板：`apps/web` 的 Node 服务托管 Rust 内核 `automed`，对浏览器提供 HTTP + WebSocket。已合并：K0、K0b、K1（任务类型与任务卡）、
K2（Bug Loop）、W1–W6、W8。计划 §7.3 把 W7 与 W9（项目管理与环境）排在同一行并行——生成时 W9 是否已在跑不确定；
两边都可能碰的已有文件：`app/routes.ts`、`app/styles/app.css`、`app/components/{bell-targets,nav-links,palette-model}.ts`、
`app/load-context.ts`、`server/{main,http}.ts`、`test/bell-view.test.ts`。

**技术栈与布局：**

- Rust workspace（`rust-toolchain.toml`，edition 2024），本任务**只读、只构建**：
  - `crates/autome-domain`——纯类型与状态转换。`task.rs`：`Node`（需求 15 个节点 + Bug 的 `locate`、`repro_check`、`fix`、
    `fix_gate`、`fix_review`、`await_info`、`not_a_bug`）、`TaskState`（序列化为 `{state: draft|queued|active|paused|stopped|
    failed|done|cancelled, node|resume|at, reason}`；`Requeued` 序列化成带 `resume` 的 `queued`）、`Trigger`、
    `FailureReason`（`setup`、`gate_same_failure`、`design_budget`、`impl_budget`、`fix_budget`、`infeasible`、`protocol`、
    `session_crashed`、`rebase_conflict`、`config`、`core_step`）、需求转换表 `apply_with_merge_request`；
    `bug_loop.rs`：`apply_for`（Bug 转换表，需求任务转交 `task.rs`）；
  - `crates/automed`——SQLite 账本、Git、会话、调度、门禁；framed JSON-RPC over stdio。`src/dispatch.rs`：
    - `task_get`（约 1668 行）回包：`task`（`task_json`：`id`、`kind`、`slug`、`title`、`request`、`state`、`budget_n`、
      `branch`、`created_at`、`completed_at`、`merge_commit`、`archived`、`cost_usd`、`cost_sessions_without_cost`、`doc_dir`，
      Bug 另有 `fix_round`）、`project`（`id`、`name`、`default_branch`）、`brief`、`gate_queue_position`、`queue_position`、
      `status_block`（`status`、`design_round(_limit)`、`impl_round(_limit)`、`current_milestone`、`milestones[]`、
      `milestones_done`、`milestones_total` 等）、`status_error(_detail)`、`sessions[]`、`cost`、`decisions[]`
      （`kind`、`item_id`、`text`、`disposition`、`ruling`、`consumed`、`carried`）、`pending_decisions`、`documents[]`
      （`name`、`label`、`path`、`absolute`、`size`）、`worktree`、`next_role`、`so_far`、`retry`
      （`node`、`not_before`、`attempts`）、`gate`（最近一条 `gate.result` 事件）；
    - `task_changes`（约 2217 行）、`task_decide`（约 2124 行：`kind` 为 `backlog` 时只能 include / ignore / none，
      `dispute` 只能 ruled / none）、`task_convert`（约 1976 行：只对停在 `not_a_bug` 的 Bug 任务，`kind` 必须是 `feature`，
      可带 `start`）、`task.rerun_from`（参数 `node`）、`task.extend_budget`（参数 `extra_rounds`）、`task.reply`
      （Bug 任务补充信息不能为空；需求任务等同 reject）、`events_since`（约 2645 行）、`READ_METHODS`（约 2708 行）；
  - `crates/autome-cli`——`autome` 命令行。`src/main.rs` 的 `task_detail`（`autome status <任务>` 的输出）与
    `src/render.rs` 的 `state_line`、`failure`、`next_step`、`gate_status_line`、`gate_queue_line`、`cost_cell`、`event_line`
    是这页信息的文字版，**字段与措辞照它**。
- `apps/web`——React Router 8 框架模式 + express 5 自定义服务器；TypeScript ~6.0、Vite 8、vitest 5、ESLint 10、React 19、
  `ws ^8`；`engines.node >= 22.22.0`。与本任务相关的现状：
  - `app/routes.ts`：四条路由（`/`、`projects/:id`、`projects/:id/tasks/new`、`tasks/:id`）；
  - `app/routes/tasks.$id.tsx`：占位页——loader 经 `gateway.read` 调一次 `task.get`，离线 / 404 / 其它错误各有中文，
    `shouldRevalidate = liveShouldRevalidate(bySubjectParam(['task.', 'session.'], 'id'))`，按会话平铺 `SessionLog`；
  - `app/routes/projects.$id.tasks.new.tsx` + `components/task-new-action.ts`：action 的现成写法（`request.formData()` →
    `context.get(servicesContext).gateway.write({ op, params })` → 失败返回 `{ error: result.body.error }`，按钮用 `useWriteGate()` 置灰）；
  - `app/components/`：`session-log{.tsx,-model.ts,-controller.ts}`（W8：`<details>` 折叠，展开时才订阅，`LogSession` =
    `{ id, label, round, running, started_at }`）、`task-card-model.ts`（`cardView`：阶段、进度、动态文案、门禁短语）、
    `task-card.tsx`、`board-data.ts`（`loadBoard`：`project.get` + 逐卡 `task.get` + 必要时 `dashboard.get` 取冷却）、
    `board-live.ts`、`task-columns.ts`、`nav-links.ts`（`/tasks/<id>` 的唯一拼法）、`bell-targets.ts`、`palette-model.ts`、
    `overview-card.tsx`、`shell.tsx`；
  - `app/lib/labels.ts`：`KIND_LABELS`、`STATE_LABELS`、`NODE_LABELS`、`AWAIT_PHRASES`、`FEATURE_STAGES`、`BUG_STAGES`、
    `stageOf`、`taskStatusLine`、`milestoneProgress`、`fixRoundProgress`、`cooldownPhrase`、`retryPhrase`、`queuePhrase`、
    `gatePhrase`、`mergedPhrase` 等；`app/lib/live.tsx`：`useLiveEvents`、`writeGate` / `useWriteGate`、横幅；
    `app/lib/live-routes.ts`：`liveShouldRevalidate`、`byKind`、`bySubjectParam`；`app/lib/live-client.ts`：`/ws` 客户端；
  - `app/load-context.ts`：`servicesContext`（`{ gateway: Pick<Gateway,'read'|'write'> | null, unread?, uploads? }`，键用
    `Symbol.for`，Node / Vite SSR / 生产包三份实例共享）；`getLoadContext` 只能返回 `RouterContextProvider`；
  - `server/gateway.ts`（`read` / `write` 经白名单后 `core.call`）、`server/gates/read-gate.ts`（放行 15 个读方法，扣下
    `events.since`、`env.detect`；读请求 JSON 上限 4096、参数不能含路径）、`server/gates/write-gate.ts`、
    `server/http.ts`（`/api/read|write`、`getLoadContext`）、`server/security.ts`、`server/main.ts`（`startServer` 接线）、
    `server/eventbus*.ts`（每次 tick 后 `events.since` 拉新事件广播到 `/ws`，`LedgerEvent = { seq, kind, subject, payload }`）、
    `server/ticker.ts`（每 3 秒 `scheduler.tick`）、`server/logtail*.ts`（W8）、`server/core/supervisor.ts`（`CoreSupervisor.call`）；
  - `test/`：65 个 `*.test.ts`；`fixtures/` 有 `fake-core.mjs`、`w4-ledger.sh`、`w5-states.sh`、`w6-project.sh`、
    `w6-task-get.json`、`wait-for-transition.ts` 等。全局守卫：`labels.test.ts`（阶段表与 Rust 节点交叉核对；
    `.tsx` 不得重复状态 / 列 / 阶段的中文字面量）、`styles.test.ts`、`bell-view.test.ts`（路由与铃铛目标一致）、
    `ci-config.test.ts`（ci.yml 与 ci-local 一致）。
- `apps/desktop`——Electron 外壳（CommonJS，`node --test`），本任务不改；`renderer/screens/task.js`（1292 行）是旧任务页，
  **只作信息参考**（卡点面板、表态抽屉、合并前改动清单、文档卡、会话卡的字段用法）。
- `protocol/`——Loop 协议源（编进二进制）；`plugins/`——插件；`scripts/`——`ci-local.sh`、打包与安装脚本。

**本机（生成时）：** Node v26.5.0。本 worktree **没有** `apps/web/node_modules`，也没有 `target/`——跑 Web 测试前先
`cd apps/web && npm ci`；需要真实内核的测试默认找 `<worktree>/target/debug/{automed,autome}`（或环境变量 `AUTOMED_BIN` /
`AUTOME_BIN`），先在 worktree 里 `cargo build -p automed -p autome-cli`；部分测试需要先 `npm run build`
（`build/server/index.js`）。Web 用例总数基线生成时未实测，设计轮装好依赖后自己取，并写明是 `npx vitest run` 运行时报告的数。
不要在主检出（worktree 之外）起面板或内核。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm --prefix apps/web test`、
  `npm run build --prefix apps/web`；单文件：`cd apps/web && npx vitest run test/<文件>`
- 内核与命令行（只构建、不修改）：`cargo build -p automed -p autome-cli`
- 全部：`sh scripts/ci-local.sh`（rust：fmt、clippy、test；desktop；contract；web：lint、typecheck、test、build）
- CI：`.github/workflows/ci.yml` 的 `web` job，由 `apps/web/test/ci-config.test.ts` 守卫，须与 ci-local 保持一致。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（生成时只有模板 `README.md`，无具体规则）；仓库内无 `docs/agent-project-profile.md`；
- `docs/development/requirements.md`（§4.3 任务、§4.4 界面）、`docs/development/technical-design.md`；`docs/adr/0001..0003`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2（D1–D6 与 UI 硬规则）、§3.2（状态与看板列）、
  §4.1–§4.3（两条 Loop 与进度表示）、§5.2（WebSocket）、§5.5（本机安全）、§6 的 W7（及 W6、W8、W9、W12 的边界）、§7.4（止损）；
- 设计 token 与样式：`apps/web/app/styles/{tokens.css,app.css}`；
- 相邻任务的设计与教训：`docs/w6-项目页与任务卡-按待开始-进行中/`（看板取数、卡片文案、`lessons.md`）、
  `docs/w8-实时会话日志/`（`SessionLog`、日志订阅、占位页、`lessons.md`）、`docs/w5-外壳与项目导航-迁移-animal/`
  （外壳、`labels.ts` 交叉核对、`getLoadContext`）、`docs/服务端每次-tick-后用-events/`（事件总线与重验）。

**前车之鉴（W6、W8 复盘教训，供参考，不是结论）：**

- **实现轮可能不能监听 127.0.0.1**（`listen EPERM`），审计轮可以。阶段条、时间线拼装、操作表、文档清单匹配、Markdown 渲染、
  等待文案尽量写成纯函数或用假网关测；必须监听的集成测试集中在少数文件。跑 Web 全量之前先跑一个不加载项目源码的 listen 探针；
  探针报 EPERM 时不要反复跑全量，写明被拦住的用例，交审计轮复验。
- **vitest 的 `beforeAll` / `afterAll` 及其辅助函数里不用 `expect.poll` 或 `expect(`**，等待一律用 `vi.waitFor`；
  注册项目与建任务不要写进会被重复调用的同一个辅助函数（W8 M-03 因此 reopen）。
- **实现轮报待审之前跑一次 `cd apps/web && npx vitest run` 全量，并写出 `npm run typecheck`、`npm run lint` 的退出码。**
  W6、W8 都是里程碑全部关闭后才被 `labels.test.ts` 第 7 条（视图里的中文字面量）和 typecheck 拦下，多花了实现轮与全量门禁。
- **写进断言的耗时上限要逐项列出计时区间内的固定等待**（ticker 3 秒、`task.stop` 提交前固定等的 3 秒、轮询间隔）并取最坏情况之和；
  A-09「一个 tick 周期内」的断言尤其如此。
- **设计或裁决写「某已有测试不受改动影响」时，附上把改动临时施加后跑全量得到的失败清单**，不凭读代码下结论
  （默认参数会让调用点不出现符号名）。
- **不要给验收标准悄悄加大小、数量、场景限定**；要加，必须逐字引用任务卡里允许它的原文（如 R-03、R-04、R-08 的 Backlog / 「没有不报错」条款）。
- **有并行任务时，设计的并行冲突风险行逐个列出各里程碑要改的每个已有文件**，并注明并行任务（W9）是否也会改它。
- **rebase 解决冲突后改动了产品源码的，把受影响的里程碑重新标 `待审`**，让审计轮复验，不以「无待审里程碑」带过。
- 设计里的用例数预期写验收命令**运行时**报告的数；实现轮不改「M-xx 验收契约」小节的文字；负向对照还原后
  `git status --porcelain` 为空并写进证据；`## 里程碑` 一节只放那一张表。
- 门禁或全量里出现任务范围外（`crates/`、`apps/desktop/`）的失败：不在本任务修，在设计文档 `## Backlog` 写一条，
  含用例全名、文件行号、断言原文与机制。
