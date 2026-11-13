# W7 任务详情：阶段进度、逐轮执行时间线、文档、门禁结果与全部控制操作

## 背景

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6「W7 任务详情：执行过程与进度」。依赖都已合并：K1（任务类型）、K2（Bug Loop）、W5（T-35 外壳）、W6（T-58 项目页与任务卡）、W8（T-59 实时会话日志）。

现状：`apps/web/app/routes/tasks.$id.tsx` 是 W8 留下的 24 行占位页——标题、一句「任务详情页还在建设中」、按会话平铺的 `SessionLog`。任务卡点进来看不到阶段、里程碑、门禁，也没有任何操作按钮；批准、合并、表态、重跑仍然只能回命令行。这是面板「可视化控制面」里最核心的一页。

内核自 W 计划写成之后又多了一批任务详情应该呈现的信息（都已在 `task.get` / 事件里，以内核实际返回为准）：费用（T-44）、门禁排队位置（T-47）、合并前门禁与快检（T-61、T-67）、门禁同因熔断（T-28）、「不是 Bug，改建成需求」（T-40）、冷却 / 重试等待、范围守卫与协议失败的原因文案。

## 要做什么

1. **页头。** 类型标签（需求 / Bug）、id、标题、所属项目（链接回项目页）、分支名、累计费用（含「有会话没有计费数据」的标注，与 CLI 口径一致）、当前状态的一句话（复用 W6 `task-card-model.ts` 的动态文案，不另写一份）。
2. **阶段条（§4.3）。** 需求与 Bug 各用 `app/lib/labels.ts` 的阶段表；当前阶段高亮，走过的阶段标完成；需求显示里程碑进度（M-01… 各自状态）与设计 / 实现轮次（已用 / 预算），Bug 显示修复轮次（已用 / 预算）与复现检查结果。任务失败 / 已停止 / 已取消时阶段条停在出事的阶段并标出状态。
3. **执行时间线。** 每个会话一条，按时间排：阶段（中文）、第几轮、CLI（claude / codex）、起止时间、用时、花费、结果摘要（例如「M-02 待审 → 已完成」「复现命令失败（符合预期）」「协议失败：…」）。内核步骤也进时间线，作为较轻的条目：快检 / 合并前门禁（命令、提交、通过 / 失败 / 排队第 N 位 / 运行中、日志路径）、rebase（有冲突时列文件）、合并、验证 main。点开会话条目就是 W8 的 `SessionLog`（默认折叠；正在运行的那一个默认展开）。数据来自 `task.get` 与该任务的事件，不新增 RPC；如果现有只读方法确实拿不到某一项，写进 Backlog，不在本任务里改内核。
4. **文档标签页。** 任务卡正文、设计文档（需求）或 Bug 文档（Bug）、评审 / 裁决 / 审计 / 复盘文档（有就列），Markdown 渲染（服务端渲染，禁用原始 HTML，链接 `rel="noreferrer"`）；设计文档的里程碑表高亮当前里程碑。任务卡正文用 `task.get` 的 `brief`；其余文档由服务端 loader 读文件：路径只能取自内核这一次 `task.get` 返回的 `documents[].absolute`（与 W8「日志路径只能来自 `session.log`」同一口径），浏览器只传文档的 `name`，不传路径；单个文档有大小上限（超过只显示前一段并注明）。`documents` 清单里没有的文档（例如 Bug 文档若未列出）写进 Backlog，不自己拼路径。
5. **改动与合并。** 合并卡点上展示 `task.changes`（文件清单、增删行数、删除文件数——删除文件数异常多时醒目提示）；只落后默认分支时（`behind` / `rebase_first`）提示「按合并后会先自动 rebase、跑合并前门禁再合并」；按下合并后页面不说「任务完成」，而是跟随状态显示「合并中 → 验证 main → 已完成」，自动路径失败时显示内核给的原因。
6. **操作区。** 按任务当前状态显示可用的操作，其余不显示或置灰并写明原因：
   - 卡点：批准 / 驳回（带意见）、Bug 的补充信息（reply）、合并、「改建成需求」（`task.convert`，Bug 判定为不是 Bug 时）；
   - 运行控制：开始（待开始）、暂停、恢复、停止、取消（二次确认）；
   - 恢复类：重跑（从哪个节点：只列内核允许的节点，需求与 Bug 各不相同）、追加预算（轮数）；
   - 内核拒绝时原样显示内核给出的那句话；输入长度上限与 write-gate 一致；内核离线时全部置灰（沿用外壳状态）。
   可用操作的判定写成一张表驱动的纯函数（状态 → 操作集合），有测试与内核的状态机交叉核对（参照 W5 `labels.ts` 与内核节点列表的交叉核对方式）。
