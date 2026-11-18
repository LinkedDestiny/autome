# 面板操作控件样式不统一：任务详情操作区的「取消」是裸链接

status: 已定位
fix-round: 6/6
复现命令: npm run build --prefix apps/web && cd apps/web && npx playwright test e2e/action-controls.spec.ts
根因: 面板并存两套按钮样式：统一的 ui/button.tsx（.btn 系列）只有约 14 处在用，其余约 48 处 <button> 无类名，落到 app.css 的裸 button 兜底样式（约 45px 高）；任务详情的「取消」「返回」是无样式的 <Link>，表单类操作是无样式的 <details><summary>，且 Button 组件没有危险变体
复核: 待复核

## 现象

任务详情页头操作区里，「撤销授权」「暂停」「停止」是约 45px 高的兜底按钮，「取消」是约 22px 高的裸链接、顶端对齐、无边框无底色；点「取消」后「确认取消这个任务」是普通按钮，「返回」是裸链接掉到下一行；带表单的操作是 summary 文字。全站同屏并存两种按钮（如会话日志「暂停滚动」45px 对时间线筛选胶囊按钮）。详见任务卡与附件。

## 复现

新增回归测试 `apps/web/e2e/action-controls.spec.ts`，两个用例（都用夹具里自建的运行中任务，结束时只放行不取消，避免改变 layout.spec 写死的「已取消」条数）：

1. 任务详情操作区：全部 `[data-op]` 等高（≤32px，差 ≤1px）、顶边一致；「取消」有可见边框或底色且圆角与其它按钮相同；点「取消」后确认按钮与「返回」同一行、等高。
2. 全站清扫：总览、项目看板、Loop 配置、技能、环境、新建任务、添加项目、项目设置、任务详情（展开会话日志）上每个可见按钮高度只能是 30±1 或 38±1px、圆角 ≥12px。用例内写明排除清单（页签/开关/单选 role、类名含 seg/switch/step/tab/icon/chev/caret/toggle/disclosure、单字符按钮、顶栏搜索入口 `.shell-topbar-title`）。

命令（仓库根目录；Rust 二进制需已 `cargo build`）：`npm run build --prefix apps/web && cd apps/web && npx playwright test e2e/action-controls.spec.ts`

实测（修复前）：两个用例都失败，且是样式断言：
- 用例 1：`expect(b.height).toBeLessThanOrEqual(32)`，实际 45。
- 用例 2：列出 63 条违例，例如 `Loop 配置 「保存」 高45`、`环境 「复制」 高45`、`新建任务 「存为待开始」 高45`、`项目看板 「开始」 btn-primary 高45`、`Loop 配置 「设计编辑」 高21`。

整套 `npx playwright test` 一起跑：41 通过、仅这 2 个新用例失败，其余用例不受影响。

格式化与检查（新文件）：
- `npx prettier --write e2e/action-controls.spec.ts` 后 `npx eslint e2e/action-controls.spec.ts --max-warnings 0` 通过，`npm run typecheck --prefix apps/web` 通过。
- 格式化后重跑同一复现命令，仍以上述样式断言失败。

## 根因分析

- `app/components/ui/button.tsx` 只提供 default/primary/text 三种变体，没有危险变体；全站只有少数地方使用。
- `app.css` 的裸 `button { … }` 兜底样式（小圆角矩形、输入框底色、上下 10px 内边距，约 45px）与 `.btn`（胶囊 38/30px）是两套外观，漏写类名的按钮就落在兜底样式上。
- `app/components/task-ops-view.tsx` 第 21 行「取消」用 `<Link>`，第 30 行「返回」用 `<Link>`，第 32 行表单类操作用 `<details><summary>`，三者都没有按钮样式；其余按钮无类名。
- 其它裸按钮分布在 session-log、loop、settings-page、task-decisions、projects.new、tasks.new、env、queue-controls、bell 等文件（以开工时代码重新清点）。

修复方向：给 Button 补危险变体与用法，操作类链接/summary 套按钮类名（保留 Link 跳转与 data-op/data-testid），把 app.css 兜底 `button` 样式改为与统一按钮一致，并把各页裸按钮迁移到统一按钮。

## 修复说明

- app.css：裸 `button` 兜底样式改为与 .btn 一致的 30px 胶囊（2px 边框、圆角 pill），漏写类名的按钮不再是 45px 方角；新增 `.btn-danger`，任务操作区 flex 对齐改为顶对齐一致、取消确认表单同行、summary 去掉原生三角。
- task-ops-view.tsx：全部操作按钮、「取消」Link、「返回」Link、表单类 `<summary>` 套 `btn btn-sm`，取消/确认取消用 `btn-danger`；保留 data-op/data-testid 与跳转。
- 验证：复现命令两个用例通过；整套 `npx playwright test` 43 通过、0 失败（基线 41 通过 + 2 新用例）。

## 修复说明（第 6 轮）

