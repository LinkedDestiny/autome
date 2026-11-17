# 任务：Web 面板按动森风格重做——找回彩色组件与信息层次，补回全局 / 项目级 Loop 配置与技能绑定

本任务的规则见 `docs/web-面板按动森风格重做/protocol/loop-protocol.md` 与
`docs/web-面板按动森风格重做/protocol/session-protocol.md`。

- 设计文档：`docs/web-面板按动森风格重做/web-面板按动森风格重做.md`
- 任务卡（用户写的，第一输入）：`docs/web-面板按动森风格重做/web-面板按动森风格重做-card.md`
- 设计原型（用户已确认，视觉与信息结构基准）：`docs/design/autome-web-panel-v3.html`
- 任务整理日期：2026-11-17；任务编号 T-101
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree）；整理时的基线提交 `04e857d3`
- 本任务固定的协议版本：`protocol/v9`；账本 schema 现为 9（`crates/automed/src/store.rs` 的 `SCHEMA_VERSION`）

## 1. 需求原文

> Web 面板按动森风格重做：找回彩色组件与信息层次，补回全局 / 项目级 Loop 配置与技能绑定

任务卡正文比这句话具体得多；两者不一致时以任务卡为准。§2–§5 照任务卡整理（编号沿用任务卡的 A-1…G-3、验-1…验-9），
§6–§7 是整理轮从调研得出的约束与待定问题，§8 是代码现状，§9 是前几个任务留下的教训，§10 是项目背景。

## 2. 背景与目标

用户 2026-11-17 看过现在的面板后的原话：「现在的面板 UI 太难看了，且信息展示不够好」「全局和项目级别的循环配置功能没有了。
要跟原版一样，支持配置 Loop 的模型，effort，以及需要支持管理项目维度的 Skill（指定 Skill 是项目级别还是全局级别）」。

任务卡的对照结论：

1. **视觉。** 面板沿用了 animal-island-ui 的令牌（`apps/web/app/styles/tokens.css`），但实现时把这套风格的组件做丢了：没有彩色
   图案卡片、丝带标题、彩色状态胶囊、进度条，只剩一片米色底配棕色字，层次全靠字号。9 月的成品稿
   `docs/design/autome-2.0-desktop-ui.html` 是用户认可的观感。用户已明确：**继续沿用动森风格**，不换审美。
2. **信息展示。** 真实账本上的六个页面截图显示：
   - 总览在没有任务时整页空白；
   - 项目看板「已完成」一列堆了 70 张高卡片，每张带一条对已完成任务没有意义的阶段条；
   - 已合并任务的卡片写着「门禁未跑过」（`apps/web/app/lib/labels.ts:68`，`gate` 为空时的文案），实际都跑过并通过；
   - 任务详情头部拥挤，时间线和文档抢空间，门禁结果与用户的批准不在时间线里，看不出每个阶段花了多久、等了用户多久；
   - 环境页是竖排的原始 UTC 时间戳。
3. **功能缺失。** 旧 Electron 面板有路由图、技能、全局设置三个页面（提交 `e80e9dc1` 删除，旧实现可用
   `git show 'e80e9dc1^:apps/desktop/renderer/screens/{routing,skills,settings}.js'` 查看）。Web 面板没有搬过来，现在的项目设置页
   只有一块只读配置。内核能力都在：`config.get`、`config.validate`、`config.set_role`、`config.reset_role`、`config.set_loop`
   （都接受可选的 `project_id`）、`skills.list`。

**设计原型。** `docs/design/autome-web-panel-v3.html` 是单文件可点击原型，1512×944 一屏，六个页面：总览、项目看板、任务详情、
环境、Loop 配置、技能。顶部棕色控制条可切换「有任务在跑 / 空闲」两种演示状态；Loop 配置页可切换全局与项目范围。
用户 2026-11-17 的结论：「没有问题，设计原型通过了」。**实现以它为准**；原型里的任务 T-101、T-102 和「审计绑定了
autome-v2-simplify」是演示数据，不是需求。（提醒：本任务自己的编号恰好也是 T-101，与原型里那张演示卡无关。）

**目标。** 一次设计、一次评审，按 A–G 分组里程碑实现：补齐组件与外壳（A），按原型重做总览（B）、项目看板（C）、任务详情（D），
补回 Loop 配置（E）与技能（F）两个页面，重排环境页与两个表单页并更新 README 截图（G）。

## 3. 任务要求（任务卡「要做什么」逐条照录；设计文档的里程碑必须全部覆盖）

### 3.1 A 组件与外壳

- **A-1** 在现有令牌之上补齐原型用到的组件，全站共用：图案卡片的颜色变体（黄 / 青 / 紫 / 蓝 / 绿 / 红 / 虚线）、丝带标题
  （含小号与颜色变体）、胶囊（描边与实心）、彩色数字块、进度条、主按钮（带立体阴影，只用于主要操作）、分段选择、开关、步进器、
  输入框、勾选块。
- **A-2** 外壳：侧栏（品牌、总览、Loop 配置、技能、项目列表带「等你处理 / 进行中」计数、添加项目、驱动状态卡、环境）、顶栏
  （面包屑、搜索、铃铛、新建任务）。驱动状态卡显示驱动是否在推进与上次检查的相对时间。
- **A-3** 硬规则不变：全中文（专有名词除外）、1512×944 下页面不出现整页纵向滚动（列与面板内部滚动）、字体离线可用、圆角不小于
  12px、不用纯黑、不做深色主题。

### 3.2 B 总览

- **B-1** 丝带旁用一句话给结论：有几件事等用户决定、几个在跑、几个在排队；都没有时写「现在没有事情等你」。
- **B-2** 「等你处理」卡片写明：要用户做什么（批准设计 / 表态 / 处理失败）、任务标题、设计有几个里程碑、评审与裁决的结果、
  几条 Backlog 待表态、已花费、已用时、已经等了多久；主按钮直达。
- **B-3** 「进行中」卡片写明：当前角色与轮次、里程碑进度条、正在做哪个里程碑、本轮已跑多久、已用时、已花费；排队中的任务
  单独一种卡片，写明在等谁。
- **B-4** 右侧「今天」四个数字块（合并任务数、花费、门禁次数与通过数、等用户的时间）与「最近合并」列表（编号、标题、一个结果
  胶囊、合并时间、花费）。空闲时这两块照常显示，左侧是两张虚线空状态卡。

### 3.3 C 项目看板

- **C-1** 四列用丝带列头。待开始 / 进行中 / 等你处理三列是卡片，信息同 B-2、B-3 的精简版。
- **C-2** 「已完成」改为按日期分组（今天 / 昨天 / 本周更早 / 更早）的紧凑条目：编号、标题、实现轮数、退回次数、花费；列头旁给
  累计花费与平均实现轮数。条目点开进任务详情。
- **C-3** 修掉已合并任务显示「门禁未跑过」的错误。

### 3.4 D 任务详情

- **D-1** 头部卡片：状态胶囊、类型与编号、分支、合并提交、操作按钮、标题；一条步骤条，每一步下面写用时与花费，「你批准」是
  单独一步并写明等了多久；里程碑状态胶囊一行。
- **D-2** 六个数字块：花费（含几个会话无计费）、总用时（其中等用户多久）、会话数（按工具分）、设计轮、实现轮（含退回次数）、
  门禁（通过数 / 总数，分快检与合并前）。
- **D-3** 「执行过程」一行一个事件，按阶段分组、倒序：会话（角色胶囊、轮次、工具与模型、一句话结果、用时、花费）、门禁结果
  （绿色行，写执行与沿用的步数、用时）、用户的操作（黄色行，写等了多久）、合并。可筛选「只看门禁」「只看退回」。点开会话仍能
  看日志。
- **D-4** 右侧保留现有四个页签的全部功能（文档、改动与合并、待你决定、备注与依赖），按新组件重排。

### 3.5 E Loop 配置（补回的功能）

- **E-1** 新页面，侧栏有入口，项目看板的「项目设置」也到这里。顶部切换范围：全局默认，或某一个项目。
- **E-2** 按 Loop 顺序排出六个角色卡片（设计、评审、裁决、实现、审计、复盘），每张显示工具、模型、Effort、绑定技能数、启用开关；
  固定步骤（任务整理、宿主快检、合并前门禁）与人工停顿是不可配置的虚线卡片。
