# 按项目管理的任务卡面板 + Bug 修复 Loop（由 Autome 自己的 Loop 开发）

日期：2026-11-10。状态：待执行。执行方式：第 5 节的任务逐个交给 `autome`，本文件用 `--doc` 随每个任务交给任务整理轮。

## 1. 目标

1. **按项目看任务。** 左边是项目，点进一个项目看到它的任务卡；点进一张任务卡看到这个任务的执行过程和进度。面板上不展示「角色」——设计、评审、实现、审计这些只体现为任务进度里的阶段名，不当作成员来呈现，也不在面板上配置。
2. **任务卡有三个来源**，落到内核里都是同一种任务卡：
   - 在 Claude Code（或 Codex、Cursor）里和 Agent 讨论清楚需求细节后，由它整理成任务卡并创建；
   - 在网页上直接填写创建；
   - 由 Agent 通过 `autome` 命令行（`--json`）程序化创建。
3. **需求开发和 Bug 修复走两条不同的 Loop。** 需求沿用现在的完整 Loop；Bug 走一条简化的 Loop：先证明能复现，再修，再独立复核，只在合并前停一次。
4. **实时。** 任何来源的变化（网页、命令行、Agent）都经 WebSocket 推到页面，局部刷新；需要你的时候发通知。

安装体验的目标：装好 `autome` 之后，一条 `autome web` 就能在浏览器里打开面板。

## 2. 已定的决策（2026-11-10）

| # | 决策 | 说明 |
|---|---|---|
| D1 | **Node 网关 + Rust 内核** | Node 服务托管 `automed` 子进程（framed JSON-RPC over stdio，沿用 `apps/desktop/src/sidecar.js` 的做法），对浏览器提供 HTTP + WebSocket。调度、状态机、门禁、Git 留在 Rust。**内核只在 K1、K2 两个任务里改**（任务类型、任务卡、Bug Loop，见 §4）；Web 任务不碰 `crates/`。 |
| D2 | **面板自己的状态用 SQLite + 文件，放在 `~/.autome/`** | 内核账本不动（`~/.autome/state/automed.sqlite3`）。面板只存内核没有的：通知、已读、视图偏好、上传附件的暂存。 |
| D3 | **Electron 留作薄壳** | 只负责启动本地服务、开窗口加载面板、提供原生目录选择与通知。`apps/desktop/renderer/` 与两套 IPC 白名单在 W11 删除。 |
| D4 | **前端用 React Router 8 框架模式，Node ≥ 22.22** | React Router 是 Remix v2 的正式后继，loader、action、SSR 写法相同。最初定的是 v7；2026-11-10 W1 设计时改为 8（当前大版本，v8 要求 Node ≥ 22.22）。这个下限同时让 `node:sqlite` 免 flag，并让服务端 TypeScript 直接走 Node 原生类型剥离，不需要 tsx。 |
| D5 | **一个包、一个进程** | `apps/web` 同时包含 RR7 应用与自定义 Node 服务器（HTTP、WebSocket、sidecar、tick、通知）。 |
| D6 | **面板不做角色** | 删除路由图、角色页。角色的 CLI、模型配置仍然有效，只是改在 `~/.autome/config.toml` 和项目的 `.autome/config.toml` 里写；Bug Loop 复用已有的「实现」与「审计」两份角色配置，不新增角色。 |

沿用的 UI 硬规则（2026-09-15 定下的，继续有效）：

- 全中文，专有名词保留原文；
- 卡片优先、低密度；
- 1512×944 下**页面本身不出现纵向滚动**。看板列、时间线、日志面板可以在内部滚动，这是对原规则的明确放宽。
- 视觉沿用 animal-island 风格，从 `apps/desktop/renderer/style.css` 迁移设计 token。

## 3. 任务卡

### 3.1 内容

任务卡 = 类型 + 标题 + 一份 Markdown 正文（下称「任务卡正文」）+ 可选附件。正文按类型有模板，三个来源都用同一套模板：

**需求**

```markdown
# <标题>
## 背景            为什么要做
## 要做什么        具体行为，讨论中定下的细节逐条写
## 不做什么        明确排除的范围
## 验收标准        可检查的条目
## 参考            仓库内文件、链接、截图（附件）
```

**Bug**

