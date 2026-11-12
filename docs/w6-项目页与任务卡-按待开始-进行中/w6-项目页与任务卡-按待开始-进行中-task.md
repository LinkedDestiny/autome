# 任务：项目页与任务卡（W6）——四列看板、需求/Bug 筛选、网页上用模板创建任务卡（含附件上传）

本任务的规则见 `docs/w6-项目页与任务卡-按待开始-进行中/protocol/loop-protocol.md` 与
`docs/w6-项目页与任务卡-按待开始-进行中/protocol/session-protocol.md`。

- 设计文档：`docs/w6-项目页与任务卡-按待开始-进行中/w6-项目页与任务卡-按待开始-进行中.md`
- 任务卡正文（用户写的第一输入）：`docs/w6-项目页与任务卡-按待开始-进行中/w6-项目页与任务卡-按待开始-进行中-card.md`。
  它与下面的需求原文不一致时，以任务卡正文为准。本文件 §2 已逐条收录它的要求与验收标准。
- 任务文件生成日期：2026-11-13；基线提交 `237a5c9`（任务 T-58，分支 `autome/w6-项目页与任务卡-按待开始-进行中`，从 main 拉出；
  K0、K0b、K1（T-13）、K2（T-15）、C1、W1–W5（W5 = T-35）、T-37（网关放行 `task.create` 的 `kind`/`brief`/`draft` 与 `task.start`）均已合并进基线）
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支，状态块不写 `repos:`）
- 证据、简报、运行记录一律写在 `docs/w6-项目页与任务卡-按待开始-进行中/` 下；超过 5 行的命令输出与临时产物写进 `.autome/output/`

## 1. 需求原文

> W6 项目页与任务卡：按待开始/进行中/等你处理/已完成四列显示任务卡，可按需求/Bug 筛选，支持在网页上用模板创建任务卡（含附件上传）

这句话对应计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6 的 **W6 项目页与任务卡**。计划 §3.1（模板）、§3.2（列）、§4.3（进度表示）、
§5.2（WebSocket）、§5.4（通知级别）、§5.5（本机安全）与 §2 的 UI 硬规则一起，构成本任务的范围。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-01..R-06 取自任务卡正文「要做什么」1–6，A-01..A-07 取自任务卡正文「验收标准」1–7，引号里的文字没有改动。
每条下面的「已知事实」是任务整理轮在基线上核对到的现状，供设计轮少走弯路；它们不是设计结论，设计轮引用前要自己再核实一遍。
编号供评审、审计追溯。

### 2.1 要做什么

- **R-01 项目页四列看板与筛选。**
  「路由 `apps/web/app/routes/projects.$id*`：项目页按 §3.2 的四列显示任务卡——**待开始 / 进行中 / 等你处理 / 已完成**（列的归属规则复用 W5 的
  `task-columns.ts`，不要再写一份）；顶部可按需求 / Bug 筛选。」
  - 计划 §3.2 原文：
    | 列 | 包含 |
    |---|---|
    | 待开始 | 新增的 `Draft` 状态：卡片已创建，还没开始跑；以及排队中的任务 |
    | 进行中 | 正在跑会话或内核步骤（含门禁、重试等待、限流冷却） |
    | 等你处理 | 等你批准设计、等你合并、Bug 无法复现等你补充、失败、已停止 |
    | 已完成 | 近 14 天合并完成的；更早的折叠到「查看全部」 |
  - 已知事实：
    1. `apps/web/app/components/task-columns.ts` 导出 `columnOf(state): 'todo'|'running'|'needs_you'|'done'|null`、`countByColumn`、`AWAITING_NODES`。
       `draft`/`queued` → `todo`；`active` 按节点是否在 `AWAITING_NODES`（`await_design_approval, await_merge, await_info, not_a_bug`）分到
       `needs_you` 或 `running`；`paused`/`stopped`/`failed` → `needs_you`（`paused` 归「等你处理」是 W5 已定的，沿用）；`done` → `done`；
       `cancelled` 与未知状态 → `null`（不进任何一列）。列的中文名在 `labels.ts` 的 `COLUMN_LABELS`。
    2. `columnOf` 不管「近 14 天」。已完成列的 14 天划分要用 `task_json.completed_at`，「查看全部」怎么呈现由设计决定；
       这一层划分写在哪里都可以，但**不得**在 `task-columns.ts` 之外另写一份状态 → 列的规则。
    3. `apps/web/app/routes.ts` 是显式路由表（现在只有 `index("routes/_index.tsx")`），不是按文件名约定自动发现；新路由文件要在这里登记。
    4. W5 已经把链接定好：`app/components/nav-links.ts` 的 `projectHref(id) = /projects/<id>`、`newTaskHref(projectId) = /projects/<id>/tasks/new`、
       `taskHref(id) = /tasks/<id>`。左侧项目列表、⌘K、「新建任务卡」入口都指向这些地址，本任务的路由要让它们不改就能用。
       `/tasks/<id>` 是 W7 的页面，现在不存在，根 `ErrorBoundary` 会显示「这个页面还没开放」。
    5. 筛选「只影响显示」（A-01）：不改变数据、计数口径与列归属。筛选状态放在哪里（URL 查询串、组件状态）由设计决定。
