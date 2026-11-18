# 面板浮层与面板内部排布缺陷（重提 T-103）

status: 已定位
fix-round: 4/4
复现命令: cd apps/web && npx playwright test e2e/overlay-layout.spec.ts
根因: 抽屉内容区 .drawer-body、页签内容区、铃铛面板没有统一的纵向排布与间距规则，浮层内按钮沿用页面级大号样式；角色抽屉的恢复按钮不看是否有项目覆盖，备注正文未走 Markdown 组件
复核: 待复核

## 现象

T-102 合并后，真实账本 1512×944 下 9 处浮层/页签排布不对（见任务卡与附件）：角色抽屉字段挤成一行且底部按钮贴在一起；技能抽屉角色名被置灰原因挤成竖排、长描述把绑定区推出视口、「打开目录」通栏；技能卡「未绑定」胶囊被拉成整行；环境页每个正常工具占两行、驱动卡被挤出；任务详情页头标题跑到右侧；文档页签区三行紧贴且重复「文档」；铃铛面板按钮过大、每条约 150px；项目范围「恢复跟随全局」不分情况可点；备注把 Markdown 当纯文本。

## 复现

复用 `refs/keep/t103-fix-7a595e22` 的思路，回归断言写进**新文件** `apps/web/e2e/overlay-layout.spec.ts`（6 个用例，未改 `layout.spec.ts`）；第 8、9 项在单测层：`apps/web/test/loop-page.test.ts`（恢复按钮置灰）、`apps/web/test/task-notes.test.ts`（备注 Markdown）。

前置：`sh scripts/ci/install-node.sh apps/web`，`cargo build`（e2e 夹具需要 `target/debug/autome`）。

- 复现命令（仓库根目录 worktree 下执行）：`cd apps/web && npx playwright test e2e/overlay-layout.spec.ts`
- 修复前结果：6 个用例全部红，原因都是排布断言：
  - 角色抽屉：模型输入框 y=167 < 标签下沿 195（与「模型」同行）。
  - 技能抽屉：描述高度 1093.7px > 104px（未收起，把角色勾选和保存推出视口）。
  - 技能卡绑定胶囊：宽 362 ≥ 卡片一半 196。
  - 铃铛：条目高 137 > 96。
  - 任务详情文档页签：胶囊行顶边 454.5 < 说明行底边+6=460.5。
  - 环境页：正常工具整块高 56 > 48。
- 整套 `npx playwright test`：35 通过，仅这 6 个失败（失败原因同上），说明与整套并行时（夹具已有十几条通知）也稳定。
- 环境页用例**确定性**构造「全部正常」：在用例内起独立面板进程，内核外包一层代理改写 `env.get` 回包为全部 present/login 正常、problems 为空，并断言 `data-env-ok="true"` 数等于条目总数；不依赖宿主 PATH 与 iTerm2，不含条件分支或 skip。
- 验证用例对修复敏感：临时应用 `git diff 4507a9cf refs/keep/t103-fix-7a595e22 -- apps/web/app`（上次的产品修复）后，6 个用例全绿；随后已还原，本轮未改任何产品代码。
- 单测（修复前红）：`cd apps/web && npx vitest run test/loop-page.test.ts test/task-notes.test.ts` 各有 1 个失败（`overridden` 属性尚不存在、备注无 `<code>`/`<ul>`）。
- 格式化/lint：`sh scripts/ci-quick.sh`（Format、Clippy、Web lint、Web typecheck）全部通过；格式化后重跑复现命令，仍以上述排布断言失败。
- 默认值卡片「恢复跟随全局」保持始终可点，`loop-defaults.test.ts` 的「不得按数值相等置灰」断言留给修复轮从 `7a595e22` 带回（本轮不改已有测试文件的其它断言）。

## 根因分析

- `apps/web/app/styles/app.css` 中 `.drawer-body` 只管滚动，不规定子元素纵向排开与间距；`RoleDrawer`（`routes/loop.tsx`）的启用开关、工具分段、「模型」`<label>` 是行内元素，挤成一行；底部按钮无间隔。
- `SkillDrawer`（`routes/skills.tsx`）顺序为描述 → 来源 → 绑定范围 → 角色，描述无行数限制；`Check` 的置灰原因 `<small>` 与角色名同行 flex，挤压角色名；「打开目录」是通栏大按钮。
- `.skill-binding` 胶囊在卡片 flex/grid 列里被拉伸。
- 环境页 `.env-tool-row` 的安装入口单独成行、页头两句各占一行，驱动卡被挤出视口。
- 任务详情页头与 `task-docs.tsx` 的页签/说明/胶囊行没有统一内边距与行距，且重复渲染「文档」标题。
- `bell.tsx` 用页面级大号按钮、通知条目无高度与截断约束。
- `RoleDrawer` 的「恢复跟随全局」不区分是否存在项目覆盖；`task-notes.tsx` 把备注当纯文本。
- 共同根因：浮层与面板内容区缺少统一的排布规则。

