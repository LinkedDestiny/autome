# 面板操作控件样式不统一：任务详情操作区的「取消」是裸链接

status: 已定位
fix-round: 0/3
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

（待修复轮填写）

## 复核意见

（待复核轮填写）
