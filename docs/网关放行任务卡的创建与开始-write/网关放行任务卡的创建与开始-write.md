# 网关放行任务卡的创建与开始：写网关加 task.start / task.reply 与任务卡字段，读网关补齐并加反向检查

status: 设计中
design-round: 0/15
implementation-round: 0/0
current-milestone: M-01
current-milestone-reopens: 0
convergence-mode: normal
next-action: M-01：动代码前先 `cargo build -p automed`、`cd apps/web && npm ci`、`npm test --prefix apps/web`，把基线用例总数 B 写进 M-01 的实现证据，再改写网关与测试

任务文件：`docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write-task.md`。
任务卡正文：`docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write-card.md`。
本任务的规则见 `docs/网关放行任务卡的创建与开始-write/protocol/loop-protocol.md` 与
`docs/网关放行任务卡的创建与开始-write/protocol/session-protocol.md`。

## 背景与现状

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 的 W6（新建任务卡、「开始」）、
W7（「补充信息」）、W9（确认前先 probe）都要经网页网关调内核。内核已经支持，网关还拦着。
以下现状均由本轮读源码核实（行号以本 worktree 为准）。

**内核（只读，`crates/automed/src/dispatch.rs`）**

- `READ_METHODS: [&str; 17]`（约 2507 行）：`project.list`、`project.probe`、`project.get`、
  `project.onboarding.artefacts`、`task.get`、`task.changes`、`session.log`、`dashboard.get`、
  `config.get`、`config.validate`、`env.get`、`env.install_recipe`、`skills.list`、`events.since`、
  `env.detect`、`protocol.get`、`protocol.versions`。
- `task.create` → `task_create()`（约 1327 行）：`kind` 缺省或 `null` 为 `feature`，其余非
  `feature`/`bug` 拒「未知的任务类型 …」；`brief` 只要出现（含 `null`）就必须是字符串，去空白后
  非空，`body.len() > 65536` 拒「任务卡正文超过 64KB（N 字节，上限 65536 字节）」（Rust `len()`
  即 UTF-8 字节数）；`draft` 只要出现（含 `null`）就必须是布尔；有 `brief` 时 `request` 可省略，
  取正文第一个 `# ` 行作标题；无 `brief` 时 `request` 必填。
- `task.start` → `str_param(p, "task_id")`，再走 `Trigger::Start` 与 `scheduler::tick`
  （draft 以外的任务被拒「只有待开始…」）。**tick 会真的启动会话**。
- `task.reply` → `task_id`、`info` 都用 `str_param`（缺失或非字符串拒「缺少参数」）；Bug 任务要求
  `info` 去空白后非空并转成 `Trigger::Reply`，需求任务转成 `Trigger::Reject { feedback: info }`。
  `scheduler.rs` 对 `Trigger::Reply` 只是把 `info` 原样写进 `task.replied` 事件。**内核与调度器都
  没有对 `info` 的长度上限。**
- `project.probe` → `str_param(p, "path")`，`shellexpand_home` 只展开 `~` 与 `~/…`（用
  `ctx.home`，即 automed 进程的 `$HOME`，`main.rs` 77 行），其它原样当路径；只读：
  `is_dir`、是否仓库根、成员仓库列表，不写盘。
- 错误码线上形式是 snake_case（`envelope.rs` `#[serde(rename_all = "snake_case")]`）：
  `unknown_method`、`invalid_params`、`not_found`、`transition_rejected` 等。

**网页端**

- `apps/web/server/gates/write-gate.ts`：`ALLOWED_WRITE_OPS` 无 `task.start`、`task.reply`；
  `PROSE_FIELDS['task.create'] = ['request']`（8000 字符）；其它字符串 512 字符；所有长度按
  `string.length`（UTF-16 码元）计；整体载荷 `JSON.stringify(params).length ≤ 32768`，
  `PAYLOAD_LIMITS` 只为 `project.onboarding.save` 例外。`brief` 现在会被当普通字符串卡在 512。
  `env.detect` 在写白名单里（desktop 留下的显式写入口）。
