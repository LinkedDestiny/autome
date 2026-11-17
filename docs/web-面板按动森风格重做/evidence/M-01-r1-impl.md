# M-01 · 实现轮 1

结论：M-01 验收命令通过，提交待审；指定快检的浏览器阶段受沙箱阻塞，不宣称快检全过。implementation-round 为 1/55。

## 实现范围与依据

对应 A-1、A-3、设计总-Q2/总-Q3 与「A 组件清单」。新增 11 个共用组件、十种内联 SVG 图标；铃铛改用 Icon，通知行为不变。原型的六色圆点卡片、虚线卡片、丝带、胶囊、数字块、表单控件落入固定类名和令牌；无新增依赖，无内核、路由或真实账本写入。

新增 57 个 `--ui-*` 令牌（原型颜色、图案、表面与阴影），逐项写死到原有令牌名值守卫。原有 12 条 styles 守卫全部保留：颜色、emoji、内联样式、圆角、焦点、主按钮立体阴影、字体许可与布局限制均未放宽。新增三条分别检查 CSS 引用均有定义、ui 令牌均有引用、SVG 所属文件仅 icons/progress；原铃铛 SVG 移入白名单文件而非扩大例外。test-groups 将 ui-kit 明确纳入 unit。

没有上一轮审计文件，也没有需逐字移植的审计回归检查。

## 命令、用例总数与基线差

1. 改动前基线：`(cd apps/web && npx vitest run test/styles.test.ts test/labels.test.ts test/testids.test.ts test/bell-view.test.ts test/test-groups.test.ts)`，5 文件、45 用例通过。原始输出 `.autome/output/m01-baseline.log`。
2. 最终验收（负向注入全部还原后）：`(cd apps/web && npx vitest run test/ui-kit.test.ts test/styles.test.ts test/labels.test.ts test/testids.test.ts test/bell-view.test.ts test/test-groups.test.ts)`，6 文件、68 用例通过，0 失败、0 跳过。原始输出 `.autome/output/m01-accept.log`。
   - 相对本轮改动前基线 **+23**：ui-kit 新增 20，styles 新增 3；其它测试不删不减。
   - 最终实际数：ui-kit 20、styles 15、labels 10、testids 11、bell-view 8、test-groups 4。
   - 设计表写的 testids 5 是静态声明数，其中路由循环实际展开 7 条，因此运行数为 11；这解释设计最低 58 与实际数的部分差异。另多写了 4 条组件用例（最低 16 → 20）。无上一实现轮，45 为本轮实测基线，不把全项目 1056 当本子集基线。
3. `sh scripts/ci-quick.sh --since a91ecd353316be452bc0674e3f1a0397c913f4f0` 已运行，日志 `.autome/output/m01-quick.log`：执行 87 步、沿用 0、失败 1；Format、Clippy、Web lint/typecheck/build 通过，所运行 Vitest 合计 645 用例通过。失败为 Web e2e 的 `spawnSync ps EPERM`（setup 与 cleanup），不是组件断言失败；剩余条目因快检自身 120 秒预算列为未跑。645 是快检按关联选择的统计，不能与里程碑基线 45 相减，也不代表全量回归。
4. 补齐三条控件回调用例后重跑 `cd apps/web && npm run lint && npm run typecheck`，通过；日志 `.autome/output/m01-static.log`。最终验收包含这些新增用例。
5. `git diff --check` 通过。未自行运行 `scripts/ci-local.sh` 或其它全量门禁。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计组件/规则 | 执行检查 | 结果 |
|---|---|---|
| Card 默认、黄/青/紫/蓝/绿/红、虚线、圆点 | ui-kit「卡片默认与六种颜色…」；styles 固定色值与引用守卫 | 通过，默认 + 六色共七种底色，虚线另作变体 |
| Ribbon 默认青色、四色及小号 | ui-kit「丝带四色…」逐色检查 span 与类名 | 通过 |
| Pill 描边、九色实心 | ui-kit「胶囊默认描边…」 | 通过 |
| Tile 七色、缺失、真实零与附注 | ui-kit 两条数字块用例 | 通过，null/undefined 为 —，0 保留 |
| Progress 0–100、SVG 属性和 aria | ui-kit 两条进度用例及负向夹取检查 | 通过 |
| Button 默认/主/文字、小号、禁用、提交语义 | ui-kit 两条按钮用例与原主按钮阴影守卫 | 通过 |
| Segmented 同名 radio、选中、禁用 | ui-kit 三条（渲染、输入域、两个方向回调） | 通过，键盘由原生 radio 提供，真实键盘另见环境限制 |
| Switch 原生 checkbox 与 role=switch | ui-kit 开关渲染与原生回调两方向 | 通过 |
| Stepper 原生 number、上下限按钮 | ui-kit 三条（边界、整体禁用、实际处理回调） | 通过 |
| Input 原生表单属性 | ui-kit「输入框保留表单约束与空值」 | 通过 |
| Check 禁用说明关联 | ui-kit 勾选块及双向回调 | 通过，aria-describedby 同时保留外部描述 |
| Icon 十个名字、Bell 消费共用图标 | ui-kit 图标用例、bell-view 原有 8 条、SVG 守卫 | 通过 |
| 既有页面布局、文案与选择器 | styles/labels/testids 原有检查 | 通过，未改页面布局规则 |