## 修复说明

从 `refs/keep/t103-fix-7a595e22` 带回上次的产品修复（`apps/web/app` 的 bell/task-docs/task-notes/loop/skills/app.css）：统一抽屉与页签纵向排布、浮层按钮缩小、技能抽屉角色在来源前并收起长描述、恢复按钮按 `overridden`（provenance=project）置灰、备注走 Markdown。
同步旧测试：`loop-page.test.ts` 的模型标签选择器改为 `label.field > span`；`skills-page.test.ts` 的顺序断言改为角色在来源之前；`loop-defaults.test.ts` 补「默认值恢复不得按数值置灰」断言。定位轮的回归用例（overlay-layout.spec.ts、新增单测）未改动。
验证：`npx playwright test e2e/overlay-layout.spec.ts` 6 例通过；vitest 的 loop-page/skills-page/task-notes/loop-defaults 59 例通过；`sh scripts/ci-quick.sh --since 4442737b` 全部通过。

第 2 轮：复核指出角色抽屉 Effort 分段只有 aria-label、无可见标签。`routes/loop.tsx` 把 Effort 分段包进 `div.field` 并加可见 `<span>Effort</span>`，与「模型」字段同一纵向布局。复核第 5 点（铃铛面板裁切）为待实测风险，未改。验证：vitest loop-page/loop-defaults 36 例通过；ci-quick 见提交。

第 3 轮：宿主门禁失败原因是 e2e globalSetup 的 w7-flow.sh 在机器高负载（load average 约 90）下耗时 150.6s，超过 150s 的 prepare 上限（ETIMEDOUT），6 个用例未执行，不是产品断言失败。产品代码与测试均未改动；在本轮会话重跑 `cd apps/web && npx playwright test e2e/overlay-layout.spec.ts`，6 例全部通过（总计 2.5 分钟，其中大部分是夹具准备）。该超时依赖宿主负载，修复轮不能修改夹具/回归测试，如门禁再因准备超时失败，请在负载较低时重跑。

第 4 轮（操作者备注 #1，两件事）：
1. 角色抽屉 footer：「恢复跟随全局」加 `btn btn-sm`（保留 overridden 判断、禁用条件与确认交互），「保存」加 `btn btn-primary`；技能抽屉「保存绑定」原为裸 `btn-primary`，同步为 `btn btn-primary`，两个抽屉写法一致。`loop-page.test.ts` 的恢复按钮用例补两条类名断言。
2. 铃铛最坏情况实测（1512×944，`page.route` 补 6 条两行长标题+正文未读通知，通知权限 default、有「开启系统通知」行）：改前每条 93px，面板 clientHeight 564（60vh），第 5 条下边缘 643.8px，面板可视区下边缘 635px，被裁约 9px。改 `.bell-panel` 为 `max-height: min(72vh, 680px)`：clientHeight 678，可视区下边缘 749px，第 5 条下边缘 643.8px 完整可见，面板底 749.7px 未超出 944 视口。`layout.spec.ts`「总览铃铛」补「前 5 条（不足则取现有条数）最后一条下边缘不超过面板可视区下边缘」断言；`bell-view.test.ts` 中钉死 60vh 的样式断言同步为新值（该文件不在回归保护内）。
验证：`cd apps/web && npx playwright test e2e/overlay-layout.spec.ts e2e/layout.spec.ts` 22 例通过；vitest loop-page/skills-page/loop-defaults/bell-view 59 例通过；ci-quick 见提交。overlay-layout.spec.ts 未改动。

### 操作者备注处理

- 备注 1（恢复按钮小号、保存主按钮、补类名断言）：照做。
- 备注 2（实测铃铛）：照做，实测被裁 9px，已改样式并补断言，数字见上。

## 复核意见

### 第 3/3 轮（当前结论）

**打回。** 定位基线 `4442737be10fe156cbc15ab72a002e344e79aa6a`，当前候选 `e0d090f3fca7d6d1dcf6d02b4b1faecaf65b78d2`。本轮重新检查累计差异，发现上一轮漏记的明确需求缺口；不因缺少全量门禁或浏览器沙箱限制打回。