```markdown
# <标题>
## 现象            看到了什么
## 复现步骤        1. … 2. … 3. …
## 期望行为
## 实际行为        报错原文、日志片段
## 环境            版本、分支、配置
## 线索（可选）    怀疑的位置、最近的相关改动
```

正文由内核写进任务的文档目录 `docs/<slug>/<slug>-card.md`，随任务分支提交。它是任务整理轮（需求）和定位轮（Bug）的第一输入，排在一句话 request 之前。正文上限 64KB。

### 3.2 状态（看板的列）

| 列 | 包含 |
|---|---|
| 待开始 | 新增的 `Draft` 状态：卡片已创建，还没开始跑；以及排队中的任务 |
| 进行中 | 正在跑会话或内核步骤（含门禁、重试等待、限流冷却） |
| 等你处理 | 等你批准设计、等你合并、Bug 无法复现等你补充、失败、已停止 |
| 已完成 | 近 14 天合并完成的；更早的折叠到「查看全部」 |

创建时可以选「创建并开始」或「存为待开始」。从 Claude 讨论里创建的卡，默认存为待开始，你在面板上看一眼再点开始。

### 3.3 三个来源

| 来源 | 怎么创建 | 由谁实现 |
|---|---|---|
| Claude Code / Codex / Cursor 对话 | 讨论完说「建成 autome 任务」。插件里的技能按 §3.1 的模板整理任务卡正文，**先完整展示给你确认**，再执行 `autome task new --brief - --kind bug\|feature --draft -p <项目> --json` | C1 |
| 网页 | 项目页「新建任务卡」：选类型 → 模板预填的 Markdown 编辑器 → 附件 → 创建并开始 / 存为待开始 | W6 |
| 其他 Agent | 同一条命令行，`--json` 输出任务 id；`autome task start <id>` 开始 | K1（命令行）、C1（写进文档供 Agent 读） |

## 4. 两条 Loop

### 4.1 需求 Loop（不变）

```
任务整理 → 设计 → 评审 ⇄ 裁决 → [批准设计] → 实现 → 门禁 → 审计 ⇄ 实现 → 复盘
        → rebase → 合并前门禁 → [合并] → 合并 → 验证 main → 清理
```

停两次：批准设计、合并。

### 4.2 Bug Loop（新增，K2）

```
定位 ──▶ 复现检查（宿主跑复现命令，必须失败）──▶ 修复 ──▶ 门禁（复现命令必须通过 + [gate]）──▶ 复核
  │ 无法复现                                       ▲          │ 未通过                            │ 打回
  ▼                                               └──────────┴───────────────────────────────────┘
[等你补充信息] ──补充后──▶ 定位
                                       复核通过 ──▶ rebase → 合并前门禁 → [合并] → 合并 → 验证 main → 清理
```

各节点：

- **定位**（用「实现」角色的配置）：读任务卡正文，复现问题，找到根因，写 Bug 文档 `docs/<slug>/<slug>-bug.md`，并提交一个能复现问题的回归测试。Bug 文档带一个精简的状态块：

  ```
  status: 已定位 | 无法复现 | 不是 Bug
  fix-round: 0/3
  复现命令：cargo test -p automed some_test
  根因：一句话
  ```

  写成「无法复现」时，任务停在「等你补充信息」。你用 `autome reply <任务> "补充的信息"`（或在网页上填）把信息交给下一轮定位。

  写成「不是 Bug」时（行为符合设计，要改的是需求），任务同样停下等你决定：取消它，或者用同一份正文改建成需求任务。

- **复现检查**（内核步骤）：宿主在定位轮的提交上跑 `复现命令`，**必须以非零退出**，证明这个测试确实抓住了 Bug。跑通了说明测试没抓到问题，直接退回定位，并在 prompt 里注明原因。
- **修复**（「实现」角色）：最小改动让复现命令通过，不顺手改无关代码。`fix-round` 加一。
- **门禁**（内核步骤）：同一个复现命令必须以 0 退出；项目配置了 `[gate]` 的话，`[gate]` 也要通过。任一失败就带日志尾部退回修复。
- **复核**（「审计」角色，保持 SAME-MODEL：必须和修复用不同的模型）：检查根因是否站得住、回归测试是否有意义、改动范围是否最小。打回就回到修复，计入 `fix-round`。
- **预算**：`fix-round` 上限默认 3，可在 `.autome/config.toml` 的 `[bug] fix_rounds` 里改。用完就进入失败，你可以 `extend`。
- **没有的东西**：任务整理、设计、评审、裁决、批准设计、复盘。人只在合并前停一次（「无法复现」和失败属于异常停顿）。