### 操作者备注处理
- 备注 #3：照做，只修环境页正常工具行高度；其余项未动。备注 #1、#2 继续有效，`e2e/action-controls.spec.ts`、`overlay-layout.spec.ts`、`layout.spec.ts` 未改。
- 备注 #3 里关于 T-123 单测计时波动（`controls.spec.ts` B-4）：不为它改产品或测试。本轮整套 Playwright 该用例通过。

### 根因与改动
- 复核推导成立：`.env-tools .env-component` 上下各 10px 内边距，加 30px 小号文字按钮与 1px 底边框，整行 51px。
- 新增 `apps/web/e2e/env-rows.spec.ts`（新文件）：照搬 overlay-layout 环境页用例的代理搭法，但先等 `.env-component[data-env-ok="true"] summary` ≥ 1 且所有 `[data-env-ok]` 均为 true，再断言带 summary 的工具行 ≤ 48px、summary 30±1px、驱动卡下沿 ≤ 944、`.env-components` 无内部滚动。无需登记（e2e 目录自动发现，`test-groups.ts` 只含 vitest）。
- **先红后绿**：未改内边距时新用例失败，实测 `Expected: <= 48, Received: 51`。
- 产品：`app/styles/app.css:578` `.env-tools .env-component` 的 `padding: 10px 0` 改为 `8px 0`（30+8+8+1=47px）。内核行与工具行同一规则；入口仍是 30px 的 `btn btn-text btn-sm`。

### 验证
- `npx playwright test e2e/env-rows.spec.ts`：修复后通过（与 overlay-layout / layout / action-controls 同跑通过）。
- 整套 `npx playwright test`：44 passed，退出码 0。
- `npx vitest run`：165 文件 / 1435 用例全部通过，退出码 0。
- 截图（临时脚本 `.autome/output/shot.mjs`，不进仓库，对新用例同款服务四工具正常）：`.autome/output/env-closed.png`、`.autome/output/env-open.png`。目视：工具行单行，点开 Git 的安装方法后说明在该行下方展开，驱动卡完整可见，无内部滚动。

## 复核意见

### 第 5/5 轮独立复核（当前结论：打回）

复核 HEAD：`4f4277c0637ba014e6cd15983f6c26d1a5aa7afa`。核对定位提交 `6d54414f` 至 HEAD 的完整差异，并检查本轮修复提交 `4f4277c0`；读取协议、任务卡、四张附件、参考 HTML 及两条操作者备注。备注 #2 取代 #1 第二节，其余约束继续适用。

#### 必须修复

1. **P2：环境页正常工具行增高，违反必须保留的 ≤48px 约束。** 在四个工具均正常、安装方法收起时，`apps/web/app/routes/env.tsx:67` 新增的 `btn btn-text btn-sm` 使 summary 按 `app.css:336,340` 取得 30px 高度；但 `app.css:578` 的 `.env-tools .env-component` 仍保留上下各 10px 内边距和 1px 底边框。该行是自动高度 grid，右列 details 必须容纳 summary，因此整块高度下界为 **30 + 10 + 10 + 1 = 51px**（行盒可能使实际值更大），已经超过 48px。扫描全部样式没有发现降低这些正常工具行内边距或覆盖 summary 高度的规则。备注 #2 A.1 明确要求该约束继续通过；未改动的 `apps/web/e2e/overlay-layout.spec.ts:179` 也逐行断言 `height <= 48`。这不是测试缺陷，不能降低断言或缩掉统一 30px 按钮来通过；修复轮应协调环境工具行的排布与内边距，保留单行信息、原生展开行为及驱动卡可见性，再复跑相关浏览器用例。本条是可核对的 CSS 尺寸约束推导，**本轮未取得浏览器计算尺寸，不将 51px 写成实测值**。

#### 操作者备注逐项核对

以下“源码满足”只表示元素与样式声明已核对，不替代运行时布局与截图验收。