- **E-3** 点角色卡片在右侧编辑：启用、工具（Claude Code / Codex）、模型（自由文本，内核不提供模型目录）、Effort（默认 / low /
  medium / high）、绑定的技能。写入走 `config.set_role`。
- **E-4** 项目范围下每个角色标明「跟随全局」还是「项目覆盖」，可以「恢复跟随全局」（`config.reset_role`）。
- **E-5** 双模型纪律：评审与设计、审计与实现的 `工具:模型` 相同时页面当场标红并禁用保存；内核的拒绝仍然原样显示。绑定到看不见
  该技能的工具同样阻止。
- **E-6** Loop 默认值四项：并行任务数、设计轮上限、实现预算系数、单个会话时长上限，走 `config.set_loop`，全局与项目各自可改。
- **E-7** 现有项目设置页的内容不能丢：Onboarding 状态、门禁与合并等只读配置、移除项目。放在哪由设计轮定（例如 Loop 配置页
  项目范围下的第二个页签）。

### 3.6 F 技能（补回的功能）

- **F-1** 新页面：技能卡片显示名称、来源是全局还是项目、哪些工具看得到、绑定给了哪些角色；可按「全部 / 全局 / 本项目 / 已绑定」
  筛选、按名称搜索、切换项目。数据来自 `skills.list`。
- **F-2** 点一个技能在右侧管理绑定：**绑定范围**（全局 · 所有项目，或仅当前项目）与「哪些角色必须使用」。用户已确认
  「指定 Skill 是项目级别还是全局级别」指的是这条绑定的生效范围；绑定就是角色的技能列表，全局范围写全局配置、项目范围写项目
  配置，都走 `config.set_role`。
- **F-3** 角色所用工具看不到该技能时不能勾选并说明原因；项目技能只能绑定在自己的项目里。
- **F-4** 面板不创建、不编辑、不移动技能文件，只显示来源路径并提供「打开目录」。

### 3.7 G 其余页面与文案

- **G-1** 环境页：每项检查一行（状态、名称、路径、版本），驱动单独一张卡；时间一律显示相对时间（「3 秒前」「34 分钟前启动」），
  悬停可看绝对时间。
- **G-2** 新建任务、添加项目两个表单页按 A 组的组件重排（现在的单选框与文件选择是浏览器默认样式）。
- **G-3** README 的面板截图用 `apps/web/scripts/screenshots.ts` 重新生成。

## 4. 验收标准（任务卡原文）

- **验-1** 六个页面在 1512×944 下与原型逐屏对照：结构、组件、配色角色一致，没有整页纵向滚动；浏览器用例钉住每页的关键元素
  （丝带标题、状态颜色、数字块、步骤条等）。
- **验-2** 夹具覆盖四种状态：空闲、有任务在跑、有任务等批准、有任务失败；总览的结论句与卡片内容随之正确变化。
- **验-3** 已完成列表按日期分组，每行的实现轮数、退回次数、花费与账本一致；已合并任务不再出现「门禁未跑过」。
- **验-4** 任务详情的步骤条、六个数字、执行过程与账本的会话、门禁事件、批准事件一致；「等用户多久」由事件时间算出，有测试。
- **验-5** 在页面上改全局的某个角色（模型与 Effort），`config.get` 读回新值；在项目范围覆盖同一角色后，该项目显示「项目覆盖」、
  别的项目不受影响；「恢复跟随全局」后回到全局值。下一个启动的会话用的是新配置（内核测试或夹具证明）。
- **验-6** 评审与设计设成同一个 `工具:模型` 时保存被禁用并有说明；绕过页面直接请求时，内核的拒绝原样显示。
- **验-7** 技能页列出全局与项目技能；把一个技能按「仅当前项目」绑定给一个角色后，只有该项目的 `config.get` 里出现它；按「全局」
  绑定后所有项目都出现。工具看不到的角色不能勾选。
- **验-8** 「不做什么」里列出的每项现有功能各有一条仍然通过的用例。
- **验-9** `sh scripts/ci-local.sh` 全部通过。

## 5. 范围

**在范围内（按任务卡「参考」与调研得出，具体落点以设计为准）。**

| 项 | 可能改的地方 |
|---|---|
| A | `apps/web/app/styles/{tokens.css,app.css}`、`apps/web/app/components/`（外壳 `shell.tsx`、`project-list.tsx`、`sidebar-footer.tsx`、`bell.tsx`、`command-palette.tsx` 与新增的共用组件）、`apps/web/app/root.tsx`、`shell-data.ts`、`server/drivers.ts`（只读取用） |
| B | `apps/web/app/routes/_index.tsx`、`components/overview-card.tsx`、`shell-data.ts`；「今天」与「最近合并」的服务端汇总（`apps/web/server/`） |
| C | `apps/web/app/routes/projects.$id.tsx`、`components/{board-data,task-card,task-card-model,task-columns}.ts(x)`、`lib/labels.ts`（`gatePhrase`） |
| D | `apps/web/app/routes/tasks.$id.tsx`、`components/task-*.ts(x)`（头部、阶段、时间线、文档、改动、表态、备注与依赖）、`session-log*.ts(x)`、`server/task-history.ts` |
| E | 新路由与组件（`apps/web/app/routes.ts`、`routes/`、`components/`）；现有 `routes/projects.$id.settings.tsx`、`settings-data.ts`、`config-view.ts` 的去向；写入经 `server/gateway.ts` → `server/gates/write-gate.ts` |
| F | 新路由与组件；`open.path` 的面板服务端处理器（`apps/web/server/admin.ts` 一带） |
| G | `apps/web/app/routes/{env,projects.new,projects.$id.tasks.new}.tsx`、`lib/clock.tsx`；`apps/web/scripts/screenshots.ts`、`docs/images/*.png`、`README.md` 的 What it looks like 一节 |
| 测试 | `apps/web/test/`（含 `test-groups.ts` 登记）、`apps/web/e2e/`（用例与 `harness/`、夹具）、`apps/web/test/fixtures/` |
| 内核（有条件） | `crates/automed/src/dispatch.rs` 等：**只加只读数据**，且只有面板服务端从现有方法与事件确实算不出来时才加；为验-5 加内核测试 |

**不做（任务卡「不做什么」，逐条照录）。**

- 不换设计风格，不引入新的字体或 UI 框架，不做深色主题。
- 不改 Loop 流程、调度、门禁、协议文本；不升账本结构版本。
- 内核只允许加只读数据（例如总览「今天」的汇总、已完成列表需要的指标字段），而且只有面板服务端从现有方法与事件确实算不出来时
  才加；写操作只用现有的 `config.*` 方法。
- 不删任何现有功能：铃铛与通知、命令面板、排队调序与依赖、备注与重新打开、Backlog 表态、会话日志、实时更新、内核离线横幅、
  添加 / 移除项目、Onboarding，全部保留且入口可达。
- 不在面板里创建、编辑或移动技能文件。
- 不动 `.autome/`。Electron 薄壳不改（它加载的就是这个面板）。

**整理轮补充的边界。** 这些边界从调研得出，不扩大范围。标「整理轮理解」的是解释，设计轮可以据证据修正。

- 「不改协议文本」指两处都不改：仓库根 `protocol/`（编译进内核的 seed）与本任务的副本
  `docs/web-面板按动森风格重做/protocol/`。
- 整理轮理解：为验-5 新增内核**测试**（证明下一个会话用新配置）不算改内核行为，在范围内；内核产品代码的改动只限「加只读
  数据」。新增只读方法或回包字段时，§6 约束 5 列的钉子要同轮同步。
- 整理轮理解：G-3「重新生成截图」必然改 `docs/images/` 下的图片文件；除此之外不动 `docs/` 下别的任务目录与
  `docs/design/` 下的原型文件（原型是已确认的基准，不随实现修改）。
- 整理轮理解：`apps/desktop` 不改。`scripts/ci/steps.sh` 的 `Desktop` 步骤把 `apps/web` 列为输入，面板改动会让它重跑；它若因
  面板改动而红，先查是不是薄壳测试钉住了面板的某个旧形状，再决定在哪一侧修，并在设计或证据里写明。
