# 面板操作控件样式不统一：任务详情操作区的「取消」是裸链接

status: 已定位
fix-round: 1/3
复现命令: npm run build --prefix apps/web && cd apps/web && npx playwright test e2e/action-controls.spec.ts
根因: 面板并存两套按钮样式：统一的 ui/button.tsx（.btn 系列）只有约 14 处在用，其余约 48 处 <button> 无类名，落到 app.css 的裸 button 兜底样式（约 45px 高）；任务详情的「取消」「返回」是无样式的 <Link>，表单类操作是无样式的 <details><summary>，且 Button 组件没有危险变体
复核: 打回

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

## 复核意见

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