| 备注条目 | 实现证据与复核结果 |
|---|---|
| #1 一.1 调整顺序 | `queue-controls.tsx:12` 为 `summary.btn.btn-sm`，保留 details；源码满足。 |
| #1 一.2 路径建议 | `projects.new.tsx:86` 为 `btn btn-sm`；`app.css:224,589` 两条规则仅保留宽度、左对齐、间距，不再自写边框；源码满足。 |
| #1 一.3 公共抽屉 | `app.css:597` 使用 `.drawer .btn`，同时覆盖正文和页脚；`ui/drawer.tsx:10-13` 的公共骨架包含两者；技能保存绑定仍保留主按钮变体，声明高度 30px；源码满足。 |
| #1 一.4、5 看板页头 | `projects.$id.tsx:59-60` 的设置为 `btn`、新建为 `btn btn-primary board-new`，均保留 Link 和文案；`.board-new` 自写样式已删除，使用默认 38px；源码满足，页头不增高及整页无滚动尚缺本轮浏览器结果。 |
| #1 二与 #2 二 专用控件去留 | 按后发备注执行。分段选择、范围切换、页签、侧栏、文档/证据/技能胶囊、技能卡/项目卡/任务卡/完成行/最近合并行、时间线和日志行、步进器、铃铛图标、顶栏标题继续归入专用样式；未要求这些控件迁移为按钮。 |
| #2 A.1 环境安装方法 | 已加 `btn btn-text btn-sm` 并保留 details，但高度组合不满足 ≤48px，见必须修复项；未通过。 |
| #2 A.2 提交标题 | `task-changes.tsx:17` 为 `summary.btn.btn-text.btn-sm`，保留 details；源码满足。 |
| #2 B.3、4 驱动卡与总览环境入口 | `driver-card.tsx:12`、`_index.tsx:32` 均为 `btn btn-text btn-sm`，Link 及文案保留；源码满足。 |
| #2 B.5 添加项目结果链接 | `projects.new.tsx:64,74` 三个链接均为 `btn btn-text btn-sm`，跳转目标保留；源码满足。 |
| #2 B.6 看板折叠行 | `projects.$id.tsx:69,71` 为 `btn btn-text btn-sm board-fold`；`app.css:104` 只留居中和间距；源码满足。 |
| #2 B.7 错误页返回 | `root.tsx:54` 为默认 `btn`，保留 Link；源码满足。 |
| #2 C.8 实体链接 | overview-card、task-card 的标题、task-header 项目名、新建任务项目名、project-alerts 任务编号、bell 条目均有公共 `link` 类；`app.css:7-8` 在悬停及键盘聚焦时使用既有 primary-active 令牌和下划线；源码满足。 |
| #2 C.9 Markdown 外链 | `markdown-view.tsx:10` 使用 `link link-ext`；`app.css:9` 默认下划线，安全 href 处理及新窗口属性未变；源码满足。 |
| #1 三、#2 三 测试变更边界 | `action-controls.spec.ts` 与定位版本逐字相同（差异文件 0 字节）；queue-deps 1 处、board 2 处、env-page 1 处仅把旧标记换成新 class 标记，数量仍为 2/4，没有删用例、跳过、改名或放宽匹配；与修复说明列出的改前改后一致。 |
| #2 三 新增 A/B/C 断言 | 新文件 `test/link-styles.test.ts` 的 3 项分别断言提交标题、查看环境、Markdown 外链的渲染类名，并加入 test-groups；本轮实跑全部通过。没有往已有用例插入新断言。 |
| #1 四、#2 三 实跑验证 | 修复说明声称整套 e2e 43 通过，但没有给出可关联本 HEAD 的原始日志；该声明不足以排除上述环境行高度反例。Vitest 说明仅记录首次全量存在失败、修后相关子集通过，未提供修后全量通过证据。本轮独立复验见下文。 |
| #1 四、#2 三 截图与目视 | 修复说明明确给出未执行理由“本沙箱无法起浏览器”，没有伪装为已截图。看板页头、调序收起/展开、路径建议、技能抽屉，以及环境正常/展开、看板折叠行、驱动卡、标题悬停、提交标题均未交付本轮新截图；记录为验证缺口，不能用自动断言替代目视确认。本轮未因单独缺少截图或全量门禁而打回。 |

已重新扫描 `apps/web/app` 下全部 `<button>`、`<summary>`、`<Link>`、`<a>`（原始清单 `.autome/output/independent-review-r5/clickable-inventory.txt`）。没有发现清单外仍未归入统一按钮、公共链接或备注 #2 专用样式的入口。修复说明的去留表已覆盖要求的类别，但“全部照做”须以上述环境行高度反例及未完成验证为限，不能据类名齐全认定全部验收通过。

#### 根因、范围与回归约束

- 原根因成立：裸按钮、统一按钮和裸操作链接/summary 并存。修复保留原事件、表单字段、跳转、文案及测试标识；公共抽屉规则和公共链接类能够统一多个页面，未发现内核或无关功能改动。定位至 HEAD 产品改动限于 26 个 Web 文件，另有获授权的单测调整、新增单测及登记。
- 原回归测试有实际价值：操作区高度、对齐、可见边框/底色、圆角、确认态同行均检查原始问题；全站按钮扫描可抓住 45px 兜底。但它不测 summary 外层工具行高度、不验证危险色、不区分抽屉 30px 与默认 38px，因而只跑该文件无法排除本轮反例。既有 overlay-layout 正好提供必要的 ≤48px 约束，应保留。
- 没有删除、跳过或弱化回归测试；不声明“回归测试: 有缺陷”。打回对象是产品排布组合，不是已获授权的旧标记断言更新。

#### 独立复验与限制