- launchd 里的 `com.autome.loop`（驱动本仓库 Loop）与 `com.autome.web`（面板自启动，若用户装了）在仓库之外，会话不得读写、
  装卸、kickstart。

## 6. 硬性约束

1. **原型为准，演示数据不是需求。** 结构、组件、配色角色以 `docs/design/autome-web-panel-v3.html` 为准。原型里的任务、数字、
   模型名、技能名、项目名都是演示数据；页面上的每个数字都要来自账本或配置，不得写死。原型与任务卡字面不一致时以任务卡为准，
   并在设计里写明取舍。
2. **不换风格、不加依赖。** 不引入新的字体或 UI 框架，不做深色主题（内核有 `config.set_theme`，本任务不为它做界面）。预计
   不需要新依赖；要加须说明理由，改 `package.json` 必须同轮提交 `package-lock.json`。
3. **UI 硬规则（A-3）。** 全中文，专有名词保留原文；1512×944 下页面本身不出现纵向滚动，列与面板内部滚动且内部滚动区带
   `data-scroll-region`（`e2e/harness/browser.ts` 的 `oneScreen` 以此判定）；字体只从本地加载；圆角不小于 12px；不用纯黑；
   立体阴影只给主按钮。新增的可交互元素键盘可达，焦点环沿用黄色。
4. **现有样式守卫不得悄悄放松。** `apps/web/test/styles.test.ts` 现在钉住：`tokens.css` 的 `:root` 名值逐条一致；`app.css` 与
   TSX 里不出现十六进制色、`rgb()/rgba()/hsl()`、`black`；圆角只用 `--animal-radius*` 或 `50%`；`--animal-shadow-btn` 只出现在
   `.btn-primary` 一条规则里；源码没有 `Extended_Pictographic` 字符；TSX 没有 `style={`；外壳、看板、新建页的若干选择器的
   限高与内部滚动写法。`test/w9-layout.test.ts` 钉住添加页、设置页、报警区、环境页的选择器。原型为了单文件演示用了自己的变量名
   （`--blue`、`--bg2`…）、内联 `style`、散写颜色与字符图标，**这些写法不能原样搬进 `apps/web`**。界面改写使某条守卫不再适用时，
   同轮改写它，并说明它对应原来的哪一条、守住的性质有没有变弱（见 §7 总-Q2）。
5. **内核改动的边界与连带钉子。** 只加只读数据，写操作只用现有的 `config.set_role` / `config.reset_role` / `config.set_loop`
   （这三个已经在面板写白名单 `ALLOWED_WRITE_OPS` 里）。账本 schema 保持 9，协议保持 v9。若加只读方法：
   - `READ_METHODS`（`crates/automed/src/dispatch.rs`，现为 `[&str; 18]`）、`apps/web/server/gates/read-gate.ts` 的放行清单与
     `apps/web/test/read-gate.test.ts`、`write-gate.test.ts`（解析 `dispatch.rs` 核对）同轮同步；`scripts/ci/steps.sh` 的
     `Contract` 步骤 grep 这个常量名，名字不能改。
   - 改了已有回包的形状，grep 方法名 / 字段名命中的 `crates/*/tests/`、`apps/web/test/`、`crates/autome-cli/` 都要进验收子集。
   - `events.since` 继续不对浏览器放行（`WITHHELD_READ_METHODS`）；需要事件的汇总由面板服务端自己调内核。
6. **不删现有功能（验-8）。** 铃铛与通知、命令面板、排队调序与依赖、备注与重新打开、Backlog 表态、会话日志、实时更新、内核离线
   横幅、添加 / 移除项目、Onboarding，每项各有一条仍然通过的用例，入口在新外壳里可达。任务页现有的全部写操作
   （`apps/web/app/components/task-action.ts` 的 `WRITE_OPS`）同样保留。
7. **鉴权不变。** 新增的写接口沿用 `server/security.ts` 的 Host 白名单与 Origin 检查；服务只绑 `127.0.0.1`；不让浏览器给出任意
   路径（「打开目录」按技能名由服务端解析路径）。
8. **体积上限。** `apps/web/test/task-detail-ssr.test.ts`（60 个会话的任务详情首屏文档与单次数据响应各 ≤ 150 KB，含
   `?from=session-40` 窗口）继续通过；`board-payload.test.ts` 的项目页 ≤ 200 KB 同样继续通过。
9. **浏览器用例覆盖面不缩小。** `apps/web/e2e/` 现有 29 条（`layout` 12、`flows` 2、`live` 2、`controls` 6、`manual-items` 7）与
   一屏判据随界面改写；每个改写的用例说明对应原来的哪一条。用例标题进入门禁的用例清单（`scripts/e2e-catalog.mjs`），改名即
   视为一条新用例，对应关系写进证据。
10. **缺失不显示 0。** 旧任务的指标里没有门禁字段（`round_gate_runs` 等五项是 `Option`，T-100 之后才有），相关数字显示为缺失。
    同理：无计费的会话写「无计费」，不写 `$0`。
11. **不接触真实账本、真实面板与真实安装位置。** 本仓库正由已安装的 `~/.local/bin/autome`（launchd `com.autome.loop`）驱动自己
    的 Loop，账本在 `~/.autome`；用户还可能开着已安装的面板。因此：
    - 任何手工验证与测试都把 `AUTOME_HOME`（面板还要 `HOME` / `userHome`）指到临时目录。E、F 的写操作会改
      `$AUTOME_HOME/config.toml`，绝不能落到 `~/.autome/config.toml`。
    - `skills.list` 扫的是 `$HOME/.claude/skills`、`$HOME/.agents/skills` 与项目下的同名目录。测试用临时 `HOME` 与自建的技能
      目录，结果才是确定的；不得依赖本机真实装了哪些技能。
    - 拉真实内核的测试**显式**指定 `<检出>/target/debug/automed` 或 `target/debug/autome` + `['core']`；缺省查找会落到 PATH 上
      已安装的另一个版本。
    - 不假定 7420 空闲，不连接、不停用户正在用的面板；测试不真的打开浏览器窗口、Finder、Terminal.app 或系统对话框（现有注入点：
      打开器、终端、对话框）。
12. **项目范围的配置写入会改项目仓库里的 `.autome/config.toml`。** `config.set_role` / `config.reset_role` / `config.set_loop`
    带 `project_id` 时，内核写的是 `<项目路径>/.autome/config.toml`。所以测试账本里注册的项目只能是测试自己在临时目录里建的
    仓库；**不得**把本 worktree、主检出 `/Users/dannie/project/autome-v2` 或它的 `.worktree/` 下任何目录注册成测试账本里的项目——
    那会让内核改掉本仓库的 `.autome/config.toml`，违反 AGENTS.md。
13. **测试纪律。**
    - 新的 Web 测试文件必须出现在 `apps/web/test-groups.ts` 的恰好一个分组里（`unit` / `kernel` / `exclusive`；带内核、端口、
      进程、构建标记的不得进 `unit`，`test/test-groups.test.ts` 检查）。
    - 没有条件跳过：`test/core-sidecar.test.ts` 的守卫扫 `apps/web/test/*.ts` 与 `apps/web/e2e/`，出现
      `skip / skipIf / runIf / fixme / todo(` 就失败。Rust 侧不加 `#[ignore]`。
    - 不留存活进程：会起服务、内核、假 CLI、浏览器的测试，结束后用 `test/fixtures/process-scan.ts` 的
      `leftoverProcesses(root)` 断言为空；e2e 走 `e2e/harness/teardown.ts`。
    - 状态、列名、阶段名的中文字面量只能出自 `app/lib/labels.ts`（`labels.test.ts` 的守卫扫其它文件）；选择器集中在
      `app/lib/testids.ts`（`testids.test.ts` 逐页渲染核对）。
    - SSR 与水合一致：e2e 的控制台错误夹具对任何控制台错误判失败。相对时间（G-1、A-2、B、D）尤其要注意这一条。