- **R-02 任务卡组件。**
  「任务卡组件 `apps/web/app/components/task-card*`：类型标签、id、标题、阶段进度条（§4.3，需求与 Bug 各一张表，来自 `labels.ts`）、一句当前动态
  （如「实现第 3 轮 · M-02 · 运行 12 分钟」「等 Claude Code 冷却到 14:20」「等你合并」）、门禁结果、等你处理角标。」
  - 计划 §4.3 原文：
    | 类型 | 阶段 |
    |---|---|
    | 需求 | 整理 · 设计 · 批准设计 · 实现 · 审计 · 合并 · 完成。实现阶段另显示里程碑进度 `已完成 3 / 5` |
    | Bug | 定位 · 修复 · 复核 · 合并 · 完成。修复阶段另显示 `修复第 2 / 3 轮` |
  - 已知事实：
    1. `apps/web/app/lib/labels.ts` 已有 `KIND_LABELS`、`STATE_LABELS`、`COLUMN_LABELS`、`NODE_LABELS`、`AWAIT_PHRASES`、`FEATURE_STAGES`、`BUG_STAGES`、
       `nodeLabel`、`stageOf(kind, state) → {label, index, total} | null`、`taskStatusLine(state)`。`apps/web/test/labels.test.ts` 解析
       `crates/autome-domain/src/task.rs` 的 `enum Node` 做交叉核对。新增的中文（动态句式、门禁结果、轮次文案）继续只在 `labels.ts` 写一份。
    2. W5 的简版卡 `app/components/overview-card.tsx`（首页总览用）只显示类型、id、标题、项目名、阶段名与 `taskStatusLine`。本任务的完整卡与它
       是替换、复用还是并存，由设计决定并写明；首页总览的现有测试（`overview.test.ts`）要保持通过。
    3. **数据来源有缺口，设计必须先实测再定方案。** `project.get({project_id})` 的 `tasks[]` 是 `task_json`：`id, kind, slug, title, request, state,
       budget_n, branch, created_at, completed_at, merge_commit, archived, protocol_ref, metrics, doc_dir`，Bug 另带 `fix_round`。它**没有**当前会话、
       重试/冷却、门禁结果、里程碑进度。这些字段在别的只读方法里：
       - `dashboard.get` → `waiting[]`/`running[]`（只含非终态任务，且 `waiting` 的口径是 `awaits_user()`，与 §3.2 的列不一一对应），每项
         `{project_id, project_name, task, gate_queue_position, pending_decisions, session, retry}`；另有 `alerts`、`cooldowns: [{runtime, until, reason}]`、
         `environment`。`session` 含 `kind, label, runtime, model, round, started_at, running` 等；`retry` 为 `{node, not_before, attempts}` 或 `null`。
       - `task.get({task_id})` → `status_block`（含 `impl_round, impl_round_limit, current_milestone, milestones, milestones_done, milestones_total`）、
         `gate`（该任务最近一条 `gate.result` 事件）、`queue_position`、`gate_queue_position`、`sessions`、`retry`、`so_far` 等。
       - Bug 修复轮上限在 `project.get` 的 `config.bug.fix_rounds`（默认 3），已用轮次是 `task_json.fix_round`。

       本任务**不新增 JSON-RPC 方法、不改 `crates/`**，只能组合这些已有方法。每张卡一次 `task.get` 的开销、根 loader 现在已经对每个项目各调一次
       `project.get`（W5 Backlog B-01 记了这个 N+1），设计要给出取数方案与调用次数，并说明哪些卡需要哪些字段（例如里程碑进度只在实现阶段显示）。
       引用返回值时逐字粘贴实测的原始 JSON，上面只是摘要。
    4. 「门禁结果」显示什么（通过/未通过/排队第几位/未跑过）由设计根据 `gate.result` 的实测负载决定。
    5. A-02 的硬要求：**Bug 任务显示修复轮次而不是里程碑**。