- 原样执行头部复现命令：客户端和服务端构建成功，Playwright 在 globalSetup 报 `spawnSync ps EPERM`，退出码 1；两个回归用例均未进入断言，teardown 也受进程查询权限限制。日志：`.autome/output/independent-review-r5/repro.log`。
- 执行 `cd apps/web && npx playwright test e2e/layout.spec.ts e2e/overlay-layout.spec.ts e2e/controls.spec.ts`：同样在 globalSetup 被 `spawnSync ps EPERM` 阻断，退出码 1。日志：`.autome/output/independent-review-r5/related-e2e.log`。没有绕过沙箱或改测试夹具。
- 直接相关 Vitest 子集：22 文件、289 项通过，退出码 0，包含 A/B/C 新测试及调序、看板、环境、CSS、按钮、操作区、抽屉相关组件。完整命令与结果见 `.autome/output/independent-review-r5/related-unit-command.txt`、`related-unit.log`。
- `npm run lint --prefix apps/web && npm run typecheck --prefix apps/web` 通过，退出码 0。日志：`.autome/output/independent-review-r5/static.log`。
- 补充按备注执行 `cd apps/web && npx vitest run`：退出码 1，1324 项通过、80 项失败、31 项跳过（总计 1435 项）。失败日志包含 `spawnSync ps EPERM`、`listen EPERM: operation not permitted 127.0.0.1` 及由监听失败引起的启动超时；这批进程/HTTP/安装集成验收未取得成功结果，不能等同产品断言通过，也不据此新增样式缺陷。本轮没有修改或跳过测试来规避限制。日志：`.autome/output/independent-review-r5/unit.log`；失败索引：`unit-errors-final.txt`。
- 用户给出的宿主“复现命令 + 快检”退出码为 **1**，不能写成门禁通过。本轮没有执行 `sh scripts/ci-local.sh`，没有项目全量门禁结论；不因缺少全量门禁而打回。
- 本轮只修改本 Bug 文档复核字段与复核意见，保持 status、fix-round、复现命令、根因与 repos 不变；未改产品、测试或格式，未注入缺陷，不合并、不推送、不启动后续会话。

### 第 3/3 轮独立复核（历史结论：打回）

复核 HEAD：`d1b93b41925ba305d0f4247304123a344d12b3dd`。检查定位提交 `6d54414f` 至 HEAD 的全部产品及测试差异，并单独核对第 3 轮修复提交 `dfaae548`、`d1b93b41`；已读取任务卡、协议、四张附件及参考 HTML 的按钮定义。本轮打回依据是任务卡范围内仍存在的迁移遗漏，不是缺少全量门禁或沙箱浏览器权限。

#### 必须修复

1. **P2：全站仍有未迁移的操作入口。** `apps/web/app/components/queue-controls.tsx:12` 的「调整顺序」仍为 `<summary>调整顺序</summary>`，没有统一按钮类名；它直接展开调序表单，不属于时间线或日志展开三角的豁免。`task-card.tsx:29` 表明，当任务处于排队态且队列多于一项时，该入口会显示。此次只迁移了表单内部三枚按钮，入口依旧是裸文字。参考 `docs/design/autome-web-panel-v3.html:350` 的同名入口也明确使用小号按钮。另一个遗漏是 `apps/web/app/routes/projects.new.tsx:86` 的路径建议按钮：有建议结果时，它仍没有 className；`app.css:222,587` 仅补宽度、边框和间距，尺寸、底色仍来自裸 button 兜底，不是统一 `.btn`。两者违反任务卡期望 0、4 的全站迁移要求。应补齐这些实际可达的操作入口，保留原生展开、选择路径行为、文案、字段与测试标识；不能以全局兜底已缩至 30px 代替迁移。
2. **P2：抽屉小号尺寸仍漏了技能抽屉。** `apps/web/app/routes/skills.tsx:43` 的「保存绑定」仍是 `btn btn-primary`，由 `app.css:334` 得到 38px；同一抽屉的「关闭」（`components/ui/drawer.tsx:11`）及「打开目录」（`routes/skills.tsx:51`）是 30px。第 3 轮新增的 `app.css:595` 只匹配 `.loop-drawer .drawer-foot .btn`，不匹配 `.skills-drawer`，因此只能修复 Loop 抽屉。任务卡期望 0 明确把抽屉列为小号 30px 场景，打开技能卡即有反例。应统一抽屉操作尺寸，保留主按钮变体与绑定行为。这是源码及 CSS 层叠结论，本轮未取得浏览器计算尺寸，不把静态核查冒充实测。

#### 上轮问题闭环、根因与范围

- Loop 角色「编辑」和「保存 N 项改动」已补统一小号类名；Loop 抽屉保存按钮通过限定选择器设为 30px。铃铛 `.bell-footer .btn-sm` 已去除 24px 高度覆盖，保留 `flex: none`，现在使用统一 30px 规则。
- 两处被修改的测试断言已恢复。`git diff --quiet 6d54414f HEAD -- apps/web/test apps/web/e2e` 退出码 0；产品仅调整新增 className 的属性顺序以兼容原断言。定位至当前版本没有删除、跳过、改名或弱化测试，不再沿用上一轮关于测试改动的打回项。
- 原根因成立：裸按钮、统一按钮、裸操作链接/summary 并存。任务详情取消/返回及表单入口已经采用 `.btn.btn-sm`，停止、取消、确认取消使用危险变体；危险样式位于基础按钮规则之后，使用既有 error 令牌。Link 跳转、表单参数、禁用条件与事件保持原行为。
- 定位至 HEAD 的产品差异为 17 个 Web 文件、增加 51 行/删除 42 行，集中于按钮类名、公共样式和危险变体，没有内核改动；第 3 轮本身围绕上轮意见修复，未发现无关功能扩张。但全站统一是原任务范围，不能只修上轮列出的例子后据此认定完成。