14. **实时不回归。** 页面不手动刷新；事件到页面更新 ≤ 5 秒（生产 tick 间隔 3000 ms）；时间线新增条目时已展开的日志不折叠、
    滚动位置不丢（`manual-items.spec.ts` 的 H-03、C-2）。新页面在配置变化（内核事件 `config.changed`）后同样自动更新。
15. **沙箱拦住时如实记录。** 实现轮的沙箱可能拒绝 `listen(127.0.0.1)` 与 `ps`。遇到时：证据文件逐条列出因此没跑成的验收用例
    名与交给审计轮 / 宿主执行的完整命令；不改弱测试、不加跳过。`npm run lint`、`npm run typecheck`、`cargo fmt`、
    `cargo clippy` 是静态检查，不受影响，当场跑。
16. **时间预算。** 浏览器整套 `npm run e2e` 的声明上限是 720 秒（`e2e/harness/limits.ts`），29 条用例串行（`workers: 1`）；宿主
    快检的浏览器阶段按本轮改动选用例、上限同为 720 秒，快检整体上限 20 分钟；宿主全量门禁上限 30 分钟。本任务会新增页面与用例，
    设计要算这笔账（见 §7 总-Q4）。
17. **里程碑的写法。** 按 A–G 分组，A 排最前（其它组都用它的组件）；实现轮只推进最小编号的开放里程碑，编号顺序即执行顺序，
    前面里程碑的验收不得依赖后面才有的东西。每个里程碑有协议要求的三行验收契约（验收命令、预期、负向对照）。全量回归由合并前
    门禁承担（验-9），不要把 `sh scripts/ci-local.sh` 写成某个里程碑的验收命令，也不单设「全量回归」里程碑。验-1 里「与原型
    逐屏对照」需要肉眼的部分写进 `## 人工验收清单`，会话能跑的部分（关键元素存在、颜色角色的类名、一屏判据）进验收命令。
18. **文案语言沿用现状。** 界面、报错与日志用中文；代码注释与 `README.md` 用英文；设计文档用中文。
19. **设计文档体量。** 协议警告线 ≤ 80KB，单轮增长 ≤ 10KB。本任务七组合在一份文档里；证据写进 `evidence/`，`## 里程碑` 一节
    只放那一张表，对比表与说明放别处。
20. **会话边界（AGENTS.md）。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改 `.autome/`
    （`.autome/output/` 是输出目录，可以写）；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成
    本轮后结束会话，不自行启动下一个会话。

## 7. 待设计轮回答的问题

任务卡留白、或与代码现状有出入的地方。这里只列问题与相关事实，不预设答案。设计文档「方案」一节须逐条给出结论与依据。
没有依据支持改动任务卡字面的，按任务卡字面做。

### 总

- **总-Q1 里程碑怎么分。** 任务卡要求「按分组里程碑实现」，七组 A–G，没有给总数上限。实现预算 = 预算系数（现为 5）× 里程碑数。
  哪几组需要拆成多个（D、E 的内容最多），哪几组可以合并（例如 G-1 / G-2），顺序与依赖是什么。
- **总-Q2 原型的写法怎么落进现有样式守卫。** 事实见约束 4。要回答：
  - 原型的颜色、图案、丝带、数字块各自映射到哪些令牌；需要新增的令牌加在哪，`Layer 1 token 名值逐条一致` 这条测试怎么跟着改
    而不变成「什么都能加」。
  - 没有内联样式时，随数据变化的宽度与颜色怎么表达（进度条的百分比、项目色块、数字块的颜色变体）。
  - 现有被钉住的选择器（`.shell`、`.board-columns` 的 `repeat(4, minmax(0, 1fr))`、`.overview-column-body`、
    `.settings-card-body`、`.env-components` 等）哪些保留、哪些改名，对应的守卫怎么改写。
- **总-Q3 图标。** 原型用内联 SVG `<symbol>` 手绘线条图标，另有 `＋ − × ✓ ▾ →` 这类字符。现在的面板只有 `bell.tsx` 一处
  `<svg>`。`styles.test.ts` 禁止 `Extended_Pictographic` 字符（其中哪些字符会被拦，设计轮实测）。图标放在哪、怎么复用。
- **总-Q4 浏览器用例的时间账。** 现有 29 条串行、声明上限 720 秒；T-98 因浏览器用例的时间与预算连续六次合并前门禁失败（§9
  第 3、4 条）。新增 Loop 配置、技能两页与四种状态夹具之后，用例数、夹具准备时间与总时长各是多少，放不放得下；放不下时怎么
  取舍（合并用例、共用夹具、把一部分断言下沉到 vitest）。
- **总-Q5 路由。** 现有七条路由（`apps/web/app/routes.ts`）。Loop 配置与技能的 URL 形状是什么，范围（全局 / 某项目）与技能页的
  「切换项目」放在路径还是查询参数；现有 `/projects/:id/settings` 是保留、改成跳转还是并入新页面（E-1、E-7）；命令面板
  （`palette-model.ts`）与铃铛的跳转目标要不要跟着改。
- **总-Q6 新页面的实时更新。** 内核在配置写入后发 `config.changed`（主体是项目 ID 或 `global`）。面板的事件总线是否转发这类
  事件、`lib/live-routes.ts` 的 `byKind` 怎么登记新路由，设计轮核对后写明。

### A

- **A-Q1 侧栏的数据从哪来。** 根 loader `loadShell`（`components/shell-data.ts`）现在调 `project.list`、逐项目 `project.get`、
  `dashboard.get`。原型的侧栏多了：每个项目的「等你处理 / 进行中」计数与「N 个已完成」、技能总数、驱动状态卡。驱动信息现由
  环境页经 `server/drivers.ts`（`driver.list`，已在读白名单）取得。把这些放进每个页面都跑的根 loader，代价与刷新时机怎么定。
- **A-Q2 驱动状态卡的口径。** 「驱动是否在推进」与「上次检查的相对时间」各取 `driver.list` 的哪个字段（`driving`、
  `last_tick_at`）；面板自己的内核与 `autome run` 同时登记时显示哪一个；旧内核不支持 `driver.list` 时（`server/drivers.ts` 已有
  `unknown_method` 的回退）显示什么。
- **A-Q3 字体。** 现状：只打包了 Nunito（`app/styles/fonts/nunito/`，哈希被 `styles.test.ts` 钉住），中文落到系统字体
  （`--animal-font` 里的 `'Noto Sans SC'` 没有打包）。原型用的是同一份 Nunito。「字体离线可用」按现状理解即可，设计轮确认
  不需要动字体。

### B

- **B-Q1 「今天」四个数字与「最近合并」从哪算。** 事实：
  - `dashboard.get` 只回未终结的任务（`waiting` / `running` / `queued`）与项目列表；`project.get` 回项目的全部任务，每个任务带
    `cost_usd`、`cost_sessions_without_cost`、`completed_at`、`merge_commit`、`metrics`。
  - 任务的花费是整个任务的累计，不是按天的；会话级的花费在 `task.get` 的会话 `metrics` 里。
  - 门禁次数与等用户的时间只能从事件算（`gate.started` / `gate.result` / `gate.reused` / `gate.skipped`，事件有 `at`）。面板
    服务端可以调 `events.since`（带 `subject` 时按任务取；不带时是全账本分页）。
  - 任务卡的规矩：只有面板服务端从现有方法与事件确实算不出来时，才给内核加只读数据。

  要回答：四个数字各自的定义（「今天」按谁的时区切日；花费算今天的会话还是今天合并的任务；门禁算哪几种事件；等用户的时间怎么
  界定）、在哪算、算一次多贵、多久重算；「最近合并」列表的结果胶囊按什么规则取（原型里出现过「门禁一次过」「门禁 7 次」
  「退回 8 次」「无退回」）。
- **B-Q2 「等你处理」卡片里的设计摘要。** B-2 要「设计有几个里程碑、评审与裁决的结果、几条 Backlog 待表态」。现有数据：
  `dashboard.get` 每项带 `pending_decisions`；`task.get` 带状态块与文档清单。评审提了几项、裁决采纳几项，内核没有结构化字段。
  这句话从哪取、取不到时显示什么（不得编造）。失败任务、Bug 补充信息、预算用完等其它「等你处理」的种类各显示什么
  （`labels.ts` 的 `AWAIT_PHRASES`、`failureText` 是现有文案）。