需求、Bug 共用的部分不变：worktree、分支、rebase、合并前门禁、验证 main、崩溃重试与限流冷却、表态租约。

### 4.3 进度怎么表示

面板的进度条和阶段条按类型各用一张表，都由内核的节点映射而来，前端不自己推断：

| 类型 | 阶段 |
|---|---|
| 需求 | 整理 · 设计 · 批准设计 · 实现 · 审计 · 合并 · 完成。实现阶段另显示里程碑进度 `已完成 3 / 5` |
| Bug | 定位 · 修复 · 复核 · 合并 · 完成。修复阶段另显示 `修复第 2 / 3 轮` |

## 5. 架构

```
浏览器 / Electron 薄壳窗口
   │  HTTP（RR7 loader/action）  +  WebSocket /ws
   ▼
autome-web（Node ≥ 22.22，127.0.0.1:7420）
   ├─ gateway   托管 automed：帧编解码、超时、崩溃重启、读写白名单
   ├─ ticker    每 3 秒 scheduler.tick（与 `autome run` 通过驱动锁轮流）
   ├─ eventbus  每次 tick 后 events.since(cursor)，把新事件广播到 /ws
   ├─ notifier  事件 → 通知，写面板库，推送
   ├─ logtail   按订阅跟随会话日志，推送增量
   └─ ~/.autome/state/web.sqlite3 · ~/.autome/web/
   │  framed JSON-RPC（stdio）
   ▼
automed（Rust）── ~/.autome/state/automed.sqlite3 · <repo>/.autome/ · .worktree/ · docs/
```

### 5.1 事件为什么从 `events.since` 拉

sidecar 只会收到本进程发出的命令所产生的事件。命令行、Agent 在别的进程里改账本，这些事件不会出现在这根管道上。

内核的 `events` 表是 append-only 的，带 `seq` 和 `subject`。每次 tick 后从上次的游标开始读，读到的就是所有来源的变化。sidecar 推过来的帧只用来让本进程发起的写操作立刻刷新。

### 5.2 WebSocket 协议

服务端发给浏览器：

```jsonc
{ "type": "hello", "seq": 1234, "core": "connected" }
{ "type": "event", "seq": 1235, "kind": "task.updated", "subject": "T-3", "payload": {} }
{ "type": "notification", "id": "ntf-…", "kind": "await_merge", "task_id": "T-3", "title": "…" }
{ "type": "log", "session_id": "ses-…", "offset": 8192, "chunk": "…" }
{ "type": "core", "status": "offline" | "connected", "reason": "…" }
```

浏览器发给服务端：

```jsonc
{ "type": "resume", "after": 1234 }        // 断线续传；缺口超过 1000 条只回 { "type": "reload" }
{ "type": "subscribe-log", "session_id": "ses-…", "offset": 0 }
{ "type": "unsubscribe-log", "session_id": "ses-…" }
```

前端用 `useLiveEvents()` 按 `kind` 和 `subject` 让受影响路由的 loader 重新验证（`useRevalidator`）。不整页刷新，也不在客户端维护状态副本。断线时指数退避重连，顶部显示「连接中断，正在重连」。

### 5.3 面板库与文件

```
~/.autome/
├── state/
│   ├── automed.sqlite3          # 内核账本
│   └── web.sqlite3              # 面板库
└── web/
    ├── server.json              # 实际端口、pid、启动时间
    ├── uploads/<uuid>/…         # 网页上传的附件暂存，交给 task.create 后由内核复制进任务目录
    └── logs/server.log
```

`web.sqlite3` 的表：

- `schema_version`
- `event_cursor(name, seq)`
- `notifications(id, kind, project_id, task_id, title, body, event_seq, created_at, read_at)`，在 `(kind, task_id, event_seq)` 上加唯一约束
- `preferences(key, value)`

数据库用 Node 内置的 `node:sqlite`，W1 先验证它在系统 Node 和 Electron 自带的 Node 下都能用；不行就退回 `better-sqlite3`，并在设计文档里说明。

### 5.4 通知

