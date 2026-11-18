# 任务：面板观感收尾——把已上线的页面对齐已确认原型的密度与控件样式

本任务的规则见 `docs/面板观感收尾/protocol/loop-protocol.md` 与
`docs/面板观感收尾/protocol/session-protocol.md`。

- 设计文档：`docs/面板观感收尾/面板观感收尾.md`
- 任务卡（用户写的，第一输入）：`docs/面板观感收尾/面板观感收尾-card.md`
- 设计原型（用户 2026-11-17 确认，密度与控件样式的基准）：`docs/design/autome-web-panel-v3.html`
- 附件（改前截图，真实账本，1512×944）：`docs/面板观感收尾/attachments/` 下十张 PNG，清单见 §2
- 任务整理日期：2026-11-18；任务编号 T-102
- 仓库：单个仓库 `autome-v2`（工作目录即本任务的 worktree，分支 `autome/面板观感收尾`）；整理时的基线提交 `27d38e2b`
- 本任务固定的协议版本：`protocol/v9`

## 1. 需求原文

> 面板观感收尾：把已上线的页面对齐已确认原型的密度与控件样式

任务卡正文比这句话具体得多；两者不一致时以任务卡为准。§2–§5 照任务卡整理（编号沿用任务卡的 A-1…E-4、验-1…验-9），
§6–§7 是整理轮从调研得出的约束与待定问题，§8 是代码现状，§9 是上一个面板任务留下的教训，§10 是项目背景。
标「整理轮理解」的句子是解释，不是用户原话，设计轮可以据证据修正。

## 2. 背景与目标

T-101（合并提交 `4f9d74b2`）按动森风格重做了 Web 面板，功能齐全：总览、项目看板、任务详情、Loop 配置（全局 / 项目、六个角色、
四项默认值）、技能绑定、环境页都能用。用户 2026-11-18 在真实账本（71 个已完成任务、126 个技能）上逐页截图，对照已确认的原型
`docs/design/autome-web-panel-v3.html`，发现一批观感没做到原型水平。

**任务卡指出的两个共同根因——先修根因，不要逐页打补丁。**

1. **分段选择只认一种写法。** `apps/web/app/styles/app.css` 里 `.seg` 的样式只覆盖 `label > input[type=radio] + span`。Loop 配置页
   的范围切换和「Loop 配置 / 项目信息」标签用的是 `.seg` 里放链接，结果是一排没有间隔、看不出选中项的文字。
2. **表单控件没有全站基础样式。** 输入框、下拉框、搜索框、复选框、`fieldset` 只有在个别页面单独写了样式的地方才是动森观感，
   其余露出浏览器原生外观（技能页的项目下拉与搜索框、角色抽屉的模型输入框、技能抽屉的复选框与 `fieldset` 边框、移除项目的
   确认输入框）。

**第三个背景：实现轮跑不了浏览器。** T-101 的复盘（`docs/web-面板按动森风格重做/lessons.md` L-01、L-04）指出，实现轮的沙箱里
不能执行 `ps`、不能监听端口，浏览器用例和截图在实现轮跑不起来。所以本任务的验收尽量写成不依赖浏览器也能判的断言（§6 约束 5）。

**附件（改前，真实账本）。** 都在 `docs/面板观感收尾/attachments/`：

| 文件 | 画面 | 对应要求 |
|---|---|---|
| `loop.png` | Loop 配置（全局）：范围切换是一排无间隔文字；角色卡每张约 290px 高；「审计」卡的 medium 掉到下一行 | A-1、B-1、B-2 |
| `loop-drawer.png` | 项目范围 + 角色抽屉：通栏「关闭」大按钮、原生模型输入框、「绑定的技能」「添加技能」是裸文字 | A-2、A-4、B-4 |
| `loop-bottom.png` | Loop 配置滚到底：四项默认值，第一张卡标签与步进器同行、其余三张分两行；保存按钮各叫各的 | B-2、B-5 |
| `loop-projinfo.png` | 「项目信息」标签页：「配置（只读）」只露一行，「移除项目」输入框被底边切掉一半且是原生外观 | A-2、B-6 |
| `skills.png` | 技能页：原生下拉与搜索框；一屏 3 张卡，描述不截断 | A-2、C-1、C-2、C-3 |
| `skills-click.png` | 技能抽屉：原生复选框与 `fieldset` 边框、「来源」被挤到屏幕外 | A-2、A-4、C-4 |
| `task.png` | 任务详情（T-101，44 个会话）：标题右侧空的圆角框、里程碑只露两个、时间线一屏 4 行、文档胶囊下半截被盖住 | D-1…D-6 |
| `env.png` | 环境页：丝带字号过大；四个工具都正常时仍把安装与登录说明全部展开 | A-3、E-1 |
| `project-new.png` | 添加项目页：丝带成了通栏色条；候选列表只露三行半 | A-3、E-2 |
| `project.png` | 项目看板：「已完成」列底部「已取消」分组被页面底边裁掉 | E-3 |

**目标。** 一次设计、一次评审，按 A–E 分组里程碑实现：先补全站基础（A：分段选择、表单控件、丝带字号、抽屉骨架），再收紧
Loop 配置（B）、技能（C）、任务详情（D），最后是环境页、添加项目页、项目看板与总览「等你的时间」的缺陷（E）。只改 `apps/web`，
不改内核。

## 3. 任务要求（任务卡「要做什么」逐条照录；设计文档的里程碑必须全部覆盖）

### 3.1 A 全站基础

- **A-1** 分段选择支持两种内容：单选框（现有）与链接 / 按钮；选中项有白底与阴影，未选中项之间有间隔。范围切换、页内标签、
  筛选都用它。
- **A-2** 表单控件的全站基础样式：文本输入框、搜索框、下拉框、多行文本、复选框、单选框、`fieldset` / `legend`。圆角不小于
  12px，用现有令牌，聚焦用现有的黄色描边。之后任何页面不单独写也不会露出原生外观。
- **A-3** 丝带标题只保留两档字号（页面标题一档、区块标题一档）。现在「新建任务卡」「添加项目」「环境检查」三页的丝带字号约为
  其它页的两倍；「添加项目」页的丝带还被拉成了通栏色条。
- **A-4** 抽屉的统一骨架：标题行右侧一个小的关闭按钮（图标或「关闭」），内容区内部滚动，底部固定操作区。现在角色抽屉顶上是
  一个通栏的大「关闭」按钮，技能抽屉的「来源」被挤到屏幕外。

### 3.2 B Loop 配置

- **B-1** 范围切换显示成分段选择，当前范围高亮；项目范围下第二行的「Loop 配置 / 项目信息」同样。
- **B-2** 角色卡收紧到原型的密度：1512×944 下，设计循环、实现循环两排角色卡加四项默认值在不滚动的情况下全部可见（现在每张卡约
  290px 高，默认值在屏幕外）。模型名与 effort 胶囊同一行不换行（现在「审计」卡的 medium 掉到了下一行）。
- **B-3** 角色卡要看得出可以点开编辑（悬停态与一个「编辑」提示），现在整张卡是按钮但没有任何提示。
- **B-4** 角色抽屉：模型输入框用 A-2 的样式；「绑定的技能」列出已绑定技能的名称，没有时写「还没有绑定技能」；「添加技能」是
  一个明确的次级按钮，跳到技能页并带上当前范围。