- **B-Q3 「已经等了多久」「本轮已跑多久」。** 前者从进入等待状态的事件时间算；后者现有 `session.elapsed_minutes` 与
  `started_at`。相对时间在 SSR 与浏览器之间怎么保持一致（现有 `lib/clock.tsx`）。
- **B-Q4 四种状态的夹具（验-2）。** 空闲、在跑、等批准、失败。现有夹具：`test/fixtures/w5-states.sh`、`w7-states.sh`、
  `w7-flow.sh`（分步假 CLI）与 `e2e/harness/fixture.ts`。哪些能复用，哪些要新建。

### C

- **C-Q1 已完成列表的口径。** 实现轮数与退回次数取 `metrics.impl_rounds_used`、`metrics.reopen_total`，花费取 `cost_usd`。
  Bug 任务没有实现轮（原型显示「—」）；更早的任务可能没有 `metrics`。日期分组按 `completed_at` 与谁的时区；「本周」从周几
  算起；「累计花费与平均实现轮数」的分母含不含 Bug、已取消与没有指标的任务。
- **C-Q2 现有折叠与体积。** 现在「已完成」默认折叠、`?done=all` 展开，另有「已取消（N）」折叠区 `?cancelled=all`
  （`layout.spec.ts` 的「全部已完成」「已取消展开」两条、`board-cancelled.test.ts`、`board-payload.test.ts` 的 200 KB）。改成
  紧凑条目后这两个入口怎么处理，「更早 · 60 个」这类分组默认展开多少。
- **C-Q3 「门禁未跑过」的根因（C-3）。** 事实：`gatePhrase(detail?.gate, …)` 在 `gate` 为空时回「门禁未跑过」；`board-data.ts`
  只给 `needsTaskDetail` 的任务取 `task.get`，已合并任务没有 `detail`。设计轮确认根因，并定下已合并任务这一格显示什么
  （紧凑条目里可能根本不再有这一格），以及在途任务确实没跑过门禁时的文案是否保留。
- **C-Q4 调整顺序。** 原型的「待开始」列头写着「可拖动调整顺序」。现有做法是按钮（`queue-controls.tsx`，走 `task.move`）加依赖
  编辑。任务卡只要求调序功能保留；要不要做拖动、键盘怎么操作，设计轮定，做不到的部分把列头文案改成与实际一致。

### D

- **D-Q1 步骤条每一步的用时与花费。** 会话有 `started_at`、`ended_at`、`runtime`、`model`、`round`、`metrics`（花费，可能
  缺失）。「你批准」这一步等了多久从事件算。哪一类事件表示用户批准、驳回、表态（时间线模型现在消费 `task.updated`、
  `task.snapshot`），设计轮从内核代码确认。Bug 任务的步骤条是另一套阶段（`labels.ts` 的 `BUG_STAGES`）。
- **D-Q2 「一句话结果」从哪来。** 原型里每个会话有一句结果（例如「验收命令 29 + 2 个用例全过」）。内核的会话记录里没有这样的
  字段。任务目录的 `retro.md` 每轮有一行 `轮次 | 里程碑 | 结果 | 证据 | 阻塞`。取哪一个、取不到时留空还是显示别的，不得编造。
- **D-Q3 门禁行的内容。** `gate.result` 的载荷有 `ok`、`exit_code`、`kind`（`quick` / `full` / `repro`）、`node`，可能有
  `steps`、`passes`、`cases`、`digest`；事件里没有耗时字段，耗时由 `gate.started` 到对应 `gate.result` 的 `at` 算。「执行 N 步 ·
  沿用 M 步」取哪个字段（设计轮对真实载荷核对形状），旧事件没有时怎么显示；`gate.reused` / `gate.skipped` 各显示成什么。
- **D-Q4 六个数字块（D-2）。** 门禁的「通过数 / 总数，分快检与合并前」：已终结任务可取 `metrics` 的五个门禁字段（可能缺失），
  在途任务只能数事件。两种来源的口径要一致。「总用时（其中等用户多久）」的总用时从哪到哪。
- **D-Q5 筛选。** 「只看退回」指什么事件（审计退回里程碑、门禁失败、用户驳回设计）。筛选状态放不放进 URL，与现有的
  `?from=session-N` 窗口、`?doc=` 文档链接（`manual-items.spec.ts` 的 C-2b）怎么共存。
- **D-Q6 体积。** 新头部、步骤条、数字块与一行一个事件的时间线之后，60 个会话的任务仍要 ≤ 150 KB（约束 8）。现有的窗口加载
  （只取最近一段会话，往前按需加载）保留，步骤条与数字块需要的却是全量汇总，在服务端怎么算而不把全量会话送到浏览器。

### E

- **E-Q1 双模型纪律的对数与判定。** 任务卡 E-5 点了两对（评审↔设计、审计↔实现）。内核 `config::validate`
  （`crates/autome-domain/src/config.rs`）检查三对：`Role::same_model_counterpart` 是 评审→设计、审计→实现、**复盘→实现**；
  任一方被禁用时这一对免检；两边同工具且模型都留空时是另一种违规 `BothDefaultModels`（「都用 CLI 默认」）。页面的「当场标红
  并禁用保存」要与内核一致，否则会出现页面放行、内核拒绝，或页面拦住、内核其实允许。内核没有「试算一份未保存的配置」的只读
  方法（`config.validate` 只接受 `project_id`，校验的是已保存的配置）。页面侧的判定放在哪、怎么保证与内核不走样，设计轮定。
- **E-Q2 字段语义。** `config.set_role` 的参数：`role`、可选 `project_id`、`enabled`、`runtime`（`claude` / `codex`）、`model`
  （任意字符串，空串表示用 CLI 默认）、`effort`、`skills`（名字数组，面板写白名单限定不能是路径）。`effort`：键不出现 = 不改；
  `null` 或空串 = 「用 CLI 默认」；内核不限定取值，任务卡给的选项是 默认 / low / medium / high。项目范围下，出现的每个键都成为
  该角色的覆盖；**没有「只取消某一个字段的覆盖」的写法**，`config.reset_role` 是整个角色一起恢复，且只对项目有意义
  （`project_id` 必填）。`config.get` 给每个角色一个 `provenance`（`global` / `project`），粒度是角色，不是字段。
- **E-Q3 Loop 默认值（E-6）。** `config.set_loop` 的参数：`parallel`、`design_rounds`、`session_minutes`、`budget_factor`，可选
  `project_id`。内核的范围：并行 1–5，设计轮上限 ≥ 1，预算系数 ≥ 1，会话时长 ≥ 5 分钟；越界由内核拒绝。项目范围下键出现即
  写覆盖，值为 `null` 即恢复跟随全局。`config.get` 没有逐项给出这四个值的来源（`defaults` 只列「整块都是内置默认」的顶层键）；
  项目范围下要不要、怎么标出哪一项是项目自己的值，怎么恢复跟随全局，设计轮定。面板写白名单对 `config.set_loop` 没有专门的
  参数校验（只有通用的形状与长度检查）。
- **E-Q4 保存的粒度。** 原型同时有页头的「保存 1 项改动」与右侧抽屉里的「保存」。一次 `config.set_role` 只写一个角色。多处
  改动一起保存时的顺序与部分失败怎么处理；尤其是成对修改（例如同时换设计与评审的模型）时，中间态可能被双模型检查拒绝。
- **E-Q5 启用开关。** 卡片上的启用开关是直接写入还是进抽屉里一起保存。关掉审计、评审这类角色对 Loop 的影响内核已有处理，
  页面要不要给出说明。
- **E-Q6 现有项目设置页的去向（E-7）。** 现页面有三块：Onboarding（状态、会话日志、两个档案文件的编辑器、运行 / 完成 / 跳过）、
  只读配置（`config-view.ts` 的行、配置文件路径、`config.validate` 的违规）、移除项目（输入项目名确认）。放在哪，原有的
  `page-settings` 选择器、`settings-*.test.ts` 与 `layout.spec.ts` 的「项目设置」怎么对应过去。