#### 回归测试审查

- `action-controls.spec.ts` 两个用例与定位版本完全一致。操作区等高、同行、边框/底色与圆角、取消确认态同行断言能抓住原始问题；全站尺寸清扫也有实际回归价值。
- 覆盖边界仍然存在：清扫只选 `button:visible`，不检查操作 summary，也不检查是否使用统一类名；没有输入路径触发建议列表、打开技能抽屉或打开铃铛；允许任意按钮为 30 或 38px，不能判断抽屉必须为 30px。因此上述遗漏与现有回归命令通过可以同时成立。
- 未声明「回归测试: 有缺陷」：本轮没有产品已满足要求却被错误回归断言阻断的证据。现有 `queue-deps.test.ts` 用例 7 固定匹配裸 summary 标记，是后续迁移时须处理的验证约束；当前产品入口尚未修好，本轮不改测试，也不以预期冲突代替协议要求的实际失败证据。

#### 独立复验与限制

- 原样执行头部复现命令：客户端、服务端构建成功；Playwright 在 globalSetup 遭遇 `spawnSync ps EPERM`，退出码 1，两个用例未进入断言，清理也被同一权限限制阻断。日志：`.autome/output/independent-review-r3/repro.log`。
- 执行 `cd apps/web && npx playwright test e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '需求详情展开日志|任务详情页头与文档页签'`：同样在 globalSetup 被 `spawnSync ps EPERM` 阻断，退出码 1。时间线密度及页头布局没有完成本轮浏览器复验。日志：`.autome/output/independent-review-r3/related-e2e.log`。没有绕过沙箱或修改测试夹具。
- 执行 `cd apps/web && npx vitest run test/task-ops-view.test.ts test/task-ops.test.ts test/testids.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/task-header.test.ts test/task-new.test.ts test/loop-defaults.test.ts test/settings-config.test.ts test/loop-page.test.ts test/bell.test.ts test/session-log-view.test.ts test/queue-deps.test.ts test/task-notes.test.ts test/task-decisions.test.ts test/add-project.test.ts test/env-page.test.ts`：实际 16 文件、218 项通过，退出码 0；其中 `test/bell.test.ts` 不存在，未计为执行或通过。日志：`.autome/output/independent-review-r3/unit.log`。
- 补充执行 `cd apps/web && npx vitest run test/bell-view.test.ts test/skills-page.test.ts`：2 文件、23 项通过，退出码 0。两组合计 18 文件、241 项通过。日志：`.autome/output/independent-review-r3/unit-overlays.log`。
- 用户提供的宿主复现命令及快检退出码 0 作为已有证据，不能等同于全量门禁或本轮独立浏览器验收。按要求未执行 `sh scripts/ci-local.sh`；全量门禁留给宿主在合并前运行，不因本轮缺少全量结果而打回。
- 本轮仅修改本 Bug 文档的复核字段与复核意见，保持 status、fix-round、复现命令、根因及 repos 不变；没有修改产品或测试，没有临时缺陷注入，不合并、不推送、不启动后续会话。

### 第 2/3 轮独立复核（历史结论：打回）

复核 HEAD：`05527e722fc43e976e9282771a9a306f699dd27b`；产品修复提交：`b7aa8181`；核对定位提交 `6d54414f` 至 HEAD 的完整差异，并单独检查上一轮修复 `f2647a41` 后的变化。已读取任务卡、协议、四张附件及参考 HTML 的按钮定义。

#### 必须修复

1. **全站迁移仍有遗漏，上一轮第二条尚未闭环。** `apps/web/app/routes/loop.tsx:57` 的角色「编辑」入口仍是无统一类名的 `<button>`；切换一个角色的启用开关产生未保存改动后，`:66` 的「保存 N 项改动」也仍是裸按钮。它们不是任务卡豁免的开关本身、页签或纯图标按钮，仍依赖 `app.css:23-24` 的兜底规则，违反任务卡期望 0、4。兜底只有 30px 且没有 `.btn:hover` 的位移行为，给兜底缩尺寸不能代替迁移。`:82` 的抽屉「保存」又只有 `btn btn-primary`，按 `app.css:334` 得到默认 38px，与同一抽屉的 30px 操作及任务卡明确的抽屉小号规则不一致。应补齐操作入口类名与场景尺寸，保留原事件、文案和字段。
2. **铃铛内仍有第三种操作按钮高度。** `apps/web/app/components/bell.tsx:46` 的「标为已读」虽然已有 `btn btn-sm`，但 `apps/web/app/styles/app.css:61` 的 `.bell-footer .btn-sm { height: 24px; }` 比 `:338` 的 `.btn-sm { height: 30px; }` 优先级更高。打开通知列表并存在未读通知时，这个文字操作按钮仍按 24px 渲染，违反任务卡期望 0、4 的铃铛小号 30px 规则。此为确定的源码层叠结论，未冒充本轮浏览器计算值。回归清扫没有打开铃铛，因而宿主现有测试通过不能排除此反例。应消除这处场景覆盖，同时保留通知列表既有布局约束。
3. **两处既有测试断言发生了非格式化修改。** `apps/web/test/loop-defaults.test.ts:38` 给匹配保存按钮的正则加入 `class="btn btn-sm"`，`apps/web/test/settings-config.test.ts:59` 给匹配移除按钮的正则加入 `class="btn btn-danger"`。禁用条件、数量 4 和写入拒绝断言仍保留，没有发现删用例、跳过或放宽这些行为断言；但正则内容确已改变，不属于空白、换行、行尾逗号例外。本轮用户明确要求「断言、输入、用例名、跳过标记一律不得改」，优先于任务卡较早的「期待旧值的断言一起改完」说明。请恢复这两处原断言，通过产品侧兼容原标记顺序完成样式迁移，例如将新增 className 放到原断言匹配的属性之后；本轮不代改产品或测试。