- **B-5** 四项默认值：四张卡同一种布局（现在第一张的标签和步进器在同一行，其余三张分两行）；保存按钮统一叫「保存」，值没改时
  不显示或置灰；保存后的结果提示就地出现。
- **B-6** 「项目信息」标签页：三个区块在页面内部滚动，不被裁切（现在「配置（只读）」只露出一行，「移除项目」的输入框被页面
  底边切掉一半）。

### 3.3 C 技能

- **C-1** 技能卡固定高度：描述最多三行，超出省略；路径一行，超出从中间或末尾省略。1512×944 下一屏至少能看到 9 张卡（现在
  3 张）。完整描述放进抽屉。
- **C-2** 顶部的项目选择与搜索框用 A-2 的样式；项目选择用分段选择或与 Loop 配置页一致的范围切换。
- **C-3** 卡片上的绑定状态用胶囊表达：未绑定是描边胶囊，已绑定写出角色名（如「审计 · 复盘」）的实心胶囊。
- **C-4** 技能抽屉：角色勾选用 A-2 的勾选块；看不到该技能的角色整行置灰并在同一行内说明原因；「保存绑定」固定在底部；「来源」
  可见。

### 3.4 D 任务详情

- **D-1** 时间线改成紧凑行：每行一条（时间、图标或角色、一句话、右侧耗时 / 花费 / 结果胶囊），行高不超过 44px，会话行可展开。
  1512×944 下时间线区域至少同时看到 10 行（现在 4 行，每条系统事件占一张大卡片）。
- **D-2** 「显示更早的 N 个会话」做成按钮样式。
- **D-3** 没有可用操作时不渲染操作区（现在已完成任务的标题右侧有一个空的圆角框）。
- **D-4** 里程碑条：全部里程碑在一行内可横向滚动，或折成「13 / 13 已完成」加可展开的列表；不能只露出前两个而没有任何还有更多的
  提示。
- **D-5** 右侧文档标签页里的文档胶囊不被裁切（现在下半截被盖住），「证据（38）」的展开入口与胶囊同一种样式。
- **D-6** 阶段条里「评审·裁决」一格的文字「$0.65 + 1 个无计费」不换行溢出。

### 3.5 E 其余页面

- **E-1** 环境页：工具已安装且已登录时只显示一行（名称、路径、状态、版本），安装与登录说明收进「查看安装方法」的展开区；未安装
  或未登录时才直接展开。
- **E-2** 添加项目页：目录候选列表占满卡片可用高度并内部滚动（现在只露出三行半，页面下方大片空白）。
- **E-3** 项目看板：「已完成」列的内部滚动区延伸到底，底部的「已取消」分组不被页面底边裁掉。
- **E-4** 总览「等你的时间」算错了（**这条是缺陷，不只是观感**）。2026-11-18 04:52「等你处理」为 0，数字块却显示「4 时 52 分 ·
  今天暂无结束的等待」，正好是当天已过去的时长。原因：`apps/web/server/overview-summary.ts` 从 `events.since` 累计所有任务的
  等待区间，不看任务是否还在面板里。真实账本里 T-7 属于 2026-10-11 已移除的项目，最后一条 `task.updated` 停在 `failed`，于是
  被当成一直在等用户，每天从零点计到现在。修法：只统计当前任务列表里存在的任务（已移除项目的任务不计），并补一条用例：
  已移除项目里停在 `failed` 的任务不计入等待时间。今天确实没有等待时数字显示「0 分」并写「今天没有等待」。

## 4. 验收标准（任务卡原文）

- **验-1** Loop 配置页：范围切换与页内标签有可见的选中态；1512×944 下两排角色卡与四项默认值无需滚动全部可见；六张角色卡的
  模型与 effort 都在同一行。
- **验-2** 角色抽屉与技能抽屉：没有任何浏览器原生外观的控件；关闭按钮在标题行；底部操作区始终可见。
- **验-3** 技能页：真实 126 个技能下一屏至少 9 张卡，卡片等高；最长描述（`agent-browser`）被截到三行。
- **验-4** 任务详情：T-101 这样 44 个会话的任务，时间线区域一屏至少 10 行；已完成任务没有空的操作框；文档胶囊完整可见；
  13 个里程碑都能看到或有明确的展开入口。
- **验-5** 环境页：四个工具都正常时，整页不需要内部滚动就能看完。
- **验-5b** 总览：真实账本上没有任务等用户时，「等你的时间」不随当天时间增长；已移除项目的失败任务不计入。
- **验-6** 添加项目页、新建任务页、环境页的丝带与总览页同一档字号；添加项目页没有通栏色条。
- **验-7** 全站检索：除被 A-2 基础样式覆盖的控件外，页面里不存在未带样式的 `select`、`input`、`textarea`、`fieldset`；布局巡检
  用例覆盖本卡提到的每个页面与两个抽屉。
- **验-8** `sh scripts/ci-local.sh` 通过；e2e 用例数不少于合并前。
- **验-9** 人工验收清单里给出每个页面改前改后的对照截图路径，用户看图确认。

## 5. 范围

**在范围内（按任务卡「参考」与调研得出，具体落点以设计为准）。**

| 项 | 可能改的地方 |
|---|---|
| A | `apps/web/app/styles/app.css`（必要时 `tokens.css`，新令牌用 `--ui-` 前缀）、`apps/web/app/components/ui/`（`segmented.tsx`、`input.tsx`、`check.tsx`、`ribbon.tsx` 与可能新增的抽屉骨架） |
| B | `apps/web/app/routes/loop.tsx`、`components/{loop-defaults.tsx,loop-defaults-data.ts,loop-data.ts,settings-page.tsx}`、`lib/loop-labels.ts` |
| C | `apps/web/app/routes/skills.tsx`、`components/skills-data.ts`、`lib/skills-model.ts` |
| D | `apps/web/app/routes/tasks.$id.tsx`、`components/{task-timeline.tsx,task-timeline-model.ts,task-header.tsx,task-ops-view.tsx,task-stage.tsx,task-summary.tsx,task-docs.tsx,session-log.tsx}` |
| E | `apps/web/app/routes/{env,projects.new,projects.$id,projects.$id.tasks.new,_index}.tsx`、`components/env-model.ts`、`lib/overview-labels.ts`、`lib/overview-model.ts`、`lib/waiting.ts`、`apps/web/server/overview-summary.ts`（E-4） |
| 测试 | `apps/web/test/`（新文件要登记进 `apps/web/test-groups.ts`）、`apps/web/e2e/`（用例与 `harness/`、夹具）、`apps/web/test/fixtures/`、`apps/web/scripts/screenshots.ts` |
| 文档 | 本任务目录 `docs/面板观感收尾/`；`docs/images/` 下 README 的五张截图（见下面「整理轮补充的边界」） |

**不做（任务卡「不做什么」，逐条照录）。**

- 不改信息结构、不加新页面、不动内核与协议。
- 不改 T-101 已定的颜色令牌与组件命名；需要新令牌时沿用 `--ui-` 前缀。
- 不做深色主题，不做窄屏适配。
- 不重拍 README 截图以外的文档。

