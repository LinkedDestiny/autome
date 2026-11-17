# Web 面板按动森风格重做：彩色组件与信息层次，补回 Loop 配置与技能绑定

status: 实现中
design-round: 1/15
implementation-round: 4/65
current-milestone: M-04
current-milestone-reopens: 0
convergence-mode: normal
next-action: 在允许 ps 与浏览器的环境复跑 M-04 验收及快检，核对一屏与截图，全部通过后标待审

任务文件：`docs/web-面板按动森风格重做/web-面板按动森风格重做-task.md`
任务卡：`docs/web-面板按动森风格重做/web-面板按动森风格重做-card.md`
设计原型（用户已确认）：`docs/design/autome-web-panel-v3.html`

本文引用任务文件的编号：§3 的 A-1…G-3 是任务要求，§4 的验-1…验-9 是验收标准，§6 的「约束 n」是硬性约束，§7 的 X-Qn 是待定问题。

## 背景与现状

任务文件 §2、§8 已写的不重复。下面只记设计轮实测得到、并且会改变方案的事实。

### 设计轮实测（2026-11-17，本 worktree 构建的 `target/debug/autome`，临时 `HOME` 与 `AUTOME_HOME`）

用 `autome rpc` 直接调内核，夹具是一个临时仓库、三个技能（全局 `alpha` 两个工具都看得到，全局 `beta` 只有 Claude 看得到，项目 `gamma` 只有 Claude 看得到）。

1. **`config.set_loop` 在绑定了任何技能之后一律被拒。** 用 `config.set_role` 给设计角色绑定全局技能 `alpha`，内核接受了。接着
   `config.set_loop {"parallel":2}` 返回「设计 绑定了不存在的技能 alpha」，全局和项目范围都这样。根因在
   `crates/automed/src/dispatch.rs:1471` 与 `:1491`：`set_loop` 用 `config::NoSkills` 做校验，`set_role` 在 `:1368`、`:1397`
   用的是真实扫描到的技能。也就是说 E-6 和 F-2 不能同时成立。方案的处理见「方案 · 对任务卡字面的取舍」第 1 条。
2. **内核缺省配置**：六个角色的模型都是空串（用 CLI 默认）。设计、实现、裁决用 Claude，评审、审计、复盘用 Codex。Effort 除裁决
   （`null`）外都是 `high`。Loop 默认值是并行 3、设计轮 15、预算系数 5、会话 120 分钟。
3. **`config.get` 的形状**：顶层键是 `defaults`、`global`、`resolved`、`scope`、`violations`。`resolved.roles` 是数组，每项
   `{role, config, provenance}`。**不返回项目文件里的原始覆盖**，所以 Loop 默认值四项没有逐项的来源。
4. **`skills.list` 的形状**：`{scope, skills:[{name, sources:[{path, runtime, scope}], visible_to, project_scoped, bound_roles,
   conflicts}]}`。没有描述字段。带 `project_id` 时同时列出全局技能和该项目的技能。
5. **拒绝文案**（`transition_rejected`，`describe_violation` 用「；」把多条违规连起来）：
   - 「评审 与 设计 都没有指定模型，会用同一个 CLI 的同一个默认模型；请至少给一边指定模型」
   - 「评审 与 设计 都配成了 claude:m1；评测侧必须与生成侧不同模型」
   - 「审计 绑定了不存在的技能 gamma」：全局范围下绑定项目技能
   - 「审计 用 Codex，看不到技能 beta」
6. **项目范围写入**：项目文件里只写出现的键（例：`[roles.audit] runtime/model/skills`），`config.reset_role` 删掉整个角色表。
7. **`validate` 对整份配置生效。** 改一个角色时，别的角色已经存在的违规同样会让这次保存被拒，停用的角色也照样检查技能。
   另外，同模型检查只在两个角色都启用时才做（`config.rs:659`）。

### 面板与内核现状中与方案直接相关的事实

- **实时**：面板服务端 `eventbus.ts` 把账本里的全部事件转发给浏览器，其中包括 `config.changed`（`set_role` / `reset_role` /
  `set_loop` 都会发，主体是项目 ID 或 `global`），但没有一个路由的 `byKind` 含 `config.` 前缀。根 loader 只在
  `task.` / `project.` / `bug.` / `merge.` 事件、`reload` 与 `env.changed` 时重跑。
- **事件**：
  - 批准与驳回都写成 `task.updated {state, trigger: approve|reject}`；进入等待没有专门事件，只是 `task.updated` 的状态变成
    `await_*` 节点或 `failed`。
  - Backlog 表态是 `task.decided`；审计收回里程碑是 `milestone.contradicted`；每轮有 `task.snapshot {milestones, …}`。
  - `gate.result` 的载荷总有 `node / ok / exit_code / kind`，有报告时带 `steps`（计数对象）与 `passes`（数组）。
  - `session.ended` 带 `duration_ms` 与 `cost_usd`。
- **指标**：`TaskMetrics` 只在任务终结时计算。其中门禁五项是 `Option`：`round_gate_runs` 统计的是 `gate` / `fix_gate` 节点的
  `gate.result`，`merge_gate_runs` 统计 `merge_gate` 节点，`gate.reused` 不计入。
- **「门禁未跑过」的根因已确认**：`needsTaskDetail` 对已完成列返回 false（`board-payload.test.ts` 第 4 条钉住只读 30 次
  `task.get`），但 `task-card.tsx:19` 除待开始列外都渲染门禁行，于是 `gatePhrase(undefined)` 返回「门禁未跑过」。
- **本地操作**：`open.path` 在写白名单里（`kind` 可以是 `skill`），但面板服务端没有处理器，调用时返回 501「网页端还不支持」。
- **验-5 的现有证据不够**：没有任何内核测试证明「`config.set_role` 之后下一个会话带新的模型、Effort 与技能」。
  - `launcher.rs` 的单元测试证明了 argv 的拼法：`--model` 只在非空时出现；Effort 对 Claude 是 `--effort`，对 Codex 是
    `--config model_reasoning_effort=`；技能写成提示词里的「本轮必须使用以下技能：…」。
  - 会话行记下了 `runtime / model / effort / skills`（`session_json`）。
  - 端到端测试的假 CLI 会把提示词存到 `.autome/fake/<n>.prompt`，但不记录 argv。
- **守卫**：
  - `styles.test.ts` 的令牌守卫拿 `:root` 与测试里写死的 54 项逐项比对。颜色守卫豁免 `tokens.css`，只查 `app.css` 和 TSX，
    不查 `.ts`。
  - `labels.test.ts` 用**子串**禁止 `app/routes`、`app/components` 下的 `.tsx` 出现状态、列名、阶段名，例如「设计」「实现」
    「合并」「完成」「审计」，嵌在长文案里同样会被拦。
  - `testids.test.ts` 钉住带 `data-scroll-region` 的 10 个类名。
- **图标字符**（Node 的 `\p{Extended_Pictographic}` 实测）：原型用到的 `＋ − × ✓ ▾ → ⌘ · …` 都不是 EP 字符，可以照用；
  `✔`（U+2714）、`⚙`（U+2699）是 EP 字符，不能用。
- **测试基线**：`npx vitest list` 共 1056 个用例；`npx playwright test --list` 共 29 条。现有 29 条浏览器用例的实测耗时见
  「风险与验证安排」。

## 目标与非目标

**目标。**

- 按原型 v3 重做六个页面：总览、项目看板、任务详情、环境、Loop 配置、技能，覆盖 A-1…G-3。
- 新增的组件全站共用，原型里的颜色、图案、丝带、数字块都落成令牌和类名，现有样式守卫不放松（约束 3、4）。
- 页面上的每个数字都来自账本、事件或配置；拿不到的数显示为缺失，不写 0（约束 1、10）。
- 补回 Loop 配置与技能绑定：写操作只用 `config.set_role` / `config.reset_role` / `config.set_loop`。页面上的判定与内核一致，
  并有对照测试证明（E-5、F-3）。
- 现有功能一项不丢（验-8），实时更新不回归（约束 14），体积上限不变（约束 8）。

**非目标。**

- 不换风格、不加字体与 UI 框架、不做深色主题，也不做 `config.set_theme` 的界面。
- 不做拖动调序（C-Q4，理由见方案 C）。
- 不显示账本版本（G-Q1）、不显示「评审提了几项、裁决采纳几项」（B-Q2）：用户已忽略 BL-02、BL-03。技能描述由 M-12 补齐。
- 不改 Loop 流程、调度、门禁、协议文本，不升账本版本。内核产品代码范围为「对任务卡字面的取舍」第 1 条及 M-12 的技能只读描述。

## 方案

### 总

**总-Q1 里程碑怎么分。** 共 13 个，主体按 A→G 排，用户纳入项追加 M-12、M-13，前面里程碑的验收不依赖后面才有的东西：

- A 拆两个：M-01 是令牌、图标与共用组件；M-02 是外壳与两个新路由的占位页。
- B、C 各一个：M-03、M-04。
- D 拆两个：M-05 服务端汇总模型，M-06 页面重排。
- E 拆两个：M-07 角色卡片与编辑；M-08 Loop 默认值、项目信息页签与 `set_loop` 修复。
- F 一个：M-09。
- G 拆两个：M-10 环境页与两个表单页；M-11 README 截图与一屏巡检收口。

G-1 与 G-2 合并成 M-10，因为两者都只是套用 A 组组件。G-3 单列，因为它要监听端口，交接方式不同（见方案 G-Q3）。实现预算由 Autome 指定为 65 轮。

**总-Q2 原型写法怎么落进样式守卫。**

- **令牌。** 新令牌追加在 `tokens.css` 现有的 `:root` 里，统一用 `--ui-` 前缀。来源是原型里不属于现有令牌的值：
  - 卡片底色、边框色、文字色，七种：`--ui-card-{yellow,teal,purple,blue,green,red}-{bg,border,text}`；
  - 圆点图案的两层颜色：`--ui-pat-{color}-{1,2}`；
  - 实心胶囊底色：`--ui-pill-{color}`；
  - 丝带正面与背衬：`--ui-ribbon-{color}-{front,back}`；
  - 执行过程门禁行与用户行的底色：`--ui-row-gate`、`--ui-row-you`；
  - 白色面板 `--ui-surface`。
  - 原型的 `--blue`、`--bg2` 等直接改用对应的 `--tile-*` / `--animal-*`，不另起名字。
- **令牌守卫的改法。** 守卫的写死清单逐项追加新令牌的名与值，仍用 `toEqual`。另加两条守卫，让清单不会变成「什么都能加」：
  - 「`app.css` 里每个 `var(--x)` 都在 `tokens.css` 有定义」；
  - 「`tokens.css` 里每个 `--ui-*` 都至少被 `app.css` 引用一次」。