7. **待你决定。** Backlog 与争议项逐条列出，可表态（`task.decide`：include / ignore / ruled / none，ruled 带裁定文字）；显示每条当前的表态与「下一轮生效」的说明。
8. **失败与等待看得懂。** 失败 / 已停止时在页头下方显示原因全文与内核建议的下一步（rerun / extend / cancel），对应按钮就在旁边；等待类状态（冷却到几点、重试第几次、门禁排队第几位、排队等槽位、等你合并但门禁结果已过期——T-70 合并后的字段，有就显示，没有不报错）有各自的一句话。
9. **实时。** 以上各区在任务被另一个进程推进时不刷新页面自动更新（W3 的 `useLiveEvents`，按 `task.` / `session.` / `gate.` 事件与任务 id 重验）；时间线新增条目时不打断正在看的日志（滚动位置与展开状态保持）。
10. **入口。** 项目页任务卡、首页总览卡、铃铛通知、⌘K 都能进到这一页（核对已有链接指向 `/tasks/<id>`）；页面内操作成功后的提示与跳转不离开本页。

## 不做什么

- 不新增 JSON-RPC 方法、不改 `crates/`；缺的数据进 Backlog。
- 不做角色 / 路由配置（D6）。
- 不做网页上给进行中的任务留备注（T-85 合并后另提）、不做拖动排序（T-53）。
- 不重写 W8 的 `SessionLog` 与日志订阅，只组合它。
- 不改 `apps/desktop/`。

## 验收标准

1. 阶段条：需求与 Bug 各有测试，覆盖进行中、卡点、失败、已完成；里程碑进度与修复轮次与 `task.get` 一致。
2. 时间线：对真实 automed 与假 CLI 跑一个需求任务到合并卡点，页面上逐轮出现任务整理、设计、评审、裁决、实现、审计、复盘，以及快检 / rebase / 合并前门禁条目，各条的阶段、CLI、用时、结果摘要正确（集成测试）；Bug 任务同样覆盖定位、复现检查、修复、复核。
3. 点开时间线条目能看到该会话日志（历史会话）与正在运行会话的实时增量（复用 W8，集成测试只断言接上了）。
4. 文档标签页：任务卡正文、设计文档 / Bug 文档渲染正确；原始 HTML 与 `javascript:` 链接不被渲染（测试）；里程碑表当前项高亮。
5. 合并：`task.changes` 展示；落后默认分支时出现 rebase 提示；按下合并后页面依次显示合并中、验证 main、已完成，不提前说完成（集成测试）。
6. 操作表：状态 → 可用操作的纯函数对需求、Bug 的每个状态有测试，并与内核状态机交叉核对（内核拒绝的操作页面不提供；内核允许的不缺）。
7. 每个操作各有一个对真实 automed 的集成测试：批准、驳回、reply、合并、开始、暂停、恢复、停止、取消、重跑、追加预算、改建成需求、表态。内核拒绝时页面显示内核原文。
8. 失败任务显示原因与下一步；冷却、重试、门禁排队、排队等槽位各有一句话（测试）。
9. 用命令行推进任务，页面在一个 tick 周期内更新，展开的日志不被折叠、滚动位置不丢（测试）。
10. §5.5：本页所有写操作走已有写入口，Host / Origin 检查有测试覆盖到这些 action。
11. 1512×944 下 `scrollHeight ≤ clientHeight`（时间线、文档、日志是内部滚动区域）。
12. `npm --prefix apps/web` 的 lint、typecheck、test、build 全绿；`sh scripts/ci-local.sh` 全绿。本任务新增的集成测试用到假 CLI / 子进程时，测试结束后不留存活进程（参照 T-86 的现象：`logtail-live` 测试收尾时目录先删、假 CLI 等不到停止信号而永不退出）。

## 参考

- 计划 §3.2（状态）、§4.1–§4.3（两条 Loop 与进度表示）、§5.2（WebSocket）、§5.5（本机安全）、§6 W7。
- 现有页面与组件：`apps/web/app/routes/tasks.$id.tsx`（占位，整页重写）、`apps/web/app/components/{session-log.tsx,session-log-model.ts,session-log-controller.ts,task-card-model.ts,task-card.tsx,board-data.ts,board-live.ts}`、`apps/web/app/lib/{labels.ts,live-routes.ts,live.tsx}`、`apps/web/app/load-context.ts`、`apps/web/app/routes/projects.$id.tasks.new.tsx`（action 的写法）。
- 网关：`apps/web/server/gates/{read-gate,write-gate}.ts`（`task.*` 各操作的字段与长度）。
- 内核：`crates/automed/src/dispatch.rs` 的 `task.get`（`task_json`）、`task.changes`、`task.decide`、`task.rerun_from`、`task.extend_budget`、`task.convert`；`crates/autome-domain/src/task.rs` 与 `bug_loop.rs`（状态机、`RerunFrom` 允许的节点）；CLI 的 `autome status <任务>` 输出（`crates/autome-cli/src/`）是这页信息的文字版，字段与措辞照它。
- 旧面板（只作信息参考）：`apps/desktop/renderer/screens/task.js`。
- 设计 token 与硬规则：`apps/web/app/styles/tokens.css`、`docs/development/` 下的设计规范。