### 2. 失败分支

本里程碑没有授权、读盘、超时或外部服务调用，这四类分支不适用（纯展示和原生表单组件）。适用的禁用/空输入分支均执行：Check 不可用时指向原因、Switch/Segmented 禁用、Stepper 整体及边界禁用；Stepper 空输入的 `valueAsNumber=NaN` 不调用数值回调，Infinity 同样不写入；Tile 缺失不伪装为 0。全部通过。浏览器启动失败没有掩盖成用例通过。

### 3. 全部迁移

组件不自行持久化状态，采用受控属性/原生输入。ui-kit 回调用例实际调用渲染出的处理器并断言：Segmented 全局→项目、项目→全局；Switch 未选→选中、选中→未选；Check 未选→选中、选中→未选；Stepper 中间→下限、中间→上限，低于下限/高于上限的手输→夹取端点，空值/非有限值→不触发更新。端点按钮与整体禁用的渲染分别检查。这里是组件处理器证据，不冒充真实浏览器事件/键盘证据。

### 4. 输入域边界

Progress 检查 -10/0/42.5/100/140/NaN/±Infinity，输出有限 0–100 且 aria 与 rect 一致。Stepper 检查 min=1/max=3 的 1/2/3、min=max=1、手输 -10/140/0/3、1.4/2.6（整数步长取整）、NaN/Infinity。Tile 检查 null/undefined/0；Input 检查空串与 maxlength/required 透传；Segmented 检查空列表/单元素及禁用。全部通过。日期、闰日、跨年、前导零文本不适用：M-01 不处理日期或标识文本解析，Stepper 是原生数值域。

## 设计列出的负向对照

执行 `.autome/output/m01-negative.mjs`，每次仅注入一项，使用 finally 恢复原文件，四次命令均退出 1，日志分别为 `.autome/output/m01-negative-<名称>.log`：

- inline-style：Progress 加 `style={{ width: pct }}`，`npx vitest run test/styles.test.ts -t 'TSX 没有内联样式'` 抓到。
- unknown-token：CSS 加 `var(--ui-nope)`，`npx vitest run test/styles.test.ts -t 'app.css 的每个 var 都有令牌定义'` 抓到。
- unclamped-progress：去掉 Progress 夹取，`npx vitest run test/ui-kit.test.ts -t '进度两端夹取及合法小数'` 抓到 140 不等于 100。
- literal-color：Card 直接写 `#fff`，`npx vitest run test/styles.test.ts -t '样式与 TSX 不散写颜色'` 抓到。

这是四次单项负向实验，不计入 68 条独立验收用例总数；按名称过滤产生的未选用例也不是产品跳过。恢复后已重跑完整 M-01 验收至绿。

## 环境限制与下一步

- 临时组件预览 `node .autome/output/preview-ui.mjs` 已构建成功，但 Chromium 在启动时报 `bootstrap_check_in ... Permission denied (1100)`；未产出截图、未执行真实键盘/点击检查，不能给视觉通过结论。日志 `.autome/output/m01-browser.log`。无用户面板或真实账本接入。
- 指定快检 Web e2e 在 setup 的 `ps EPERM` 阶段中止，29 条浏览器用例均未开始；cleanup 同样不能执行进程扫描，不能宣称已验证残留为空。清理错误记录的临时根为 `autome-e2e-Vr6LIs`、`autome-e2e-live-duGLl8`，由宿主按原 teardown 检查。没有为通过而移除 ps 检查或修改门禁预算。
- 审计/宿主完整复验命令：`sh scripts/ci-quick.sh --since a91ecd353316be452bc0674e3f1a0397c913f4f0`；可补跑 `node .autome/output/preview-ui.mjs` 获取组件截图与浏览器交互证据。M-01 的正式验收不含浏览器命令，已满足可运行验收后报待审；后续页面视觉验收仍沿用 H-01…H-08。