| 内核事件 | 通知 | 级别 |
|---|---|---|
| 进入 `await_design_approval` | 「T-3 设计已定稿，等你批准」 | 需要你 |
| 进入 `await_merge` | 「T-3 可以合并了」 | 需要你 |
| Bug 进入「等你补充信息」 | 「T-7 无法复现，需要更多信息」 | 需要你 |
| 进入 `failed` | 「T-3 停在失败上：<原因>」 | 需要你 |
| `project.alert` | 「main 门禁失败」 | 需要你 |
| 进入 `done` | 「T-3 已合并」 | 知会 |

两个级别的去向：

- **需要你：** 进顶栏的通知铃铛，并给任务卡加「等你处理」角标。标签页不在前台时，发浏览器系统通知；在 Electron 薄壳里发原生通知。
- **知会：** 只进铃铛。

### 5.5 本机安全

服务能启动会读写仓库的 Agent，所以按「任何网页都可能来访问它」来防：

- 只绑 `127.0.0.1`；Host 头必须是 `127.0.0.1:<port>` 或 `localhost:<port>`，防 DNS rebinding。
- 写请求和 WebSocket 升级都要检查 Origin，必须是本服务自己的 `http://127.0.0.1:<port>` 或 `http://localhost:<port>`。
- 鉴权就到这一层为止：面板只是内核状态的展示和控制面，不做令牌、Cookie 和登录会话。跨站网页读不到响应（没有 CORS 头），也写不进来（Origin 不对）；本机进程本来就能直接用 `autome` 命令行。
- 读写白名单从 `apps/desktop/src/{ipc-gate,write-gate}.js` 迁移，连同测试。读白名单以 `dispatch.rs` 的 `READ_METHODS` 为准，测试解析源码交叉核对。
- 浏览器不能给出任意路径。添加项目先经服务端 `project.probe`；附件只能是上传后服务端生成的路径；日志路径只能来自内核。

## 6. 任务拆分（交给 Loop 的单位）

每个任务就是一次 `autome task new`。

- 「一句话需求」原样作为 request；
- 「验收」是设计定稿时你对照的清单，设计文档的里程碑必须覆盖；
- 「目录」限定改动范围，并行的任务互不重叠。

这些任务本身都按**需求**类型跑。K1、K2 合并并重新安装 CLI 之后，开发过程中发现的 Bug 就按 Bug 类型提，顺带验证 Bug Loop。

### 内核（Rust）

**K0 恢复从原节点继续（可选，先做）**
- 一句话需求：修复已停止或已暂停的任务恢复时从任务整理重新开始的问题，让它回到停下时的节点
- 目录：`crates/`
- 验收：
  1. 停在实现、审计、门禁时恢复，都回到原节点；
  2. 恢复时槽位已满就排队，轮到时仍回原节点；
  3. 新增 e2e 覆盖上面两点。

**K0b 合并时自动 rebase**（2026-11-10 第 1 轮后加入）
- 一句话需求：用户按合并时如果默认分支已经前进，任务自动 rebase、跑合并前门禁再合并，不再卡在等你合并
- 目录：`crates/`
- 背景：第 1 轮 T-8、T-9 同时等合并，先合并 T-8 后，T-9 的合并被拒（「不在最新的 main 之上，需要先 rebase」）并退回等你合并；而等你合并的任务不能停止也不能重跑，只能手工 rebase。
- 验收：
  1. 在等你合并时按合并，如果任务分支（工作区项目是任一成员仓）不在默认分支最新提交之上，就自动走 rebase → 合并前门禁（配置了门禁时）→ 合并，中途不再停下来等你；
  2. 默认分支没动时，行为和现在完全一样，现有测试不改就能通过；
  3. 合并这一步因为「不在最新提交之上」被拒（按下合并到真正合并之间 main 又动了），走同一条自动路径；因为主工作树有冲突的未提交改动被拒，仍然退回等你合并；
  4. 自动 rebase 冲突、合并前门禁失败，都按现有路径退回实现；修好后再次停在等你合并；
  5. e2e：两个任务同时等合并，先合并 A，再合并 B，B 自动 rebase、过门禁、合并，两个都完成，全程没有手工步骤；
  6. 事件里记录这次自动 rebase（如 `merge.rebased`），命令行 `autome run` 能打印出来。