- **R-03 新建任务卡。**
  「新建任务卡：选类型 → 模板预填的 Markdown 编辑器（§3.1 的两套模板）→ 附件（有大小上限，服务端 `apps/web/server/uploads*` 校验后落到任务附件目录，
  经 `task.create.attachments` 交给内核）→ 「创建并开始」或「存为待开始」（`draft`）。」
  - 计划 §3.1 的两套模板原文（标题行后面的说明文字是模板里的提示）：

    需求：
    ```markdown
    # <标题>
    ## 背景            为什么要做
    ## 要做什么        具体行为，讨论中定下的细节逐条写
    ## 不做什么        明确排除的范围
    ## 验收标准        可检查的条目
    ## 参考            仓库内文件、链接、截图（附件）
    ```
    Bug：
    ```markdown
    # <标题>
    ## 现象            看到了什么
    ## 复现步骤        1. … 2. … 3. …
    ## 期望行为
    ## 实际行为        报错原文、日志片段
    ## 环境            版本、分支、配置
    ## 线索（可选）    怀疑的位置、最近的相关改动
    ```
    C1 插件（`plugins/autome/`）按同一套模板生成正文；网页模板的小节标题要与 §3.1 一致。模板文本放在哪里、是否与插件的模板做交叉核对，由设计决定。
  - 已知事实（写通道）：
    1. `apps/web/server/gates/write-gate.ts`（T-37 之后）对 `task.create` 的规则：`kind` 只能是 `feature`/`bug`；`draft` 必须是布尔值；`brief` 必须是字符串，
       UTF-8 不超过 `MAX_BRIEF_BYTES = 65536` 字节；`request` 不超过 `MAX_PROSE_LENGTH = 8000`；`doc_refs` 只能缺省或为空数组（「网页端暂不支持指定参考文档」）；
       `attachments` 必须是**小写 UUID** 字符串数组（「附件必须是上传编号（小写 UUID）」），最多 256 项；整个载荷不超过 `MAX_TASK_CREATE_PAYLOAD_LENGTH = 512KB`。
       `task.start` 要求非空字符串 `task_id`。
    2. `apps/web/server/gateway.ts` 的 `createGateway({core, home, uploads})`：`task.create` 时对每个附件编号调用 `uploads.resolve(id)`，要求返回的是
       `<home>/web/uploads/` **之下**的绝对路径，否则回「附件 <id> 不存在或已过期，请重新上传」；通过后把编号换成路径交给内核，并删掉 `doc_refs`。
       `uploads` 现在的缺省值是 `{ resolve: () => null }`，`server/main.ts` 调用 `createGateway({ core, home })` 时**还没有传 `uploads`**——
       这个挂点就是留给本任务的 `server/uploads*` 的。`test/gateway.test.ts` 已经覆盖了 `resolve` 返回各种越界路径的情形。
    3. 内核 `task.create`（`crates/automed/src/dispatch.rs` 的 `task_create`）：有 `brief` 时 `request` 可缺省，取正文第一个 `# ` 标题行；正文为空或缺标题会被拒绝
       （「任务卡正文不能为空」「任务卡正文缺少标题：第一行写 `# 标题`」）；每个附件路径必须 `is_file()`，否则「附件不存在：…」；`draft: true` 建成 `draft` 状态，
       否则直接进入排队/开始。
    4. **内核复制附件的时机不是 `task.create`。** `task.create` 只把路径记进账本；复制发生在 `crates/automed/src/scheduler.rs` 的 `write_task_inputs`——
       任务第一次准备 worktree、第一个会话启动之前，把每个源文件按**文件名**（`Path::file_name`）复制到 `<worktree>/<doc_dir>/attachments/`，源文件不在就失败
       （「无法复制附件 …」）。由此得出三条设计必须处理的后果：一、「存为待开始」和排队中的任务，暂存文件要一直留到任务真正开始之后，不能在创建成功时就清掉；
       二、两个附件文件名相同时后一个会覆盖前一个，上传侧要保证同一张卡的附件文件名互不相同（或拒绝重名）；三、暂存路径的最后一段就是 Agent 看到的附件名，
       要保留用户的原文件名（清洗后），不能只剩 UUID。计划 §5.3 的约定是暂存在 `~/.autome/web/uploads/<uuid>/…`。任务卡正文里「落到任务附件目录」指的是这条链路的终点；
       服务端 `uploads*` 自己写入的位置受上面第 2 条的网关检查约束。以上要用真实内核实测后写进设计。
    5. 内核拒绝时网关返回 `{status: 409, body: {ok: false, error, code}}`，白名单拒绝返回 400「已拒绝：…」，内核离线返回 503「内核未连接：…」。
       页面要原样显示这句中文。
  - 已知事实（上传通道）：
    1. 现有 HTTP 层（`apps/web/server/http.ts`）只有 `/healthz`、`POST /api/read`、`POST /api/write`、`GET /api/notifications`、`POST /api/notifications/read`；
       `/api` 下挂了 `express.json({ limit: '1mb' })`。还没有任何上传端点。上传走 express 路由还是 React Router 的 action、用什么编码
       （multipart、原始字节流……），由设计决定。
    2. 计划 §5.5：「上传同样要过 Host/Origin 检查与大小限制」；「浏览器不能给出任意路径……附件只能是上传后服务端生成的路径」。
       `securityMiddleware`（`server/security.ts`）挂在 express 最前面，所有非 GET/HEAD 请求都要过 Origin 检查，Host 一律检查；新端点不得绕到它前面。
       浏览器给的文件名不能用来拼出 `uploads/<uuid>/` 之外的路径。
    3. 「大小上限」的具体数值（单个文件、单次创建的总量、个数）任务卡没有给，由设计提出并写明理由；A-03 要求超限时**服务端**拒绝并给中文提示
       （不能只靠前端拦）。上限常量只写一处，前端提示与服务端校验用同一个值。
    4. 上传了但没有创建任务的暂存文件怎么清理、任务开始之后暂存文件留不留，由设计决定（注意上面写通道第 4 条：任务开始之前不能清）；做不完的写进 `## Backlog`。
  - 「Markdown 编辑器」：仓库现有依赖里没有任何编辑器或 Markdown 渲染库。默认用现有依赖手写（例如文本框加模板预填）；设计认为确实需要新依赖时，
    按 §4 第 8 条作为争议项交用户决定。
  - 「选类型」之后切换类型时已输入的正文怎么处理（覆盖、保留、提示），由设计决定并有测试。