- `apps/web/server/gates/read-gate.ts`：`ALLOWED_READ_METHODS` 13 项，与内核差
  `project.probe`、`events.since`、`env.detect`、`config.validate` 四项；对所有方法一律拒绝路径
  形状的字符串（`/`、`\`、`~` 开头、`..`、空字符）；载荷上限 4096。
- `apps/web/server/gateway.ts`：`write()` 过门禁后复制 `params`，只对 `task.create` 把
  `attachments` 解析成上传目录内的路径、删掉 `doc_refs`，其余字段原样转发；内核错误回 409 并带
  `code`。`kind`/`brief`/`draft` 会被原样转发，**`gateway.ts` 不需要改**。
- 外层限制：`server/http.ts` `express.json({ limit: '1mb' })`；sidecar 帧上限 8MB
  （`server/core/sidecar.ts` 与 `crates/automed/src/main.rs`）。
- 服务端自己用 `events.since`（`server/eventbus.ts`、`server/notifier.ts`，直接 `core.call`，不过
  读网关），浏览器经 `/ws` 收事件（计划 §5.1）。
- 测试：vitest，`apps/web/test/*.test.ts` 静态统计 286 处 `test(`（`test.each` 与循环生成的用例
  使运行时总数不同，基线以第一实现轮实测为准）；`write-gate.test.ts` 35、`read-gate.test.ts` 11、
  `gateway.test.ts` 11。已有依赖真实 `automed` 的 HTTP 测试 `gateway-http.test.ts`
  （`startServer` + 临时 `AUTOME_HOME`），并有守卫禁止按条件跳过真实内核用例。
  本 worktree 目前没有 `apps/web/node_modules` 与 `target/debug/automed`。

## 目标与非目标

**目标**（编号对应任务文件 §2）

- G-1（R-01、A-02）写网关放行 `task.start {task_id}`、`task.reply {task_id, info}`，参数缺失或
  类型不对在网关就以中文原因拒绝。
- G-2（R-02、R-03、A-01）写网关识别 `task.create` 的 `kind`、`draft`、`brief`；`brief` 按 UTF-8
  字节计、上限 65536，超限消息与内核同文；只为 `task.create` 把载荷上限提高到一定装得下「最大
  合法 brief + 其余合法字段」。
- G-3（R-04、A-03）读网关放行 `project.probe`（路径只能在 `$HOME` 内）与 `config.validate`；
  其余差集方法进「有意不放行」清单并写明理由。
- G-4（R-05、A-04）读白名单测试加反向检查，并用注入的内核源码证明它会变红。
- G-5（A-01..A-03）用真实 `automed` 经 HTTP 网关取证到达内核。
- G-6（A-05）`npm test --prefix apps/web` 全绿，用例总数只增不减；宿主门禁
  `sh scripts/ci-local.sh` 全绿。

**非目标**

- 不改 `crates/`；不改 `apps/desktop/src/ipc-gate.js`、`write-gate.js`（W11 删除）。
- 不做 W6/W7/W9 的任何页面、路由、表单，也不做 W9 的服务端目录补全（`server/paths*`）。
- 不放宽任何其它方法的路径参数；`project.add` 仍在两个网关都不可达。
- 网关不复述内核的全部校验：`brief` 去空白非空、缺标题、`request`/`brief` 二选一、Bug 任务
  `info` 非空、状态机合法性都留给内核（内核拒绝同样以中文原因经网关回 409）。
- 不经网关真正启动任务（见风险 K-3）。

## 设计

### D-1 写网关（`apps/web/server/gates/write-gate.ts`）

1. `ALLOWED_WRITE_OPS` 追加 `'task.start'`、`'task.reply'`（放在 `task.create` 一组里）。
2. `PROSE_FIELDS` 加 `'task.reply': ['info']`，上限沿用 `MAX_PROSE_LENGTH = 8000`（字符）。
3. 新常量 `MAX_BRIEF_BYTES = 65536`，新表 `BYTE_LIMITED_FIELDS = { 'task.create': { brief: MAX_BRIEF_BYTES } }`。
   `validateShape` 的字符串分支**先**查这张表：命中则按 UTF-8 字节计，超限返回
   `任务卡正文超过 64KB（${bytes} 字节，上限 ${MAX_BRIEF_BYTES} 字节）`（与内核逐字一致）；
   不命中才走原来的散文 / 512 字符规则。空字符规则对 `brief` 照旧适用。
   字节数用全局 `TextEncoder`（`new TextEncoder().encode(v).length`）计算——不需要 `import`，
   保持零依赖（约束 4）；对合法 Unicode 字符串它与 Rust `str::len()` 相等。导出一个
   `utf8ByteLength(value)` 供测试使用。
4. 新常量 `MAX_TASK_CREATE_PAYLOAD_LENGTH = 512 * 1024`，`PAYLOAD_LIMITS['task.create']` 取它；
   其它 op 维持 32768（约束 6）。计量单位不变（`JSON.stringify(params).length`），理由见 D-4。
5. `validateOpSpecific`：
   - `task.create`：`kind` 不是 `undefined`/`null` 时必须是 `'feature'` 或 `'bug'`，否则
     `kind 必须是 feature 或 bug`；`draft` 不是 `undefined` 时必须是布尔，否则 `draft 必须是布尔值`
     （内核对 `draft: null` 同样拒绝）；`brief` 不是 `undefined` 时必须是字符串，否则
     `任务卡正文必须是字符串`（内核对 `brief: null` 同样拒绝）。原有 `doc_refs`、`attachments`
     规则不变。
   - `task.start`、`task.reply`：`task_id` 必须是非空字符串，否则 `task_id 必须是非空字符串`。
   - `task.reply`：`info` 必须是字符串，否则 `info 必须是字符串`；**允许空串**——需求任务的空
     `info` 是否可接受由内核决定，Bug 任务内核会拒「补充信息不能为空」。
   - 所有新增拒绝都写成 `message: '…'` 或 ``message: `…` `` 字面量，以便现有守卫
     「写门禁全部拒绝消息为中文」扫描到。
6. `export` 增加 `MAX_BRIEF_BYTES`、`MAX_TASK_CREATE_PAYLOAD_LENGTH`、`utf8ByteLength`。

### D-2 读网关（`apps/web/server/gates/read-gate.ts`）

1. `ALLOWED_READ_METHODS` 追加 `'project.probe'`、`'config.validate'`（13 → 15）。
2. 新导出 `WITHHELD_READ_METHODS = Object.freeze(['events.since', 'env.detect'])`，每项一行注释
   写理由（见 D-3）。它不是内核清单的副本，只是两条例外。
3. 新导出 `isHomeRelativePath(value)`：`value === '~'`，或以 `'~/'` 开头且不含 `\`、不含空字符、
   按 `/` 切分后没有任何一段等于 `'..'`。
4. 新表 `HOME_PATH_PARAMS = { 'project.probe': 'path' }`——全网关**唯一**允许路径形状字符串的
   方法与键。`validateReadRequest` 在通用的逐值检查之前：若方法在表中，对应键必须存在、是字符串
   且 `isHomeRelativePath` 为真，否则 `project.probe 的 path 只能是 ~ 或 ~/ 开头的家目录路径`；
   逐值检查时跳过这一个键，其余键仍按原规则（路径形状一律拒）。其它方法不受影响。
5. 载荷上限 4096 不变（家目录内路径远小于它）。

### D-3 读白名单差集的处置（回答 Q1）

| 方法 | 处置 | 理由 |
|---|---|---|
| `project.probe` | 放行，`path` 限 `~` / `~/…` | 任务卡 A-03 要求可读。内核把 `~` 展开成 automed 进程的 `$HOME`，所以浏览器能问的只是家目录内的目录，与 W9「服务端只在 `$HOME` 范围内补全目录」同一范围；绝对路径、相对路径、`~user`、`..` 段一律拒，「浏览器不能给出任意路径」（§5.5）仍成立。probe 只读，不初始化、不写盘（内核测试已钉）。 |
| `config.validate` | 放行 | 纯读，参数只有可选的 `project_id`（标识符，仍受路径形状检查），返回的 `violations` 本来就包含在已放行的 `config.get` 里，不新增暴露面。补齐即可，不留例外。 |
| `events.since` | 有意不放行 | 服务端 eventbus/notifier 的拉取源，浏览器经 `/ws` 收事件（计划 §5.1）；再给浏览器一条翻阅整个事件账本的通道既无用途，也会让两条事件路径并存。 |
| `env.detect` | 有意不放行（读网关） | 会启动后台环境探测（有副作用，内核有频率限制），已经作为显式写入口在写白名单里（现有测试钉住「读方法中只有它在写白名单」），写请求要过 Origin 检查；读网关再开一条没有必要。 |

### D-4 长度计量与载荷上限（回答 Q2）

- `brief` 按 UTF-8 字节计，上限 65536，与内核同值同文（约束 2）。其它字段维持原单位不变。
- 载荷上限仍按 `JSON.stringify(params).length` 计，取 524288，理由是它对合法请求有可证明的余量：
  JSON 转义下每个 UTF-8 字节最多膨胀成 6 个字符（控制字符 → `\u00XX`；多字节字符反而变短），
  所以最坏情况为 brief 65536×6 = 393216，`request` 8000×6 = 48000，`project_id` 512×6 = 3072，
  `attachments` 256 个 UUID ≈ 10 000，`kind`/`draft`/键名与括号不足 200，合计约 454 500 < 524 288。
- 外层：这样的请求体最多约 0.46MB，低于 `express.json` 的 1MB；帧上限 8MB 更远。超过 1MB 的
  请求体在 express 层就以「请求 JSON 无效或超过大小上限」（中文）拒绝，不变。
- 拒绝顺序：先载荷上限、再字段上限。60KB～几百 KB 的超限 brief 落在载荷上限之内，得到的是与内核
  同文的字节超限消息；更大的被载荷上限拒（同样中文）。两种都满足 A-01「给出中文原因」。

### D-5 `info` 上限（回答 Q3）

内核与调度器都不限 `info` 长度（见背景）。网关取 `MAX_PROSE_LENGTH = 8000` 字符：需求任务上
`task.reply` 在内核里就是 `Trigger::Reject { feedback }`，与网关已有的 `task.reject.feedback`
（8000）是同一条内核路径，两个入口给同一个值才一致；Bug 任务的补充信息也是写进事件与文件的散文。
8000 个中文字符序列化后约 8000 字符，远低于 32768 载荷上限。若 W7 需要更长，应与
`task.reject.feedback` 一起调整（见 K-5）。

### D-6 `task.create` 字段组合校验到哪一层（回答 Q4）

网关只做「形状」：类型、枚举、长度、路径与空字符；组合与语义（`brief` 去空白非空、有无标题、
`request`/`brief` 至少一个、项目是否存在）留给内核。这些拒绝经网关回 409，消息是内核的中文原话。
网关对 `kind: null` 放行（内核视为缺省），对 `draft: null`、`brief: null` 拒绝（内核也拒），
不比内核宽也不比内核严。`request` 仍是 8000 字符散文，有 `brief` 时可省略；`doc_refs` 只接受空
数组、`attachments` 只接受小写 UUID，均不变。唯一比内核严的是空字符（全网关既有规则，合法正文
不含空字符）。

### D-7 现有测试的改写（回答 Q5；不删除，只改写成等价守卫）

`write-gate.test.ts`

1. 「an oversized payload is refused」：原样本（`task.create` + 200 个 1000 字符附件 ≈ 200KB）
   在新上限下不再超载荷，会改由「附件必须是上传编号」拒绝，`/上限/` 断言失败。改写为两条：
   同样的 200×1000 数组放进 `config.set_role` 的 `skills`（仍是 32KB 上限的 op）→ 以 `/上限/`
   拒绝；`task.create` 的 `brief` 长度取 `payloadLimitFor('task.create')` → 以 `/上限/` 拒绝。
   守的仍是「超载荷在网关被拒」。
2. 「only the Onboarding save gets document-sized headroom」：原断言
   `payloadLimitFor('task.create') === MAX_PAYLOAD_JSON_LENGTH` 与 R-03 直接矛盾。改名为
   「only the Onboarding save and task.create get headroom」，改为遍历 `ALLOWED_WRITE_OPS`：除这两个
   外全部等于 32768，`task.create` 等于 `MAX_TASK_CREATE_PAYLOAD_LENGTH`；大文档两条断言保留。
   守的仍是「例外只有点名的 op」，且比原来覆盖全部 op。
3. 「长度上限钉死，边界两侧和全部 op 正路径可达」：`specific` 增加
   `'task.start': { task_id: 't1' }`、`'task.reply': { task_id: 't1', info: '' }`（否则空对象会被新
   必填规则拒）；边界表增加 `['task.reply', 'info', 8000, { task_id: 't1' }]`；增加
   `MAX_BRIEF_BYTES === 65536`、`MAX_TASK_CREATE_PAYLOAD_LENGTH === 524288` 两条钉值。
   `MAX_PAYLOAD_JSON_LENGTH === 32768` 与 `task.approve` 的 32768/32769 边界不变。
4. 「dispatch 中取任意 path 的方法在两个浏览器门禁都不可达」：**不改**。M-02 之后
   `project.probe {path:'/etc'}` 在读网关仍被拒（只放家目录路径），写网关不含它；`reads.length === 17`
   与「读方法中只有 `env.detect` 在写白名单」都仍成立。

`read-gate.test.ts`

5. 「each allowed method passes with no params」：`project.probe` 现在必须带 `path`。改写为
   「each allowed method passes with its minimal params」：最小参数表只有
   `{ 'project.probe': { path: '~' } }`，其余仍是 `{}`，并断言返回的 `params` 等于最小参数；另加
   一条断言 `project.probe` 不带参数被拒。守的仍是「白名单里的每个方法都真能过门禁」。
6. 「服务端内部读方法不向浏览器开放」：原断言 `project.probe`、`config.validate` 被拒，与 A-03 和
   D-3 矛盾。改写为「有意不放行的读方法逐个被拒，清单钉死」：`WITHHELD_READ_METHODS` 排序后等于
   `['env.detect', 'events.since']`，其中每个方法 `validateReadRequest` 都拒绝。
7. 「各拒绝分支都有中文原因」：追加 `project.probe` 缺 `path` 与 `path: '/etc'` 两个请求。

`gateway.test.ts`：「读拒绝不调用内核」里的 `{ method: 'project.probe' }` 仍是 400（缺 `path`），
**不改**。

### D-8 「到达内核」怎么取证（回答 Q6）

三层，各管各的，替身只用于证明「网关原样转发」，到达内核一律用真实二进制：

- 门禁单元（`write-gate.test.ts`、`read-gate.test.ts`）：形状、边界、中文消息；`brief` 的上限与
  消息**解析 `dispatch.rs` 源码**交叉核对（读 `body.len() > N` 的 N 与 `format!` 模板），不在测试里
  抄常量。
- 网关替身（`gateway.test.ts`）：`core.call` 收到的方法与参数与请求逐字段相等（`kind`/`brief`/
  `draft`/`task_id`/`info`/`path` 不被增删改）。
- 真实内核（新文件 `apps/web/test/gateway-task-card.test.ts`，沿用 `gateway-http.test.ts` 的
  `startServer({ port: 0, mode: 'production', home, binaryPath })` 与临时 `AUTOME_HOME`）：经 HTTP
  `POST /api/write`、`/api/read`，所以 express 1MB、网关、sidecar 帧、内核整条链都在路径上。
  - 夹具：测试准备阶段用 `s.core.call('project.add', { path: <临时空目录> })` 建项目——这一步**故意
    绕过网关**（`project.add` 本来就不该对浏览器开放），只为给 `task.create` 一个真实项目。
  - A-01：不带 `request`，`brief = '# 网关六十KB缺陷\n' + 正文`，正文用中英混排补到 60×1024 字节以上、
    65536 以下；`kind:'bug'`、`draft:true` → 200，回包 `task.kind === 'bug'`、
    `task.state.state === 'draft'`、`task.request === '网关六十KB缺陷'`（标题只能从 `brief` 来，证明
    正文完整到达内核）；再经 `/api/read task.get` 读回同样的 kind 与 state。65537 字节的 `brief` →
    400，`error` 以「已拒绝：」开头、含「64KB」与中文。
  - A-02：`task.start {task_id:'不存在的任务'}` 与 `task.reply {task_id:<上面的 draft>, info:'补充'}`
    → 409，`code` 既不是 `unknown_method` 也不是 `invalid_params`（证明进了内核对应分支且参数被
    接受，拒绝来自任务查找或状态机）；实际 `code` 与消息记入证据。缺 `task_id`、`info: 7` → 400 中文。
  - A-03：测试先把 `process.env.HOME` 指向临时目录（核心进程在启动时继承环境，必须在 `setup()`
    之前设置，`afterEach` 恢复），建 `~/repo` 目录；`project.probe {path:'~/repo'}` → 200，
    `payload.path === join(临时HOME, 'repo')`、`exists === true`；`{path:'/etc'}`、
    `{path:'~/../etc'}` → 400。
  - **不在测试里经网关成功启动任务**：`task.start` 对 draft 会触发 `scheduler::tick` 真的启动会话
    （K-3）。`task.start` 的成功路径由内核自己的
    `task_start_queues_a_draft_behind_the_parallel_limit_and_records_task_started` 覆盖，网关这侧只需
    证明原样转发（替身）与确实进了内核的 `task.start` 分支（上面的 409 非 `unknown_method`）。

### D-9 改动文件清单

- 改：`apps/web/server/gates/write-gate.ts`、`apps/web/server/gates/read-gate.ts`、
  `apps/web/test/write-gate.test.ts`、`apps/web/test/read-gate.test.ts`、`apps/web/test/gateway.test.ts`。
- 新增：`apps/web/test/gateway-task-card.test.ts`（真实内核取证；文件名在任务文件允许的
  `gateway*.test.ts` 范围内）。
- 不改：`apps/web/server/gateway.ts`（新字段已原样透传，`attachments` 解析与 `doc_refs` 删除逻辑
  不涉及新字段）、`server/http.ts`（1MB 足够，见 D-4）、`crates/`、`apps/desktop/`。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 开放 | 写网关放行 task.start、task.reply 与任务卡字段，经真实内核取证 | 0 | write-gate |
| M-02 | 开放 | 读网关放行 project.probe 与 config.validate，反向检查，经真实内核取证 | 0 | read-gate |

## 里程碑说明与验收契约

两个里程碑互不依赖代码，但 M-02 的用例计数以 M-01 完成后的数为基线，所以按编号顺序做。
M-01 先做，因为它承担本任务最大的不确定性：真实内核端到端（临时项目、60KB 正文经 HTTP 与 sidecar
到达内核）。新文件 `gateway-task-card.test.ts` 在 M-01 建立，M-02 在其中追加一条。

**M-01** 覆盖 R-01、R-02、R-03、R-05（写网关部分）、A-01、A-02、A-05；前置：无。
可观察结果：网页网关能建出 60KB 正文的 Bug 草稿，`task.start`/`task.reply` 能进内核。
实现内容：D-1、D-4、D-5、D-6；D-7 第 1–3 条；D-8 的写网关单元、替身与 A-01/A-02 真实内核用例。
新增用例（8 + 1 + 2 = 11）：

- `write-gate.test.ts` +8：`task.start` 必填与类型；`task.reply` 必填、类型与 8000 边界；`kind`
  枚举与 `null`；`draft` 布尔（含 `null` 被拒）；`brief` 字节边界（`'中'.repeat(21845) + 'a'` 恰好
  65536 通过、再加 1 字节被拒；`'😀'.repeat(16384)` 恰好 65536 通过、加 `'a'` 被拒；
  `'中'.repeat(21846)`（21846 码元、65538 字节）被拒；ASCII 65536/65537；超限消息与内核同文）；
  `brief` 非字符串（`null`、数字、布尔）被拒；载荷上限（最大合法组合：65536 个 `\u0001` 的 brief +
  8000 个 `\u0001` 的 request + 512 个 `\u0001` 的 project_id + 256 个 UUID + `kind:'bug'` +
  `draft:true` 通过；超过 524288 被拒）；解析 `dispatch.rs` 核对 65536 与超限消息模板。
- `gateway.test.ts` +1：`task.create` 的 `kind`/`brief`/`draft` 与 `task.start`、`task.reply` 原样转发。
- `gateway-task-card.test.ts` +2：A-01、A-02（D-8）。

**M-02** 覆盖 R-04、R-05（读网关部分）、A-03、A-04、A-05；前置：M-01（仅为用例计数基线）。
可观察结果：网页网关能对家目录内的路径做 probe；内核新增读方法而网页端未处置时测试变红。
实现内容：D-2、D-3；D-7 第 5–7 条；D-8 的 A-03 真实内核用例。
新增用例（4 + 1 + 1 = 6）：

- `read-gate.test.ts` +4：反向检查（解析内核 `READ_METHODS`，`[&str; N]` 的 N 必须等于解析出的项数，
  防止解析失败得到空清单而假绿；内核 − 放行 − 有意不放行 = ∅；有意不放行 ⊆ 内核；两者不相交）；
  反向检查会变红（把同一个解析函数喂给在 `];` 前注入 `"task.fake_read",` 的源码，返回
  `['task.fake_read']`）；`project.probe` 家目录路径通过（`~`、`~/`、`~/code/repo`、`~/中文 目录`、
  `~/a..b`）；`project.probe` 拒绝（缺失、`''`、数字、`/etc`、`/Users/x`、`etc`、`./x`、`~root`、
  `~root/x`、`~/..`、`~/../etc`、`~/a/../../etc`、`~/a\\b`、`~/a\0b`；`{path:'~/x', other:'~/y'}`；
  且 `project.get {project_id:'~/x'}`、`session.log {session_id:'~/x'}` 仍被拒，例外不外溢）。
- `gateway.test.ts` +1：`project.probe {path:'~/x'}` 原样转发；`{path:'/etc'}` 400 且不调用内核。
- `gateway-task-card.test.ts` +1：A-03（D-8）。

### M-01 验收契约

- 验收命令：`cargo build -p automed && (cd apps/web && npm ci) && (cd apps/web && npx vitest run test/write-gate.test.ts test/gateway.test.ts test/gateway-task-card.test.ts) && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web`
- 预期：指定三个文件共 57 个用例全部通过（write-gate 35→43、gateway 11→12、gateway-task-card 0→2）；全量 `npm test` 0 失败、用例总数 = B + 11（B 为本里程碑第一实现轮改代码前实测的基线，写进实现证据）；lint 0 警告、typecheck 通过。
- 负向对照：`brief` 用 `value.length` 而不是 UTF-8 字节计时，`'中'.repeat(21846)` 被放行，brief 字节边界用例失败；不给 `task.create` 加载荷例外时，最大合法组合用例失败，且 A-01 真实内核用例的 60KB brief 收到「params 超过 32768 字符上限」400 而失败；网关丢掉 `brief` 时内核因缺 `request` 回 `invalid_params`，A-01 用例失败。

### M-02 验收契约

- 验收命令：`cargo build -p automed && (cd apps/web && npm ci) && (cd apps/web && npx vitest run test/read-gate.test.ts test/write-gate.test.ts test/gateway.test.ts test/gateway-task-card.test.ts) && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web`
- 预期：指定四个文件共 74 个用例全部通过（read-gate 11→15、write-gate 43、gateway 12→13、gateway-task-card 2→3）；全量 `npm test` 0 失败、用例总数 = B + 17；lint 0 警告、typecheck 通过。
- 负向对照：反向检查写成原来的单向检查（只查「放行 ⊆ 内核」）时，注入 `task.fake_read` 的用例拿到空差集而失败；`project.probe` 的例外放行任意路径形状时，`/etc`、`~/../etc` 拒绝用例失败；例外按值而不是按「方法+键」生效时，`project.get {project_id:'~/x'}` 不外溢用例失败；不把 `project.probe` 加进白名单时，A-03 真实内核用例收到 400 而失败。

## 测试与验证

- 每个实现轮先跑本里程碑的验收命令；输出超过 5 行的一律重定向到 `.autome/output/`（例如
  `… > .autome/output/m01-test.txt 2>&1; tail -40 .autome/output/m01-test.txt`），证据文件只写结论、
  用例数与判据。
- 基线 B：M-01 第一实现轮在改任何代码前，按 next-action 跑一次全量 `npm test --prefix apps/web`，
  记下 vitest 报告的用例总数与失败数。若基线本身有失败，照实记录并在证据里区分「既有失败」，不得
  借此删用例。
- 用例数只增不减：D-7 列出的改写都是改断言、不删 `test(`；三个受影响文件的 `test(` 计数只增。
- 真实内核用例依赖 `target/debug/automed`，与 `gateway-http.test.ts` 一致不做条件跳过（既有守卫
  「测试源码不按条件跳过真实内核用例」覆盖新文件）。
- 宿主门禁 `sh scripts/ci-local.sh` 由 Autome 在每个实现轮后、合并前后执行；本任务不改 Rust，
  Rust 与 Desktop 部分应保持原状。
- 审计轮先跑负向对照（临时改坏实现、确认对应用例变红、再还原），再独立复验验收命令。

## 人工验收清单

- H-01 无需人工项：本任务不含界面改动，A-01..A-05 全部由会话可跑的测试取证；合并前确认两个里程碑的审计证据为「通过」即可。

## 风险与未决问题

- K-1 真实内核夹具：`project.add` 对临时空目录会 `git init`、写脚手架并做初始提交（身份由内核
  自带），需要本机有 `git`；ci-local 与开发机都有。若实现期发现 `project.add` 的前置条件不同
  （例如要求先跳过 Onboarding），在夹具里按内核实际要求补齐，并在证据里写明，不改设计结论。
- K-2 改 `process.env.HOME`：只影响本测试文件（vitest 每个文件独立进程且 `fileParallelism: false`），
  并在 `afterEach` 恢复；`startServer` 的 `home` 参数是 Autome 自己的 home，与 `$HOME` 无关。
- K-3 会话启动：经网关成功 `task.start` 一个 draft 会触发 tick 并真的拉起 CLI 会话，测试里禁止这样
  做；A-02 以「进入内核 `task.start`/`task.reply` 分支」为判据（D-8）。`startServer` 若带着 ticker
  周期性 `scheduler.tick`，夹具里只有 draft 任务，draft 不会被调度，同样不会拉起会话；实现轮须确认
  夹具中没有任何非 draft 任务。
- K-4 家目录内的符号链接可能指向家目录外，probe 会跟随它报告「是否存在、是否仓库根、成员仓库名」。
  这是只读信息，范围与 W9 服务端补全相同，且仍在 Host/Origin 防线之内（§5.5「鉴权到此为止」）；
  不在本任务处理。
- K-5 `info` 8000 字符对贴日志的 Bug 补充可能偏短；若 W7 需要更长，应与 `task.reject.feedback`
  一起调整，不在本任务处理。
- K-6 含孤立代理项（lone surrogate）的字符串：`TextEncoder` 按 3 字节计，`JSON.stringify` 输出
  `\udXXX`，serde_json 会拒绝整帧。这是所有字符串字段的既有问题，不由本任务引入，也不影响合法正文。
- K-7 `npm ci` 需要 npm 缓存或网络。拿不到依赖时照实记录「未取证」与原因，不算协议失败。