- **颜色守卫不变。** 原型散写的 `#fff`、`rgba(...)` 一律进令牌。
- **随数据变化的宽度与颜色**（不用内联 style）：
  - 进度条是内联 SVG：`<svg viewBox="0 0 100 10"><rect width={pct}>`，宽度是 SVG 属性而不是 `style`；外层带
    `role="progressbar"` 与 `aria-valuenow`。百分比在组件里夹到 0–100。
  - 项目色块按项目 ID 的稳定哈希，从九种 `--tile-*` 里选，写成类名 `.sq-{color}`。
  - 数字块颜色是类名 `.tile-{color}`。
  - 卡片、胶囊、丝带的颜色变体都是固定类名，由数据映射成类名，不拼样式。
- **被钉住的选择器**：
  - 保留原名与写法：`.shell`、`.shell-main`、`.project-list`、`.overview-column-body`、`.board`、`.board-columns`（仍是四列
    `repeat(4, minmax(0, 1fr))`）、`.board-column`、`.board-column-body`、`.task-new*`、`.task-page`、`.task-detail-*`、
    `.task-timeline`、`.task-docs-body`、`.task-changes`、`.task-decisions`、`.task-controls`、`.add-project`、
    `.path-suggestions`、`.probe-members`、`.env-page`、`.env-components`、`.board-alerts`、`.palette-list`、`.bell-panel`。
  - `.project-settings` 与 `.settings-card-body` 随项目设置搬进 Loop 配置页的「项目信息」页签，类名不变。
  - 新的内部滚动区追加进 `testids.test.ts` 的类名清单：`loop-roles`、`loop-drawer`、`skills-grid`、`skills-drawer`、
    `done-list`、`overview-merged-list`。
  - 每改一条守卫，证据里写明它对应原来哪一条、守住的性质是否变弱。

**总-Q3 图标。** 新建 `app/components/icons.tsx`，导出 `<Icon name="home|plus|gear|bell|search|hand|check|route|bag|leaf" />`。
每个图标是内联的 `<svg viewBox="0 0 24 24" aria-hidden="true">` 加 `<path>`，路径从原型 280–289 行搬过来（是原型自己画的线条，
不是第三方图标库）。`bell.tsx` 改用它。

- 不用 `<symbol>` 加 `<use>`：SSR 时每页都要注入一次 sprite，收益不大。
- 守卫：TSX 里出现 `<svg` 的文件只能是 `icons.tsx` 和进度条组件。
- 字符图标 `＋ − × ✓ ▾ →` 照用（实测都不是 EP 字符）。

**总-Q4 浏览器用例的时间账。**

- **用例数**：29 条，增加 4 条，共 33 条。改写的用例保持原标题，用例清单不变；新增的四条是：
  - `layout.spec.ts`：「1512×944 Loop 配置」「1512×944 技能」；
  - `controls.spec.ts`：「E-1 页面改全局角色与项目覆盖后 config.get 读回并能恢复」「F-1 技能按仅当前项目绑定后只进该项目」。
- **四种状态（验-2）不进浏览器。** 用 kernel 组的 vitest（`overview-states-real.test.ts`）对真实内核逐个状态渲染总览，
  不新增浏览器夹具。浏览器的全局夹具（两个根目录、`w7-flow.sh`、30 张草稿与 30 张已取消）不变；新增四条只在现有主根目录上
  写配置，不新建根目录。
- **时间估算**：新增四条按每条 ≤ 15 秒估，合计约 60 秒。与实测基线相加后的总量见「风险与验证安排 · 时间账」。
- **超出时怎么办**：放不下 720 秒上限时，先把新增两条 `controls` 用例的断言下沉到 kernel 组 vitest，只留页面交互部分。
  不提高任何预算。

**总-Q5 路由。** 新增两条路由，范围与项目切换放在查询参数里，因为它们是同一页面上的切换，不是不同资源：

- `/loop` 是全局默认，`/loop?project=<id>` 是某个项目，`/loop?project=<id>&tab=project` 是该项目的「项目信息」页签（E-7）。
- `/skills` 是全局，`/skills?project=<id>` 是某个项目；筛选与搜索放在 `?filter=all|global|project|bound&q=`。
- `/projects/:id/settings` 保留为跳转，302 到 `/loop?project=<id>&tab=project`。添加项目成功后的跳转目标直接改成新地址。
- 命令面板（`palette-model.ts`）增加「Loop 配置」「技能」「环境」三项，以及每个项目的「<项目> 的 Loop 配置」。铃铛的目标不变，
  仍是任务或项目页。
- 两个新页面 M-02 先建占位页（丝带标题与一张空状态卡），M-07 / M-09 再填实。

**总-Q6 新页面的实时更新。**

- 不需要改转发：服务端已经转发 `config.changed`。
- `lib/live-routes.ts` 给 `/loop` 登记 `config.` 与 `project.`；给 `/skills` 登记 `config.` 与 `project.`。
- 用户在自己页面上保存后照常走 action 的重新验证。
- 编辑抽屉里有未保存的改动时，收到 `config.changed` 只更新卡片上的「当前值」，不覆盖草稿，并提示「配置已在别处修改」。

### A 组件与外壳

**组件清单（A-1）。** 新建 `app/components/ui/`，每个组件一个文件，样式写在 `app.css`，类名与原型对应：

| 组件 | 类名 | 说明 |
|---|---|---|
| `Card` | `.card`、`.card-pat`、`.card-{yellow,teal,purple,blue,green,red}`、`.card-dash` | 图案是两层 `radial-gradient` 圆点，颜色取 `--ui-pat-*` |
| `Ribbon` | `.ribbon`、`.ribbon-sm`、`.ribbon-{yellow,blue,green,brown}` | 背衬用 `::before` |
| `Pill` | `.pill`、`.pill-fill`、`.pill-{teal,blue,purple,orange,pink,brown,green,yellow,red}` | |
| `Tile` | `.tile`、`.tile-{teal,yellow,blue,pink,purple,orange,green}` | 数值缺失时显示「—」，并加 `data-missing` |
| `Progress` | `.prog` | 内联 SVG，见总-Q2 |
| `Button` | `.btn`、`.btn-primary`、`.btn-sm`、`.btn-text` | 立体阴影只在 `.btn-primary` |
| `Segmented` | `.seg` | 内部是同名 radio 或链接，键盘左右切换 |
| `Switch` | `.switch` | 原生 `checkbox`，加 `role="switch"` |
| `Stepper` | `.stepper` | 原生 `number` 输入加两个按钮，到 `min` / `max` 时禁用 |
| `Input` | `.inp` | |
| `Check` | `.chk` | 原生 `checkbox`；禁用时用 `aria-describedby` 指向原因 |

**页面文案。** 所有含「设计 / 实现 / 审计 / 合并 / 完成」等被守卫子串的文案都放进 `app/lib/*-labels.ts`（新增
`loop-labels.ts`、`skill-labels.ts`、`overview-labels.ts`），TSX 只引用。这是 T-98 三次踩过的坑，在每个里程碑的验收命令里
都跑 `labels.test.ts`。

**外壳（A-2）。**

- 侧栏依次是：品牌、总览（带等你处理数）、Loop 配置、技能（带全局技能总数）、「项目」小标题、项目列表、添加项目、驱动状态卡、
  环境。
- 项目行显示：色块、名称；有任务时显示「等你处理 / 进行中」两个计数点，没有任务时显示「N 个已完成」或「没有任务在跑」。
- 顶栏依次是：面包屑（各路由的 `handle.crumb`）、搜索（打开现有命令面板，显示 `⌘K`）、铃铛、新建任务（主按钮）。
- 内核离线横幅、报警区留在顶栏下方原位置。

**A-Q1 侧栏数据。** `loadShell` 在现有 `project.list`、逐项目 `project.get`、`dashboard.get` 之外再调两个只读方法：

- `driver.list`：经 `server/drivers.ts`；
- `skills.list`：不带 `project_id`，只扫两个全局目录、只扫一层，取数量。

两次调用都在每次根 loader 重跑时发生（即任务、项目类事件到达时）。`skills.list` 失败时不显示数字，不让外壳失败。各项目的
「N 个已完成」取自已经在调的 `project.get` 的任务列表，不另加调用。

**A-Q2 驱动状态卡的口径。**

- 有 `driving: true` 的驱动：「驱动在推进」，副文案为「{last_tick_at 的相对时间}检查过 · 已运行 {started_at 到现在}」。
  有多个时取 `last_tick_at` 最新的那个。
- 有驱动登记但都不在推进：「驱动空闲」。
- 没有登记：「没有驱动在运行」，带一个指向环境页的链接。
- 旧内核（`unknown_method`）：「驱动状态未知」。如果 `other_driver` 推断为真，显示「另一个驱动可能在推进」。
- 面板自己的内核（`self`）只在它真在推进时才被选中。
- **刷新**：根 loader 不会每个 tick 都重跑，所以面板服务端在每次 tick 后读一次 `driver.list`。摘要（是否在推进、选中的 pid、
  `last_tick_at`）变了，就经 WebSocket 发一条扩展消息 `drivers`（与现有的 `env.changed` 同一机制），驱动卡订阅它；没变就不发。

**A-Q3 字体。** 确认不动：原型引用的就是已打包的 `Nunito-Variable.ttf`，中文走系统字体，与现状一致。

**A-3 一屏。** 所有新的内部滚动区都带 `data-scroll-region`，`oneScreen` 照旧判定。新增的可交互元素都可以用 Tab 到达，焦点环
沿用 `--animal-focus-yellow`。

### B 总览

**B-1 结论句。** 由 `overview-model.ts` 的纯函数生成，输入是三个计数（等你处理、进行中、排队）：

- 三个都是 0：「现在没有事情等你」，副句是「没有任务在跑，驱动空闲」或「没有任务在跑，驱动在推进」。
- 否则：「有 N 件事等你决定」（N 为 0 时写「没有事情等你决定」），后接「另有 M 个任务在跑，K 个在排队」。为 0 的项省略。

**共用的等待口径（B、D 都用）。** 新建纯函数模块 `app/lib/waiting.ts`，服务端与组件都从这里取，不各写一份：

- **等待状态**：`task.updated` 的新状态是 `await_design_approval`、`await_merge`、`await_info`、`not_a_bug`（与内核
  `Node::awaits_user` 及 `task-columns.ts` 的 `AWAITING_NODES` 一致，按状态枚举判断，不用 `await_` 前缀），或任务状态
  `failed`。`paused` / `stopped` 是用户自己的操作，**不计入**「等你的时间」（卡片仍显示在「等你处理」列，那是另一回事）。
- **区间**：从进入等待状态那条 `task.updated` 的 `at`，到同一任务下一条改变该状态的 `task.updated` 的 `at`（批准、驳回、
  补充信息 `reply`、合并、重跑、取消……任何离开都算）；还没离开的是「开放区间」。
- **退出等待的用户操作**：这条 `task.updated` 的 `trigger` 是用户操作时，在时间线上就是一条用户行，写「等了 X」。
- **门禁口径**：只数 `gate.result` 事件，且 `node` 是 `gate` / `fix_gate`（快检）或 `merge_gate`（合并前）。`repro_check`、
  `main_check` 等其它节点不计；`gate.reused` / `gate.skipped` 不计。这是 `task_metrics.rs` 的 `gate_counts` 的口径。
  总览与任务详情都叫「门禁」，数字按这一个口径出。

