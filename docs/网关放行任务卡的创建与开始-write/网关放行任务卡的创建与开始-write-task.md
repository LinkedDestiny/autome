# 任务：网关放行任务卡的创建与开始（W6/W7/W9 的前置）

本任务的规则见 `docs/网关放行任务卡的创建与开始-write/protocol/loop-protocol.md` 与
`docs/网关放行任务卡的创建与开始-write/protocol/session-protocol.md`。

- 任务卡正文（第一输入）：`docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write-card.md`
- 设计文档：`docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write.md`
- 任务文件生成日期：2026-11-12
- 所在仓库：autome-v2（单仓库，本 worktree 即任务分支）

任务卡正文与本文件不一致时，以任务卡正文为准。

## 1. 需求原文

> 网关放行任务卡的创建与开始：write-gate 加 task.start/task.reply/task.create 的 kind、brief(64KB)、draft，read-gate 补齐 project.probe 并加反向检查

它是计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` 中 W6（新建任务卡、「开始」
按钮）、W7（「补充信息」）、W9（确认前先 probe）的前置：内核已经支持这些方法和参数，
网页端的两份白名单还没跟上，请求会被网关拒绝。

## 2. 任务要求（设计文档的里程碑必须全部覆盖）

R-xx 取自任务卡「要做什么」，A-xx 取自任务卡「验收标准」，编号供评审、审计追溯。

**要做什么**

- **R-01 写网关放行开始与补充。** `apps/web/server/gates/write-gate.ts` 放行
  `task.start {task_id}`、`task.reply {task_id, info}`；`info` 作为散文字段，给合理
  上限（与内核一致）。
- **R-02 写网关识别任务卡字段。** `task.create` 识别 `kind`（`feature` | `bug`）、
  `draft`（布尔）、`brief`（散文，上限 64KB，**按 UTF-8 字节计**）。
- **R-03 载荷上限。** `task.create` 的整体载荷上限提高到能装下 64KB 的 brief 加其余
  字段。
- **R-04 读网关补齐。** `apps/web/server/gates/read-gate.ts` 补齐内核 `READ_METHODS`
  里缺的读方法，至少 `project.probe`。
- **R-05 测试。** `apps/web/test/write-gate.test.ts`、`apps/web/test/read-gate.test.ts`
  补正例与反例；读白名单测试加反向检查：列出「内核 `READ_METHODS` 有而网页端没有」
  的差集，断言为空，或者显式列出有意不放行的方法。

**验收标准**

- **A-01** 通过网关发 `task.create {kind:"bug", brief:<60KB 文本>, draft:true}` 能到达
  内核并创建 draft 任务；brief 超过 64KB 被拒，并给出中文原因。
- **A-02** `task.start`、`task.reply` 经网关可用；参数缺失或类型不对被拒。
- **A-03** `project.probe` 经网关可读。
- **A-04** 反向检查测试在「内核新增读方法而网页端未放行」时会变红。
- **A-05** `npm test --prefix apps/web` 全绿，用例数相对基线只增不减。

## 3. 范围

**可以改动：**

- `apps/web/server/gates/write-gate.ts`
- `apps/web/server/gates/read-gate.ts`
- `apps/web/test/write-gate.test.ts`、`apps/web/test/read-gate.test.ts`
- 为满足 A-01..A-03「经网关到达内核」所必需的 `apps/web` 内其它文件（例如
  `apps/web/server/gateway.ts` 及其测试 `apps/web/test/gateway*.test.ts`）。是否需要动
  它们由设计决定，并在设计文档里逐个说明理由；不需要就不动。

**明确不做（任务卡「不做什么」，出现即越界）：**

- 不碰 `crates/`（只读，用来核对内核行为）；
- 不做 W6 的表单与页面，也不做 W7、W9 的任何页面或路由；
- 不改桌面端的白名单副本（`apps/desktop/src/ipc-gate.js`、`write-gate.js`，W11 删除）。

## 4. 硬性约束

1. **内核是权威，网关不得比内核宽。** 读白名单以 `crates/automed/src/dispatch.rs` 的
   `READ_METHODS` 为准（计划 §5.5），测试解析源码交叉核对，不在测试里再抄一份清单。
   网关的字段规则不得放行内核会拒绝的形状以外的东西，也不得拒绝任务卡要求放行的
   合法请求。
2. **brief 上限按 UTF-8 字节计，65536 字节**，与内核一致（内核用 Rust `str::len()`，
   超限消息为「任务卡正文超过 64KB（N 字节，上限 65536 字节）」）。现有网关的字符串
   上限用的是 JS `string.length`（UTF-16 码元），两者对中文差三倍，不能混用。
3. **所有拒绝消息为中文**（现有测试「写门禁全部拒绝消息为中文」「各拒绝分支都有中文
   原因」已经在守这一条，新增分支同样遵守）。
4. **两份门禁保持零依赖**：现有测试断言 `read-gate.ts`、`write-gate.ts` 不含任何
   `import` / `require`，互不引用，也不引用 desktop。
5. **浏览器不能给出任意路径**（计划 §5.5）是既有的安全边界。任何放宽都必须是按方法
   点名的、最小的，并有反例测试；不得顺手放开其它方法的路径参数。`project.add` 仍然
   在两个网关上都不可达。
6. **只为 `task.create` 提高载荷上限**，其它 op 维持 `MAX_PAYLOAD_JSON_LENGTH = 32KB`
   （现有做法是 `PAYLOAD_LIMITS` 按 op 列出例外，目前只有 `project.onboarding.save`）。
7. **用例数只增不减**（A-05）。现有用例里与新要求矛盾的断言只能改写成新的等价守卫，
   不能直接删掉；改写了哪些、为什么，写进设计文档。第一个实现轮在动代码之前先记录
   基线用例数（本 worktree 生成任务文件时 `apps/web/node_modules` 不存在，未能实测；
   静态统计 `apps/web/test/*.test.ts` 里约有 286 处 `test(`，其中 write-gate 35、
   read-gate 11，仅供参考）。
8. **宿主门禁会跑 `sh scripts/ci-local.sh`**（`.autome/config.toml` 的 `[gate]`，超时
   30 分钟），每个实现轮之后、合并前、合并后各一次。它包含 Web lint
   （`--max-warnings 0`）、typecheck、test、build，以及 Rust 与 Desktop 全套，任何一步
   红都算失败。
9. 不修改 `.autome/` 下任何内容；不切分支、不推送、不操作 `.worktree/` 下的其它目录；
   超过 5 行的命令输出与临时验证产物写进 `.autome/output/`（已 gitignore），不进任务
   文档目录。

## 5. 调研中发现、设计必须回答的问题

以下是事实与待决问题，不是结论。

**Q1 `project.probe` 与现有守卫直接冲突。**

- `read-gate.test.ts` 的「服务端内部读方法不向浏览器开放」明确断言
  `project.probe`、`events.since`、`env.detect`、`config.validate` 四个方法被读网关
  **拒绝**——这正是内核 17 项与网页端 13 项的全部差集。
- `write-gate.test.ts` 的「dispatch 中取任意 path 的方法在两个浏览器门禁都不可达」
  断言 `project.probe {path:'/etc'}` 在读、写两个网关都被拒。
- `read-gate.ts` 对所有方法统一拒绝路径形状的字符串参数（含 `/`、`\`、`~`、`..`、
  空字符），而内核的 `project.probe` 只接受一个 `path` 参数，路径是它唯一的输入。
- 计划 §5.5 的原话是「浏览器不能给出任意路径。添加项目先经**服务端**
  `project.probe`」；W9 是「服务端只在 `$HOME` 范围内补全目录，确认前先调
  `project.probe`」。
- 任务卡要求 A-03「`project.probe` 经网关可读」，并允许反向检查「显式列出有意不放行
  的方法」。

设计要定：`project.probe` 的 `path` 怎样通过读网关（按方法点名的例外、形状约束），
上面两条现有测试改写成什么，其余三个差集方法（`events.since`、`env.detect`、
`config.validate`）是放行还是进入「有意不放行」清单及各自理由。以任务卡为准：
`project.probe` 必须可读。

**Q2 长度的计量单位。** 现有上限全部按 `string.length` 和 `JSON.stringify(params).length`
计。brief 要按 UTF-8 字节计（约束 2）；`task.create` 的载荷上限要保证「恰好 65536 字节
的合法 brief + 其余字段」一定通过——注意 JSON 转义会让序列化长度大于原文（换行、
引号、反斜杠翻倍，控制字符变成 `\u00XX`）。上限取多少、按什么计、边界两侧怎么测，
由设计给出。相关的外层限制：`apps/web/server/http.ts` 的 `express.json({ limit: '1mb' })`，
sidecar 帧上限 8MB（`apps/web/server/core/sidecar.ts` 与 `crates/automed/src/main.rs`）。

**Q3 `info` 的上限。** 任务卡写「与内核一致」。调研时在 `dispatch.rs` 的 `task.reply`
分支里没有看到对 `info` 的长度限制（只要求是字符串；Bug 任务还要求去空白后非空，
需求任务等同 `task.reject` 的 `feedback`）。网关里同类字段 `feedback`、`ruling` 的上限
是 `MAX_PROSE_LENGTH = 8000`。设计先核实内核与调度器（`scheduler.rs` 把补充信息写成
文件并提交）是否另有限制，再定值并说明理由。

**Q4 `task.create` 字段之间的关系。** 内核的规则：`kind` 缺省为 `feature`，非
`feature`/`bug` 被拒；`brief` 必须是字符串、去空白后非空、不超过 65536 字节；`draft`
必须是布尔；有 `brief` 时 `request` 可省略（取正文第一行 `# 标题`），没有 `brief` 时
`request` 必填。网关现在把 `request` 当 8000 字符的散文字段，`doc_refs` 只接受空数组，
`attachments` 只接受小写 UUID。网关对这些组合校验到哪一层、哪些留给内核，由设计
决定；网关不必复述内核的全部校验，但 A-01、A-02 要求的拒绝必须发生。

**Q5 现有测试中会受影响的断言**（供设计盘点，不止这些）：

- `write-gate.test.ts`「长度上限钉死，边界两侧和全部 op 正路径可达」：断言
  `MAX_PAYLOAD_JSON_LENGTH === 32768`，并对 `ALLOWED_WRITE_OPS` 的每个 op 用
  `specific[op] ?? {}` 断言通过——新增的 `task.start`、`task.reply` 若要求必填参数，
  空对象就不再通过；
- 同文件「dispatch 中取任意 path 的方法…」：见 Q1；它还断言 `reads.length === 17`；
- `read-gate.test.ts`「every allowed method is a read in the core」是现有的单向检查，
  反向检查与它并列；「服务端内部读方法不向浏览器开放」见 Q1。

**Q6 「到达内核」怎么取证。** A-01 要求 60KB brief 经网关到达内核并建出 draft 任务。
`apps/web/test/` 已有 `gateway.test.ts`、`gateway-http.test.ts` 以及依赖真实 `automed`
二进制的 `core-*.test.ts`。设计要说明 A-01..A-03 各自用哪一层测试取证；替身必须与
真实内核接口对齐，而不是与调用方对齐。另注意 `gateway.ts` 的 `write()` 在转发
`task.create` 前会把 `attachments` 的上传编号解析成路径，新字段需要原样透传。

## 6. 项目背景摘要（生成时）

**autome-v2** 是 Autome 2.0：用 Loop（设计 → 评审/裁决 → 实现/审计 → 合并）驱动
Agent 会话开发软件的本地工具，仅支持 macOS。本仓库用自己的 Loop 开发自己。工作目录
下只有这一个仓库。

**技术栈与布局：**

- Rust workspace（`rust-toolchain.toml` 固定版本）：
  - `crates/autome-domain` —— 纯类型与状态转换（`task.rs` 的 `TaskKind`、`Trigger`，
    `bug_loop.rs`）；
  - `crates/automed` —— SQLite、Git、会话启动、调度、宿主门禁、JSON-RPC 分发。
    `src/dispatch.rs`：`dispatch()` 的 match 臂是方法清单，`task_create()` 是任务卡
    参数的校验处，文件末尾的 `pub const READ_METHODS: [&str; 17]` 是读白名单的源头；
  - `crates/autome-cli` —— `autome` 命令行。
- `apps/web` —— Web 面板：React Router 8 框架模式 + 自定义 Node 服务器，TypeScript，
  ESM，Node ≥ 22.22（本机 v26.5.0）。独立包，自带 `package-lock.json`，仓库根没有
  `package.json`。
  - `server/gateway.ts` —— `read()` / `write()` 先过门禁再 `core.call`；部分 op 由网关
    本地处理（`core.restart`、`rules.*`）；
  - `server/gates/read-gate.ts` —— `ALLOWED_READ_METHODS`（13 项）、
    `MAX_PAYLOAD_JSON_LENGTH = 4096`、`validateReadRequest`；
  - `server/gates/write-gate.ts` —— `ALLOWED_WRITE_OPS`、`PROSE_FIELDS`
    （`MAX_PROSE_LENGTH = 8000`）、`ARRAY_FIELDS`、`PAYLOAD_LIMITS`、普通字符串上限
    512、`validateShape` + `validateOpSpecific` + `validateWriteRequest`；
  - `server/core/` —— 托管 `automed` 子进程（帧编解码、重启）；
  - `test/*.test.ts` —— vitest，断言用 `node:assert/strict`；用例名中英文都有。
- `apps/desktop` —— Electron 外壳，持有白名单的旧副本，本任务不碰。
- `protocol/` —— Loop 协议的源头（编进二进制）；`plugins/`；`scripts/`。

**测试命令：**

- 本任务最直接的：`npm test --prefix apps/web`（`vitest run`）；
  `npm run lint --prefix apps/web`；`npm run typecheck --prefix apps/web`；
  `npm run build --prefix apps/web`。全新 worktree 要先 `cd apps/web && npm ci`。
- 全套（门禁用的就是它）：`sh scripts/ci-local.sh` —— cargo fmt / clippy / test、
  端到端、Desktop、Contract（检查 `dispatch.rs` 里仍有 `pub const READ_METHODS`）、
  Web lint / typecheck / test / build。每一步都会跑，最后汇总。
- `.github/workflows/ci.yml` 与 ci-local 对应；仓库没有 remote，实际执行的只有 ci-local。

**权威文件：**

- `AGENTS.md`（仓库根）；`.autome/rules/`（目前只有模板 README，没有具体规则）；
  仓库内没有 `docs/agent-project-profile.md`；
- 上游计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md`：§3.1（任务卡内容，正文
  上限 64KB）、§3.3（三个来源）、§5（架构）、§5.5（本机安全：读写白名单、浏览器不能
  给出任意路径）、§6（W6、W7、W9、W11）；
- `docs/development/requirements.md`、`docs/development/technical-design.md`、
  `docs/adr/0001..0003`。