**整理轮补充的边界。** 从调研得出，不扩大范围。

- 「只改 `apps/web`，不改内核」：`crates/`、仓库根 `protocol/`、本任务的协议副本 `docs/面板观感收尾/protocol/`、`apps/desktop`、
  `plugins/`、`scripts/` 都不改；账本结构与 RPC 回包形状不变。E-4 的修法在面板服务端（`apps/web/server/`）内完成，不向内核加
  方法或字段。
- 整理轮理解：「不重拍 README 截图以外的文档」的意思是 `docs/images/` 的五张 README 截图可以随界面变化重新生成，其它文档
  （`README.md` 正文、`docs/development/`、`docs/design/` 下的原型）不动。原型是已确认的基准，不随实现修改。是否重拍、由谁重拍
  由设计轮决定（§7 总-Q4）。
- 整理轮理解：「不改信息结构」指页面有哪些区块、区块里有哪些信息、有哪些路由与入口都不变。D-1（时间线改成紧凑行）、D-4
  （里程碑折叠）、E-1（安装说明收进展开区）改的是同一批信息的排布与默认展开状态，不增删信息；任何一条信息改完之后都还能在
  原来的页面上看到。
- 整理轮理解：「不改 T-101 已定的组件命名」指 `.card`、`.ribbon`、`.pill`、`.tile`、`.btn`、`.seg`、`.switch`、`.stepper`、`.inp`、
  `.chk` 这些共用组件的类名与 `components/ui/` 下组件的名字不改；页面级的类名与结构为了收紧密度可以改，但要遵守 §6 约束 4。
- launchd 里的 `com.autome.loop`（驱动本仓库 Loop）与 `com.autome.web`（面板自启动）在仓库之外，会话不得读写、装卸、kickstart。

## 6. 硬性约束

前五条来自任务卡「约束」，其余来自调研与上一任务仍然生效的约束。

1. **UI 硬规则不变。** 全中文（专有名词除外）；1512×944 下不出现整页纵向滚动，列与面板内部滚动且内部滚动区带
   `data-scroll-region`（`apps/web/e2e/harness/browser.ts` 的 `oneScreen` 以此判定）；圆角不小于 12px；不用纯黑；立体阴影只用于
   主按钮。新增或改动的可交互元素键盘可达，焦点环沿用黄色。
2. **三样东西不减少。** Host / Origin 鉴权不变；任务详情 150 KB 的服务端渲染上限测试（`apps/web/test/task-detail-ssr.test.ts`，
   60 个会话的首屏文档与单次数据响应各 ≤ 150 KB）继续通过——D-1 改时间线的标记结构时尤其要看这条；现有 e2e 用例数不减少
   （整理时按源码数是 33 条：`layout` 14、`controls` 8、`flows` 2、`live` 2、`manual-items` 7；这个数没有用 `npx playwright test
   --list` 核对过，设计轮先核对并把基线数写进设计）。
3. **改旧值要同轮改完断言（T-101 教训 L-03）。** 改掉任何已有的用户可见文案、类名或结构时，同一轮把 `apps/web/test` 与
   `apps/web/e2e` 里期待旧值的断言一起改完，并在证据里写出检索命令与命中数。`apps/web/scripts/`（截图脚本、`quick-related.mjs`）
   里也有引用类名的地方，一并检索。
4. **现有样式守卫不得悄悄放松。** `apps/web/test/styles.test.ts` 钉住：`tokens.css` 的令牌名值逐条一致、`app.css` 的每个 `var`
   都有定义、每个 `--ui-` 令牌都被引用；`app.css` 与 TSX 里不散写颜色（十六进制、`rgb()/rgba()/hsl()`、`black`）；圆角只用
   `--animal-radius*` 或 `50%`；焦点环全是黄色；`--animal-shadow-btn` 只出现在 `.btn-primary`；源码没有 emoji 图标；TSX 没有
   `style={`；SVG 只出现在图标与进度条组件；外壳、看板、新建页若干选择器的限高与内部滚动写法。`test/w9-layout.test.ts` 钉住
   添加页、设置页、报警区、环境页的选择器；`test/ui-kit.test.ts` 钉住共用组件的渲染形状。原型为了单文件演示用了自己的变量名
   （`--blue`、`--bg2`…）、内联 `style`、散写颜色，**这些写法不能原样搬进 `apps/web`**。界面改写使某条守卫不再适用时，同轮
   改写它，并说明它对应原来的哪一条、守住的性质有没有变弱。
5. **验收优先不依赖浏览器。** 能对服务端渲染出的 HTML 与样式表下断言的，就写成 vitest 用例（例如「`.seg` 内的链接有选中态
   规则」「技能卡描述带行数限制的类」「没有操作时不输出操作区节点」）。必须用浏览器才能判的（一屏可见行数、是否裁切）写进
   e2e 的布局用例，由宿主门禁与审计轮执行。
6. **截图与布局用例先查脏文本（T-101 教训 L-02）。** 截图脚本与布局用例在判定前断言页面可见文本不含字面量 `undefined`、
   `NaN`、`null`、`[object Object]`。现状：`scripts/screenshots.ts` 只对 `shell-main` 断言不含 `undefined`，`e2e/layout.spec.ts`
   没有这类断言。
7. **原型为准，演示数据不是需求。** 密度、控件样式以 `docs/design/autome-web-panel-v3.html` 为准；原型里的任务、数字、模型名、
   技能名都是演示数据。原型与任务卡字面不一致时以任务卡为准，并在设计里写明取舍。
8. **不换风格、不加依赖。** 不引入新的字体或 UI 框架。预计不需要新依赖；要加须说明理由，改 `package.json` 必须同轮提交
   `package-lock.json`。
9. **不删现有功能。** 铃铛与通知、命令面板、排队调序与依赖、备注与重新打开、Backlog 表态、会话日志、实时更新、内核离线横幅、
   添加 / 移除项目、Onboarding、Loop 配置与技能绑定的全部写操作，入口都保留且可达。D-3 只是不渲染**空的**操作区，有可用操作
   时照常显示。
10. **实时不回归。** 页面不手动刷新；时间线新增条目时已展开的会话日志不折叠、滚动位置不丢（`e2e/manual-items.spec.ts` 的
    H-03、C-2）；SSR 与水合一致（e2e 的控制台错误夹具对任何控制台错误判失败）。D-1 改时间线行结构、E-1 改默认展开状态时都要
    过这一条。
11. **不接触真实账本、真实面板与真实安装位置。** 本仓库正由已安装的 `~/.local/bin/autome`（launchd `com.autome.loop`）驱动
    自己的 Loop，账本在 `~/.autome`；用户还可能开着已安装的面板。因此：
    - 任何手工验证与测试都把 `AUTOME_HOME`（面板还要 `HOME`）指到临时目录。验-3、验-4、验-5b 说的「真实 126 个技能」「T-101
      这样 44 个会话的任务」「真实账本」是用户改前观察到的规模，**会话不得为了取证去读写 `~/.autome` 或本机真实技能目录**；
      用夹具造出同等规模（见 §7 总-Q1），真实账本上的最后确认进人工验收清单。
    - `skills.list` 扫的是 `$HOME/.claude/skills`、`$HOME/.agents/skills` 与项目下的同名目录；测试用临时 `HOME` 与自建的技能
      目录，结果才是确定的。
    - 拉真实内核的测试显式指定 `<检出>/target/debug/automed` 或 `target/debug/autome` + `['core']`；缺省查找会落到 PATH 上已
      安装的另一个版本。
    - 不假定 7420 空闲，不连接、不停用户正在用的面板；测试不真的打开浏览器窗口、Finder、Terminal.app 或系统对话框。
    - 测试账本里注册的项目只能是测试自己在临时目录里建的仓库；不得把本 worktree、主检出 `/Users/dannie/project/autome-v2` 或它
      的 `.worktree/` 下任何目录注册成测试账本里的项目（项目范围的配置写入会改那个仓库的 `.autome/config.toml`）。