- **R-04 待开始的卡上有「开始」按钮。**
  「待开始的卡上有「开始」按钮（`task.start`）。」
  - 已知事实：内核 `task.start` 返回 `{task, started, queue_position}`，并发槽占满时任务进入 `queued`（`started: false`，带排队位置）。
    待开始列同时包含 `draft` 与 `queued`；只有 `draft` 可以「开始」，`queued` 的卡显示什么由设计决定。A-04 的「排队 / 进行中」对应这两种结果。
  - 写操作按钮要接 W5 的 `useWriteGate()`（`app/lib/live.tsx`）：内核离线或连接中断时置灰，并显示原因。
- **R-05 实时换列。**
  「任务被另一个进程（命令行、驱动）推进时，卡片不刷新页面就换列（W3 的 `useLiveEvents`）。」
  - 已知事实：
    1. `useLiveEvents()` 在根 `App` 里已经挂好；`/ws` 推来的账本事件经 `revalidateForEvents` 触发路由重验。每个带 `loader` 的路由模块必须导出
       `shouldRevalidate = liveShouldRevalidate(<matcher>)`——`apps/web/test/live-routes.test.ts` 的守卫会扫描 `app/root.tsx` 和 `app/routes/**`，漏了就红。
       可用的 matcher：`byKind(...前缀)`、`bySubjectParam(前缀[], 参数名)`（`app/lib/live-routes.ts`）。
    2. 根 loader（`loadShell`）现在对 `task.`、`project.`、`bug.`、`merge.` 四类事件重验，**不**对 `session.`、`gate.`、`config.` 重验。卡片上的「当前动态」
       「门禁结果」依赖会话与门禁事件；项目页的数据是复用根 loader（像 `routes/_index.tsx` 那样 `useRouteLoaderData('root')`）还是自己有 loader、
       各自对哪些事件重验，由设计决定并有测试。
    3. 账本事件里任务的 `subject` 是任务 id，不是项目 id；项目页按项目过滤事件的办法要实测事件负载后再定（`task.started` 的负载带 `project_id`，
       别的事件不一定带）。
    4. 计划 §5.2：前端不在客户端维护状态副本。「运行 12 分钟」这类随时间变化的文字怎么更新（重验、还是只在渲染时按 `started_at` 计算）由设计决定，
       但不得引入第二份任务状态。
- **R-06 「等你处理」角标的未读数。**
  「「等你处理」角标的未读数用 W4 面板库的 `store.unreadByTask(<级别为「需要你」的通知 kind>)`，库对象经 `getLoadContext` 取（W4 的 Backlog B-02）。」
  - 已知事实：
    1. `apps/web/server/store.ts` 的 `openWebStore(home)` 返回的对象有 `unreadByTask(kinds: string[]): {task_id, count}[]`（只数未读且 `task_id` 非空的通知）。
    2. 通知级别在 `apps/web/server/notifier-rules.ts`：`NOTIFICATION_RULES` 每条带 `level: 'need_you' | 'fyi'`，`levelOf(kind)` 查级别。
       「需要你」的 kind 现在是 `await_design_approval, await_merge, await_info, failed, project_alert, not_a_bug`（`project_alert` 没有 `task_id`）。
       这份 kind 列表要从规则表**算出来**，不能在页面里再抄一份。
    3. `apps/web/app/load-context.ts`：`WebServices = { gateway: KernelGateway | null }`，注释里给 W6 留了位置（「W6 can add store/notifications here when
       their first loader needs them」）；`servicesContext` 用 `Symbol.for('autome.web.services')` 注册在 `globalThis` 上，Node、Vite SSR、生产构建三处共用同一个键
       （W5 踩过的坑：`createContext` 的键在三份模块实例之间不共享，不这样做 loader 取不到值）。`server/http.ts` 的 dev、prod 两处
       `createRequestHandler` 都用 `createLoadContext({ gateway: gateway ?? null })`；`server/main.ts` 里 `store` 已经打开，但没有传给 `createHttpServer`。
       交出整个 `store` 还是一个只读的窄接口，由设计决定；没有库对象时（现有测试有不传的用法）页面怎么表现也要写明，且不能让现有测试变红。
    4. 铃铛里把通知标为已读之后，卡片角标要跟着变（W4 会广播 `notification` / `notification-read` 消息；这些是面板自己的扩展消息，不是账本事件，
       不会自动触发 `liveShouldRevalidate`）。怎么让角标「与 W4 库一致」（A-06）且实时，由设计决定并有测试。

### 2.2 验收标准（任务卡正文原文）