- **E-Q7 「下一个启动的会话用新配置」（验-5）。** 内核在启动会话时解析配置（`scheduler.rs` 里 `config::resolve` 的调用点）。
  现有内核测试是否已经证明「改配置后启动的会话带新的工具 / 模型 / Effort / 技能」，没有就补一条；或者用面板夹具（假 CLI 记下
  自己收到的参数）证明。

### F

- **F-Q1 `skills.list` 给了什么。** 每个技能：`name`、`sources`（每个来源的工具、范围、路径）、`visible_to`、`project_scoped`、
  `bound_roles`、`conflicts`；不带 `project_id` 时只扫两个全局目录。**没有描述字段**——原型的技能卡片在名称下有一行描述，
  任务卡 F-1 列的字段里没有它。设计轮定：不显示，或另想只读的取法（面板不读写技能文件是 F-4 的边界，读 `SKILL.md` 算不算
  越界也要说清）。
- **F-Q2 绑定范围与内核的覆盖语义对不上的地方（F-2、验-7）。** 事实：
  - 项目配置里一个角色的 `skills` 是**整体替换**，不是在全局列表上追加（`config::resolve`：
    `over.skills.unwrap_or_else(|| base.skills.clone())`）。
  - 所以「仅当前项目」绑定 = 给这个项目的这个角色写一份完整的技能列表。此后这个角色在该项目里不再跟随全局技能列表的变化；
    验-7 的「按『全局』绑定后所有项目都出现」对已经有技能覆盖的项目不成立。
  - 写了技能覆盖，这个角色在 Loop 配置页就变成「项目覆盖」；「恢复跟随全局」会连同模型等其它覆盖一起清掉。
  - `skills.list` 的 `bound_roles` 来自解析后的结果，不区分这条绑定出自全局还是项目。要区分，可以比较 `config.get` 回包里的
    `global` 与 `resolved`，或比较带与不带 `project_id` 的两次 `skills.list`。
  - 写操作只能用现有的 `config.*`，不能为此改内核的写语义。

  要回答：两种范围在界面上各自对应哪些 `config.set_role` 调用；在项目范围取消一条全局绑定算什么；已有技能覆盖的项目遇到新的
  全局绑定时界面怎么说明；验-7 的断言在这些事实下怎么写才既符合任务卡又不说假话。确有做不到的，写明并进 Backlog 或争议项，
  不要悄悄改变验收的含义。
- **F-Q3 可见性与项目技能（F-3）。** 内核在保存时拒绝两种情况：技能对角色所用的工具不可见（`SkillNotVisible`）、技能不存在
  （`SkillNotFound`——全局范围下绑定一个项目技能就是这种，因为全局只扫全局目录）。页面的禁用与说明要与这两条一致。角色的工具
  在全局与项目里可能不同（项目可以覆盖 `runtime`），所以「哪些角色能勾」随范围而变。
- **F-Q4 「打开目录」（F-4）。** `open.path` 在面板写白名单里，`kind` 允许 `skill`，且按约定只传名字不传路径；但面板服务端
  **没有注册它的处理器**（`server/admin.ts` 只注册了 `env.install`、`env.login`、`project.confirm`），现在调用会得到
  「网页端还不支持」。需要补一个处理器：路径由服务端从内核的 `skills.list` 回答里解析；一个技能可能有多个来源目录；测试里
  打开器必须是注入的替身。
- **F-Q5 数量与项目切换。** 用户本机有一百多个全局技能（原型写 126）。卡片网格在内部滚动区里渲染；筛选、搜索在浏览器端还是
  服务端做；「已绑定」「本项目」的计数随所选项目变化；没有任何项目时页面显示什么。

### G

- **G-Q1 环境页。** 现有数据：`env.get` 的组件快照（`env-model.ts`）与驱动列表（`server/drivers.ts`）。原型把「内核」也列成一行
  （路径、账本版本、构建标识）——这些现在从哪取（`/api/instance`、内核启动日志的构建标识）。安装 / 登录 / 复制命令等现有操作
  （`env-ops.test.ts`、`manual-items.spec.ts` 的 W9 H-04）保留在哪。
- **G-Q2 两个表单页。** 新建任务页有草稿、类型单选、附件上传、依赖；添加项目页有路径补全、原生目录选择（薄壳里才有）、工作区
  成员、跳过 Onboarding。按 A 组组件重排时，哪些控件换成分段选择 / 勾选块 / 输入框，表单的提交字段名与校验不变。
- **G-Q3 README 截图（G-3）。** `apps/web/test/readme-images.test.ts` 钉住 README 恰好引用三张图（`dashboard.png`、
  `project.png`、`task.png`，各 1512×944）；截图脚本是手动步骤，不在门禁里，而且要监听端口与 `ps`（T-98 的 M-07 因此用了三个
  实现轮，§9 第 2 条）。要回答：还是这三张，还是加上 Loop 配置与技能（那就要同轮改测试与 README 文字）；脚本的夹具要不要
  换成能显示彩色卡片的状态；由哪一轮来跑它，跑不成时怎么交接。

## 8. 代码现状（整理轮调研，基线 `04e857d3`；行号为约数，设计轮引用前自行复核）

**原型 `docs/design/autome-web-panel-v3.html`**（625 行，69 KB）。结构：7–276 行是样式，280–289 行是图标 `<symbol>`，
293–308 行侧栏，311–317 行顶栏，六个 `<section class="screen">`：`s-overview`（319 行起）、`s-project`（382）、`s-task`（443）、
`s-env`（520）、`s-loop`（538）、`s-skills`（582）；607 行是演示状态切换条，609 行起是切屏脚本。主要类名：`.ribbon`
（`.sm`、`.yellow`、`.blue`、`.brown`、`.green`）、`.card`（`.pat` 图案，`.c-yellow / .c-teal / .c-purple / .c-blue / .c-green /
.c-red / .c-dash`）、`.pill`（`.f` 实心与 `.f-*` 颜色）、`.tile`（`.t-*` 数字块）、`.prog`、`.btn`（`.pri`、`.sm`、`.text`）、
`.seg`、`.sw`、`.stepper`、`.inp`、`.chk`、`.steps` / `.stp`（步骤条，`.you` 是用户那一步）、`.ev`（执行过程的行，`.gate`、
`.you`）、`.rc`（角色卡片，`.fixed`、`.human`）、`.drawer`、`.sk`（技能卡片）、`.lane`、`.li` / `.dl`（紧凑条目）。它引用的
字体就是 `apps/web/app/styles/fonts/nunito/Nunito-Variable.ttf`，没有外部资源。

**面板前端 `apps/web/app`。** 文件行数少但行很长（多数文件一行一个函数）。

- 路由七条（`routes.ts`）：`/`、`/env`、`/projects/new`、`/projects/:id`、`/projects/:id/settings`、
  `/projects/:id/tasks/new`、`/tasks/:id`。
- 样式：`styles/tokens.css`（`--animal-*` 令牌与 `--tile-*` 十一种应用色，`@font-face` 只有 Nunito）、`styles/app.css`
  （234 行，20 KB）。
- 外壳：`components/shell.tsx`（侧栏：品牌、总览、项目列表、页脚的「添加项目」「环境」；顶栏：「搜索或跳转」、新建任务卡、
  铃铛；横幅区）、`project-list.tsx`、`sidebar-footer.tsx`、`nav-links.ts`、`bell*.ts(x)`、`command-palette.tsx`、
  `palette-model.ts`、`shell-data.ts`（`loadShell`、`overviewTasks`）。
- 总览：`routes/_index.tsx`（两列「等你处理」「进行中」，空时一句话）、`overview-card.tsx`。
- 项目看板：`routes/projects.$id.tsx`、`board-data.ts`、`task-card.tsx`、`task-card-model.ts`、`task-columns.ts`（列
  `todo / running / needs_you / done`）、`queue-controls.tsx`、`queue-deps.ts`、`project-alerts.tsx`。
- 任务详情：`routes/tasks.$id.tsx`、`task-detail-data.ts`、`task-header*.ts(x)`、`task-stage*.ts(x)`、
  `task-timeline-model.ts`（170 行；已消费 `session.*`、`gate.queued / started / result / reused / skipped`、`merge.*`、
  `task.updated / snapshot / protocol_failure / processes_*`）、`task-timeline.tsx`、`task-docs*.ts(x)`、
  `task-changes*.ts(x)`、`task-decisions*.ts(x)`、`task-notes.tsx`、`task-dependencies.tsx`、`task-action.ts`、
  `task-ops.ts`、`session-log*.ts(x)`。