12. **测试纪律。**
    - 新的 Web 测试文件必须出现在 `apps/web/test-groups.ts` 的恰好一个分组里（`unit` / `kernel` / `exclusive`；带内核、端口、
      进程、构建标记的不得进 `unit`，`test/test-groups.test.ts` 检查）。
    - 没有条件跳过：`test/core-sidecar.test.ts` 的守卫扫 `apps/web/test/*.ts` 与 `apps/web/e2e/`，出现
      `skip / skipIf / runIf / fixme / todo(` 就失败。
    - 不留存活进程：会起服务、内核、假 CLI、浏览器的测试，结束后用 `test/fixtures/process-scan.ts` 的 `leftoverProcesses(root)`
      断言为空；e2e 走 `e2e/harness/teardown.ts`。
    - 状态、列名、阶段名的中文字面量只能出自 `app/lib/labels.ts`（`labels.test.ts` 的守卫扫其它文件）；选择器集中在
      `app/lib/testids.ts`（`testids.test.ts` 逐页渲染核对）。
    - 测试替身的形状取自真实内核的回答，不是取自调用方的期待（现有做法：`test/fixtures/w7/` 下是从真实内核抓的原始回包）。
13. **沙箱拦住时如实记录。** 实现轮的沙箱可能拒绝 `listen(127.0.0.1)` 与 `ps`。遇到时：证据文件逐条列出因此没跑成的验收用例名
    与交给审计轮 / 宿主执行的完整命令；不改弱测试、不加跳过。`npm run lint`、`npm run typecheck` 是静态检查，不受影响，当场跑。
14. **时间预算。** 浏览器整套 `npm run e2e` 的声明上限是 720 秒（`e2e/harness/limits.ts`），用例串行（`workers: 1`）；宿主快检的
    浏览器阶段按本轮改动选用例、上限同为 720 秒，快检整体上限 20 分钟；宿主全量门禁上限 30 分钟。验-7 要求布局巡检覆盖每个
    页面与两个抽屉，会新增用例，设计要算这笔账（§7 总-Q3）。
15. **里程碑的写法。** 按 A–E 分组，A 排最前（其它组都用它）；实现轮只推进最小编号的开放里程碑，编号顺序即执行顺序，前面
    里程碑的验收不得依赖后面才有的东西。每个里程碑有协议要求的三行验收契约（验收命令、预期、负向对照），验收命令必须是实现
    轮沙箱里能跑的（不监听端口、不 `ps`）。全量回归由合并前门禁承担（验-8），不要把 `sh scripts/ci-local.sh` 写成某个里程碑的
    验收命令，也不单设「全量回归」里程碑。需要肉眼或浏览器截图才能确认的（验-9、与原型逐屏对照）写进 `## 人工验收清单`；
    凡须由要监听端口的命令生成并提交进仓库的产物，生成与目视检查进人工验收清单，里程碑只验生成脚本与其断言（T-101 教训 L-01）。
16. **文案语言沿用现状。** 界面、报错与日志用中文；代码注释与 `README.md` 用英文；设计文档用中文。
17. **设计文档体量。** 协议警告线 ≤ 80KB，单轮增长 ≤ 10KB。证据写进 `evidence/`；`## 里程碑` 一节只放那一张表，对比表与说明
    放别处。
18. **会话边界（AGENTS.md）。** 只在本 worktree 内工作；不切分支、不动 `.worktree/` 下其它目录、不推送远端、不修改 `.autome/`
    （`.autome/output/` 是输出目录，可以写）；超过 5 行的命令输出与临时验证产物写进 `.autome/output/`，不进任务文档目录；完成
    本轮后结束会话，不自行启动下一个会话。

## 7. 待设计轮回答的问题

整理轮只提问，不给答案。每条都要在设计文档「方案」里有明确回答。

### 总

- **总-Q1 规模夹具。** 验-3（126 个技能、`agent-browser` 那样约 20 行的描述、两条来源路径）、验-4（44 个会话、13 个里程碑、
  38 份证据的已合并任务）、验-5（四个工具都正常）、验-5b（已移除项目里停在 `failed` 的任务）要用夹具复现。现有 e2e 夹具
  （`e2e/harness/fixture.ts`、`test/fixtures/w7-flow.sh`）与 `layout.spec.ts` 里「技能」用例临时造的 24 个技能够不够？不够的话
  夹具怎么造、造在哪一层（单元用例的静态 JSON，还是 e2e 的真实内核夹具），对 720 秒预算的影响多大？
- **总-Q2 守卫的改写清单。** 列出本任务会碰到的现有守卫与形状测试（`styles.test.ts`、`w9-layout.test.ts`、`ui-kit.test.ts`、
  `testids.test.ts`、`labels.test.ts`、各页的 `*-page.test.ts` / `*-view.test.ts`），逐条写明保留、改写还是新增，改写的守住的
  性质有没有变弱。
- **总-Q3 浏览器用例的账。** 先用 `npx playwright test --list` 核对现有用例数；再算新增布局用例（两个抽屉、Loop 配置项目信息
  标签页、时间线行数、技能卡数等）之后整套的预计耗时，与 720 秒上限对一遍。哪些断言并进现有用例、哪些必须新开用例？
- **总-Q4 改前改后对照图（验-9）。** 改前图就是 `attachments/` 下十张（真实账本）。改后图由谁、用什么命令、基于什么数据生成，
  存在哪里？约束：实现轮沙箱跑不了浏览器；临时产物按规则进 `.autome/output/`（被 `.gitignore` 忽略，不随分支走）；本项目
  `[merge] auto = true`，过了合并前门禁就自动合并，设计批准之后没有别的停顿点。`docs/images/` 的五张 README 截图是否随本任务
  重新生成（`test/readme-images.test.ts` 钉住五张的文件名与尺寸）？T-101 留下的人工项 H-09（重拍五图）、H-10（布局 14 条）
  与本任务的关系要说清。
- **总-Q5 分组与里程碑。** A–E 五组、24 条要求拆成几个里程碑、顺序如何；E-4 是行为缺陷，与观感项是否分开成独立里程碑。

### A

- **A-Q1** 链接 / 按钮形态的选中态用什么标记（现状：范围切换的链接带 `aria-current="page"`；看板筛选是
  `.board-filters button[aria-pressed="true"]`；时间线筛选是 `.timeline-filters [aria-current="page"]`；任务详情右侧标签页与技能
  筛选另有写法）。任务卡说「范围切换、页内标签、筛选都用它」——逐个列出全站现有的分段式导航与筛选，哪些迁到 `.seg`。
  `components/ui/segmented.tsx` 已有 `SegmentedTabs`（一个带 `.seg` 的 `nav`），与此的关系。