**K1 任务类型与任务卡**
- 一句话需求：内核支持任务类型（需求/Bug）、任务卡正文和待开始状态，命令行 task new 增加 --kind、--brief、--draft 并新增 task start 与 reply
- 目录：`crates/`、`protocol/prompts/`（只改任务整理模板对任务卡正文的引用）
- 验收：
  1. `Task.kind` 取 `feature` 或 `bug`，默认 `feature`；store 迁移到 v5，旧任务都视为需求；
  2. `task.create` 接受 `kind`、`brief`（≤ 64KB）、`draft`；`brief` 写成 `docs/<slug>/<slug>-card.md` 并在第一个会话之前提交；
  3. 新状态 `Draft` 不占槽位，`task.start` 让它进入排队或开始；`Draft` 可以直接取消；
  4. 任务整理的 prompt 把任务卡正文列为第一输入，金样测试更新；
  5. `project.get`、`dashboard.get`、`task.get` 都带 `kind`；
  6. 命令行：`autome task new --kind bug --brief <文件|-> --draft`、`autome task start <任务>`、`autome reply <任务> "…"`（K2 之前对需求任务等同 reject）；集成测试覆盖从标准输入读取 brief；
  7. 现有需求任务的 e2e 不做任何修改就能通过。

**K2 Bug Loop**
- 一句话需求：按计划 4.2 节实现 Bug 修复 Loop：定位、复现检查、修复、门禁、复核，只在合并前停顿，无法复现时等用户补充信息
- 目录：`crates/`、`protocol/`
- 验收：
  1. 节点与转换和 §4.2 一致；需求任务的转换不变，有测试证明；
  2. 协议新增定位、修复、复核三份 prompt 和 Bug 文档的状态块规范；协议版本加一，`protocol eval` 新增对应用例；
  3. 复现检查：复现命令在定位提交上以 0 退出时退回定位；修复后以非 0 退出时退回修复；都有 e2e；
  4. 「无法复现」→ 等你补充 → `reply` → 带着补充信息回到定位，有 e2e；「不是 Bug」停下等你决定，有 e2e；
  5. `[bug] fix_rounds` 默认 3，用完进入失败，`extend` 可以追加；
  6. 复核与修复的 SAME-MODEL 约束与审计/实现相同；
  7. 一条 Bug 从创建到合并提交的完整 e2e。

### 创建入口

**C1 对话里创建任务卡的插件**
- 一句话需求：在 plugins/ 新增 autome 插件：讨论后把需求或 Bug 整理成任务卡、确认后用 autome 命令行创建，同时支持 Claude Code、Codex、Cursor
- 目录：`plugins/`
- 验收：
  1. 技能按 §3.1 的模板生成正文，判断类型（需求或 Bug），**创建前把完整正文、类型、项目展示给用户并等待确认**；
  2. 项目按当前目录推断；推断不出就列出 `autome project ls --json` 让用户选；
  3. 默认 `--draft`，用户说「直接开始」才不加；创建后回显任务 id 和面板链接（`autome web` 在运行时）；
  4. 没装 `autome` 时给出安装命令，不尝试别的方式创建；
  5. 插件的 eval 用例：讨论信息不全时先追问，而不是编造验收标准；
  6. README 写明 Agent 程序化创建的方法：命令行与 `--json` 的输出格式。

### Web 面板

**W1 骨架**
- 一句话需求：在 apps/web 建 React Router 8 框架模式 + 自定义 Node 服务器的 TypeScript 工程，接入测试与 CI
- 目录：`apps/web/`、`scripts/ci-local.sh`、`.github/workflows/ci.yml`
- 验收：
  1. `npm run dev` 和 `npm run build && npm start` 都能起，`GET /healthz` 返回 200；
  2. 启动时检查 Node ≥ 22.22，版本不够就用中文提示后退出；
  3. 用 vitest 跑测试；ci-local 和 CI 各加一个 Web 步骤（lint、类型检查、测试、构建）；
  4. 按 §5.3 验证 `node:sqlite`。

**W2 内核网关**
- 一句话需求：apps/web 的服务端托管 automed 子进程，提供带读写白名单的 RPC 网关、每 3 秒的 tick 和本机安全检查
- 目录：`apps/web/server/`
- 验收：
  1. 从 `apps/desktop/src` 迁移 framing、sidecar、core-restart 和两份白名单，原测试一起迁移并通过；
  2. automed 的查找顺序：`AUTOMED_BIN` → 同一发布包里的 → `~/.local/bin` → cargo 构建目录；
  3. 内核崩溃后按原退避策略重启，连续失败就停下，等用户手动重启；
  4. §5.5 的每一条检查都有测试；
  5. tick 返回 `skipped` 时安静跳过。