- **A-01** 「列定义与 §3.2 一致，有测试与 `task-columns.ts` 交叉核对；筛选只影响显示。」
- **A-02** 「卡片内容覆盖上面第 2 条的每一项，Bug 任务显示修复轮次而不是里程碑。」
- **A-03** 「新建任务卡：两种类型的模板预填正确；附件超过上限被服务端拒绝并给中文提示；「创建并开始」后卡片出现在进行中列，「存为待开始」出现在待开始列。」
- **A-04** 「待开始的卡点「开始」后不刷新页面进入排队 / 进行中。」
- **A-05** 「用命令行推进一个任务（`autome approve` 等），浏览器里的卡片在一个 tick 周期内换列（集成测试）。」
- **A-06** 「未读角标与 W4 库一致（测试）。」
- **A-07** 「`npm --prefix apps/web test`、lint、typecheck、build 全绿；`sh scripts/ci-local.sh` 全绿；1512×944 下项目页 `scrollHeight ≤ clientHeight`（列内部滚动除外）。」

对这几条的说明（不改变验收标准本身）：

- A-03「创建并开始」之后卡片在哪一列取决于并发槽：槽满时任务是 `queued`，按 §3.2 属于待开始列。验收用例要让夹具留出空槽，或者把两种结果都写清楚。
- A-05 的「浏览器」：仓库没有 Playwright，也没有 jsdom（Playwright e2e 归 W12）。先例是 `apps/web/test/shell-live.test.ts`
  （「another CLI process stops active work; ledger event revalidates the root and updates counts」）与 `eventbus-live.test.ts`：真实 `automed` + 真实 `autome` 命令行 +
  WebSocket 客户端 + loader 重验。设计要写明用会话自己能跑的命令怎样证明「一个 tick 周期内换列」（ticker 周期 3 秒），做不到的部分写进 `## 人工验收清单`。
- A-07 的一屏约束：vitest 里没有布局引擎。先例是 `apps/web/test/styles.test.ts` 的「外壳约束页面高度，四个列表内部滚动」（静态检查 CSS 规则）。
  设计给出能跑的检查，真实浏览器里的 `scrollHeight ≤ clientHeight` 测量如果会话跑不了，写进 `## 人工验收清单`，不要假装测过。
- A-07 的 `sh scripts/ci-local.sh` 全绿：项目配置是 `[gate] run = "before_merge"`，完整门禁由宿主在合并前跑一次；实现轮、审计轮只跑里程碑验收与直接相关的测试
  （见 §5「前车之鉴」）。

## 3. 范围

**可以改动的目录与文件（计划 W6「目录」，外加接线必需的最小改动）：**

- `apps/web/app/routes/projects.$id*`：项目页、新建任务卡页（或对话框）及它们的 loader / action；
- `apps/web/app/routes.ts`：**只登记**上面的新路由；
- `apps/web/app/components/task-card*`：完整任务卡；看板列、筛选条、新建表单等本任务页面用到的新组件也放在 `app/components/` 下；
- `apps/web/server/uploads*`：上传的接收、校验、暂存与 `resolve(id)`；
- `apps/web/app/lib/labels.ts`：**只增加**本任务用到的中文条目与查表函数，不改已有条目的含义（改了就要同步 `labels.test.ts` 并逐条说明）；
- `apps/web/app/load-context.ts`：给 `WebServices` 加面板库（R-06）以及上传需要的服务；不改 `servicesContext` 的键与 `createLoadContext` 的形状约定；
- `apps/web/server/http.ts`：**只加**上传端点（如果设计选 express 路由）和 `createLoadContext` 的新入参；不改已有路由、安全中间件与 `/api/*` 的语义；
- `apps/web/server/main.ts`：**只改接线**——把 `uploads` 传给 `createGateway`，把库对象等传给 `createHttpServer`；
- `apps/web/app/styles/app.css`：本任务组件的样式（只用 `tokens.css` 的 token）；
- `apps/web/app/components/` 里 W5 的文件：只在接线需要时做最小改动（例如 `bell-targets.ts` 把 `/projects/:projectId` 标为可用、`overview-card.tsx`
  与完整卡共用部件），不改变它们已有的行为，逐条说明理由；
- `apps/web/app/lib/live.tsx`、`live-routes.ts`：**只在** R-05 / R-06 的实时更新确实需要时加东西（例如新的 matcher），不改已有导出的语义；
- `apps/web/test/`：新测试与 `test/fixtures/` 下的夹具；已有测试只在接线引起必要变化时改，并逐条说明理由。

**明确不做（出现即越界；前三条是任务卡正文「不做什么」原文）：**

- 「任务详情页（W7）、会话日志（W8）、项目设置与 Onboarding（W9）。」
- 「不新增 JSON-RPC 方法；不改 `crates/`；不改 `apps/desktop/`。」
- 「不做拖动排序（T-53 之后另提）。」
- 除「开始」「创建并开始」「存为待开始」之外的写操作（批准、驳回、合并、暂停、停止、重跑、取消……都归 W7）；
- 改 `write-gate.ts`、`read-gate.ts` 的白名单与字段上限（T-37 已经放行本任务需要的字段；发现还缺什么，写进 `## 争议项`，不自己放行）；
- 改 W2 的安全检查语义、W3 的事件总线语义、W4 的通知规则与已读语义；任何令牌、Cookie、登录会话（计划 §5.5）；
- `doc_refs`（网页端暂不支持，网关会删掉它）；任务卡创建后的编辑、删除；附件的预览与下载；
- 多角色展示、路由图（计划 D6）；主题切换；手机端适配；Playwright e2e（W12）；`autome web`（W10）；Electron（W11）。