- **A-Q2** 基础样式挂在元素选择器上还是类上，才能做到「之后任何页面不单独写也不会露出原生外观」？它与已有的专用规则
  （`.seg input`、`.switch input` 把原生控件藏起来；`.stepper input`、`.palette input`、`.chk input`、`.add-project input…`、
  `.task-new textarea`、`.onboarding-editor textarea`）以及 `type=hidden` / `type=file` 的关系；特异性怎么安排才不回归。
  下拉框的箭头、复选框与单选框的勾，在「不散写颜色、SVG 只出现在图标与进度条组件」的守卫下怎么画。
- **A-Q3** 验-7 的「全站检索」怎么写成会话能跑的断言（扫 TSX 源码、扫各页服务端渲染出的 HTML，还是两者都要），「被 A-2 基础
  样式覆盖」的判据是什么。
- **A-Q4** 丝带两档各是多少。整理轮观察到的成因：`env.tsx`、`projects.new.tsx`、`projects.$id.tasks.new.tsx` 把 `<h1>` 放在
  `Ribbon` 里，只有 `.overview-heading h1` 写了字号（22px），其余三页的 `h1` 用的是浏览器默认字号；设计轮核对，并查清添加项目
  页通栏色条的成因。两档定在组件上还是定在各页的选择器上。
- **A-Q5** 抽屉骨架是否抽成共用组件。现状两个抽屉都是 `position: fixed; inset: 64px 16px 16px auto` 的 `aside role="dialog"
  aria-modal="false"`：角色抽屉整体滚动，技能抽屉内层滚动。固定底部操作区里放哪些按钮（角色抽屉有「保存」「恢复跟随全局」，
  技能抽屉有「保存绑定」），结果提示放哪里；键盘焦点与 Esc 的行为是否保持现状。

### B

- **B-Q1** 角色卡的目标高度与卡内各行的取舍。现状每张卡依次是：角色名、工具、模型、effort 胶囊、「N 个绑定技能」、（项目范围）
  「项目覆盖 / 跟随全局」、启用开关、（停用时）一句说明；项目范围比全局多一行。算出 1512×944 下两排卡 + 两个小标题 + 四项
  默认值 + 页头（项目范围还多一行页签与说明）的高度预算。模型名很长时怎样保证与 effort 同行（截断规则）。
- **B-Q2** 「编辑」提示的形态；现状只有角色名那个 `button` 可点，整张卡不是按钮——点击区域是否扩大到整卡，卡内的启用开关怎么
  避免嵌套交互元素。
- **B-Q3** B-5「值没改时不显示或置灰」二选一；保存按钮改名「保存」之后四个同名按钮在测试与读屏里怎么区分；项目范围下每张卡
  还有「恢复跟随全局 · 字段名」按钮与「与全局相同 / 项目值」一行，四张卡同一布局时它们放哪里。
- **B-Q4** B-4「添加技能」跳走时，角色抽屉里未保存的草稿怎么办（现状是直接 `Link` 跳转，草稿丢失）。任务卡没有要求改这个行为，
  设计轮确认保持现状还是需要提示，不要扩大范围。
- **B-Q5** B-6 的三个区块（Onboarding、配置（只读）、移除项目）现在是 `.project-settings` 下三张各自 `flex: 1 1 0` 的卡；改成
  页面内部滚动后，`w9-layout.test.ts` 的「设置页限高与卡片/编辑器内部滚动」守卫怎么改写。`/projects/:id/settings` 现在只是一个 302 跳转到
  `/loop?project=<id>&tab=project` 的路由，`layout.spec.ts` 的「项目设置」用例走的就是这条跳转。

### C

- **C-Q1** 卡片固定高度是多少，列数是否保持 3 列，才能一屏 ≥ 9 张；「路径一行，从中间或末尾省略」二选一（中间省略纯 CSS 做
  不到时的做法）；一个技能有两条来源路径时显示哪条。
- **C-Q2** 项目选择用分段选择还是与 Loop 配置页一致的范围切换；项目很多时怎么不撑破页头。它与下面那排「全部 / 全局 / 本项目 /
  已绑定」筛选是两个分段选择，怎么区分层级。
- **C-Q3** 已绑定胶囊写角色名：绑了五六个角色时怎么不撑破卡片；胶囊颜色用哪一种；项目范围下「跟随全局的绑定」与「项目自己的
  绑定」是否要区分（任务卡没要求，默认不区分）。
- **C-Q4** 技能抽屉的角色行改用 `Check` 组件（`components/ui/check.tsx`，现在把禁用原因放在勾选块下方的 `small` 里）；任务卡要
  「整行置灰并在同一行内说明原因」——改 `Check` 组件本身还是只改这里的用法，`ui-kit.test.ts` 的「勾选块禁用原因可追溯」怎么
  跟着改。完整描述放进抽屉的哪个位置。

### D

- **D-Q1** 紧凑行的列与各类条目（会话、门禁、用户动作、系统事件、合并步骤）的一句话取自 `TimelineEntry` 的哪些字段；现在每条
  有标题、摘要、明细三段，压成一行之后其余内容去哪（展开后可见，还是悬停提示）。`task-timeline-model.ts` 的条目数据是否要改
  形状，还是只改渲染。失败原因的 `details`、未核实标记（`timeline-unverified`）在紧凑行里怎么表达。
- **D-Q2** 会话行展开后放 `SessionLog`；正在运行的会话现在默认展开（`defaultOpen={entry.session.running}`），保持。展开 / 折叠、
  新条目到达、切换筛选时的滚动位置与已展开状态（约束 10）。44px 的行高上限对展开后的行不适用——在设计里写明判据只量折叠态。
- **D-Q3** D-3 的空操作框从哪来：`TaskHeader` 的 `children` 是 `.task-ops` 容器，`app.css` 给它固定了边框与内边距；「没有可用
  操作」的判据由哪一层给出（`task-ops.ts` 的模型，还是渲染结果为空），已完成任务的「重新打开」「备注」这类操作算不算可用
  操作。
- **D-Q4** D-4 二选一：横向滚动，还是「13 / 13 已完成」加展开列表；进行中的任务当前里程碑怎么突出；横向滚动区要不要
  `data-scroll-region`；折叠状态在实时更新后是否保持。
- **D-Q5** D-5 的裁切成因（现状 `.task-docs nav` 有 `max-height: 30%; overflow-y: auto`，文档胶囊多时换行）；「证据（38）」现在是
  `details` / `summary`，改成与胶囊同一种样式后展开的证据列表放哪。
- **D-Q6** D-6 阶段条一格的宽度有限：是缩短文案、允许省略并给完整提示，还是调整格子布局；同一处理要覆盖所有格子里可能出现的
  最长文案（列出来）。

### E

- **E-Q1** 环境页「正常」的判据（已安装且已登录；不需要登录的工具只看已安装）；展开区用 `details` 还是受控状态，重新检测后
  展开状态是否保持；验-5 的「整页不需要内部滚动」在下方还有「驱动」区块时的高度预算。`env-page.test.ts`、`env-real.test.ts`、
  `w9-layout.test.ts` 的环境页守卫怎么改。