#### 已修正项、根因与改动范围

- 上一轮危险样式问题已从源码上修正：`app.css:366-368` 位于 `.btn` 规则之后，普通、悬停使用既有 error 令牌；`task-ops-view.tsx:20` 同时覆盖 stop/cancel，取消 Link、确认取消和禁用态均有危险类；`ui/button.tsx` 已增加 danger 类型。没有新增纯红色或危险按钮立体阴影。
- 新建任务的「创建并开始」「存为待开始」、添加项目的检查/确认等已补 `.btn`，默认尺寸规则为 38px；会话日志及多数行内控件已补 `.btn.btn-sm`。取消/返回保留 Link 跳转，表单字段、data-op、data-testid 和原操作事件未改变。
- 根因仍成立：裸 button、统一按钮及裸操作链接/summary 并存。定位至 HEAD 的产品变更为 16 个 Web 文件、增加 47 行/删除 39 行，以类名和公共 CSS 为主，未修改内核；范围围绕任务，但不能据此忽略以上未迁移入口和局部尺寸覆盖。

#### 回归测试审查

- `git diff 6d54414f HEAD -- apps/web/e2e` 为空；定位新增的 `action-controls.spec.ts` 两个用例、输入、断言、名称及跳过标记均未改变。几何尺寸、圆角、可见边框/底色、确认态同行断言能检查原始问题，具有实际价值。
- 现有覆盖边界仍在：危险按钮用例不检查危险色；全站清扫允许任意按钮为 30 或 38px、不检查是否采用统一类名，也不打开角色抽屉、切换开关产生待保存状态或打开通知铃铛。因此本轮发现可以与当前测试通过并存，不能将通过解释为全站迁移完成。
- 不声明「回归测试: 有缺陷」：本轮不存在产品已满足全部要求、却被回归用例错误夹具或错误断言阻断的证据；当前必须先解决产品遗漏及测试修改边界。

#### 独立复验与限制

- 原样执行头部复现命令：Web 客户端和服务端构建成功；Playwright 在 globalSetup 报 `spawnSync ps EPERM`，退出码 1，两个回归用例均未进入断言。清理阶段也报进程查询权限错误。日志：`.autome/output/independent-review-r2/repro.log`。
- 执行 `cd apps/web && npx playwright test e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '需求详情展开日志|任务详情页头与文档页签'`：同样在 globalSetup 被 `spawnSync ps EPERM` 阻断，退出码 1。日志：`.autome/output/independent-review-r2/related-e2e.log`。这两项是已知沙箱验证缺口，不是产品断言失败，也不是本轮打回依据。
- Vitest 第一组实际执行 task-ops-view、task-ops、testids、ui-kit、ui-base、task-header、task-new、loop-defaults、settings-config，共 9 文件、132 项通过，退出码 0；命令另外包含的 session-log、queue-controls、project-add 文件筛选项未匹配到文件，不计入通过。日志：`.autome/output/independent-review-r2/unit.log`。
- 补充执行 `cd apps/web && npx vitest run test/loop-page.test.ts test/session-log-view.test.ts test/queue-deps.test.ts test/task-notes.test.ts test/task-decisions.test.ts test/add-project.test.ts test/env-page.test.ts`：7 文件、86 项通过，退出码 0。日志：`.autome/output/independent-review-r2/unit-migration.log`。两组合计 16 文件、218 项通过。
- 用户提供的宿主复现命令与快检退出码 0 仅作为已有证据；不等同于独立浏览器复验或全量门禁。本轮未运行 `sh scripts/ci-local.sh`，全量门禁由宿主合并前统一运行；不因缺少全量门禁而打回。
- 本轮仅修改本 Bug 文档复核字段和复核意见；状态、fix-round、复现命令、根因保持不变。没有修改产品或测试，没有临时缺陷注入，不自行合并或启动后续会话。

### 第 1/3 轮复核记录（历史）

独立复核第 1/3 轮，结论：打回。复核对象为 `f2647a41102d8ec7f657713d1417c83171c9fd2c`，对比定位提交 `6d54414f` 至该提交的全部差异，并核对任务卡「期望」、四张用户附件及 `docs/design/autome-web-panel-v3.html`。