## 4. 硬性约束

1. **不碰 `crates/`、`apps/desktop/`、`protocol/`、`plugins/`、`scripts/`、`.github/`、`.autome/`。** 这些只能**读**或**运行**（解析源码做交叉核对、构建并运行
   `target/debug/automed` 与 `target/debug/autome`、读插件模板）。计划 D1：Web 任务不碰 `crates/`。
2. 只在当前 worktree 内工作，不切分支、不推送、不操作 `.worktree/` 下的其他目录，也不启动下一个会话（`AGENTS.md`）。
3. **列归属只有一份**：状态 → 列一律调用 `task-columns.ts` 的 `columnOf`，并有测试与它交叉核对（A-01）。
4. **中文只写一份**：阶段、状态、列名、类型名、动态句式都从 `labels.ts` 取；页面按内核给的节点与状态查表，不自己推断进度（计划 §4.3）。
   未知节点、未知状态按 `labels.ts` 现有约定回退为原始值，不猜。
5. **前端不维护状态副本**（计划 §5.2）：卡片、列、角标都来自 loader，实时更新靠事件触发重验。
6. loader、action 访问内核只经 `gateway.read`、`gateway.write`（白名单），不得直接 `core.call`。附件路径只能由服务端生成，浏览器只拿到上传编号。
7. **上传端点要过 Host / Origin 检查与大小限制**（计划 §5.5），超限由服务端拒绝并给中文提示；不得把文件写到 `<AUTOME_HOME>/web/uploads/` 之外。
8. **默认不新增 npm 依赖**（W5 的先例：⌘K、图标、样式都用现有依赖手写；现有依赖见 §5）。设计认为确实需要（例如 multipart 解析、Markdown 编辑器）时，
   在 `## 争议项` 列出「加依赖」与「不加依赖的简化做法」两个备选，交用户决定。
9. **样式守卫**（`apps/web/test/styles.test.ts`，对新代码同样生效）：TSX 不写内联 `style={…}`；样式与 TSX 不散写颜色（`#hex`、`rgb()`、`hsl()`、`black`）；
   圆角只用 `var(--animal-radius…)` 或 `50%`；`:focus-visible` 的焦点环用黄色 token；`--animal-shadow-btn` 只给 `.btn-primary`；源码里不用 emoji 当图标。
10. **UI 硬规则**（计划 §2，原文）：全中文，专有名词保留原文；卡片优先、低密度；1512×944 下**页面本身不出现纵向滚动**，看板列、时间线、日志面板可以在内部滚动；
    视觉沿用 animal-island 风格。看板四列各自在列内滚动。
11. TypeScript 严格类型；`npm run lint`（`--max-warnings 0`）、`typecheck`、`test`、`build` 全部通过；`skipped` 必须为 0。
12. 计划 §7.4 止损：设计文档的里程碑**不超过 6 个**；每次修订设计后重新核对这个上限。

## 5. 项目背景摘要（生成时）

**仓库：** autome-v2，单仓库。Autome = Rust 内核 + `autome` 命令行 + Web 面板（进行中）+ Electron 外壳（将改成薄壳）。它用自己的 Loop 开发自己：
`.autome/config.toml` 配置 `[loop] parallel = 3`、`[gate] command = "sh scripts/ci-local.sh"`、`run = "before_merge"`、`timeout_minutes = 30`、`[retry] attempts = 2`。

- `crates/autome-domain`：纯域。`src/task.rs` 有 `enum Node`（需求 15 个节点 `intake, design, review, adjudicate, await_design_approval, implement, gate, audit, retro,
  rebase, merge_gate, await_merge, merging, main_check, cleanup`；Bug 7 个 `locate, repro_check, fix, fix_gate, fix_review, await_info, not_a_bug`）、`TaskKind`
  （`feature`/`bug`）、`TaskState`（序列化为 `{"state": "draft"|"queued"|"active"|"paused"|"stopped"|"failed"|"done"|"cancelled", "node"|"resume"|"at": …}`，
  `failed` 另带 `reason`；等槽恢复的任务对外也是 `queued`，带 `resume`）。
- `crates/automed`：内核进程，framed JSON-RPC over stdio。`src/dispatch.rs` 的 `READ_METHODS` 共 17 个：`project.list, project.probe, project.get,
  project.onboarding.artefacts, task.get, task.changes, session.log, dashboard.get, config.get, config.validate, env.get, env.install_recipe, skills.list, events.since,
  env.detect, protocol.get, protocol.versions`。本任务相关的返回值摘要见 §2 的 R-02、R-03、R-04。账本事件 kind 有 `task.created`、`task.started`、`task.updated`、
  `gate.queued`/`gate.started`/`gate.result`、`bug.fix_round_started` 等。
- `crates/autome-cli`：`autome` 命令行，与面板共用 `$AUTOME_HOME`（默认 `~/.autome`）。测试里可以用
  `task new [需求] [--kind feature|bug] [--brief <文件|->] [--draft] [-p 项目] [--attach 文件]…`、`task start`、`approve`、`reject`、`stop`、
  `project add <路径> --skip-onboarding`、`rpc <方法> '<JSON>'` 等驱动状态变化。