- 设置：`routes/projects.$id.settings.tsx`（Onboarding、只读配置、移除项目）、`settings-data.ts`（`loadSettings` 已经调
  `config.get` 与 `config.validate`）、`config-view.ts`。
- 其它：`routes/env.tsx`、`env-model.ts`、`routes/projects.new.tsx`、`routes/projects.$id.tasks.new.tsx`；文案
  `lib/labels.ts`（`gatePhrase` 在约 66–69 行）、`lib/admin-labels.ts`、`lib/guard-labels.ts`；时间 `lib/clock.tsx`；实时
  `lib/live-client.ts`、`lib/live.tsx`、`lib/live-routes.ts`；选择器 `lib/testids.ts`（29 个）。

**面板服务端 `apps/web/server`。** `gateway.ts`（`read` / `write` 两个入口；`localOps` 六个本地操作，其中 `open.path`、
`open.terminal`、`project.pick` 没有注册处理器）、`gates/read-gate.ts`（放行 16 个只读方法，含 `driver.list`、`config.get`、
`config.validate`、`skills.list`；保留 `events.since`、`env.detect`）、`gates/write-gate.ts`（`ALLOWED_WRITE_OPS` 含
`config.set_role`、`config.reset_role`、`config.set_loop`、`config.set_theme`；`ROLES` 六个；`config.set_role` /
`config.reset_role` 校验 `role` 与 `runtime`，`skills` 是只许名字的数组）、`security.ts`（Host 与 Origin 检查）、
`task-history.ts`（按任务分页读 `events.since`）、`task-docs.ts`、`drivers.ts`、`admin.ts`、`opener.ts`、`eventbus*.ts`、
`ticker.ts`、`notifier*.ts`、`store.ts`（面板自己的 `web.sqlite3`）。

**内核（只读为主）。** `crates/automed/src/dispatch.rs`（7952 行）：`config_get`（约 1251 行，回 `global`、`resolved`、
`defaults`、`violations`、`scope`）、`config_validate`（1287）、`config_set_role`（1308）、`config_reset_role`（1415）、
`config_set_loop`（1435）、`skills_list`（3271）、`dashboard_get`（3012）、`task_json_with_config`（约 693 行，任务 JSON 的
字段）、`session_json`（2308）、`READ_METHODS`（3387，18 项）。`crates/automed/src/config_io.rs`：全局配置
`<AUTOME_HOME>/config.toml`，项目配置 `<项目>/.autome/config.toml`，`skill_bindings`（911）。`crates/automed/src/skills.rs`：
只扫一层、从不失败、从不写。`crates/autome-domain/src/config.rs`：`RoleConfig`、`RoleOverrides`、`LoopDefaults`、
`Provenance`、`resolve`（501）、`ConfigViolation`（577）、`validate`（659）；`role.rs` 的 `same_model_counterpart`（95）；
`metrics.rs` 的 `TaskMetrics`（127，含五个 `Option` 的门禁字段）。

**旧 Electron 页面（只作参考）。** `git show 'e80e9dc1^:apps/desktop/renderer/screens/routing.js'`（732 行）、`skills.js`
（311 行）、`settings.js`（264 行）；同目录还有 `style.css`（旧的动森组件样式）。

**测试。** `apps/web/test/`：vitest 5，按 `test-groups.ts` 分 `unit` / `kernel` / `exclusive` 三组。与本任务直接相关的守卫与
既有测试：`styles.test.ts`、`w9-layout.test.ts`、`labels.test.ts`、`testids.test.ts`、`shell*.test.ts`、`overview.test.ts`、
`board*.test.ts`、`task-card.test.ts`、`task-columns.test.ts`、`task-timeline*.test.ts`、`task-stage.test.ts`、
`task-header.test.ts`、`task-detail-*.test.ts`、`settings-*.test.ts`、`env-*.test.ts`、`add-project*.test.ts`、
`task-new*.test.ts`、`palette.test.ts`、`bell-*.test.ts`、`read-gate.test.ts`、`write-gate.test.ts`、
`readme-images.test.ts`。`apps/web/e2e/`：`layout.spec.ts`（12 条一屏巡检）、`flows.spec.ts`（D-1、D-2）、`live.spec.ts`
（D-3、D-4）、`controls.spec.ts`（B-1…B-4、BL-01）、`manual-items.spec.ts`（W7 H-03 / H-04、W9 H-01 / H-02 / H-04、C-2、C-2b）；
`harness/` 用真实内核、假 CLI、临时根目录；`playwright.config.ts` 视口 1512×944、`workers: 1`。

**门禁。** `scripts/ci/steps.sh` 的步骤：`Format`、`Clippy`、`Test`、`Desktop`、`Contract`、`Scripts`、`Web lint`、
`Web typecheck`、`Web build`、`Web test`、`Web e2e`，以及准备步骤 `Desktop install`、`Build core`、`Web install`；每个步骤声明
输入目录，输入没变的沿用上次结果；`docs`、`.autome`、`.claude`、`AGENTS.md`、`README.md` 不触发门禁。快检 `scripts/ci-quick.sh`
跑静态检查、与本轮改动相关的 vitest 文件（`apps/web/scripts/quick-related.mjs`）与按改动选出的浏览器用例。

**本机现状（整理时）。** Node v26.5.0；Playwright 的 `chromium-1248` 已装；worktree 里没有 `apps/web/node_modules` 与
`target/`（第一次要装依赖与 cargo 构建）。整理轮没有运行任何测试，没有起面板，也没有对真实内核做实测；§7 里要「核对」「实测」
的都留给设计轮。

## 9. 前车之鉴（来自已合并任务的复盘，逐条可核对）

1. **实现轮多半不能监听端口、不能 `ps`。** T-98 的 29 份实现证据里有 24 份记了 EPERM，浏览器用例与截图脚本在实现轮基本跑
   不起来。对策：让每个里程碑都有一部分验收不依赖监听（模型层纯函数、`renderToStaticMarkup` 的组件测试、替身内核）；跑不成的
   用例名与完整命令列进证据交给审计轮。
2. **README 截图别指望实现轮。** T-98 的 M-07 用了三个实现轮，交付的图最后是从审计轮的输出目录复制来的。G-3 要提前安排由谁跑。
3. **浏览器用例的时间要算账。** T-98 连续六次合并前门禁失败，都出在浏览器用例上，其中四次是超时与预算：整套超过全局上限
   （两次）、夹具准备与单例轮询各自的预算不够。改一个预算之前，把「用例数 × 单例耗时 + 夹具准备」与总上限对一遍。
4. **门禁失败后不要只修眼前那一条。** 同一次失败报告里的其它症状（未执行的用例、非用例错误）一并查清，否则每修一条往返一次。
5. **实现轮结束前原样跑一次宿主快检命令。** T-99 有三个实现轮跑的静态检查比宿主快检窄（只对改动文件跑 eslint、只跑
   `clippy --lib`），宿主的 typecheck / clippy 随即退回，各多花一轮。
   快检命令在简报里给出（`sh scripts/ci-quick.sh --since <本轮起点>`）。
6. **改了模块，跑引用它的全部测试。** T-96、T-97 只跑验收契约列的文件，各打破了几条早已存在的测试。改 `apps/web/app` 的轮次跑
   `npx vitest related --run <改动文件…>` 或 `--project unit` 全量。本任务几乎每个里程碑都会碰到 `styles.test.ts`、
   `labels.test.ts`、`testids.test.ts` 这三个全站守卫——T-98 里 `labels.test.ts` 的中文字面量守卫重复失败三次，
   `styles.test.ts` 的内联样式守卫失败一次。
7. **★ 验收命令在定稿前跑一遍。** T-99 的 M-06 验收命令本身有缺陷（`--repeat-each` 共用夹具根），到审计才暴露。设计轮写下的
   浏览器验收命令，能跑就先跑一次；没跑的在裁决记录里标明。