**B-Q1 「今天」四个数字与「最近合并」。** 新建 `server/overview-summary.ts`，在面板服务端计算，只在总览 loader 里调用：

- **「今天」的口径**：按面板服务端的时区切日，与 `clock.tsx` 下发给浏览器的 `timeZone` 一致。
- **事件来源**：服务端用不带 `subject` 的 `events.since`（面板服务端的 `eventbus.ts` 已经这样读，浏览器不经过它，不放开浏览器的
  `events.since`）按账本顺序增量消费，不按「任务终结与否」筛候选任务：任务今天先花钱、等你、然后被取消（`cancelled` 不写
  `completed_at`）或旧的终结任务今天又跑了一次复盘，它们今天发生的事件都照常计入。
  - 累加器只保留需要的小记录：每个任务当前的等待状态与起点，已闭合的等待区间（`{start, end, 种类}`），会话结束记录
    （`at`、`cost_usd`），门禁结果记录（`at`、`ok`、`node`）。闭合区间与记录只保留昨天 00:00 之后的，其余丢弃。
  - 冷启动读一遍账本；之后每次只读 `after_seq` 之后的增量。账本回退（`latest_seq` 变小）时重建。
- **合并**：`state = done` 且 `completed_at` 是今天的任务数（只有进入 Done 才写 `completed_at`），来自根 loader 已有的
  `project.get` 结果。
- **花费**：今天的 `session.ended` 事件的 `cost_usd` 之和；`cost_usd` 为空的会话单独计数，副文案写「N 个会话无计费」，全部无计费
  时主数字显示「无计费」。
- **门禁**：今天的门禁（上面的共用口径）中 `ok = true` 的个数 / 总个数。
- **等你的时间**：今天每个任务的等待区间（共用口径）与今天取交集后求和；开放区间算到现在。副文案写「批准设计 N 次」等，按今天
  结束的等待种类计数。
- **缓存与随时间增长的数字**：
  - 事件的消费进度与闭合区间按账本序号缓存，序号不变就不重读事件。
  - 汇总**不把最终的等待时长算死**：loader 只返回「已闭合部分的毫秒数」、每个开放区间的起点，以及今天的起止时刻
    （`dayStart` / `dayEnd`）。组件用 `lib/clock.tsx` 的 `now` 现算「已闭合 + Σ(now − max(起点, dayStart))」，每 30 秒随时钟
    刷新，账本没有新事件时开放区间照样增长。这个现算是 `overview-model.ts` 里的纯函数。
  - `now` 越过 `dayEnd` 时，客户端触发一次根 loader 的重新验证（即使没有新事件），今天的切片随之换新。
  - 花费、合并数、门禁数不依赖 `now`，只在日期切换或有新事件时变化。
- **最近合并**：所有项目中 `state = done` 的任务按 `completed_at` 倒序取前 8 个，显示编号、标题、结果胶囊、合并时间（`clock`
  格式）与花费，每行链接到任务详情。只有一个项目时，列表底部有「全部 N 个 →」指向该项目看板的 `?done=all`；多个项目时不
  显示这个链接（B-4 不要求跨项目的「全部」页）。
- **结果胶囊**按指标依次判断，取第一条成立的：
  1. `merge_gate_runs > 1`：「门禁 N 次」，橙色；
  2. `reopen_total ≥ 5`：「退回 N 次」，橙色；
  3. `reopen_total` 在 1–4：「退回 N 次」，描边；
  4. `merge_gate_runs = 1`：「门禁一次过」，绿色；
  5. `reopen_total = 0`（门禁字段缺失）：「无退回」，绿色；
  6. 没有 `metrics`：「—」，描边。

**B-2 「等你处理」卡片。** 数据来自 `dashboard.get` 的 `waiting` 加逐项 `task.get`（等你处理的任务一般只有个位数）。

- **种类胶囊**沿用 `labels.ts`：`await_design_approval`「批准设计」、`await_merge`「批准合并」、`await_info`「补充信息」、
  `not_a_bug`「确认不是 Bug」、`failed`「处理失败」、`paused` / `stopped`「已暂停 / 已停止」。
- **摘要句（B-Q2）**：
  - 批准设计：「设计已定稿：{milestones_total} 个里程碑。评审 {n} 轮、裁决 {m} 轮。」——n、m 是 `review`、`adjudicate` 会话数。
    后接「另有 {k} 条 Backlog 等你表态」，k 由现有「待你决定」页签的同一个模型从设计文档的 `## Backlog` 与 `## 争议项` 读出
    未表态条数；k 为 0 时省略。
  - 失败：用 `failureText`。
  - 其它种类：用 `AWAIT_PHRASES`。
  - **不写**「提了 2 项、全部采纳」，内核没有这个数（BL-03）。
- **事实条**：设计轮 d/N、已花费（`cost_usd`；缺失时写「无计费」）、已用时（`created_at` 到现在）、设计文档大小（`task.get`
  的文档清单里有字节数时才显示）。
- **「已等 X」（B-Q3）**：从最后一次进入当前等待状态的 `task.updated` 的 `at` 算到现在（共用等待口径，`not_a_bug` 与 `failed` 同样适用）。
- **主按钮直达**：批准设计 → `/tasks/:id?doc=design`；失败 → `/tasks/:id`；合并 → `/tasks/:id`，并开「改动与合并」页签。

**B-3 「进行中」与排队卡片。**

- **进行中卡片**：
  - 角色与轮次胶囊，如「实现 2 / 15」，取 `session.kind` 与 `status_block.impl_round / impl_round_limit`；设计阶段用 `design_round`；
  - 里程碑进度条 `milestones_done / milestones_total`，以及「正在做 {current_milestone}」；
  - 本轮已跑多久：`session.started_at` 到现在；
  - 已用时、已花费；
  - 有当前门禁时附一句门禁短语（`gatePhrase` 返回空时不写）。
- **排队卡片**写在等谁：有 `blocked_by` 时写「等 {id} {原因}」；否则写「排第 {queue_position} 位 · 等空闲槽位」。

**B-Q3 相对时间与水合。** 全部用 `lib/clock.tsx`：首屏用 loader 下发的服务端 `now` 渲染，挂载后切到客户端时钟、每 30 秒刷新。
SSR 和首次水合用的是同一个 `now`，不会有不一致。新增 `formatRelative(at, now)`（「3 秒前」「34 分钟前」）与
`formatSpan(ms)`（「6 分 03 秒」「2 时 29 分」），带 `<time dateTime title>`，悬停看绝对时间（G-1 也用它）。

**B-4 空闲。** 左侧两张虚线空状态卡：

- 「都处理完了」，说明哪些事会出现在这里；
- 「没有任务在跑」，带新建任务主按钮。没有项目时这个按钮指向添加项目。

右侧「今天」与「最近合并」照常显示；没有已合并任务时列表写「还没有合并过的任务」。

**B-Q4 四种状态的夹具。** 新建 `test/fixtures/overview-states.sh <mode>`，复用 `w5-states.sh` / `w7-flow.sh` 的假 CLI 与临时根，
每次只造一种状态：

- `idle`：只注册一个项目，没有任务；
- `running`：一个任务，假 CLI 睡眠；
- `awaiting`：一个需求任务走到 `await_design_approval`；
- `failed`：假 CLI 以 3 退出。

kernel 组的 `overview-states-real.test.ts` 对每种状态跑一次真实内核与 `overview-summary`，断言结论句与卡片。

### C 项目看板

**C-1 卡片。** 列头用小号丝带：待开始（棕）、进行中（默认绿松）、等你处理（黄）、已完成（绿）。

- 进行中、等你处理两列的卡片内容是 B-3、B-2 的精简版，共用 `overview-model.ts` 的视图函数，信息与总览一致。
- 待开始卡片显示种类、编号、等待原因、「排第 N 位」、创建时间。

**C-Q4 调序。** 不做拖动：拖动没有键盘等价操作，而现有 `queue-controls.tsx` 的按钮（`task.move`）本来就能用键盘操作。按钮按
A 组组件重排，列头副文案改成「用卡片上的按钮调整顺序」，文案放进 `labels.ts`。

**C-Q1 已完成列表的口径（C-2）。**

- 列表行显示编号、标题、实现轮、退回、花费：
  - 实现轮取 `metrics.impl_rounds_used`；Bug 任务与没有 `metrics` 的任务显示「—」；
  - 退回取 `metrics.reopen_total`，没有 `metrics` 时显示「—」；
  - 花费取 `cost_usd`，为空时显示「无计费」。
- 按 `completed_at` 在服务端时区分组：
  - 今天；
  - 昨天；
  - 本周更早：本周一 00:00 起、昨天之前；
  - 更早。
- 列头旁写「累计 ${Σ cost_usd} · 平均实现 {x} 轮」：
  - 累计花费的分母是全部已完成任务（需求与 Bug），不含已取消；
  - 平均实现轮只算有 `metrics` 的需求任务；分母为 0 时写「—」；
  - 两个口径在悬停提示里写明。
- 行是链接，点开进任务详情。

**C-Q2 折叠与体积。**

- 默认展开「今天 / 昨天 / 本周更早」三组；「更早」只显示一行「更早 · N 个」，链接到 `?done=all`，在那里展开全部。
- 「已取消（N）」折叠区放在列表底部，`?cancelled=all` 行为不变。
- 紧凑条目只带 6 个字段，`board-payload.test.ts` 的 200 KB 与「只读 30 次 `task.get`」两条不变。
- 原来「最近 14 天、最多 20 张」的默认截断，改成按分组截断。

**C-3 「门禁未跑过」。** 已完成任务改成紧凑条目，不再有门禁格。

- `gatePhrase` 在 `gate` 为空、又不在排队时返回 `null`，卡片不渲染门禁行。这样在途任务确实没跑过门禁时也不再显示这句话。
  信息不丢：任务详情的门禁数字块写「0 / 0」，它是从事件数出来的真实计数。
- 测试断言：任何列、任何卡片都不出现「门禁未跑过」。

### D 任务详情

**D-Q6 结构。** 新建纯函数模块 `app/components/task-summary-model.ts`，由服务端 loader 调用。它的输入是 `task.get` 的全部会话
（loader 已经拿到）加 `task-history.ts` 读出的本任务事件，输出一个小的 `TaskSummary`：步骤条、六个数字、阶段分组的时间范围。
浏览器只收 `TaskSummary` 与现有的会话窗口（默认最近 20 个，`?from=` 钉住，`?timeline=all` 全量）。60 个会话时 `TaskSummary`
只有几百字节，150 KB 上限不受影响。

**D-1 头部与步骤条（D-Q1）。**

