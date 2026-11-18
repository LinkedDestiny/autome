# 面板操作控件样式不统一：任务详情操作区的「取消」是裸链接

status: 已定位
fix-round: 2/3
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

### 第 2/3 轮独立复核（当前结论：打回）

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