8. **负向对照照原文注入并核实会变红。** T-95、T-98 都有按契约注入后并不变红的负向对照。需要浏览器的负向对照尽早固化成仓库里
   一条能直接执行的脚本或用例。
9. **审计不要用「负载」「计时脆弱」放行失败。** T-99：被放行的计时失败后来原样复现。要么给出确定性的复现条件，要么按观测
   失败率 p 取 n 使 (1-p)^n ≤ 0.05。
10. **实现证据里交给审计的缺口，审计逐条接手。** T-100 的 M-03：实现轮写明「全脚本回归未取得终结结果，交审计复验」，审计没跑
    却写了「无验证缺口」。
11. **测试替身要与真实接口对齐，不是与调用方对齐。** E、F 的组件测试如果手写 `config.get` / `skills.list` 的回包，形状要取自
    真实内核的回答（现有做法：`test/fixtures/w7/` 下的 JSON 是从真实内核抓的原始回包，见该目录的 README；
    `test/fixtures/m02-config-get.json` 是 `settings-config.test.ts` 用的 `config.get` 回包，来源由设计轮核对），并至少有一条
    拉真实内核的测试。
12. **改了 RPC 回包形状，grep 全部使用方。** T-95 四个里程碑各自过审，合并前门禁在它们改过的回包上红了 7 个。
13. **已关闭里程碑的「最新证据」指针不要动；rebase 之后跑受影响包的全量。** 本任务里程碑多、周期长，中途 rebase 的概率高。
14. **改了 `.rs` 就 `cargo fmt --all`；断言子进程输出时固定 `NO_COLOR` / `FORCE_COLOR`。**

## 10. 项目背景摘要（生成于 2026-11-17）

**这是什么。** Autome 2.0 是一个驱动 Loop 的本地工具：

- 需求走「整理 → 设计 → 评审 / 裁决 → 用户批准 → 实现 → 审计 → 复盘 → rebase → 合并前门禁 → 合并 → 清理」的 Loop。
- Bug 另有「定位 → 复现检查 → 修复 → 修复门禁 → 复核」的 Loop。

运作方式：

- 内核持有全部状态（SQLite 账本 + 事件流），以 stdio 上带长度前缀的 JSON-RPC 服务面板。命令行 `autome` 直接链接内核库，
  `autome core` 就是面板用的内核；多个驱动（`autome run`、面板的内核）靠驱动锁轮流推进。
- 面板（`autome web`）是内核状态的展示与控制面：Node 服务 + 浏览器页面，只绑 `127.0.0.1`，鉴权只有 Host / Origin 检查。
  Electron 薄壳加载的就是这个面板。
- 配置分两层：全局 `~/.autome/config.toml`（完整），项目 `<项目>/.autome/config.toml`（只写覆盖项，随仓库提交）。六个角色
  （`plan / review / adjudicate / impl / audit / retro`）各有工具、模型、Effort、技能列表与启用开关。技能是
  `.claude/skills/<名>/SKILL.md`（Claude Code 看得到）或 `.agents/skills/<名>/SKILL.md`（Codex 看得到），全局在用户主目录下，
  项目的在仓库里；内核只扫描，从不写。

本仓库用 Autome 自己开发自己，`.autome/config.toml` 的现值：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`、
`run = "before_merge"`、`timeout_minutes = 30`；每轮快检有 `scripts/ci-quick.sh` 就跑它，`quick_timeout_minutes = 20`，
给会话的快检命令是 `sh scripts/ci-quick.sh --since {round_start}`；`[merge] auto = true`（过了合并前门禁自动合并，设计批准仍要
人）；`[retry] attempts = 2`。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1，含 rustfmt / clippy），三个 crate：
  - `crates/autome-domain`：纯域（状态机、配置形状与校验、角色、技能清单的形状、指标类型、队列规则），不做 I/O。
  - `crates/automed`：内核（调度、JSON-RPC 分发、账本、门禁、会话启动与提示词、配置读写、技能扫描、Git、内置协议）。
  - `crates/autome-cli`：`autome` 命令行。
- `apps/web`：Node（≥ 22.22）服务（`server/`，TypeScript 由 Node 原生类型剥离直接运行）+ React Router 8 框架模式的面板
  （`app/`，SSR，React 19）；vitest 5、Playwright 1.64.1、eslint 10、TypeScript 6。没有 UI 框架，样式是两份手写 CSS。
- `apps/desktop`：Electron 薄壳，`node --test`。本任务不改。
- `protocol/`：内置协议 seed（v9，编译进内核；本任务不改）。`plugins/`：Claude Code / Codex / Cursor 插件。
- `scripts/`：门禁（`ci-local.sh`、`ci-quick.sh`、`ci/`、`test/`）、安装与打包。
- `.github/workflows/ci.yml`：本仓库无远端，从未真正执行；步骤清单被 `apps/web/test/ci-config.test.ts` 钉住。
- 文档：`README.md`（英文）、`docs/development/{requirements,technical-design}.md`、`docs/plans/`、`docs/adr/`、
  `docs/design/`（原型与成品稿，HTML）、`docs/images/`（README 的三张截图）；每个任务的文档在 `docs/<slug>/`。前几个面板任务的
  文档（`docs/w5-…`、`docs/w6-…`、`docs/w7-…`、`docs/w9-…`、`docs/面板收尾批次-…`）都很大，按节读，不要整份读。
- 状态位置：`$AUTOME_HOME`（缺省 `~/.autome`）下 `config.toml`、`state/automed.sqlite3`（账本）、`state/web.sqlite3`
  （面板库）、`web/`（`server.json`、日志、上传）。

**测试与检查命令（在 worktree 根目录运行）。**

- Web：
  - 新 worktree 先 `sh scripts/ci/install-node.sh apps/web`。
  - `npm run lint|typecheck|build|test --prefix apps/web`。
  - 单个文件 `npm run test --prefix apps/web -- test/<文件>`；单个分组 `(cd apps/web && npx vitest run --project unit)`；
    相关测试 `(cd apps/web && npx vitest related --run <源文件…>)`。
  - 拉真实内核的测试先 `cargo build -p automed -p autome-cli`。
- 浏览器端到端：`npm run build --prefix apps/web && npm run e2e --prefix apps/web`（缺浏览器 / 缺构建 / 缺内核时 preflight 会
  报出来）；单个文件 `(cd apps/web && npx playwright test e2e/<文件>)`；只列用例 `npx playwright test --list`。
- 截图：`npm run screenshots --prefix apps/web`（手动步骤，写 `docs/images/`；`--out <目录>` 可改输出位置）。
- Rust：`cargo test --workspace --all-targets --no-fail-fast`；单个 crate `cargo test -p automed --lib`、
  `cargo test -p autome-domain --lib`；内核端到端 `cargo test -p automed --test end_to_end <用例名>`；
  `cargo fmt --all -- --check`；`cargo clippy --all-targets -- -D warnings`。
- 门禁脚本自身的测试：`node --test scripts/test/*.test.mjs`。Desktop：`npm test --prefix apps/desktop`。
- 整套本地 CI（即宿主全量门禁命令）：`sh scripts/ci-local.sh`（`--plan` 只看哪些步骤会跑）。门禁由宿主在合并前跑一次；实现轮 /
  审计轮跑里程碑验收命令与直接相关的测试；宿主在实现轮后跑快检。
- 输出长的命令重定向到 `.autome/output/`，只读头尾，例如
  `npm run test --prefix apps/web > .autome/output/web.txt 2>&1; tail -20 .autome/output/web.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）规定：
  - 不改 `.autome/` 下任何内容；
  - 只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、不推送远端；
  - 完成本轮后结束会话，不自行启动下一个会话；
  - 超过 5 行的命令输出与临时验证产物写入 `.autome/output/`。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/web-面板按动森风格重做/protocol/`（见文首指路）。

**环境注意。** 运行环境是 macOS（Darwin），系统 `sh` 是 bash 3.2。

- `/tmp` 是 `/private/tmp` 的符号链接。
- macOS 对改名的二进制副本可能 SIGKILL。需要第二个名字时用符号链接，并先实测。
- launchd 起的进程只有最小 PATH。