**W3 事件总线与 WebSocket**
- 一句话需求：服务端每次 tick 后用 events.since 拉取所有来源的新事件，经 WebSocket 推给浏览器，前端 useLiveEvents 按事件让对应页面重新加载
- 目录：`apps/web/server/eventbus*`、`apps/web/app/lib/live*`
- 验收：
  1. 消息格式和 §5.2 一致，支持断线续传；
  2. 集成测试：命令行在另一个进程里改变任务状态，WebSocket 客户端在一个 tick 周期内收到事件；
  3. 内核离线和恢复都会推 `core` 消息；
  4. 只刷新受影响的路由，有测试。

**W4 通知**
- 一句话需求：实现 web.sqlite3 与通知器：按规则把内核事件变成通知并实时推送，顶栏铃铛支持已读，标签页在后台时发系统通知
- 目录：`apps/web/server/store*`、`apps/web/server/notifier*`、`apps/web/app/components/bell*`
- 验收：
  1. 表结构和 §5.3 一致，带版本迁移；
  2. 规则和 §5.4 一致，表驱动，每条规则一个测试；
  3. 服务重启后不重复、不漏发（游标测试）；
  4. 已读、全部已读，未读数实时更新。

**W5 外壳与项目导航**
- 一句话需求：面板外壳：迁移 animal-island 设计 token，左侧项目列表（带进行中与等你处理计数）、首页总览、⌘K 跳转与中文标签表
- 目录：`apps/web/app/root.tsx`、`apps/web/app/components/`、`apps/web/app/styles/`、`apps/web/app/lib/labels.ts`、`apps/web/app/routes/_index*`，以及 `apps/web/server/http.ts`（只加 `getLoadContext`，见验收 6）
- 验收：
  1. 左侧是项目列表，每个项目显示进行中和等你处理的数量，实时更新；
  2. 首页总览：所有项目里「等你处理」和「进行中」的任务卡，点击进入任务；
  3. ⌘K 能跳到任意项目、任务，以及「新建任务卡」；
  4. `labels.ts` 包含需求、Bug 两张阶段表（§4.3），有测试与内核节点列表交叉核对；
  5. 内核离线时显示横幅，写操作按钮置灰。
  6. `server/http.ts` 的 `createRequestHandler` 加上 `getLoadContext`，把 W2 的 `gateway` 交给各页面的 loader 与 action（W2 的 Backlog B-03）。

**W6 项目页与任务卡**
- 一句话需求：项目页：按待开始/进行中/等你处理/已完成四列显示任务卡，可按需求/Bug 筛选，支持在网页上用模板创建任务卡（含附件上传）
- 目录：`apps/web/app/routes/projects.$id*`、`apps/web/app/components/task-card*`、`apps/web/server/uploads*`
- 依赖：K1
- 验收：
  1. 列定义与 §3.2 一致；
  2. 卡片内容：类型标签、id、标题、阶段进度条（§4.3）、一句当前动态（如「实现第 3 轮 · M-02 · 运行 12 分钟」「等 Claude Code 冷却到 14:20」）、门禁结果、等你处理角标；
  3. 新建任务卡：选类型 → 模板预填的 Markdown 编辑器 → 附件（有大小上限）→ 创建并开始 / 存为待开始；
  4. 待开始的卡上有「开始」按钮；
  5. 任务被另一个进程推进时，卡片不刷新页面就换列。

**W7 任务详情：执行过程与进度**
- 一句话需求：任务详情页：按类型显示阶段进度、逐轮执行时间线、任务卡正文与设计或 Bug 文档、门禁结果，以及批准、驳回、补充信息、合并、开始、暂停、恢复、停止、重跑、追加预算、取消等操作
- 目录：`apps/web/app/routes/tasks.$id*`
- 依赖：K1、K2
- 验收：
  1. 顶部是阶段条（§4.3），当前阶段高亮，需求显示里程碑进度，Bug 显示修复轮次；
  2. 执行时间线：每一轮一条，写明阶段、CLI、起止时间、用时、花费和结果摘要（如「M-02 待审 → 已完成」「复现命令失败（符合预期）」「门禁未通过」），点开就是这一轮的日志（W8）；
  3. 文档标签页：任务卡正文、设计文档或 Bug 文档（Markdown 渲染，里程碑表高亮当前项）；
  4. 合并前展示 `task.changes`；只落后于默认分支时（`behind` / `rebase_first`），提示「按合并后会先自动 rebase、跑合并前门禁再合并」；按下合并后不说「任务完成」，自动路径要过门禁才算完成（K0b 的 Backlog B-01）；
  5. 可用的操作随状态变化；内核拒绝时原样显示内核给出的那句话；输入长度上限与 write-gate 一致；
  6. 待你决定的 Backlog 与争议项在这里表态。