- `apps/web`：React Router 8.5 框架模式（`ssr: true`）+ express 5 自定义服务器，React 19，TypeScript ~6.0，Vite 8，vitest 5（`fileParallelism: false`，
  用例与钩子超时 60 秒），ESLint 10，`ws` 8，`yaml`（仅开发依赖），`engines.node >= 22.22.0`。服务端 TypeScript 走 Node 原生类型剥离（不用 tsx）。
  运行时依赖只有 `react-router`、`@react-router/node`、`@react-router/express`、`react`、`react-dom`、`express`、`isbot`、`ws`。**没有 jsdom、没有 testing-library**：
  组件测试用 `react-dom/server` 的 `renderToString` 加 `createRoutesStub` / `createMemoryRouter`，交互逻辑写成纯函数来测。现状：
  - `server/main.ts`：`startServer({port, mode, home, binaryPath})` 依次 `openWebStore(home)`、`createEventBus`、`CoreSupervisor`、`createNotifier`、
    `createNotificationApi`、`createGateway({ core, home })`、`createHttpServer({ mode, gateway, notifications })`，监听 `127.0.0.1`，`registerUpgrade('/ws', …)`，启动 ticker（3 秒）；
  - `server/http.ts`、`server/gateway.ts`、`server/security.ts`、`server/gates/{read-gate,write-gate}.ts`、`server/store.ts`、`server/notifier*.ts`、
    `server/eventbus*.ts`、`server/core/*`、`server/ticker.ts`、`server/home.ts`（`resolveAutomeHome`）；**还没有** `server/uploads*`；
  - `app/root.tsx`：根 loader `loadShell(gateway)`（`project.list` + 每个项目一次 `project.get`，返回 `{kernel, projects, tasks}`，`tasks` 是精简过的
    `ShellTask {id, kind, title, project_id, state, fix_round?}`）、`shouldRevalidate`、外壳 `Shell`、⌘K、`WriteGateProvider`、`ErrorBoundary`；
  - `app/routes.ts`：只有 `index("routes/_index.tsx")`；`app/routes/_index.tsx` 是首页总览；**还没有** `projects.$id*`；
  - `app/components/`：`task-columns.ts`、`shell-data.ts`、`shell.tsx`、`project-list.tsx`、`overview-card.tsx`、`nav-links.ts`、`command-palette.tsx`、
    `palette-model.ts`、`bell*.ts(x)`、`bell-targets.ts`；
  - `app/lib/`：`labels.ts`、`live.tsx`（`useLiveEvents`、`liveBanner`、`writeGate`、`useWriteGate`、`useKernelRecovery`）、`live-client.ts`、`live-routes.ts`；
  - `app/styles/`：`tokens.css`（animal-island token，头部注明出处与 CC BY-NC 4.0）、`app.css`、`fonts/`；
  - `app/load-context.ts`：见 R-06；
  - `test/`：45 个测试文件加 `fixtures/`（`fake-core.mjs`、`w4-ledger.sh`、`w5-states.sh`、`w5-project-get.json` 等）。与本任务最相关的先例：
    `task-columns.test.ts`、`labels.test.ts`、`overview.test.ts`、`shell-live.test.ts`、`load-context.test.ts`、`gateway.test.ts`、`gateway-task-card.test.ts`、
    `gateway-task-lifecycle.test.ts`、`write-gate.test.ts`、`security.test.ts`、`live-routes.test.ts`、`styles.test.ts`。
- `apps/desktop`：旧 Electron 渲染进程，W11 前不动，本任务只读。`renderer/screens/` 里旧的项目页与新建任务交互可作参考。
- `plugins/autome/`：C1 插件，按计划 §3.1 的模板在对话里生成任务卡正文（只读，可用来核对模板小节）。
- `docs/design/autome-2.0-desktop-ui.html`：2026-09-15 定稿的交互设计稿，animal-island 风格的视觉与文案参考。
- `protocol/`、`scripts/`（`ci-local.sh` 等）：本任务不改。

**本机（生成时）：** Node v26.5.0；本 worktree 里还没有 `target/`、`apps/web/node_modules`、`apps/desktop/node_modules`（首次需要
`cargo build -p automed -p autome-cli` 和 `cd apps/web && npm ci`）；`~/.local/bin/autome` 存在，`~/.local/bin/automed` 不存在
（真实内核用例用 `AUTOMED_BIN` 或 `target/debug/automed`）。

**测试命令：**