- **头部**：状态胶囊（已合并时写合并时间）、类型与编号、分支、合并提交短哈希、操作按钮（现有 `task-ops` 的全部写操作）、标题。
- **需求步骤条**：整理 → 设计 → 评审·裁决 → 你批准 → 实现 × n → 审计 × n → 复盘 → 合并前门禁 → 合并。
  - 会话类步骤的用时是该类会话 `ended_at − started_at` 之和（在跑的算到现在），花费是 `metrics.cost_usd` 之和；有无计费的会话
    时写「无计费」或「$x + N 个无计费」，全无计费时写所用工具名。
  - 「你批准」的用时是全部 `await_design_approval` 区间之和，写「等了 X」。
  - 合并前门禁取 `merge_gate` 节点 `gate.started → gate.result` 的时长之和，并写「一次过 / N 次」。
  - 合并写合并时间。
  - 进行到哪一步由现有 `task-stage-model.ts` 判定。
- **Bug 步骤条**：沿用 `BUG_STAGES`（定位 → 复现检查 → 修复 → 修复门禁 → 复核），之后接「合并」，口径同上。
- 里程碑胶囊一行，来自 `status_block.milestones`：状态胶囊加标题。

**D-2 六个数字块（D-Q4）。**

- **花费**：`cost.usd_total`，副文案「N 个会话无计费」。
- **总用时**：`created_at` 到 `completed_at`（未终结时到现在），副文案「其中等你 X」，X 是共用等待口径下全部区间之和（含 `not_a_bug`；开放区间由组件用 `now` 现算，与总览同一个纯函数，总用时在途时同样如此）。
- **会话数**：副文案按 `runtime` 分组，如「Claude 7 · Codex 4」。
- **设计轮**：`design_round / design_round_limit`，副文案「评审 n 轮」。
- **实现轮**：`impl_round / impl_round_limit`，副文案「退回 N 次」。N 在终结任务上取 `metrics.reopen_total`，在途任务取状态块里程碑
  表的 `reopen` 列之和。两者同源（都是 reopen 计数）。
- **门禁**：通过数 / 总数，副文案「快检 a · 合并前 b」。口径就是上面的「共用的等待口径」里的门禁口径，与总览「今天」的门禁、
  `TaskMetrics` 一致，在途任务与终结任务口径一致。事件被截断（`truncated`）时显示缺失。

**D-3 执行过程。**

- **行的种类**：
  - 会话行：角色胶囊、「{角色} #{round} · {里程碑}」、工具与模型、一句话结果、用时、花费；
  - 门禁行（绿色底）：「{每轮快检|合并前门禁} · 通过/未通过」、「执行 a 步 · 沿用 b 步」、用时；
  - 用户行（黄色底）：批准、驳回、表态、按合并，写「等了 X」；
  - 合并行。
- **分组与顺序**：按阶段分组（整理 / 设计 / 实现与审计 / 复盘 / 合并），组内与组间都是倒序，组头写时间范围。
- **一句话结果（D-Q2）**：沿用现有 `sessionSummary`，即从 `task.snapshot` 的里程碑变化、`bug.located` 等事件派生；取不到就留空。
  不解析 `retro.md`，那里的一行是按轮写的人工摘要，与会话不能一一对应，而且会诱导编造。
- **门禁行内容（D-Q3）**：
  - 用时 = 同一 `node` 与 `log` 的 `gate.started → gate.result`。
  - 步数取 `gate.result.steps` 的计数（「执行」= 实际跑的步数，「沿用」= 复用的步数；实现轮按 `crates/automed/src/gate.rs` 的
    `read_report` 核对字段名后写进 `labels`）。旧事件没有 `steps` 时只写通过与否。
  - `gate.reused` 显示为「沿用上次通过」，`gate.skipped` 显示为「跳过 · {reason}」，两者都不计入门禁数。
- **用户行**：沿用现有 `task-timeline-model.ts` 里 `operations` 的全部用户操作（`approve`、`reject`、`reply`、`merge`、`pause`、
  `resume`、`stop`、`cancel`、`rerun_from`、`extend_budget`），加 `task.decided`；新的分组与行种类不得丢掉其中任何一种。
  退出等待状态的那条（含 `not_a_bug` 后的 `reply`）写「等了 X」，X 是共用等待口径下该区间的时长；其余用户行不带时长。
- **筛选（D-Q5）**：
  - 「只看门禁」= 门禁行与 `gate.reused` / `gate.skipped` 行；
  - 「只看退回」= 审计把里程碑从「待审」改回「开放」的会话行（来自 `task.snapshot` 的里程碑变化）、`milestone.contradicted`、
    `ok = false` 的门禁行、用户驳回行。
  - 筛选状态放进 URL 的 `?show=gates|reopens`，经现有 `withParams` 与 `from`、`doc` 共存。筛选只作用于当前窗口内的行。
- 点开会话行仍是现有的会话日志，展开状态与滚动位置的保持机制不变（H-03、C-2）。

**D-4 右侧四个页签。** 文档、改动与合并、待你决定、备注与依赖的全部功能保留，只换组件：

- 页签用 `Segmented` 的页签样式；
- 文档导航用胶囊；
- 改动页签显示「+a −d」；
- 待你决定页签的计数用胶囊。

`task-action.ts` 的 `WRITE_OPS` 不变。

### E Loop 配置

**E-1 页面。**

- 侧栏「Loop 配置」进入 `/loop`；项目看板头部的「项目设置」进入 `/loop?project=<id>`。
- 页头是丝带加范围切换（`Segmented`，「全局默认」与各项目），再加一句范围说明。
- 「保存 N 项改动」主按钮只在有草稿时出现。

**E-2 角色卡片。**

- **设计循环一行**：任务整理（固定，虚线）→ 设计 → 评审 → 裁决 → 你批准设计（人工停顿，黄色虚线）。
- **实现循环一行**：实现 → 宿主快检（固定）→ 审计 → 复盘 → 合并前门禁（固定）。
- **卡片颜色**：设计紫、评审蓝、裁决黄、实现青、审计红、复盘绿。
- **卡片内容**：工具、模型（空串写「CLI 默认」）、Effort 胶囊（`null` 写「Effort 默认」）、绑定技能数、启用开关。
- **行标题旁**：「最多 {design_rounds} 轮」「预算 = {budget_factor} × 里程碑数」，数字取 `resolved.loop_defaults`。

**E-3 编辑抽屉。**

- 字段：启用（`Switch`）、工具（`Segmented`：Claude Code / Codex）、模型（`Input`，自由文本）、Effort（`Segmented`：
  默认 / low / medium / high）、绑定的技能（已绑定的技能列成可删除的胶囊，加一个「添加技能」链接到 `/skills?project=…`）。
- 当前值不在四个选项里时（内核允许任意字符串），多显示一个只读选项「其它：{值}」，保存时原样带回，不丢。
- 保存走 `config.set_role`，带上全部五个字段（与旧 Electron 页面相同），全局范围不带 `project_id`。

**E-Q2 项目范围的字段语义。**

- 项目范围下，保存只带用户在抽屉里**改过的字段**，避免把全局值抄成项目覆盖（项目文件头部的注释也这样要求）。
- 角色卡片按 `provenance` 标「跟随全局」或「项目覆盖」。
- 「恢复跟随全局」调 `config.reset_role`，确认文案写明「这个角色的工具、模型、Effort、技能、启用的项目覆盖会一起清掉」，因为
  内核的粒度是整个角色。

**E-Q1 双模型纪律的判定（E-5）。**

- 新建 `app/lib/role-rules.ts`，逐条照抄 `config::validate` 中与角色相关的规则：
  - 三对：评审→设计、审计→实现、复盘→实现；任一方停用时这一对免检；
  - 同工具且模型都为空：「都用 CLI 默认」（`both_default_models`）；
  - 同工具且模型相同：`same_model`；
  - 技能对角色工具不可见：`skill_not_visible`；技能在当前范围不存在：`skill_not_found`。
- 页面对「已保存配置 + 当前草稿」合成的那份配置跑这个函数。违规时冲突的两张卡片当场标红，抽屉与页头的保存按钮禁用，并显示
  与内核同义的说明。
- **保证与内核一致**：kernel 组测试 `loop-real.test.ts` 准备一张约 14 行的配置表（三对 × 同 / 异模型 × 启停，加 both_default、
  不可见技能、不存在技能、项目覆盖 runtime），逐行既跑 `role-rules.ts`，又对真实内核调 `config.set_role`，断言「页面判拒」与
  「内核拒绝」逐行相同。规则不相符的那一行会让测试失败。
- **原样显示内核拒绝**：内核拒绝时，action 把内核的 `message` 原样显示在抽屉顶部（不翻译、不改写）。

**E-Q4 保存的粒度。** 抽屉里的「保存」只写当前角色。页头的「保存 N 项改动」按 Loop 顺序逐个调 `config.set_role`：

- 发出前先用 `role-rules.ts` 模拟各种顺序（脏角色最多 6 个，最多 720 种排列），选一个每一步中间态都合法的顺序。
- 找不到这样的顺序时（例如设计与评审互换模型），禁用页头保存，说明「这几项必须分开保存：先把 X 改成别的模型」。
- 执行中遇到内核拒绝就停下，已保存的保留，未保存的留在草稿里，并显示内核原文。

**E-Q5 启用开关。** 卡片上的开关只改草稿，与其它字段一起保存，不直接写入。关掉评审或审计时，卡片显示「已停用：Autome 不再为
这个角色启动会话，双模型检查对这一对免检」。实现轮须按调度代码核对第一句是否属实；不属实时改为与代码一致的说法，写进证据。

**E-6 Loop 默认值（E-Q3）。**

- 四个 `Stepper` 的范围与内核一致：并行 1–5，设计轮上限 ≥ 1，预算系数 ≥ 1，会话时长 ≥ 5 分钟；越界在页面上禁用，内核的拒绝
  原样显示。
- 每项改动单独调一次 `config.set_loop`，与旧设置页相同。
- 项目范围下，每项标「与全局相同」或「项目值」：拿 `resolved.loop_defaults` 与 `global.loop_defaults` 逐项比较。
  - 用「与全局相同」，不写「跟随全局」：项目里显式写了与全局相同的值时，内核不回原始覆盖，页面分不出两者，所以不说假话。
  - 每项都有「恢复跟随全局」，发 `{key: null}`。
- 精确到逐项的来源需要内核加只读字段，进 Backlog（BL-04）。

**E-Q6 / E-7 项目信息页签。** 项目范围下的第二个页签「项目信息」，`/loop?project=<id>&tab=project`。

- 内容原样搬现有设置页的三块：Onboarding（状态、会话日志、两个档案编辑器、运行 / 完成 / 跳过）、只读配置
  （`config-view.ts` 的行、配置文件路径、`config.validate` 违规）、移除项目。
- 根元素保留 `data-testid="page-settings"` 与 `.project-settings`，`settings-*.test.ts` 改为渲染新路由。
- `/projects/:id/settings` 改成 302 跳转。
- `layout.spec.ts` 的「1512×944 项目设置」保持标题，访问旧地址并断言跳转后一屏。

**E-Q7 验-5 的「下一个会话用新配置」。** 在 `crates/automed/tests/end_to_end.rs` 新增测试
`a_role_written_through_set_role_reaches_the_next_session`：