**W8 实时会话日志**
- 一句话需求：服务端按订阅跟随会话日志文件并经 WebSocket 推送增量，任务时间线中可展开查看正在运行或历史会话的日志
- 目录：`apps/web/server/logtail*`、`apps/web/app/components/session-log*`
- 验收：
  1. 日志路径只能来自内核 `session.log` 返回的路径；
  2. 每个连接最多同时订阅 4 个日志，单条消息不超过 64KB；
  3. 会话结束后自动停止跟随；可以暂停滚动、跳到最新。

**W9 项目管理与环境**
- 一句话需求：项目的添加（服务端路径校验与目录补全）、Onboarding（运行、编辑项目画像与 AGENTS.md、完成）、移除，以及环境检查页
- 目录：`apps/web/app/routes/projects.new*`、`apps/web/app/routes/projects.$id.settings*`、`apps/web/app/routes/env*`、`apps/web/server/paths*`
- 验收：
  1. 服务端只在 `$HOME` 范围内补全目录，确认前先调 `project.probe`；
  2. Electron 薄壳里额外提供原生目录选择（接口由 W11 提供，这里预留）；
  3. Onboarding 走完能生成 init commit；
  4. 支持工作区项目；
  5. 环境页显示 Git、Claude Code、Codex 及登录状态，安装和登录由服务端打开终端执行。

### 交付

**W10 启动与安装**
- 一句话需求：autome-web 启动器与发布包：start/stop/status/open、可选 launchd 自启动，面板进入 CLI 的 tar 包与 install-cli.sh，Rust CLI 新增 autome web 子命令转发
- 目录：`apps/web/bin/`、`scripts/{install-cli,package-cli}.sh`、`crates/autome-cli/src/`（只新增 `web` 子命令）
- 验收：
  1. `autome web` 默认端口被占用时自动换一个，写进 `server.json`，再打开浏览器；
  2. 重复执行时不起第二个服务，而是打开已有的那个；
  3. `autome web service install|uninstall` 管理 `~/Library/LaunchAgents/` 下的条目；
  4. 发布包包含 `autome`、`automed`、构建好的 `apps/web`；没有 Node ≥ 22.22 时只装命令行，并提示怎样装 Node；
  5. 在空的 PREFIX 和空的 AUTOME_HOME 下，从 tar 包安装到打开面板全程走通。
  6. 内核子进程拿到用户登录 shell 的 PATH（从 `apps/desktop/src/login-path.js` 迁移），launchd 自启动时也能找到 `claude`、`codex`（W2 的 Backlog B-02）。

**W11 Electron 薄壳**
- 一句话需求：把 apps/desktop 改成薄壳：用 Electron 自带的 Node 运行 apps/web 服务、窗口加载面板、提供原生目录选择与通知，删除旧渲染进程与 IPC 白名单
- 目录：`apps/desktop/`、`scripts/package.sh`
- 验收：
  1. .app 不依赖系统 Node；
  2. preload 只暴露 `pickDirectory()` 和 `notify()`；
  3. 删除 `renderer/`、`src/{ipc-gate,write-gate,sidecar,framing}.js` 及对应测试；
  4. 已经有 `autome web` 服务在运行时，直接连上它；
  5. 打包后的 .app 能启动并显示面板。

**W12 端到端与一屏验收**
- 一句话需求：用 Playwright 对真实 automed 和假 CLI 跑面板端到端测试（需求与 Bug 各一条），并逐页检查 1512×944 不出现页面滚动，接入 CI
- 目录：`apps/web/e2e/`、`.github/workflows/ci.yml`、`scripts/ci-local.sh`
- 验收：
  1. 需求：网页建卡（存为待开始）→ 开始 → 卡片实时换列 → 铃铛收到「等你批准」→ 批准 → 合并 → 完成，全程不手动刷新；
  2. Bug：用命令行 `--kind bug --brief -` 建卡 → 面板里看到定位、复现检查、修复、复核逐轮出现在时间线上 → 合并；
  3. 用命令行批准，浏览器 5 秒内更新；
  4. 内核被杀掉后显示离线横幅，恢复后补齐期间的事件；
  5. 每页 `scrollHeight ≤ clientHeight`（内部滚动区域除外）。

