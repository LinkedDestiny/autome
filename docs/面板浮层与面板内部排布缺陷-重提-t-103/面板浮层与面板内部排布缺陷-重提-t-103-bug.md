# 面板浮层与面板内部排布缺陷（重提 T-103）

status: 已定位
fix-round: 0/3
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

（修复轮填写）

## 复核意见

（复核轮填写）