1. 经 dispatch 调 `config.set_role` 给设计角色设置模型、Effort 和一个临时 `HOME` 下的技能；
2. 用现有假 CLI 启动一个需求任务；
3. 断言第一个设计会话的会话行记下了新的 `model / effort / skills`，提示词文件含「本轮必须使用以下技能：{名}」；
4. 再改一次，断言下一个会话换成新值。

argv 层由 `launcher.rs` 现有单元测试负责（见背景）。新增的只是测试，不改内核行为。

### F 技能

**F-1 页面。**

- 页头是丝带、筛选（「全部 N / 全局 N / 本项目 N / 已绑定 N」）、项目切换、搜索。
- 卡片网格在内部滚动区里。每张卡片显示：名称、来源胶囊（全局 / 项目）、工具胶囊（`visible_to`）、绑定角色胶囊（「审计 必用」）
  或「未绑定」，名称下一行显示来源路径（代替原型的描述行）。
- 筛选与搜索在浏览器端做：全量数据只有名称与路径，126 个技能约 20 KB。计数随所选项目重算。
- 没有项目时项目切换只有「全局」，「本项目」筛选隐藏。
- `conflicts` 非空的技能卡片标红，写「{角色} 用 {工具}，看不到它」。

**F-Q2 绑定范围与内核覆盖语义。** 抽屉里「绑定范围」两档：

- **全局 · 所有项目**：对勾选变化的每个角色调 `config.set_role {role, skills: global.roles[r].skills ± 技能}`，不带 `project_id`。
  只写成员关系有变化的角色，与旧 Electron 页面相同。
- **仅当前项目**：对勾选变化的每个角色调 `config.set_role {role, project_id, skills: 该项目 resolved 列表 ± 技能}`。这会给该角色
  写一份完整的项目技能列表，该角色随之变成「项目覆盖」。抽屉里写明：「此后这个角色在本项目里不再跟随全局技能列表的变化；
  要恢复，去 Loop 配置点『恢复跟随全局』」。
- **在项目范围取消一条全局绑定**：也是写一份不含它的项目列表，抽屉同样提示。
- **已有技能覆盖的项目遇到新的全局绑定**：角色级的 `provenance = project` 只说明这个角色在该项目里有**某个**覆盖（可能只改了
  模型或 Effort，不碰技能），不能据此断言技能列表被覆盖，也不能据此说「不生效」。内核回包里也分不出「项目自己写了技能列表」
  与「碰巧和全局相同」。所以：
  - 保存前，选「全局」时抽屉只写一句中性提示「某个项目如果给这个角色写了自己的技能列表，就不会跟随这条全局绑定；保存后会列出
    没有拿到的项目」，**不列项目名，不断言不生效**；
  - 保存后，服务端对每个项目重读 `config.get`，把这个角色的 resolved 技能里**没有**该技能的项目列出来：「以下项目的 {角色} 没有
    拿到这条绑定：A、B（它们在项目里有自己的技能列表）」。这是读回的事实，不是推断。取消全局绑定时同理，列出读回后仍含该技能的项目。
- **验-7 的写法**：用三个项目断言：
  - A：选「仅当前项目」绑定后，只有 A 的 `config.get` 出现；
  - B 没有任何覆盖、A 只覆盖了模型（不碰技能）：选「全局」绑定后，B 与 A 都出现，页面不把 A 列为「没拿到」；
  - 先给 C 写技能列表覆盖、再绑全局：C 不出现，保存后页面列出 C。
  这样既覆盖任务卡的两句话，也不说假话；「仅覆盖模型」与「显式技能列表覆盖」两种场景必须各有一个判别用例。

**F-Q3 可见性。** 判定的输入按绑定范围取，不混用：

- **绑定到「全局 · 所有项目」**：只看技能的**全局来源**（`sources` 中 `scope = global` 的那些）。技能存在与否、哪些工具能看到它，
  都以全局来源为准，这与 `set_role` 全局范围用 `skills::scan_global` 校验一致。
- **绑定到「仅当前项目」**：看当前项目 `skills.list` 回包里的全部来源（全局加项目）。
- 因此 `project_scoped` 只表示「**任一**来源在项目里」，不能当作「仅存在于项目」。三类要分开处理：
  - 纯全局技能：两档都可选；
  - 纯项目技能（没有 global 来源）：「全局 · 所有项目」一档禁用，说明「项目技能只能绑定在自己的项目里」；
  - 同名混合（既有 global 来源又有 project 来源）：「全局」一档**可用**，可见性只按其 global 来源算，不把项目来源的工具并进去；
    「仅当前项目」按全部来源算。
- 「哪些角色必须使用」的六个勾选块，按当前范围的 resolved 角色工具与上面选定的来源判断：工具不在对应来源的运行时里的角色禁用，
  并说明「{角色} 用 {工具}，看不到这个技能（{目录} 下没有它）」。
- 判定同样用 `role-rules.ts`，保存时内核的拒绝原样显示。

**F-Q1 描述。** 用户纳入 BL-01：M-12 由内核只读解析 `SKILL.md` 头部的 `description` 并通过 `skills.list` 返回，技能卡片显示描述行。缺失或读盘失败时不编造描述，保留来源路径；不写技能文件。

**F-Q4 打开目录（F-4）。** 新建 `server/open-path.ts`，经 `gateway.registerLocalOp('open.path', …)` 注册：

- 只实现 `kind: 'skill'`，参数是 `name`、可选 `project_id`、可选 `source`（序号）。
- 服务端调 `skills.list` 找到该技能，取 `sources[source ?? 0].path` 打开。名字不存在时回 404「没有这个技能」，其它 `kind`
  仍是 501。
- 打开器是注入的，在 `startServer` 的选项里，缺省是 `/usr/bin/open`；测试传入记录调用的替身。浏览器给不出路径（写闸门已经拒绝
  像路径的参数）。
- 多来源时，抽屉的「来源」区每行一个「打开目录」。

**F-Q5 数量与切换。** 见 F-1。卡片是 `<button>`，键盘可选。

### G 其余页面

**G-1 环境页（G-Q1）。**

- 页头写「N 项检查全部正常」或「N 项有问题」与「{相对时间}检测过」，加「重新检测」按钮。
- **工具卡**每项一行：状态圆点、名称、路径与登录状态、版本胶囊。安装 / 登录 / 复制命令按钮留在对应行的末尾（`env-ops` 现有行为
  与 testid 不变）。
- **内核**也作为一行：路径与构建标识取自 `/api/instance` 的 `kernel`；账本版本内核没有给出，不显示（BL-02）。
- **驱动卡**（`card-pat card-green`）每个驱动一行：命令、是否面板自己的内核、「pid · {相对时间}启动」，以及「{相对时间}检查过」
  胶囊。
- 所有时间都是 `<time title=绝对时间>` 加相对时间。

**G-2 两个表单页（G-Q2）。** 只换控件，表单字段名与校验不变：

- **新建任务**：类型单选换成 `Segmented`（内部仍是 `name="kind"` 的 radio）；附件的文件选择换成 `.btn` 样式的 `<label>`，`<input
  type=file>` 视觉隐藏但仍可聚焦；依赖勾选换成 `Check`；正文用 `Input` 的多行样式。
- **添加项目**：路径用 `Input`，补全列表样式更新；工作区、跳过 Onboarding 换成 `Check`；文档仓库下拉用 `.inp` 样式；原生目录选择
  按钮只在薄壳里出现，行为不变。

**G-Q3 README 截图。** 改成五张：`dashboard.png`、`project.png`、`task.png`、`loop.png`、`skills.png`，都是 1512×944。

- 同轮改 `readme-images.test.ts`（恰好五张）与 README 的 What it looks like 一节（英文说明）。
- 截图脚本的夹具在 `w7-flow.sh` 之上，再让一个任务停在 `await_design_approval`、一个任务在跑，使总览出现黄、青两种卡片；
  Loop 配置截全局范围。
- **交接**：截图要监听端口。实现轮先跑 `npm run screenshots --prefix apps/web`。被沙箱拒绝时，在证据里写明，并把
  `npm run screenshots --prefix apps/web -- --out .autome/output/screens-M11` 交给审计轮。审计轮能跑时，以「验证缺口」以外的
  「实现缺陷：截图未替换」退回，并在审计文件写明产物路径；下一实现轮把产物复制进 `docs/images/`，这一步不需要监听。这就是
  T-98 M-07 实际走通的路径，这里提前写明，省掉摸索的轮次。

### 对任务卡字面的取舍

1. **修 `config.set_loop` 的技能校验**（实测第 1 条）。任务卡写「内核只允许加只读数据……写操作只用现有的 `config.*` 方法」。
   - 这里不新增写方法、不改写语义：只把 `set_loop` 两处 `config::NoSkills` 换成与 `set_role` 相同的真实技能扫描（全局范围
     `skills::scan_global`，项目范围 `skills::scan`）。
   - 不修，E-6（改 Loop 默认值）与 F-2（绑定技能）就不能同时成立，任务卡的两条要求自相矛盾。
   - 改动放在 M-08，带一条先红后绿的单元测试。
   - 这是对「内核只加只读数据」字面的偏离，请评审与用户确认；若被否决，退路是 M-08 只交付「没有任何技能绑定时」可用的
     Loop 默认值，并另开 Bug 任务。
2. **不显示技能描述、账本版本、评审 / 裁决的项目数**：数据不存在，不编造（约束 1）。三项进 Backlog。
3. **调序不做拖动**：列头文案改成与实际一致（C-Q4）。
4. **「最近合并」的「全部 N 个 →」**：原型只有一个项目；多项目时不显示该链接，见 B-Q1。

## 风险与验证安排

**主要风险与对策。**

| 风险 | 对策 |
|---|---|
| 实现轮不能监听端口或 `ps`（§9 第 1 条） | 每个里程碑的验收都有一部分是 unit 组 vitest（模型纯函数与 `renderToStaticMarkup`）。kernel 组只起子进程不监听。浏览器用例标 ★，跑不成时把用例名与命令列进证据，交审计轮 |
| 文案守卫反复失败（§9 第 6 条） | 每个改 `app/` 的里程碑，验收命令都带 `labels.test.ts`、`testids.test.ts`、`styles.test.ts`。实现轮结束前跑 `npx vitest related --run <改动文件…>` 与宿主快检命令 |
| 页面判定与内核不一致（E-5、F-3） | `role-rules.ts` 与真实内核逐行对照（`loop-real.test.ts`） |
| 替身与真实接口不符（§9 第 11 条） | M-07 先用真实内核抓 `config.get`（全局与项目、有覆盖）与 `skills.list` 原始回包，存进 `test/fixtures/m07-*.json`，在证据里写抓取命令。unit 测试只用这些回包 |
| 相对时间造成水合不一致 | 一律经 `clock.tsx` 的服务端 `now` 渲染首屏；e2e 的控制台错误夹具兜底 |
| 浏览器用例超时 | 见下面的时间账；新增用例不建新根目录 |
| 改回包形状漏改使用方（§9 第 12 条） | 本任务不改任何 RPC 回包形状。唯一的内核产品改动是 `set_loop` 的校验输入 |