- Web：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm test --prefix apps/web`、`npm run build --prefix apps/web`
  （单跑一个文件：`cd apps/web && npx vitest run test/<文件>`）
- Desktop（确认没被误改）：`npm test --prefix apps/desktop`
- 内核与命令行：`cargo build -p automed -p autome-cli`；`cargo test --workspace --all-targets`
- 全部：`sh scripts/ci-local.sh`（宿主门禁跑的就是它，分 rust、desktop、contract、web 四段；web 段只在 `apps/web/node_modules` 不存在时才执行 `npm ci`）
- CI：`.github/workflows/ci.yml`，由 `apps/web/test/ci-config.test.ts` 逐字守卫。仓库没有 remote，真正会跑的只有 ci-local。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，没有具体规则）；仓库内没有 `docs/agent-project-profile.md`；
- `docs/development/requirements.md`（U-11：1512×944 一屏不纵向滚动、界面全中文）、`docs/development/technical-design.md`；`docs/adr/0001..0003`。
  任务卡正文提到的「`docs/development/` 下的设计规范」就是这两份；animal-island 的硬规则在仓库里以 `apps/web/app/styles/tokens.css` 的文件头和
  `apps/web/test/styles.test.ts` 的守卫为准（§4 第 9 条）；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§2 决策 D1–D6 与 UI 硬规则、§3.1 模板、§3.2 看板列、§4.3 阶段表、§5.2 WebSocket 协议、§5.3 面板库与文件、
  §5.4 通知、§5.5 本机安全、§6 W6（以及 W7、W8、W9、W12 的边界，用于判断哪些不归本任务）、§7.4 止损；
- 已实现的上游设计：W5 `docs/w5-外壳与项目导航-迁移-animal/w5-外壳与项目导航-迁移-animal.md`（外壳、计数、标签表、`getLoadContext`；Backlog B-01 取数 N+1、
  B-03 里程碑进度与修复轮次留给 W6/W7），T-37 `docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write.md`（写白名单的字段规则与真实内核用例），
  W4 `docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器.md`（面板库、通知规则、Backlog B-02），
  W3 `docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md`（总线、扩展消息、路由重验），
  W2 `docs/apps-web-的服务端托管-automed-2/apps-web-的服务端托管-automed-2.md`（网关、白名单、安全检查）。

**前车之鉴（W1–W5、T-37 的复盘教训，供设计参考，不是结论）：**

- 实现轮的执行环境可能不允许监听 127.0.0.1（`listen EPERM`，常伴随 watcher EMFILE、Electron SIGABRT），审计轮与宿主门禁可以。W1–W5 与 T-37 每个任务都撞上了。
  所以：列归属、筛选、卡片文案、模板预填、上传校验（大小、文件名、路径）、未读数合并，都尽量写成纯函数或用 `renderToString` / `createRoutesStub` 来测，
  不依赖监听端口；需要监听的用例（真实上传端点、真实内核创建并开始、跨进程推进后换列）集中放在少数文件里。实现轮跑 Web 全量之前先跑一个不加载项目源码的
  listen 探针；探针报 EPERM 时，本轮不跑 `sh scripts/ci-local.sh` 全量（「全仓门禁以提交后的宿主门禁为准」），只跑本里程碑的验收命令与它所在的测试组，
  分别报出每一段的退出码（不要用 `&&` 串成一条），写明被拦住的用例数，交审计轮复验。
- 用例总数的预期写**验收命令运行时报告的数**（`npm test` 的输出），不是 `vitest list` 的静态枚举数（参数化用例会展开）；这个数依赖环境时
  （能否监听、Electron 能否启动）在同一句里写出各环境下的数（W5 L-03）。审计发现总数与预期不一致时，写出差额来自哪个测试文件（W5 L-04）。
- 测试替身要与**真实接口**对齐，不是与调用方对齐：给 `gateway`、`store`、上传服务写替身时，至少有一条用例拿真实对象跑同一段逻辑
  （本仓库四次「测试全绿、真机坏」都出在替身跟错了对象）。
- 驱动真实内核让任务进入 `failed` 的夹具，必须显式写 `retry attempts=0`、`backoff_seconds=0`（W4 L-02）。
- 调用真实 `autome`/`automed` 进程的测试，要在证据里写出实测耗时，超时上限不小于实测值的 2 倍（W4 L-05）；宿主门禁并发时这类用例最容易超时。
  宿主门禁的超时失败在本轮无法本地复现时，在原失败用例里加分阶段耗时输出（不改断言和超时），并写明下一次宿主日志该看哪个前缀的行（W5 L-06）。
- 引用实测数据结构时逐字粘贴原始 JSON（W3 L-03）。
- 验收契约里每条负向对照（「改坏 X → 点名的测试变红」），实现轮都要实跑，并在证据里写出退出码；实现轮不得改动「M-xx 验收契约」的文字（W3 L-05）。
  负向对照全部还原后，在仓库根执行 `git status --porcelain`，输出必须为空，并写进证据。
- 裁决轮如果为解决评审意见引入设计里原本没有的新机制，要在 `## 争议项` 列出「放宽范围或接受取舍」的简化备选，交给用户选（W4 L-03）。
- 用户纳入的 Backlog 条目，如果实现由别的里程碑交付，就并进那个里程碑的验收契约，不单立里程碑（W4 L-04）。
- 宿主门禁在基线上可能本来就不是全绿（W4 的 B-05、B-06 与 W5 点名的三条都是 `crates/` 里的时序类失败）。与本任务无关、按范围约束不修的失败，
  **同一提交**在设计文档 `## Backlog` 写一条，包括失败用例全名、文件行号、断言原文和机制（W5 L-07），不在本任务里修。
- 报验收结果时，要报 Web 与 desktop 的**用例总数**及其相对上一轮基线的差，`skipped` 为 0（本机有二进制时，真实内核用例不允许跳过）。
