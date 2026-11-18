# 面板浮层与面板内部排布缺陷（重提 T-103）

status: 已定位
fix-round: 2/3
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

## 复核意见

本轮独立复核结论：**打回**。定位基线 `4442737be10fe156cbc15ab72a002e344e79aa6a`，修复候选 `d92c066c81e36aefa9e42fd018145a3d1717735f`；检查 `git diff 4442737b...d92c066c`。不是因未跑全量门禁打回，而是仍有明确的需求遗漏。

1. **需求复核：P2，角色抽屉仍缺可见的 Effort 标签。** 任务卡「期望」第 1 项明确要求“Effort 分段选择一行（带「Effort」标签）”，参考原型 `docs/design/autome-web-panel-v3.html:571` 也有可见标签。`apps/web/app/routes/loop.tsx:82` 只给 `Segmented` 传 `label="Effort"`；`apps/web/app/components/ui/segmented.tsx:5` 将它写入 `aria-label`，第 6–9 行只渲染选项文字。打开任意角色抽屉，用户仍看到无字段名称的“默认 / low / medium / high”。本轮用真实 `RoleDrawer`、现有配置夹具和 React SSR 隔离渲染，确认 HTML 含 `aria-label="Effort"`，去标签后的正文含“模型”却不含“Effort”。证据在 `.autome/output/review-t103/effort-render.html`、`effort-text.txt`、`effort-probe.log`；临时探针命令 `apps/web/node_modules/.bin/vitest run --config .autome/output/review-t103/probe.config.mjs` 退出码 0。交回修复轮在角色抽屉补可见标签并保持纵向排布及现有回归断言，本轮不修产品代码。
2. **规范复核：未发现回归测试被删除、跳过或弱化，范围基本最小。** `overlay-layout.spec.ts`、`layout.spec.ts`、`task-notes.test.ts` 从定位到修复无差异，`loop-page.test.ts` 定位新增的覆盖来源用例也未改。模型标签选择器与旧技能顺序断言的同步，属于任务卡明确允许的旧结构适配；默认值测试增加了“相同值不能冒充无覆盖”的约束。产品差异限于 `apps/web` 的 6 个既有文件，没有内核改动或无关功能扩展。
3. **根因与测试意义。** 原 `.drawer-body` 缺少纵向布局、技能描述无截断、通知沿用大按钮等根因与差异相符；修复增加公共抽屉 flex/gap、字段布局、紧凑通知与来源行、文档内边距，并按项目来源控制角色恢复按钮、复用 Markdown 渲染。6 个浏览器用例直接量几何尺寸，环境夹具无条件构造并断言全部正常；角色恢复单测同时检查有覆盖和无覆盖，备注单测检查实际 `<code>` / `<ul>`，均有回归价值。但这些检查不能证明遗漏的 Effort 可见标签已实现。
4. **独立复验结果。** `cd apps/web && npx playwright test e2e/overlay-layout.spec.ts` 退出码 1：`globalSetup` 在 `apps/web/test/fixtures/process-scan.ts:5` 调用 `ps` 时遇到 `spawnSync ps EPERM`，收尾同样受限，6 个浏览器用例全部未执行，不记为产品断言失败或通过。日志 `.autome/output/review-t103/reproduction.log`。`cd apps/web && npx vitest run test/loop-page.test.ts test/skills-page.test.ts test/task-notes.test.ts test/loop-defaults.test.ts` 退出码 0，4 文件 59 例通过；直接相关子集 `test/bell-view.test.ts test/task-docs.test.ts test/task-docs-view.test.ts test/markdown.test.ts test/drawer-focus.test.ts test/w9-layout.test.ts test/env-page.test.ts test/task-detail-ssr.test.ts` 通过 `npx vitest run` 复验，退出码 0，8 文件 64 例通过。日志分别为 `.autome/output/review-t103/unit.log`、`related.log`。
5. **额外覆盖缺口，交宿主实测。** `apps/web/e2e/overlay-layout.spec.ts:88–92` 把 `.bell-list` 的全内容边界当作可视区，而实际裁切发生在 `.bell-panel`（`app.css:46`，`max-height:60vh; overflow-y:auto`）；第 5 条处于列表内不等于它完整处于面板视口内。待验证场景：通知权限为 `default`、前 5 条均未读且都有两行长标题和正文。按当前 CSS 静态预算约 `62 + 46 + 5×95 = 583px`，超过 944 高视口的 `60vh=566.4px`；这是待浏览器实测的风险，不冒充本轮已复现结果。未做缺陷注入，未改测试；本轮按产品遗漏退回修复，不声明“产品已修好但测试自身卡住”。
6. **验证边界。** 用户提供的宿主复现加快检退出码 0，只代表该次修复门禁；本轮没有将其表述成全量门禁通过。遵照要求未运行 `sh scripts/ci-local.sh`，由宿主在合并前统一执行。当前沙箱下不能确认真实浏览器几何结果，也未补做浏览器截图验收。本轮只更新本 Bug 文档的复核字段与意见，状态、fix-round、复现命令、根因保持原样。