- **E-Q2** 添加项目页候选列表现在是 `.path-suggestions { max-height: 160px }`；占满卡片可用高度后，检查结果、成员列表、报错
  出现时怎么分配高度（`styles.test.ts` / `w9-layout.test.ts` 钉着「添加页限高与补全/成员内部滚动」）。
- **E-Q3** 看板「已完成」列被裁的成因（列体 `.board-column-body` 已是内部滚动，「更早 · N 个」链接与「已取消」分组在它之内
  还是之外），其余三列是否有同样的问题。
- **E-Q4** E-4 的实现边界。现状 `createOverviewSummary` 返回的函数收到了 `tasks`（当前任务列表），但 `open`（进行中的等待）与
  `closed`（已结束的等待）都不对照它。要回答：`open` 与 `closed` 是否都按当前任务列表过滤；返回值里的 `waiting`（每个任务的
  等待起点）是否同样过滤；任务后来被移除、项目被重新添加时累计值怎么变；`lib/waiting.ts` 的 `waitingKind` 把 `failed` 与停在
  `await_design_approval / await_merge / await_info / not_a_bug` 四个节点的 `active` 都算作「等用户」，这四种在已移除项目的任务上
  是否有同类漏洞（一次处理同类问题）。显示侧：没有等待时数字写「0 分」、附注写「今天没有等待」——现在的附注
  「今天暂无结束的等待」在「有进行中的等待但今天还没有结束的」情形下怎么写。

## 8. 代码现状（整理轮调研，基线 `27d38e2b`；行号为约数，设计轮引用前自行复核）

整理轮只读了源码与截图，**没有安装依赖、没有运行任何测试、没有起面板**；下面每一条都是读源码得到的。

**原型 `docs/design/autome-web-panel-v3.html`**（625 行，69 KB，单文件）。7–276 行是样式；六个 `<section class="screen">`：
`s-overview`（319 行起）、`s-project`（382）、`s-task`（443）、`s-env`（520）、`s-loop`（538）、`s-skills`（582）。与本任务直接
相关的原型样式：`.ribbon` 与 `.ribbon.sm`（75–80 行，17px / 13px 两档）、`.seg`（181–183，`button.on` 白底加阴影；251 行
`.seg.wide`）、`.ev`（185–195，时间线行：五列网格、`padding: 8px 14px`、一行文字加省略）、`.rc`（223–236，角色卡：
`padding: 12px 14px`、`gap: 5px`、模型名单行省略）、`.stepper`（244–246）、`.drawer`（247–248）、`.inp`（250）、`.sk`（258–264，
技能卡：名称与描述各一行省略、胶囊 20px 高）、`.chk`（267–271，勾选块与 `.dis` 置灰）、`.li` / `.dl`（129–154，紧凑条目）。
注意任务卡对技能卡的要求（描述三行）比原型（一行）宽，以任务卡为准。

**样式 `apps/web/app/styles/`。** `tokens.css` 82 行；`app.css` 485 行、42 KB，行很长（一条规则一行）。

- 共用组件（238–324 行）：`.card*`、`.ribbon*`（253–267：`.ribbon { font-size: 17px }`、`.ribbon-sm { font-size: 13px }`）、
  `.pill*`、`.tile*`、`.prog`、`.btn*`、`.seg`（300–306：只有 `.seg label`、`.seg input`、`.seg span`、
  `.seg input:checked + span` 等规则，**没有针对 `.seg a` 或 `.seg button` 的规则**）、`.switch`、`.stepper`（312–314）、
  `.inp`（315–316）、`.chk`（317–320）。
- 没有元素级的 `input` / `select` / `textarea` / `fieldset` / `legend` 基础规则；表单样式散在页面选择器下：`.palette input`
  （64）、`.task-new fieldset / textarea`（92–93）、`.task-ops fieldset / textarea`（144–146）、`.task-decisions`（166–168）、
  `.add-project input…, .add-project select`（178）、`.onboarding-editor textarea`（190、194）、`.task-notes`（224–225）、
  `.queue-controls`（232–233）。
- 页面：外壳与看板（59–81、391–401）、任务详情（107–162、404–426；`.task-ops` 142 行带边框与内边距，
  `.task-detail-header > .task-ops` 406 行占网格第二列；`.task-docs nav` 130、161）、添加项目（174–185；
  `.path-suggestions { max-height: 160px }` 180）、设置（187–195）、环境（206–214、465–481）、总览（362 起）、Loop 配置
  （430–442：`.loop-row` 五列网格、`.loop-row .card { padding: 18px }`、`.loop-drawer` 437、`.loop-defaults` 四列）、技能
  （444–462：`.skills-grid` 三列、`.skill-card { padding: 20px; gap: 12px }`、`.skill-description` 不限行数、`.skills-drawer`）。

**共用组件 `apps/web/app/components/ui/`**（每个文件 4–16 行）：`button`、`card`、`check`、`input`、`pill`、`progress`、
`ribbon`、`segmented`（`Segmented`：`div.seg[role=radiogroup]` 里放 `label > input[type=radio] + span`；`SegmentedTabs`：
`nav.seg`，子元素由调用方给）、`stepper`、`switch`、`tile`。

**页面与组件 `apps/web/app`。** 路由九条（`routes.ts`）：`/`、`/loop`、`/skills`、`/env`、`/projects/new`、`/projects/:id`、
`/projects/:id/settings`、`/projects/:id/tasks/new`、`/tasks/:id`。

- `routes/loop.tsx`（72 行）：页头是 `Ribbon` + `nav.seg[aria-label=配置范围]`（一排 `Link`，当前项带 `aria-current="page"`）；
  项目范围下第二个 `nav.seg[aria-label=项目配置页签]`；带 `?tab=project` 时渲染 `SettingsView`（`components/settings-page.tsx`）。
  角色卡是 `Card.loop-role`，卡内只有角色名包在 `button` 里；effort 是 `<p className="pill">`。抽屉是
  `aside.loop-drawer[role=dialog]`：第一个子元素就是 `<button>关闭</button>`，模型是裸 `<input>`（没用 `Input` 组件），已绑定
  技能渲染成「名称 ×」的裸按钮，「添加技能」是裸 `Link`（已带 `?project=`）。
- `components/loop-defaults.tsx`（24 行）：四张 `Card.loop-default-field`，保存按钮文案是「保存」+ 字段名，值没改时已经
  `disabled`；保存成功就地显示「已保存」。
- `routes/skills.tsx`（68 行）：页头是 `Ribbon` + 裸 `<select>`（项目）+ 裸 `<input type="search">`；筛选用 `Segmented`；卡片
  `data-testid=skill-card`；抽屉 `aside.skills-drawer`，角色勾选是 `fieldset > label.skill-role > input[type=checkbox]`，禁用原因
  在同一 `label` 的 `small` 里，「保存绑定」是内容流里的 `button.btn-primary`，「来源」在它之后。
