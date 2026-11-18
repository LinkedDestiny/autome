# M-02 实现证据 · 实现 #3

- 范围：A-2、A-Q2/A-Q3、验-7 静态部分。上一审计 M-01 通过，无退回缺陷，不适用搬入审计检查及红绿复现要求。
- 实现：七类表单控件使用低特异性 `:where` 基础规则；下拉箭头、复选勾和单选点使用 CSS 与已有颜色令牌。清理添加项目、命令面板、新建任务、Onboarding 重复外观。步进器保留透明外观并明确 30px 高、零内边距/阴影，避免继承 38px 输入高度溢出 34px 容器。禁用勾选控件降低透明度，选中底色保留。
- 测试：新增七类基础规则与状态覆盖、TypeScript AST 全站控件扫描（含 Input 包装组件调用）、原生外观反向守卫三项；既有样式守卫不改弱。

## 命令与结果

1. 验收命令（改前与改后均执行）：`(cd apps/web && npx vitest run test/ui-base.test.ts test/styles.test.ts test/ui-kit.test.ts test/add-project.test.ts test/w9-layout.test.ts test/settings-config.test.ts test/task-ops-view.test.ts test/test-groups.test.ts)`。
   - 改前 8 文件、84 用例全部通过；改后 8 文件、87 用例全部通过，失败 0、跳过 0。总数 +3，全部来自 ui-base 三项新增。
   - 上一轮 M-01 审计为 108 用例；本轮 87，相差 -21：验收集合移除 task-timeline 29、board 19、testids 15（-63），加入 add-project 17、w9-layout 4、settings-config 14、task-ops-view 4（+39），新增 ui-base 3，净 -21；没有静默丢层。设计最低 86，实际多出的 1 是 styles 中已有的 C-2 守卫。
   - 原始输出：`.autome/output/m02-baseline.txt`、`m02-accept.txt`、`m02-accept-final.txt`。
2. 设计四条负向对照：逐项修改后运行 `(cd apps/web && npx vitest run test/ui-base.test.ts test/styles.test.ts)`，每次还原后才执行下一项。
   - 基础选择器删除 select → 退出 1，七类覆盖失败。
   - 追加 `appearance: auto` → 退出 1，反向守卫失败。
   - TSX 追加 `<input type="date" />` → 退出 1，源码类型守卫失败。
   - 勾颜色替换为 `#fff` → 退出 1，既有不散写颜色守卫失败（基础规则断言也失败）。
   - 输出：`.autome/output/m02-negative-{select,appearance,date,color}.txt`。全部注入已还原，最终验收 87 通过。
3. 关联测试：`(cd apps/web && npx vitest related --run app/styles/app.css)`：8 文件、53 用例，52 通过、1 失败，0 跳过。失败为 `shell-live.test.ts` 的监听 SSR 用例，`listen EPERM: operation not permitted 127.0.0.1`，属于沙箱环境阻塞。该关联集合没有上轮同命令的可比基线；不与 87 个验收用例相加（存在重叠）。输出 `.autome/output/m02-related.txt`。
4. 旧守卫检索：`grep -rnE '\.add-project input|\.palette input|\.onboarding-editor textarea|\.task-new textarea' apps/web/test apps/web/e2e apps/web/scripts`。改前对应起点 git grep 为 2 处，改后 3 处：原 2 处均为应保留的滚动布局守卫，新增 1 处为外观清理断言，没有遗留要求旧外观的断言。输出 `m02-old-guards-before.txt`、`m02-old-guards.txt`。

## 标待审之前的自审清单

1. **情形表逐行**（A-Q2/A-Q3）
   - 文本/搜索/数字输入：基础字体、38px 高、2px 边框、背景、圆角、阴影均由新增规则断言通过；未指定 type 视为 text。
   - select：覆盖元素规则；36px 右内边距、两段渐变箭头断言通过。
   - textarea：基础规则不限制高度；独立内边距；既有新建/设置滚动守卫通过。
   - checkbox/radio：18px、none 外观、圆角/圆形、选中勾/点、禁用态断言通过。
   - fieldset/legend：边框、间距、最小宽度及图例字体断言通过。
   - 聚焦：所有 input/select/textarea 的黄色 3px 环及原有 seg/switch 焦点规则通过。
   - hidden/file/range：基础文本选择器排除；hidden 不展示，file 必须 visually-hidden；range 当前不允许进入源码集合。seg/switch 仍由高特异性规则隐藏原控件；stepper 保留专用外观。
   - 清理四处页面外观：新增断言逐处确认不再定义边框、背景、字体、颜色或 padding；只留布局。反向层无 appearance 非 none 或 all 重置。
2. **失败分支**：不适用授权/读盘/超时/外部服务等业务失败路径，本轮仅 CSS 与静态测试。设计点名的四条退化分别以负向注入实际触发失败，已还原并复验。
3. **全部迁移**：不新增业务状态机。原生控件未选中↔选中、启用↔禁用、聚焦/失焦由对应 CSS 规则断言及既有 ui-kit 事件测试覆盖；seg/switch 隐藏规则保持。此处是静态与组件契约证据，不宣称浏览器视觉已实测。
4. **输入域边界**：AST 扫描全部 TSX 控件，含无 type 的默认 text、明确的 text/search/number/checkbox/radio/hidden/file；新增 date 被拒绝的负向对照通过。空值、最大数字不改变本轮样式且没有新增值校验；日期、闰日、跨年等不适用。浏览器外观与视口验证仍由 H-03 等人工项覆盖。

## 用户裁定

- BL-01 纳入 M-12，BL-02 纳入 M-13，已补方案、里程碑及可执行验收契约；遵守一轮一个里程碑，本轮只实现 M-02，M-12/M-13 保持开放。
- BL-03 忽略，不自动重拍 README，保留人工 H-09。预算按本轮指令改为 3/65，不按新增里程碑推算。

## 快检与交付

- 已执行指定 `sh scripts/ci-quick.sh --since 58712c883118aea8c0fc30f72c7b77b4767b0c41`，原始输出 `.autome/output/m02-quick.txt`。21 步中 19 步成功、2 步失败：Format、Clippy、Web lint/typecheck/build 等通过；shell-live 监听 SSR 因 listen EPERM 失败，Web e2e 在 globalSetup 中因 spawnSync ps EPERM 整套未执行，并有同原因清理失败。相关运行还报告 watcher EMFILE。
- 快检执行 60 个 Vitest 用例（59 通过、1 环境失败）；浏览器发现 33 个，与设计基线差 0，实际运行 0 个，不能报通过。没有上轮同快检范围的可比 Vitest 基线，不与验收/关联测试重复相加。
- 未运行 `scripts/ci-local.sh`。环境受阻不计协议失败；本里程碑验收已取得通过证据，交审计轮独立复验。浏览器外观未实测。
- 最终代码再次执行同一指定快检，输出 `.autome/output/m02-quick-final.txt`：61 秒、21 步、19 成功/2 环境失败，与首遍相同；最终 CSS 已在这次 Web build 中验证。`git diff --check` 与状态块/单一里程碑表检查通过。