**时间账（总-Q4）。** 设计轮 2026-11-17 在本 worktree 跑了 `npm run build && npm run e2e`：29 条全过，整套 5.6 分钟
（336 秒），日志在 `.autome/output/plan1-e2e-run.txt`。

- 用例本身合计约 242 秒。最慢的几条是 C-2 60 秒、D-1 37 秒、B-1 28 秒、BL-01 26 秒、D-3 24 秒；layout 的 12 条每条不到 1 秒。
- 其余约 94 秒是全局夹具准备与构建检查。
- 新增 4 条按每条 ≤ 15 秒估，合计 ≤ 60 秒，并且不增加夹具准备。
- 预计整套约 400 秒，相对 720 秒上限余量约 320 秒；快检只选本里程碑的 spec，一次在 100–250 秒之间。改写的用例沿用原夹具，单条耗时不应明显变化；实现轮在证据里记下每次跑 `layout.spec.ts` 的总耗时，
发现单条翻倍时先查原因再继续。

**设计轮已跑与未跑的命令（§9 第 7 条）。**

- 已跑：`npx vitest list`（1056 个）、`npx playwright test --list`（29 条）、上面那次浏览器整套、实测一节的 `autome rpc` 探针。
- 未跑：各里程碑验收命令里的新测试文件（尚不存在），以及 `cargo test -p automed --test end_to_end`。

## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
| M-01 | 已完成 | A·令牌、图标与共用组件，样式守卫随之扩展 | 0 | |
| M-02 | 已完成 | A·外壳：侧栏、顶栏、驱动状态卡与两个新路由占位 | 0 | |
| M-03 | 已完成 | B·总览：结论句、等你处理 / 进行中卡片、今天与最近合并、四状态夹具 | 0 | |
| M-04 | 开放 | C·项目看板：丝带列头、精简卡片、已完成按日期分组、去掉门禁未跑过 | 0 | |
| M-05 | 开放 | D·任务详情汇总模型：步骤条、六个数字、阶段分组与筛选 | 0 | |
| M-06 | 开放 | D·任务详情页面重排：头部、执行过程、右侧四页签 | 0 | |
| M-07 | 开放 | E·Loop 配置：角色卡片、编辑抽屉、范围切换与双模型判定 | 0 | |
| M-08 | 开放 | E·Loop 默认值、项目信息页签与 set_loop 技能校验修复 | 0 | |
| M-09 | 开放 | F·技能页：列表筛选、绑定范围、可见性与打开目录 | 0 | |
| M-10 | 开放 | G·环境页与两个表单页按新组件重排，相对时间 | 0 | |
| M-11 | 开放 | G·README 五张截图与全站一屏巡检收口 | 0 | |
| M-12 | 开放 | BL-01·技能只读描述与卡片描述行 | 0 | |
| M-13 | 开放 | BL-06·批准设计卡片的评审裁决入口与摘要缺失回退 | 0 | |

## 各里程碑说明

| ID | 可独立观察的结果 | 覆盖 | 前置 | 主要改动 |
|---|---|---|---|---|
| M-01 | 组件库可在测试里逐个渲染出原型的类名与变体；令牌守卫扩展后仍逐项钉住 | A-1、A-3；总-Q2、总-Q3 | 无 | `tokens.css`、`app.css`、新 `components/ui/*`、新 `icons.tsx`、`bell.tsx`、`styles.test.ts` |
| M-02 | 侧栏、顶栏按原型排列；驱动卡给出四种口径；`/loop`、`/skills` 可访问 | A-2、A-3；总-Q5、总-Q6（路由登记）、A-Q1、A-Q2 | M-01 | `shell*.tsx`、`project-list.tsx`、`sidebar-footer.tsx`、`nav-links.ts`、`shell-data.ts`、`server/drivers.ts`、`eventbus.ts`（`drivers` 消息）、`live*.ts(x)`、`routes.ts`、`palette-model.ts` |
| M-03 | 总览四种状态的结论句与卡片正确；「今天」四个数与最近合并来自事件 | B-1…B-4；验-2 | M-02 | 新 `server/overview-summary.ts`、新 `overview-model.ts`、`_index.tsx`、`overview-card.tsx`、`clock.tsx`（`formatRelative` / `formatSpan`）、新 `test/fixtures/overview-states.sh` |
| M-04 | 已完成按四组紧凑条目、数值与账本一致；任何卡片不再出现「门禁未跑过」 | C-1…C-3、C-4 的文案；验-3 | M-03 | `board-data.ts`、`task-card*.ts(x)`、`task-columns.ts`、`projects.$id.tsx`、`queue-controls.tsx`、`labels.ts` |
| M-05 | `TaskSummary` 的步骤、六数、阶段与筛选在真实事件上与账本一致；体积不超 | D-1（数据）、D-2、D-3（模型）；D-Q1…D-Q6；验-4 | M-04 | 新 `task-summary-model.ts`、`task-detail-data.ts`、`task-timeline-model.ts`、`task-stage-model.ts` |
| M-06 | 详情页按原型重排；W7 的展开日志与滚动保持、文档链接、一屏判据不回归 | D-1、D-3（视图）、D-4 | M-05 | `tasks.$id.tsx`、`task-header*.tsx`、`task-stage.tsx`、`task-timeline.tsx`、`task-docs*.tsx`、`task-changes.tsx`、`task-decisions.tsx`、`task-notes.tsx` |
| M-07 | 页面改全局角色后 `config.get` 读回；项目覆盖与恢复；页面判拒与内核逐行一致；下一个会话带新配置 | E-1…E-5；验-5、验-6 | M-06 | 新 `routes/loop.tsx`、新 `loop-*.ts(x)`、新 `lib/role-rules.ts`、`lib/loop-labels.ts`、`live-routes.ts`、`end_to_end.rs`（新测试） |
| M-08 | 四项默认值全局与项目可改、可逐项恢复；已绑定技能时 `set_loop` 不再被拒；旧设置地址跳到项目信息页签 | E-6、E-7 | M-07 | `dispatch.rs`（`set_loop` 两处校验输入）、`loop-defaults*.ts(x)`、`routes/projects.$id.settings.tsx`（跳转）、`settings-data.ts`、`projects.new.tsx`（跳转目标） |
| M-09 | 技能页筛选与计数；三种绑定场景与 `config.get` 一致；不可见角色不能勾；「打开目录」打开服务端解析的路径 | F-1…F-4；验-7 | M-08 | 新 `routes/skills.tsx`、新 `skills-*.ts(x)`、新 `server/open-path.ts`、`main.ts`（注入打开器）、`lib/skill-labels.ts` |
| M-10 | 环境页每项一行、驱动单独一卡、时间都是相对时间并可悬停；两个表单用新组件且字段不变 | G-1、G-2 | M-09 | `routes/env.tsx`、`env-model.ts`、`routes/projects.new.tsx`、`routes/projects.$id.tasks.new.tsx`、`task-dependencies.tsx` |
| M-11 | README 五张新图；14 条一屏巡检全过 | G-3；验-1 的可运行部分 | M-10 | `scripts/screenshots.ts`、`docs/images/*.png`、`README.md`、`readme-images.test.ts`、`e2e/layout.spec.ts` |
| M-12 | skills.list 返回头部描述；技能卡片有描述行 | BL-01、F-Q1 | M-09 | `skills.rs`、技能域类型、`skills-model`、`skills-page` 与真实内核夹具 |
| M-13 | 批准设计卡片可打开评审与裁决文档，缺失时明示不可得 | BL-06、B-Q2 | M-03 | `overview-card`、`overview-model`、文档入口与测试 |

验-8 的「不做什么」逐项都有现成或改写后的用例：

- 铃铛：`bell-*`、layout「总览铃铛」；
- 命令面板：`palette`、layout「总览命令面板」；
- 调序与依赖：controls B-2 / B-3；
- 备注与重新打开：controls B-1 / B-4、`task-notes`；
- Backlog 表态：`task-decisions`；
- 会话日志：W7 H-03、C-2；
- 实时：live D-3；
- 离线横幅：live D-4；
- 添加 / 移除项目：W9 H-01 / H-02、`settings-config`；
- Onboarding：`settings-onboarding*`。

这些用例分别落在改动它的里程碑的验收命令里。验-9 由宿主合并前门禁承担。

## 验收契约

**通用约定。**

- 所有命令在 worktree 根目录执行。kernel 组与 Rust 测试先 `cargo build -p automed -p autome-cli`。
- 浏览器部分先 `npm run build --prefix apps/web`。带 ★ 的部分需要监听端口或 `ps`，实现轮被沙箱拒绝时按约束 15 交审计轮。
- 用例数以本设计实测的基线为准（括号内是改动前的数）。实现轮开工先复核基线，与此不符时以实数为准，并在证据里说明差从哪来。
  新文件的用例数是下限：多出的写明来源。

### M-01 验收契约

最新证据：docs/web-面板按动森风格重做/evidence/M-01-r1-audit.md · 通过 · 审计 #1

- 验收命令：`(cd apps/web && npx vitest run test/ui-kit.test.ts test/styles.test.ts test/labels.test.ts test/testids.test.ts test/bell-view.test.ts test/test-groups.test.ts)`
- 预期：6 个文件，至少 58 个用例全过：
  - ui-kit：新，unit 组，至少 16 个。覆盖：
    - 卡片七种颜色与虚线；丝带四色与小号；胶囊描边与九种实心；数字块七色与缺失显示「—」；
    - 进度条 SVG 宽度夹到 0–100，带 `aria-valuenow`；
    - 主按钮唯一带 `btn-primary`；
    - `Segmented` 渲染同名 radio；`Switch` 有 `role=switch`；`Stepper` 在 `min` / `max` 时禁用对应按钮；`Check` 禁用时有
      `aria-describedby`；
    - `Icon` 十个名字都能渲染。
  - styles：15（12 +3）。新增三条：`var()` 都有定义；每个 `--ui-*` 都被引用；`<svg` 只出现在 `icons.tsx` 与进度条组件。令牌
    清单逐项追加。
  - labels 10、testids 5、bell-view 8、test-groups 4。