1. **需求轴：应修——角色抽屉的次要按钮仍沿用页面级尺寸。** 任务卡「期望」第 0 项要求「浮层内的次要按钮一律用小号按钮」。`apps/web/app/routes/loop.tsx:82` 的项目「恢复跟随全局」仍为没有 className 的 button，`apps/web/app/components/ui/drawer.tsx:13` 原样渲染 footer。`apps/web/app/styles/app.css:23` 给裸按钮设置页面级 `padding: 10px 16px`，`:544` 的 drawer-foot 仅处理排布，没有尺寸覆盖；已有 `:333` 小号规则为 `height: 30px; padding: 0 14px; font-size: 12px`。关闭、添加技能、打开目录等已使用小号样式，角色恢复按钮遗漏。这由实际 JSX 与样式选择器确认，不宣称已测得浏览器高度。建议修复轮为该次要按钮复用既有小号规则，保留覆盖来源判断、禁用条件与确认交互，不改定位回归断言。原型 `docs/design/autome-web-panel-v3.html:576` 没有对此按钮的小号要求作明确豁免，以任务卡具体要求为准。此项足以打回，不将下述铃铛风险充作已复现缺陷。
2. **规范轴：回归保护与范围检查通过。** `git diff 4442737b...HEAD` 的产品改动限于 apps/web 的 6 个既有文件；抽屉缺纵向排布、技能描述未限高、通知使用大按钮等根因与差异吻合。定位至当前候选的 overlay-layout.spec.ts、layout.spec.ts、task-notes.test.ts 无差异；loop-page.test.ts:204–211 的覆盖/继承对照保持原样。旧模型标签选择器和旧技能来源/角色顺序断言适配有任务卡「线索」的明确授权；默认值恢复测试增加禁止按数值相等置灰约束，没有删除、跳过或弱化定位回归。累计差异中的三份 brief 来自 Autome 独立 chore 提交，本轮不改。第 2 轮产品候选 `9ad5b744` 至当前候选的 apps/web 差异为空，证据 `.autome/output/review-t103-round3/round3-web.diff`；`git diff --check 4442737b...HEAD` 退出码 0。
3. **独立复验：相关单测通过，浏览器复现未执行用例。** 原命令 `cd apps/web && npx playwright test e2e/overlay-layout.spec.ts` 退出码 1；globalSetup 在 apps/web/test/fixtures/process-scan.ts:5 调用 ps 遭 `spawnSync ps EPERM`，清理同样受限，6 例全部未执行。日志 `.autome/output/review-t103-round3/reproduction.log`；这是任务卡记录的沙箱限制，不是排布断言失败，不记为通过。独立执行 `cd apps/web && npx vitest run test/loop-page.test.ts test/skills-page.test.ts test/task-notes.test.ts test/loop-defaults.test.ts test/bell-view.test.ts test/task-docs.test.ts test/task-docs-view.test.ts test/markdown.test.ts test/drawer-focus.test.ts test/w9-layout.test.ts test/env-page.test.ts test/task-detail-ssr.test.ts`，退出码 0，12 文件、123 例通过，日志同目录 related.log。通过的测试没有断言角色恢复按钮尺寸，不能覆盖第 1 项缺口。
4. **铃铛覆盖缺口保留为风险。** overlay-layout.spec.ts:88–92 检查 bell-list 全内容边界，实际裁切发生在 app.css:46 的 bell-panel（max-height:60vh、overflow-y:auto），不能独立证明前 5 条在面板可视区内完整显示。权限为 default、前 5 条均未读且有两行长标题和正文的高度预算风险仍在，本轮没有新增浏览器测量。宿主应比较该场景第 5 条底边与面板实际可视区；本轮不修改测试，也不声明「回归测试: 有缺陷」，因为未出现产品修复后因错误测试而失败的证据。
5. **其他要求与门禁边界。** Effort 可见标签仍在 loop.tsx:82，与原型 :571 一致，上一轮该项修复未回退。其余已检查要求有对应实现，未发现额外范围扩展。用户提供的宿主「复现命令 + 快检」退出码 0 作为外部修复门禁结果记录，不能等同全量门禁或本轮浏览器验收。本轮未运行 `sh scripts/ci-local.sh`，留给宿主合并前统一执行。
6. **写入与后续。** 本轮只改本 Bug 文档的复核字段和本节，status、fix-round、复现命令、根因保持不变；原文没有 repos 字段，不新增。未改产品或测试，未做负向缺陷注入；日志及检查产物仅写入被 Git 忽略的 `.autome/output/review-t103-round3/`。第 3/3 轮仍按协议写打回，预算及后续流程交由 Autome 处理，不自行增轮、修代码、合并或启动下一个会话。

### 第 2/3 轮（历史记录）

第 2/3 轮独立复核结论：**通过**。定位基线 `4442737be10fe156cbc15ab72a002e344e79aa6a`，当前候选 `9ad5b7448bb78570a567e4e6bec93b4a312f2b6d`。检查定位至当前候选的完整产品/测试差异，并单独核对上一轮候选 `d92c066c81e36aefa9e42fd018145a3d1717735f` 至当前候选的增量。本结论不表示本轮浏览器验收或全量门禁已通过。