### 必须修复

1. **危险变体实际被覆盖，且「停止」未使用危险变体。** `apps/web/app/styles/app.css:181-182` 的 `.btn-danger` 与 `.btn-danger:hover:not(:disabled)` 分别被后面的 `:336-337` 同优先级 `.btn` 与 `.btn:hover:not(:disabled)` 覆盖：普通态文字/边框最终来自 `--animal-text` / `--animal-border`，悬停态来自 `--animal-primary-active` / `--animal-primary`，不能呈现要求的错误色令牌。`apps/web/app/components/task-ops-view.tsx:20` 又仅对 `op === 'cancel'` 添加危险类，「停止」仍为普通按钮。任务卡「期望 0、1、2」明确要求停止、取消和确认取消具备危险外观。应修正样式层叠顺序或选择器，并覆盖停止操作；不得以类名已存在或尺寸测试已通过代替有效外观。此结论为源码层叠核查，未声称本轮取得浏览器计算样式。
2. **全站统一及默认尺寸的修复未完成。** 本次产品差异仅涉及任务操作组件和全局 CSS；其余操作控件没有迁移到统一组件或基础类名。具体反例：`apps/web/app/routes/projects.$id.tasks.new.tsx:75-76` 的「创建并开始」只有 `btn-primary`，没有 `btn`；「存为待开始」仍为裸按钮。`app.css:23` 将兜底高度统一设为 30px，而 `:367` 的 `.btn-primary` 不补高度，因此表单主提交按钮依然不是任务卡要求的默认 38px。`routes/projects.new.tsx:70` 的确认提交同样只有 `btn-primary`，`components/settings-page.tsx:53` 的保存仍无统一类名。兜底样式与 `.btn` 的悬停行为也仍不同（`:24` 对比 `:337`）。应按任务卡「期望 0、4」补齐统一类名/组件和场景尺寸，保留紧凑操作区的小号规则；不能仅靠所有裸按钮降到 30px 满足宽泛尺寸断言。

### 根因、范围与回归测试审查

- 原根因成立：定位版本同时存在约 45px 的裸按钮兜底与 38/30px 的 `.btn`，取消/返回 Link 和表单 summary 没有统一按钮外观。修复为这些入口添加类名并保留跳转、表单字段和测试标识，方向正确。
- 产品改动集中于两个 Web 文件，没有内核或无关产品变更；但范围小不代表需求完整，以上两项仍未修好。定位至修复差异还含宿主生成的简报与 Bug 文档。
- `git diff 6d54414f f2647a41 -- apps/web/e2e apps/web/test` 为空：回归测试及已有测试没有删除、跳过、弱化或改名。新回归测试通过浏览器几何尺寸、圆角、可见边框/底色和确认态同行关系检查原问题，具备实际价值。
- 测试覆盖存在明确边界：名为「取消是危险按钮」的用例没有断言危险色；全站用例允许任意按钮为 30 或 38px，没有区分表单主按钮的默认尺寸，也没有证明所有操作已迁移。因此这两项产品缺口可以在现有测试通过时继续存在。本轮不修改测试，不声明「回归测试: 有缺陷」：没有证据表明产品修好后会被错误夹具或错误断言阻断，应先交回修复轮完成产品修复。

### 独立验证与限制

- 原样执行头部复现命令：Web 客户端与服务端构建成功；Playwright 在 globalSetup 的进程检查阶段遭遇 `spawnSync ps EPERM`，退出码 1，两个用例均未执行。清理阶段也报告同一权限错误。日志：`.autome/output/independent-review/repro.log`。这是沙箱限制，不作为产品断言失败或回归测试缺陷。
- 执行直接相关详情布局子集：`cd apps/web && npx playwright test e2e/layout.spec.ts e2e/overlay-layout.spec.ts --grep '需求详情展开日志|任务详情页头与文档页签'`，同样在 globalSetup 被 `spawnSync ps EPERM` 阻断，退出码 1；「一屏至少 10 行」及页头布局本轮未完成浏览器复验。日志：`.autome/output/independent-review/related-e2e.log`。
- 执行 `cd apps/web && npx vitest run test/task-ops-view.test.ts test/task-ops.test.ts test/testids.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/task-header.test.ts test/task-new.test.ts`：7 个文件、108 项通过，退出码 0。日志：`.autome/output/independent-review/unit.log`。
- 用户提供的宿主复现命令与快检退出码 0 作为已有证据记录，不等同于本轮独立浏览器验收，也不等同于项目全量门禁。按要求未运行 `sh scripts/ci-local.sh`，全量门禁仍由宿主在合并前统一执行。
- 打回依据是上述可定位的产品缺口，不是缺少全量门禁。本轮未修改产品代码或测试、未注入临时缺陷；仅更新本 Bug 文档的复核字段和复核意见，保持状态、fix-round、复现命令与根因不变。

## 修复说明（第 2 轮）