- `routes/tasks.$id.tsx` 与 `components/task-*.tsx`：`TaskTimeline`（`task-timeline.tsx` 37 行）每条是 `li`，内含 `h3`（标题）、
  `p`（摘要）、`p.timeline-meta`（明细）、失败 `details`、会话的 `SessionLog`；「显示更早的 N 个会话」是裸 `Link`；筛选是
  `nav.timeline-filters`。`TaskHeader` 把 `children`（操作区）放在标题上方的网格第二列。`TaskStage` 的里程碑是
  `ul.task-milestones`，每个里程碑一个 `li`（胶囊 + 标题）。`task-timeline-model.ts` 217 行。
- `routes/env.tsx`（82 行）、`routes/projects.new.tsx`（93 行）、`routes/projects.$id.tasks.new.tsx`（80 行）：三页都是
  `<Ribbon><h1>…</h1></Ribbon>`；`.env-page h1`、`.add-project h1`、`.task-new h1` 只写了 `margin: 0`。总览 `_index.tsx` 同样的
  写法，但有 `.overview-heading h1 { font-size: 22px }`。
- `routes/_index.tsx` 47 行附近：`<Tile label="等你的时间" value={formatSpan(waitingMilliseconds(today, now))}>`，附注是
  `endedWaiting(today.ended) || '今天暂无结束的等待'`。
- 文案与选择器：`lib/labels.ts`、`lib/loop-labels.ts`、`lib/overview-labels.ts`、`lib/admin-labels.ts`；`lib/testids.ts`（33 个，
  含 `loop-role`、`skill-card`、`timeline`、`timeline-entry`、`page-*`）。

**E-4 的位置。** `apps/web/server/overview-summary.ts`（72 行）：`createOverviewSummary(call)` 持有跨请求的 `cursor`、`open`
（`Map<任务, 等待起点>`）、`closed`；`consume()` 分页读 `events.since`，对每条带 `state` 的 `task.updated` 调
`lib/waiting.ts` 的 `waitingTransition` 更新 `open` / `closed`。返回的函数签名是 `(tasks, clock)`，`tasks` 只用来数当天合并数；
`closedMs`、`openStarts`、`ended`、`waiting` 都直接取自 `open` / `closed`，不看 `tasks`。相关测试：`test/overview-summary.test.ts`、
`test/overview.test.ts`、`test/overview-states-real.test.ts`。

**测试。** `apps/web/test/` 有 160 个 `*.test.ts`，vitest 5，按 `test-groups.ts` 分 `unit` / `kernel` / `exclusive` 三组。与本任务
直接相关：`styles.test.ts`、`ui-kit.test.ts`、`w9-layout.test.ts`、`testids.test.ts`、`labels.test.ts`、`loop-page.test.ts`、
`loop-defaults.test.ts`、`loop-real.test.ts`、`skills-page.test.ts`、`skills-model.test.ts`、`skills-real.test.ts`、
`task-timeline*.test.ts`、`timeline-failure-path.test.ts`、`task-header.test.ts`、`task-stage.test.ts`、`task-ops-view.test.ts`、
`task-docs-view.test.ts`、`task-detail-ssr.test.ts`、`task-page.test.ts`、`env-page.test.ts`、`add-project.test.ts`、
`board*.test.ts`、`settings-*.test.ts`、`overview*.test.ts`、`readme-images.test.ts`。

`apps/web/e2e/`：`layout.spec.ts`（一个参数化用例展开成 14 条「1512×944 …」：技能、Loop 配置、总览、总览命令面板、总览铃铛、
项目、全部已完成、新建任务、需求详情展开日志、Bug 详情、添加项目补全、项目设置、环境、已取消展开；**没有覆盖两个抽屉与
「项目信息」标签页**）、`controls.spec.ts`（8）、`flows.spec.ts`（2）、`live.spec.ts`（2）、`manual-items.spec.ts`（7）；
`harness/` 用真实内核、假 CLI、临时根目录；`playwright.config.ts` 视口 1512×944、`workers: 1`、单例上限 90 秒。用例标题进入
门禁的用例清单（`apps/web/scripts/e2e-catalog.mjs`），改名即视为一条新用例。

**旧值在测试里的分布（整理轮粗查，只说明检索是必要的；实现轮按约束 3 用自己的命令重查并记命中数）。** 在 `apps/web` 下
`grep -rlE '<模式>' test e2e scripts`：`task-timeline|timeline-` 命中 test 9 个文件、e2e 3 个；`board-done|已取消` 命中 test 7、
e2e 1；`skill-role|skill-card|skill-description` 命中 test 2、e2e 2、scripts 1；`显示更早` 命中 test 1、e2e 1；
`loop-drawer|skills-drawer` 命中 test 1；`未绑定` 命中 test 1；`查看安装方法|env-tool` 命中 test 1。

**门禁。** `scripts/ci/steps.sh` 的步骤：`Format`、`Clippy`、`Test`、`Desktop`、`Contract`、`Scripts`、`Web lint`、
`Web typecheck`、`Web build`、`Web test`、`Web e2e`，以及准备步骤；每个步骤声明输入目录，输入没变的沿用上次结果；`docs`、
`.autome`、`AGENTS.md`、`README.md` 不触发门禁。`Desktop` 步骤把 `apps/web` 列为输入，面板改动会让它重跑。快检
`scripts/ci-quick.sh` 跑静态检查、与本轮改动相关的 vitest 文件（`apps/web/scripts/quick-related.mjs`）与按改动选出的浏览器用例。

**本机现状（整理时）。** Node v26.5.0；worktree 里没有 `apps/web/node_modules` 与 `target/`（第一次要装依赖与 cargo 构建）。

## 9. 前车之鉴（T-101 的复盘 `docs/web-面板按动森风格重做/lessons.md` 与更早任务，逐条可核对）

T-101 的数字：13 个里程碑、21 个实现轮、reopen 3 次（全在截图里程碑）、实现缺陷 9、验证缺口 5、每轮门禁 21 次失败 4、合并前
门禁失败 1 次。

1. **L-01 需要浏览器才能生成的产物不要当里程碑验收条件。** T-101 把「重新生成并提交 README 五张截图」做成里程碑，实现轮沙箱
   跑不了 `npm run screenshots`，审计轮跑得了却不能写 `docs/images`，5 个实现轮、3 次 reopen 才关闭，关闭时图仍是旧的。对策见
   约束 15。
2. **L-02 没有任何检查读页面上实际渲染出来的文字。** 总览卡片「设计轮 undefined / undefined」通过了 72 个用例，直到目视截图
   才发现。对策见约束 6；单元夹具也要包含字段缺失的情形。
3. **L-03 改契约的里程碑只更新了自己验收命令里的测试。** 四处旧断言潜伏到后面的里程碑或合并前门禁才变红，两个实现轮整轮只
   修这个。对策见约束 3；另外改 `apps/web/app` 的轮次跑 `npx vitest related --run <改动文件…>` 或 `--project unit` 全量，本任务
   几乎每个里程碑都会碰到 `styles.test.ts`、`labels.test.ts`、`testids.test.ts`、`ui-kit.test.ts` 这几个全站守卫。
4. **L-04 实现轮与审计轮环境不一致。** 21 份实现证据里 20 份记了 `spawnSync ps EPERM` 或 `listen EPERM`，浏览器验收全靠审计轮
   补跑，审计轮也有两次布局命令挂起无输出。对策见约束 5、13：每个里程碑都要有一部分验收不依赖监听（纯函数、
   `renderToStaticMarkup` 的组件测试、对 `app.css` 文本的断言）。