- 负向对照：
  - 进度条改用 `style={{width}}`：styles 的「TSX 无 style={」失败。
  - 在 `app.css` 写 `var(--ui-nope)`：新增的「`var()` 都有定义」失败。
  - 进度条不夹取，传 140：ui-kit 的夹取用例失败。
  - 在组件里直接写 `#fff`：styles 的颜色守卫失败。

### M-02 验收契约

最新证据：docs/web-面板按动森风格重做/evidence/M-02-r2-audit.md · 通过 · 审计 #2

- 验收命令：`(cd apps/web && npx vitest run test/shell.test.ts test/shell-data.test.ts test/eventbus.test.ts test/live-client.test.ts test/palette.test.ts test/live-routes.test.ts test/styles.test.ts test/labels.test.ts test/testids.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts -g "总览")`
- 预期：9 个文件，至少 107 个用例全过：
  - shell：8（4 +4）。导航顺序；项目行两种显示；驱动卡四种口径；面包屑。
  - shell-data：11（8 +3）。`driver.list` 的 `unknown_method` 回退；`skills.list` 失败不拖垮外壳；「N 个已完成」计数。
  - eventbus：25（23 +2）。驱动摘要变化时发一条 `drivers`；不变时不发。
  - live-client：9（8 +1）。收到 `drivers` 更新订阅者。
  - palette：8（7 +1）。新增四类入口。
  - live-routes：15（14 +1）。`/loop`、`/skills` 收到 `config.changed` 重新验证。
  - styles 15、labels 10、testids：6（5 +1，两个新路由逐页渲染）。
  - ★ 浏览器 3 条：layout「总览」「总览命令面板」「总览铃铛」。断言侧栏有总览、Loop 配置、技能、环境四个入口和驱动卡，并且一屏。
- 负向对照：
  - 驱动卡选 `self` 而不是在推进的那个：shell「两个驱动只显示在推进的那个」失败。
  - 每个 tick 都发 `drivers`：eventbus「不变时不发」失败。
  - `/loop` 的 `byKind` 漏 `config.`：live-routes 新用例失败。

### M-03 验收契约

最新证据：docs/web-面板按动森风格重做/evidence/M-03-r3-audit.md · 通过 · 审计 #3

- 验收命令：`(cd apps/web && npx vitest run test/overview.test.ts test/overview-summary.test.ts test/overview-states-real.test.ts test/clock.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts -g "1512×944 总览$")`
- 预期：7 个文件，至少 62 个用例全过：
  - overview：17（7 +10）。现算等待时长的纯函数（开放区间随 `now` 增长、与今天取交集、`now` 越过 `dayEnd` 要求重新验证）；结论句的四种计数组合；等你处理六种种类的胶囊与摘要；没有 Backlog 时不写「待表态」；进行中卡片的
    进度与本轮时长；排队卡片「等谁」两种；空闲两张虚线卡；结果胶囊六条规则。
  - overview-summary：新，unit 组，至少 12 个。用从真实内核抓的事件加构造事件：
    - 按服务端时区切日（含跨午夜的等待区间只算今天那段）；
    - `gate.reused` / `gate.skipped` 不计；`repro_check`、`main_check` 的 `gate.result` 也不计，只数 `gate` / `fix_gate` / `merge_gate`；
    - 今天有活动后被取消的任务（`completed_at` 为空）与今天又跑了一次复盘的旧终结任务，它们今天的花费、门禁、等待照常计入；
    - `not_a_bug` 的等待区间被计入，并在 `reply` 处结束；
    - 无计费会话单独计数；
    - 开放区间返回起点与 `dayStart` / `dayEnd`，不返回算死的时长；
    - 合并数只数今天 `done`；
    - 最近合并取前 8 并倒序；多项目时不出现「全部 N 个」；
    - 序号不变时不重读事件，但换一个更大的 `now` 现算，开放区间的时长随之增长；
    - 没有已合并任务时列表为空。
  - overview-states-real：新，kernel 组，4 个。`idle` / `running` / `awaiting` / `failed` 各一个，断言结论句与卡片。
  - clock：9（7 +2）。`formatRelative`、`formatSpan`。
  - labels 10、testids 6、test-groups 4。
  - ★ 浏览器 1 条：layout「总览」，加断言结论句、黄色等你处理卡、四个数字块，并且一屏。
- 负向对照：
  - 把 `gate.reused` 计入门禁次数：overview-summary「reused 不计」失败。
  - 结论句漏掉排队数：overview「在跑与排队」组合失败；`running` 状态下也会写错。
  - 等待区间不与今天取交集：「跨午夜」用例失败。
  - 候选任务改回只看 `completed_at` 为今天或未终结：「取消后的任务」用例失败。
  - 等待状态用 `await_` 前缀判断：`not_a_bug` 用例失败。
  - 开放区间按序号缓存成固定值：「`now` 前进后增长」用例失败。

### M-04 验收契约

最新证据：docs/web-面板按动森风格重做/evidence/M-04-r4-impl.md · 自动化子集通过，浏览器环境阻塞，保持开放 · 实现 #4

- 验收命令：`(cd apps/web && npx vitest run test/board.test.ts test/task-card.test.ts test/task-columns.test.ts test/board-payload.test.ts test/board-cancelled.test.ts test/board-done-real.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "项目|全部已完成|已取消展开|B-2|B-3")`
- 预期：9 个文件，至少 75 个用例全过：
  - board：19（13 +6）。四个丝带列头；按日期分组（今天 / 昨天 / 本周一起算的本周更早 / 更早，含周一 00:00 边界与跨年）；
    「更早」只给一行和 `?done=all` 链接；累计花费与平均实现轮的分母；Bug 实现轮显示「—」；没有 `metrics` 时显示「—」。
  - task-card：16（14 +2）。任何列都不出现「门禁未跑过」；在途无门禁时不渲染门禁行。
  - labels：11（10 +1）。`gatePhrase` 无门禁返回 `null`。
  - task-columns 6、board-payload 6（200 KB 与 30 次 `task.get` 不变）、board-cancelled 5、testids 6、test-groups 4。
  - board-done-real：新，kernel 组，至少 2 个。用 `w7-flow.sh` 加 `release-main-check` 与 `autome merge`，把一个需求任务推到
    `done`（做法同 e2e D-1）。断言紧凑条目的实现轮、退回、花费与同一内核 `project.get` 的 `metrics.impl_rounds_used`、
    `metrics.reopen_total`、`cost_usd` 相等，且页面不含「门禁未跑过」。
  - ★ 浏览器 5 条：layout「项目」「全部已完成」「已取消展开」，controls「B-2」「B-3」。
- 负向对照：
  - 恢复旧 `gatePhrase`（空时返回「门禁未跑过」）：task-card 的「在途无门禁不渲染门禁行」与 labels 的 `gatePhrase` 返回 `null` 用例失败（已完成条目本身不渲染门禁行，判别落在仍调用 `gatePhrase` 的在途卡片）。
  - 「本周更早」从周日算起：board 的周边界用例失败。
  - 实现轮取 `budget_n`：board-done-real 的相等断言失败。

### M-05 验收契约

- 验收命令：`(cd apps/web && npx vitest run test/task-summary.test.ts test/task-summary-real.test.ts test/task-timeline.test.ts test/task-stage.test.ts test/task-detail-data.test.ts test/task-detail-payload.test.ts test/task-detail-ssr.test.ts test/test-groups.test.ts)`
- 预期：8 个文件，至少 84 个用例全过：
  - task-summary：新，unit 组，至少 16 个。用 `test/fixtures/w7/` 的真实回包加构造事件：
    - 需求九步与 Bug 六步的用时与花费；
    - 「你批准」= `await_design_approval` 区间之和（含驳回后第二次等待）；`not_a_bug` 与 `failed` 区间计入总用时的「其中等你」，`paused` 不计；
    - 用户行保留 `reply` 等全部现有操作，退出等待的那条带「等了 X」；
    - 总用时与「其中等你」；
    - 会话按工具分组；
    - 门禁「快检 / 合并前」按节点分、只数 `gate.result` 且节点为 `gate` / `fix_gate` / `merge_gate`，`repro_check` 的结果不计；
    - 截断时门禁缺失；
    - 在途实现轮的退回取状态块之和；
    - 无计费会话；
    - 「只看门禁」「只看退回」两种筛选；
    - 阶段分组倒序。
  - task-summary-real：新，kernel 组，至少 3 个。`w7-flow.sh` 跑到 `await_merge`。断言：会话数与工具分组等于 `task.get`；门禁数
    等于事件中节点为 `gate` / `fix_gate` / `merge_gate` 的 `gate.result` 个数（与同一任务终结后的 `TaskMetrics` 一致）；「你批准」等于从两条 `task.updated` 的 `at` 算出的差。
  - task-timeline：28（24 +4）。用户行与等待时长；门禁行步数；`gate.reused` / `skipped` 行；阶段分组。
  - task-stage：14（12 +2）。
  - task-detail-data：12（11 +1）。`TaskSummary` 只在服务端算、不带全量会话。
  - task-detail-payload 4、task-detail-ssr 3（150 KB 不变）、test-groups 4。
- 负向对照：
  - 「你批准」改用会话间隙计算：task-summary 与 task-summary-real 的等待用例失败。
  - 门禁计数包含 `gate.reused` 或 `repro_check`：两处门禁用例失败。
  - 等待状态漏掉 `not_a_bug`：task-summary 的等待用例失败。
  - 把全量会话放进 loader 返回值：task-detail-data 新用例失败；60 会话时 ssr 体积用例失败。

### M-06 验收契约

- 验收命令：`(cd apps/web && npx vitest run test/task-page.test.ts test/task-header.test.ts test/task-timeline.test.ts test/task-docs-view.test.ts test/task-changes.test.ts test/task-decisions.test.ts test/task-notes.test.ts test/task-ops-view.test.ts test/labels.test.ts test/testids.test.ts test/styles.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts -g "详情|W7 H-03|W7 H-04|C-2")`
- 预期：11 个文件，至少 107 个用例全过：
  - task-page：9（6 +3）。头部、步骤条、六个数字块的结构，并改写原有的布局钉子。
  - task-header：6（3 +3）。已合并的头部胶囊与合并提交；操作按钮集合等于 `WRITE_OPS` 中可用的那些；「你批准」步骤的类名。
  - task-timeline：29（28 +1）。`?show=` 与 `from`、`doc` 共存的链接。
  - task-docs-view 8、task-changes 9、task-decisions 3、task-notes 7、task-ops-view 4、labels 11、testids 6、styles 15。
  - ★ 浏览器 6 条：layout「需求详情展开日志」「Bug 详情」，manual-items「W7 H-03」「W7 H-04」「C-2」「C-2b」。
- 负向对照：
  - 筛选链接丢掉 `from`：task-timeline 新用例与 C-2b 失败。
  - 执行过程容器去掉 `data-scroll-region`：W7 H-04 的一屏判据与 testids 的滚动区清单失败。
  - 新增操作按钮漏掉「重新打开」：task-header 的按钮集合用例失败。

### M-07 验收契约

- 验收命令：`cargo test -p automed --test end_to_end a_role_written_through_set_role_reaches_the_next_session && (cd apps/web && npx vitest run test/role-rules.test.ts test/loop-page.test.ts test/loop-real.test.ts test/live-routes.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "Loop 配置|E-1")`
- 预期：Rust 1 个用例通过。vitest 7 个文件，至少 63 个用例全过：
  - role-rules：新，unit 组，至少 12 个。三对；停用免检；both_default；同模型；不可见技能；不存在技能；项目覆盖 runtime 后的
    可见性；保存顺序规划（有合法顺序、无合法顺序）。
  - loop-page：新，unit 组，至少 10 个。用 M-07 抓的真实 `config.get` 回包：
    - 六张角色卡片的顺序与颜色；四张固定或人工虚线卡；
    - 「CLI 默认」「Effort 默认」；项目范围的「跟随全局 / 项目覆盖」；
    - 违规时两张卡标红、保存禁用并有说明；
    - Effort 非四选项时出现「其它」；
    - 项目范围只带改过的字段；
    - 「恢复跟随全局」的确认文案。
  - loop-real：新，kernel 组，至少 4 个。
    - 全局改设计的模型与 Effort 后 `config.get` 读回；
    - 项目覆盖后该项目是 `project`、另一项目不受影响，恢复后回到全局值；
    - 约 14 行的对照表，`role-rules` 与内核判定逐行相同；
    - 内核拒绝的 `message` 原样出现在 action 结果里。
  - live-routes 15、labels 11、testids 7（6 +1）、test-groups 4。
  - ★ 浏览器 2 条：layout「Loop 配置」（新），controls「E-1 …」（新；用例结束时把改过的配置恢复）。
- 负向对照：
  - `role-rules.ts` 漏掉复盘→实现：loop-real 对照表的复盘行失败。
  - 项目范围保存带上全部字段：loop-page「只带改过的字段」失败。
  - 新 Rust 测试在不调 `set_role` 时运行：会话行的模型断言失败（审计轮按此注入核实）。

### M-08 验收契约

- 验收命令：`cargo test -p automed --lib set_loop && (cd apps/web && npx vitest run test/loop-defaults.test.ts test/loop-real.test.ts test/settings-config.test.ts test/settings-onboarding.test.ts test/settings-real.test.ts test/settings-onboarding-real.test.ts test/w9-layout.test.ts test/add-project.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts -g "项目设置|W9 H-01")`
- 预期：
  - Rust：名字含 `set_loop` 的用例全过。原有 `session_minutes_set_loop_round_trip_and_rejection`，加新的
    `set_loop_accepts_when_a_bound_skill_exists`（全局与项目两种范围），至少 2 个。
  - vitest 11 个文件，至少 85 个用例全过：
    - loop-defaults：新，unit 组，至少 6 个。四项的范围与禁用；「与全局相同 / 项目值」；逐项恢复发 `null`。
    - loop-real：6（M-07 的 4 +2）。已绑定技能时全局与项目 `set_loop` 成功；项目逐项恢复。
    - settings-config 14、settings-onboarding 8、settings-real：5（4 +1，旧地址 302 到项目信息页签）、
      settings-onboarding-real 4、w9-layout 4、add-project：16（15 +1，成功后的跳转目标）、labels 11、testids 7、test-groups 4。
  - ★ 浏览器 2 条：layout「项目设置」（访问旧地址，断言跳转并一屏），manual-items「W9 H-01」。
- 负向对照：
  - 把 `set_loop` 改回 `config::NoSkills`：新 Rust 用例与 loop-real 的「已绑定技能」用例失败。
  - 删掉旧地址的跳转：settings-real 新用例失败。

### M-09 验收契约

- 验收命令：`(cd apps/web && npx vitest run test/skills-model.test.ts test/skills-page.test.ts test/skills-real.test.ts test/open-path.test.ts test/write-gate.test.ts test/gateway.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "技能|F-1")`
- 预期：9 个文件，至少 120 个用例全过：
  - skills-model：新，unit 组，至少 14 个。用真实 `skills.list` 回包：
    - 四种筛选的计数随项目变化；搜索；没有项目时隐藏「本项目」；
    - 两种范围对应的 `set_role` 调用（只写成员有变化的角色；全局用 global 列表，项目用 resolved 列表）；
    - 取消全局绑定写项目列表；
    - 保存后「没拿到这条绑定」的项目名单取自读回的 resolved 技能，不取自 `provenance`；
    - 三类技能（纯全局、纯项目、同名混合）的绑定可用性与可见性：同名混合时「全局」一档可用且只按 global 来源算可见；
    - 不可见角色禁用及原因；项目技能禁用「全局」一档；`conflicts` 标红。
  - skills-page：新，unit 组，至少 4 个。卡片字段；来源路径；抽屉结构；每个来源一个「打开目录」。
  - skills-real：新，kernel 组，至少 7 个。临时 `HOME` 下自建全局与项目技能、三个临时仓库项目：
    - 列出全局与项目技能；
    - 「仅当前项目」只进 A 的 `config.get`；
    - 「全局」进 B 与 C；
    - A 只覆盖模型时全局绑定仍进 A，页面不把 A 列为没拿到；C 先有技能列表覆盖时不进 C，保存后页面列出 C；
    - 同一个名字在全局与项目都有：项目范围 `skills.list` 回 `project_scoped = true`，全局绑定仍成功；
    - 不可见角色的勾选被禁用，绕过页面直接写时内核拒绝原样显示。
  - open-path：新，unit 组，至少 5 个。按名解析路径；多来源按序号；未知名 404；其它 `kind` 仍 501；打开器是注入的替身且收到
    服务端解析的路径。
  - write-gate 54、gateway 13、labels 11、testids 8（7 +1）、test-groups 4。
  - ★ 浏览器 2 条：layout「技能」（新），controls「F-1 …」（新；结束时恢复）。
- 负向对照：
  - 「仅当前项目」写成不带 `project_id`：skills-real 的 B / C 断言失败。
  - 「全局」用 resolved 列表而不是 global 列表：skills-model 的调用断言失败。
  - 打开器直接用请求参数里的路径：open-path「收到服务端解析的路径」失败。
  - 用角色 `provenance` 判断技能覆盖：skills-real 里 A（只覆盖模型）的用例失败。
  - 把 `project_scoped` 当作「仅项目」禁用全局一档：同名混合用例失败。

### M-10 验收契约

- 验收命令：`(cd apps/web && npx vitest run test/env-page.test.ts test/env-ops.test.ts test/env-real.test.ts test/clock.test.ts test/task-new.test.ts test/add-project.test.ts test/w9-layout.test.ts test/labels.test.ts test/testids.test.ts test/styles.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts -g "环境|新建任务|添加项目补全|W9 H-02|W9 H-04")`
- 预期：10 个文件，至少 99 个用例全过：
  - env-page：12（8 +4）。每项一行四列；内核行取 `/api/instance`；驱动卡；时间是 `<time title>` 相对时间，不出现原始 ISO 字符串。
  - clock：10（9 +1）。「34 分钟前启动」。
  - task-new：12（11 +1）。字段名 `kind`、`brief`、`attachments`、`after`、`intent` 不变。
  - add-project：17（16 +1）。`path`、`workspace`、`docs_repo`、`skip_onboarding` 不变。
  - env-ops 7、env-real 3、w9-layout 4、labels 11、testids 8、styles 15。
  - ★ 浏览器 5 条：layout「环境」「新建任务」「添加项目补全」，manual-items「W9 H-02」「W9 H-04」。
- 负向对照：
  - 驱动时间改回直接输出 `started_at`：env-page「不出现原始 ISO」失败。
  - 类型单选的 `name` 改成 `type`：task-new 字段用例失败。

### M-11 验收契约

- 验收命令：`(cd apps/web && npx vitest run test/readme-images.test.ts test/docs-web.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts)`
- 预期：
  - readme-images：4（3 +1）。README 恰好引用五张图且 `docs/images` 只有它们；每张都是 1512×944 的 PNG；README 提到截图脚本；
    新增「`loop.png` 与 `skills.png` 存在且非空」。
  - docs-web 2。
  - ★ layout.spec.ts 全部 14 条（原 12 +「Loop 配置」「技能」）全过。
- 负向对照：
  - 只保留旧的三张图：readme-images 的五图用例失败。
  - 任一页去掉内部滚动区的 `data-scroll-region`：该页的 layout 一屏判据失败。

## 人工验收清单

- H-01 在 1512×944 下打开总览（有任务在跑、有任务等批准），与原型「总览」逐屏对照：结构、组件、配色角色一致
- H-02 同上，空闲状态的总览与原型「空闲」演示一致：两张虚线空状态卡，右侧「今天」与「最近合并」照常
- H-03 项目看板与原型「项目看板」逐屏对照，已完成列的日期分组与紧凑条目观感一致
- H-04 一个已合并需求的任务详情与原型「任务详情」逐屏对照：步骤条、六个数字块、执行过程的绿色门禁行与黄色用户行
- H-05 环境页与原型「环境」逐屏对照；悬停时间能看到绝对时间
- H-06 Loop 配置页（全局与某项目两种范围）与原型逐屏对照；改一个角色的模型后，下一个真实启动的会话用的是新模型
- H-07 技能页与原型逐屏对照；在真实机器上点「打开目录」，Finder 打开对应的技能目录
- H-08 Electron 薄壳加载新面板，六个页面都能到达，原生目录选择按钮仍在添加项目页出现

### M-12 验收契约

- 验收命令：cargo test -p automed --lib skills && (cd apps/web && npx vitest run test/skills-model.test.ts test/skills-page.test.ts test/skills-real.test.ts test/labels.test.ts)。
- 情形：头部单行及多行 description 正确返回；无头部、空描述、文件不可读时缺失回退；多个来源的描述按稳定来源顺序取首个非空值，保留全部来源；HTML 文本按普通文本显示。
- 只读扫描不改技能文件、不改绑定与可见性；真实内核夹具证明字段与页面一致。预期用例总数以 M-09 已关闭基线加上述回归用例计，证据逐项报告增量。
- 负向对照：删掉 description 回包或隐藏卡片描述行，上述测试必须失败。

### M-13 验收契约

- 验收命令：(cd apps/web && npx vitest run test/overview.test.ts test/overview-summary.test.ts test/overview-states-real.test.ts test/labels.test.ts)。
- 情形：批准设计卡片在评审、裁决文档均存在时分别显示可访问入口；仅一份或均缺失时明示相应结果摘要不可得；不编造问题数与采纳数；其它卡片行为不变。
- 复用受控文档入口，不直接开放任意磁盘路径。预期用例总数以 M-03 已关闭基线加上述回归用例计，证据报告差值。
- 负向对照：删去文档入口及缺失提示，批准设计用例必须失败。

## Backlog

- BL-01 已纳入 M-12：`skills.list` 增加只读的 `description`（取自 `SKILL.md` 头部），技能卡片显示原型里的描述行
- BL-02 已忽略（用户裁定）：`/api/instance` 或 `env.get` 给出账本版本，环境页内核行显示「账本版本 N」
- BL-03 已忽略（用户裁定）：内核记录评审提出的问题数与裁决采纳数，总览「等你处理」卡片写出原型那句「评审提了 2 项问题，裁决全部采纳」
- BL-04 已忽略（用户裁定）：`config.get` 给出项目覆盖的原始键，Loop 默认值在项目范围精确显示「跟随全局 / 项目值」
- BL-05 已忽略（用户裁定）：看板「待开始」列支持拖动调序（需要同时提供键盘等价操作）
- BL-06 已纳入 M-13：总览「等你处理」的批准设计卡片给出评审与裁决文档的入口，或明确写出结果摘要不可得（来自评审建议 N3，B-Q2 允许数据缺失时回退，不阻塞）

## 争议项