针对复核两条打回：
1. `.btn-danger` 移到 `.btn` / `.btn-primary` 规则之后并用 `--animal-error` / `--animal-error-active`（普通、悬停、禁用态），不再被覆盖；「停止」「取消」「确认取消」「移除项目」都用危险变体；`Button` 组件新增 `danger` 变体。
2. 全站迁移：表单页提交按钮（创建并开始、存为待开始、添加项目的检查/确认、设置页保存/运行/完成/跳过/移除、环境安装/登录/复制）加 `btn`（默认 38px）；行内/卡片/抽屉等紧凑场景（会话日志、队列、依赖、备注、表态、告警、Loop 默认值、命令面板关闭、任务卡「开始」、目录选择）用 `btn btn-sm`（30px）；兜底 `button` 的悬停与 `.btn` 一致。
- 同步更新两处依赖旧标记的单测断言（loop-defaults、settings-config，仅类名前缀）。
- 验证：复现命令通过；整套 playwright 43 通过；vitest 全量 1432 个仅上述 2 个因标记变化失败，已修后子集 24 通过；eslint、typecheck 通过。

## 修复说明（第 3 轮）

针对第 2 轮复核三条：
1. loop.tsx：角色「编辑」入口、「保存 N 项改动」补 `btn btn-sm` / `btn btn-primary btn-sm`；抽屉「保存」类名被既有单测固定为 `btn btn-primary`，改用 CSS（`.loop-drawer .drawer-foot .btn`）压到 30px 小号，与同抽屉其它操作一致。
2. app.css：去掉 `.bell-footer .btn-sm { height: 24px }` 的高度覆盖，保留 `flex: none`，铃铛「标为已读」恢复 30px。
3. 恢复 `test/loop-defaults.test.ts`、`test/settings-config.test.ts` 到定位提交版本（断言不再改动）；产品侧把 className 放在原断言匹配的属性之后（`aria-label…disabled=""` 之后、`<button disabled=""` 之前后），兼容原正则。
- 验证：vitest loop-defaults / settings-config / loop-page / bell 共 75 项通过。

## 修复说明（第 5 轮）

### 操作者备注处理
- 备注 #1 第一节 5 处必须迁移：全部照做。备注 #1 第二节「豁免」被备注 #2 取代，按 #2 执行。其余（第三、四节）照做；`e2e/action-controls.spec.ts` 未动。
- 截图目视：本沙箱无法起浏览器看图，未取得截图；以整套 Playwright（43 通过，含 1512×944 各页无整页纵向滚动、环境页单行高度、抽屉布局断言）代替，不冒充目视。

### 完整去留表
| 元素 | 归类 |
|---|---|
| queue-controls「调整顺序」summary | `btn btn-sm` |
| projects.new 路径建议按钮 | `btn btn-sm`，CSS 只留整行宽/左对齐/间距 |
| 抽屉内全部按钮（页脚+正文，含技能「保存绑定」） | `.drawer .btn` 30px，主按钮变体保留 |
| 看板「新建任务卡」 | `btn btn-primary`（38px），`.board-new` 样式已删 |
| 看板「设置」 | `btn`（38px） |
| env「查看安装方法」、task-changes「提交标题」 | `btn btn-text btn-sm`（保留 details） |
| driver-card「查看环境」、_index 去环境页、projects.new「打开这个项目」及添加成功两个跳转、board-fold（更早/收起/已取消） | `btn btn-text btn-sm`（`.board-fold` 只留居中间距） |
| root「回到总览」 | `btn` |
| 任务标题（overview-card、task-card）、task-header 项目名、新建任务页项目名、告警任务编号、铃铛条目链接 | 公共 `.link`（悬停/聚焦变 primary-active 并下划线，聚焦环黄色） |
| Markdown 外链 | `.link .link-ext`（平时即下划线） |
| 任务详情操作区各按钮/取消/返回/表单入口、Loop/设置/环境/会话日志等已迁移项 | 前几轮已是 `.btn` 系列 |
| 分段选择/页签/侧栏、胶囊（文档/证据/技能）、整卡整行可点、时间线与日志行展开、步进器、铃铛图标、顶栏标题 | 不改（专用样式） |

### 改动的既有断言（只换期望标记）
- `test/queue-deps.test.ts` 用例 7：`<summary>调整顺序</summary>` → `<summary class="btn btn-sm">调整顺序</summary>`。
- `test/board.test.ts` 用例 13 两处：`<summary>调整顺序` → `<summary class="btn btn-sm">调整顺序`（数量 2 不变）。
- `test/env-page.test.ts:116`：`<summary>查看安装方法` → `<summary class="btn btn-text btn-sm">查看安装方法`（数量 4 不变）。
- 新增 `test/link-styles.test.ts`（A/B/C 三类各一条）并登记到 `test-groups.ts`。

### 验证
- 复现命令所在的整套 `npx playwright test`：43 通过。
- `npx vitest run` 全量：除我新增测试的两处写法错误与 `.link:focus-visible` 焦点环规则（styles 测试要求黄色环，已补）外全过；修后相关子集通过。typecheck、eslint 通过（修了铃铛链接重复 className）。