5. **L-05 只有整套安装启动用例能发现的问题会潜伏到合并前。** `server/` 新增对 `app/lib` 的值导入时，发布包暂存清单
   （`apps/web/scripts/stage-panel.ts`）要同步；E-4 若让 `server/overview-summary.ts` 导入新的 `app/lib` 模块，核对这一条。
6. **实现轮结束前原样跑一次宿主快检命令**（简报里给出，`sh scripts/ci-quick.sh --since <本轮起点>`）。只对改动文件跑 eslint 比
   宿主快检窄，会被退回多花一轮。
7. **浏览器用例的时间要算账。** T-98 连续六次合并前门禁失败，四次是超时与预算。改一个预算之前，把「用例数 × 单例耗时 + 夹具
   准备」与总上限对一遍。
8. **负向对照照原文注入并核实会变红。** 样式类的负向对照尤其容易写成注入后并不变红的（例如删掉一条已被别的规则兜住的声明）。
9. **审计不要用「负载」「计时脆弱」放行失败；实现证据里交给审计的缺口，审计逐条接手。**
10. **已关闭里程碑的「最新证据」指针不要动；rebase 之后跑受影响包的全量。**

## 10. 项目背景摘要（生成于 2026-11-18）

**这是什么。** Autome 2.0 是一个驱动 Loop 的本地工具：需求走「整理 → 设计 → 评审 / 裁决 → 用户批准 → 实现 → 审计 → 复盘 →
rebase → 合并前门禁 → 合并 → 清理」；Bug 另有「定位 → 复现检查 → 修复 → 修复门禁 → 复核」。

- 内核持有全部状态（SQLite 账本 + 事件流），以 stdio 上带长度前缀的 JSON-RPC 服务面板。命令行 `autome` 直接链接内核库，
  `autome core` 就是面板用的内核。
- 面板（`autome web`）是内核状态的展示与控制面：Node 服务 + 浏览器页面，只绑 `127.0.0.1`，鉴权只有 Host / Origin 检查。
  Electron 薄壳加载的就是这个面板。
- 配置分两层：全局 `~/.autome/config.toml`，项目 `<项目>/.autome/config.toml`（只写覆盖项）。六个角色
  （`plan / review / adjudicate / impl / audit / retro`）各有工具、模型、Effort、技能列表与启用开关。技能是
  `.claude/skills/<名>/SKILL.md` 或 `.agents/skills/<名>/SKILL.md`，全局在用户主目录下，项目的在仓库里；内核只扫描，从不写。

本仓库用 Autome 自己开发自己，`.autome/config.toml` 的现值：`[loop] parallel = 3`；`[gate] command = "sh scripts/ci-local.sh"`、
`run = "before_merge"`、`timeout_minutes = 30`；每轮快检跑 `scripts/ci-quick.sh`，`quick_timeout_minutes = 20`，给会话的快检命令
是 `sh scripts/ci-quick.sh --since {round_start}`；`[merge] auto = true`（过了合并前门禁自动合并，设计批准仍要人）；
`[retry] attempts = 2`。

**技术栈与布局。**

- Rust workspace（edition 2024，工具链固定 1.93.1）：`crates/autome-domain`（纯域，不做 I/O）、`crates/automed`（内核）、
  `crates/autome-cli`（`autome` 命令行）。**本任务不改。**
- `apps/web`：Node（≥ 22.22）服务（`server/`，TypeScript 由 Node 原生类型剥离直接运行）+ React Router 8 框架模式的面板
  （`app/`，SSR，React 19）；vitest 5、Playwright 1.64.1、eslint 10、TypeScript 6。没有 UI 框架，样式是两份手写 CSS
  （`app/styles/tokens.css`、`app/styles/app.css`），字体只从本地加载。**本任务只改这里。**
- `apps/desktop`：Electron 薄壳，`node --test`。不改。
- `protocol/`：内置协议 seed（v9，编译进内核）。`plugins/`：Claude Code / Codex / Cursor 插件。都不改。
- `scripts/`：门禁（`ci-local.sh`、`ci-quick.sh`、`ci/`、`test/`）、安装与打包。不改。
- 文档：`README.md`（英文）、`docs/development/`、`docs/plans/`、`docs/adr/`、`docs/design/`（原型，HTML）、`docs/images/`
  （README 的五张截图）；每个任务的文档在 `docs/<slug>/`。上一个面板任务的设计文档
  `docs/web-面板按动森风格重做/web-面板按动森风格重做.md` 有 85 KB，按节读，不要整份读。
- 状态位置：`$AUTOME_HOME`（缺省 `~/.autome`）下 `config.toml`、`state/automed.sqlite3`（账本）、`state/web.sqlite3`（面板库）、
  `web/`。

**测试与检查命令（在 worktree 根目录运行）。**

- 新 worktree 先装依赖：`sh scripts/ci/install-node.sh apps/web`。
- 静态检查：`npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`。
- 单元与组件测试：`npm run test --prefix apps/web`；单个文件 `npm run test --prefix apps/web -- test/<文件>`；单个分组
  `(cd apps/web && npx vitest run --project unit)`；相关测试 `(cd apps/web && npx vitest related --run <源文件…>)`。
- 拉真实内核的测试先 `cargo build -p automed -p autome-cli`。
- 浏览器端到端（要监听端口，实现轮沙箱里多半跑不了）：`npm run build --prefix apps/web && npm run e2e --prefix apps/web`；单个
  文件 `(cd apps/web && npx playwright test e2e/<文件>)`；只列用例 `(cd apps/web && npx playwright test --list)`。
- 截图（要监听端口）：`npm run screenshots --prefix apps/web`，默认写 `docs/images/`，`-- --out <目录>` 改输出位置；用的是
  隔离夹具与临时账本，不是真实账本。
- 整套本地 CI（即宿主合并前门禁命令）：`sh scripts/ci-local.sh`（`--plan` 只看哪些步骤会跑）。门禁由宿主在合并前跑一次；
  实现轮 / 审计轮跑里程碑验收命令与直接相关的测试；宿主在实现轮后跑快检。
- 输出长的命令重定向到 `.autome/output/`，只读头尾，例如
  `npm run test --prefix apps/web > .autome/output/web.txt 2>&1; tail -20 .autome/output/web.txt`。

**权威规则文件。**

- `AGENTS.md`（仓库根）规定：不改 `.autome/` 下任何内容；只在当前 worktree 内工作，不切分支、不动 `.worktree/` 下其它目录、
  不推送远端；完成本轮后结束会话，不自行启动下一个会话；超过 5 行的命令输出与临时验证产物写入 `.autome/output/`。
- `.autome/rules/`：目前只有一份说明性的 `README.md`（讲规则应当怎么写），没有额外的项目规则。
- `docs/agent-project-profile.md`：不存在。
- 本任务固定的协议：`docs/面板观感收尾/protocol/`（见文首指路）。

**环境注意。** 运行环境是 macOS（Darwin），系统 `sh` 是 bash 3.2；`/tmp` 是 `/private/tmp` 的符号链接；macOS 对改名的二进制
副本可能 SIGKILL，需要第二个名字时用符号链接；launchd 起的进程只有最小 PATH。