1. **上一轮打回项已消除。** 任务卡期望 1 与参考原型 `docs/design/autome-web-panel-v3.html:571` 要求可见 Effort 标签。当前 `apps/web/app/routes/loop.tsx:82` 将分段控件放进 div.field 并增加可见 span，复用 app.css 的纵向字段布局；原有选项、值、禁用条件、编辑回调和 aria-label 均保留。使用真实 RoleDrawer、配置夹具及 React SSR 的独立临时探针，确认抽屉正文包含 Effort、字段容器包含可见标签且分组仍有可访问名称。命令 `apps/web/node_modules/.bin/vitest run --config .autome/output/review-t103-round2/probe.config.mjs` 退出码 0，1 例通过；证据为同目录 effort-probe.log、effort-render.html、effort-text.txt。SSR 证明渲染结构，不能代替浏览器几何验收。
2. **根因成立，修复范围合理。** 原抽屉内容区缺纵向排布、技能描述未限高、通知使用大按钮等现象与代码差异吻合。修复通过公共 drawer-body 的 flex/gap 和字段规则解决排布，收起描述并前置绑定区，紧凑化通知、环境工具行及文档间距；角色恢复依据实际项目来源，默认值恢复保持可点，备注复用 Markdown 组件。累计产品改动限于 apps/web 下 6 个既有文件，未改内核。第 2 轮产品增量只有 Effort 标签及包裹容器，没有新增状态、依赖或无关功能。
3. **回归约束未被删除、跳过或弱化。** 定位至当前候选，overlay-layout.spec.ts、layout.spec.ts、task-notes.test.ts 无差异；逐段比对确认定位新增的角色覆盖来源单测原样保留。第一轮仅适配旧模型标签选择器、按任务卡要求反转旧技能来源/角色顺序断言，并增加默认值不得按数值相等置灰的约束；旧结构适配与任务卡授权相符。第 2 轮 apps/web/test、apps/web/e2e 全部无差异。浏览器测试测量几何尺寸，环境用例无条件构造并断言全部正常；角色单测含有覆盖/无覆盖两个对照，备注断言实际 code/ul/li 元素，具有回归价值。本轮未注入缺陷、未修改测试，未声明「回归测试: 有缺陷」。
4. **独立复验结果。** 原复现命令 `cd apps/web && npx playwright test e2e/overlay-layout.spec.ts` 退出码 1：globalSetup 在 `apps/web/test/fixtures/process-scan.ts:5` 执行 ps 时遇到 `spawnSync ps EPERM`，清理也遇到相同限制，6 例全部未执行。这是任务卡已记录的沙箱限制，不是产品断言失败，也不记为通过；日志 `.autome/output/review-t103-round2/reproduction.log`。直接相关子集命令 `cd apps/web && npx vitest run test/loop-page.test.ts test/skills-page.test.ts test/task-notes.test.ts test/loop-defaults.test.ts test/bell-view.test.ts test/task-docs.test.ts test/task-docs-view.test.ts test/markdown.test.ts test/drawer-focus.test.ts test/w9-layout.test.ts test/env-page.test.ts test/task-detail-ssr.test.ts` 退出码 0，12 文件、123 例通过；日志同目录 related.log。
5. **保留上一轮的覆盖缺口与实测风险。** `apps/web/e2e/overlay-layout.spec.ts:88–92` 用 bell-list 的全内容边界检查前 5 条，实际裁切容器是 `app.css:46` 的 bell-panel（max-height:60vh、overflow-y:auto），因此该断言不能独立证明前 5 条在面板视口内完整可见。通知权限 default、前 5 条均未读且都有两行长标题及正文时，静态高度预算约 583px，高于 944px 视口的 60vh（566.4px）；本轮未获得浏览器实测，继续列为风险而非已复现缺陷。宿主需用该场景核对第 5 条底边与面板 clientHeight 所界定的可视区，不能仅凭现有铃铛用例通过认定该边界场景已验收。第 2 轮没有改铃铛，也没有弱化既有断言。
6. **门禁与写入边界。** 用户提供的宿主「复现命令 + 快检」退出码 0，仅作为宿主修复门禁结果记录；本轮没有将其等同于全量门禁通过。遵照要求未运行 `sh scripts/ci-local.sh`，留给宿主合并前统一运行；没有补做浏览器截图验收。当前已确认的上一轮阻断项消除，独立相关单测通过，未发现新的已证实阻断项，故通过本轮复核。仅更新本 Bug 文档的复核字段与本节，status、fix-round、复现命令、根因保持不变；原文无 repos 字段，不新增。临时探针与日志全部位于被 Git 忽略的 `.autome/output/review-t103-round2/`，不提交；不自行合并或启动后续会话。