## 7. 怎么用 Loop 驱动

### 7.1 准备（人工，一次）

1. **合并命令行分支。** 把 `feat/cli-and-host-gate` 合并到 main，然后运行 `sh scripts/install-cli.sh`。
2. **把本文件提交到 main。** `--doc` 传的是仓库内的相对路径，任务的 worktree 从 main 拉出，所以文件必须已经在 main 上。
3. **注册项目并配置。** 运行 `autome project add ~/project/autome-v2 --skip-onboarding`，然后在仓库的 `.autome/config.toml` 里写入以下配置并提交：

   ```toml
   [loop]
   parallel = 2

   [gate]
   command = "sh scripts/ci-local.sh"
   timeout_minutes = 30

   [retry]
   attempts = 2
   backoff_seconds = 60
   ```

4. **在一个终端里常驻 `autome run`。** 驱动用的是已安装的 `~/.local/bin/autome`，任务分支上对 `crates/` 的改动，在重新安装之前不会影响它。

### 7.2 每个任务

```sh
autome task new "<一句话需求>" --doc docs/plans/2026-11-10-web-dashboard-via-loop.md
```

两个停顿点上要做的事：

- **设计定稿时：** 对照本文件中这个任务的「验收」和「目录」。缺条目或越出目录就驳回，在驳回意见里写明缺什么。
- **待合并时：** 门禁已经在 rebase 后的 head 上通过，看过改动范围后合并。

### 7.3 顺序（并行数 2，左右两列同时跑）

| 轮 | 内核 / 入口 | Web |
|---|---|---|
| 1 | K0（可选） | W1 |
| 2 | K0b（第 1 轮后加入） | W2 |
| 3 | K1 | W3 |
| 4 | K2 | W4 |
| 5 | C1（K1 之后） | W5 |
| 6 | — | W6 + W8 |
| 7 | — | W7（K2 之后）+ W9 |
| 8 | W10 | W11 |
| 9 | — | W12 |

一行两个都合并后，再开下一行。

切换节点：

- **K1、K2 合并后**，重新运行 `sh scripts/install-cli.sh`。之后发现的 Bug 用 `autome task new --kind bug --brief -` 提交，或者装好 C1 之后在 Claude 对话里提。
- **W6、W7 合并后**，用 `npm --prefix apps/web start` 起面板，改用面板本身来新建、开始、批准、合并后面的任务。

### 7.4 止损

- 同一个任务因为同一个原因被驳回两次，就停下来改本文件（提交到 main），然后再 `rerun`，不要继续驳回。
- 设计文档的里程碑超过 6 个就驳回，并在驳回意见里要求拆分成更小的任务。
- 门禁每次跑完整的 ci-local，大约 1–2 分钟；worktree 里第一次运行还要装 `node_modules`。如果门禁成了瓶颈，另开一个小任务，把 `[gate] command` 换成只跑受影响部分的脚本。

## 8. 风险

| 风险 | 对策 |
|---|---|
| 复现命令在不同环境下结果不稳定 | 复现检查失败时把输出尾部交给定位轮，定位轮必须写出稳定的复现命令；连续两次不稳定就进入「等你补充信息」 |
| Bug 本身就是需求（要改行为，而不是修错） | 定位轮写 `status: 不是 Bug`，任务停下等你决定（§4.2）；W7 提供「改为需求」按钮，用同一份正文新建一个需求任务 |
| `node:sqlite` 在 Electron 的 Node 里不可用 | W1 先验证，不行退回 `better-sqlite3` |
| 本机服务被网页跨站调用 | §5.5；W2 的验收 4 逐条测试 |
| 面板与 `autome run` 同时推进 | 内核的驱动锁保证一次只有一个在推进；面板遇到 `skipped` 安静跳过 |
| 删除 Electron 渲染进程后，角色配置没有界面了 | D6：先改配置文件。之后真的需要时，单独提一个需求 |

## 9. 不在本计划内

- 面板上的多角色展示与路由图（D6）。
- 多用户、远程访问、登录体系。
- 把 Rust 内核迁到 Node。
- 手机端适配。
