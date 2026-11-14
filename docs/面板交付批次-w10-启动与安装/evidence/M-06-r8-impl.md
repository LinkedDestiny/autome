# M-06 · 实现轮 8

结论：验收通过，提交待审。仅推进 M-06；上一轮 M-05 审计通过，没有要求搬入的缺陷判别检查。

## 实现与修复

- `app/lib/testids.ts` 集中定义 B5 的 27 个选择器，M-11 的已取消区选择器按设计暂不加入。
- 七个页面、外壳、卡片、任务操作（含表态）、铃铛、横幅、时间线、日志、补全、原生目录按钮、环境复制按钮与命令面板接入常量和对象属性。看板、设置、新建任务错误分支仍保留页面标识。
- 时间线模型保留原始事件 node，供条目输出 `data-node`；不从中文标题反推。缺少事件锚点时不虚构 node。
- 七类允许的内部滚动区标记齐全；日志标记在实际滚动的 `.session-log-text`，而非包含工具栏的外壳。新建任务页面的标记外层用 CSS `display: contents` 保持原布局。
- 提取无状态 PathSuggestions、DirectoryPicker，真实页面复用它们，静态渲染可覆盖补全空/非空与目录按钮有/无。
- 首次验收发现内联样式违反既有样式守卫，已改为样式表规则；未放宽守卫。

## 命令与用例数

1. 契约命令：`cd apps/web && npx vitest run test/testids.test.ts test/w9-layout.test.ts test/styles.test.ts`。
   最终 3 文件、26 用例全部通过，失败 0、跳过 0。上一轮该范围基线为 16（w9-layout 4 + styles 12），新增 testids 10，所以 **总数 +10**。原始输出：`.autome/output/m06-acceptance.txt`。
2. 直接相关回归：`cd apps/web && npx vitest run test/testids.test.ts test/w9-layout.test.ts test/styles.test.ts test/board.test.ts test/overview.test.ts test/task-new.test.ts test/task-timeline.test.ts test/task-decisions.test.ts`。
   8 文件 **63 用例全过**，失败/跳过 0；同范围原有测试 53，本轮新增 10。原始输出：`.autome/output/m06-final.txt`。此范围包含上述 26 条，不能重复相加。
3. 曾扩大到 env-page、settings-onboarding（未包含 task-decisions）：9 文件 74 用例，72 通过、2 失败、0 跳过。临时还原本轮全部已跟踪 app 改动到 HEAD `07fa38e4` 后，以同范围旧文件复跑：8 文件 64 用例，62 通过、同样 2 失败。恢复全部改动后继续验收。差值仍为新增 testids **+10**，无删除或跳过。
   既有失败分别为 env-page 第 7 条、settings-onboarding 第 7 条：扩展消息发送后立即断言刷新回调，期望 1 次、实际 0 次。原始输出：`.autome/output/m06-related.txt`、`m06-baseline.txt`。本轮未改它们或消息调度逻辑，不把该扩大范围声称为通过。
4. 三项设计负向对照均在最终代码执行，分别临时删除 env-copy 的 data-command、在 shell 手写未知 testid、在 shell-main 加滚动豁免；均退出 1，分别被第 8、9、10 条拦住。逐项 finally 恢复源码，再运行契约全绿。日志：`.autome/output/m06-negative-{env,unknown,scroll}.txt`。
5. TypeScript API 按项目配置仅筛选本轮相关文件诊断：0 条；`git diff --check` 通过。未运行宿主快检或全量门禁。

本轮运行到的唯一项目测试共 **77 条**（63 + env-page/settings-onboarding 14），相较相同范围旧基线 67 增加 10；75 通过、2 个既有失败、0 跳过。重复执行和临时负向注入不另计用例总数。上一实现轮的验收 41 条属于 M-05 不同范围，不冒充本轮基线。

## 标待审之前的自审清单

### 1. 情形表逐行

| B5 行 | 检查与结果 |
|---|---|
| shell-main / 七个 page | testids 第 1–7 条逐页 SSR；新建任务/设置/看板错误分支同样有页面标识，通过 |
| board-column | 第 8 条四列 data-column 全量检查，通过 |
| task-card | 第 8 条卡片 T-01 身份；总览与看板相关回归，通过 |
| task-op | 第 8 条遍历 start/approve/merge 等全部 TaskOps 操作；表态按钮接入同常量，task-decisions 回归通过 |
| bell-button/panel/item | 第 8 条开启/关闭面板及条目 task-id，通过 |
| live-banner | 第 8 条 offline/outdated/reconnecting 属性及 connected 无横幅，通过 |
| timeline/entry | 第 8 条渲染定位会话，检查 key/node；timeline 12 条既有回归通过 |
| session-log/body | 第 8 条闭合/展开、session-id；正文选择器落在滚动 pre，通过 |
| task-new-form/draft | 新建页 SSR 同时检查，task-new 9 条回归通过 |
| path-input/suggestions/suggestion/pick-directory | 新建项目页输入、补全空/单项、原生能力有/无分别 SSR，通过 |
| env-copy | 第 8 条真实 EnvView 的缺组件安装命令，检查 data-command，通过；删除属性负向对照失败 |
| palette | 第 8 条打开/关闭状态，通过 |
| board-cancelled / toggle | 不适用：设计明确 M-11 才加入 |
| 内部滚动区 | 第 10 条 AST 遍历验证七类全部存在且没有表外标记，通过；shell-main 注入被拒 |
| 常量唯一来源 | 第 9 条 AST 遍历所有 JSX testid，常量唯一且每项使用，无字面量逃逸，通过 |

### 2. 失败分支

M-06 没有新增授权、读盘、超时或外部服务调用。适用的展示分支已执行：页面读盘错误/项目不存在、横幅离线/过期/重连、空补全、无原生能力。三个设计点名的负向对照全部执行并拦住。其余失败分支不适用：本轮仅增加标记，不改变调用链路；既有 env/settings 回调失败单独列在上文。

### 3. 全部迁移

本轮不引入状态机。对标记的条件输出执行了铃铛闭合/展开、日志闭合/展开、命令面板闭合/展开、补全空/单项、原生能力无/有，以及横幅四种输出状态；分别静态渲染检查，不宣称验证真实点击或网络恢复时序。既有看板全部筛选迁移与新建任务类型切换回归通过。

### 4. 输入域边界

标记本身没有新增数值或日期输入域，闰日/跨年/最大值不适用。检查空列表、单元素、保留前导零的 T-01/S-01、中文含空格路径；原有 overview 导航边界用例覆盖空 ID、保留字符及前导零，通过。常量表唯一性、未使用项、未知键由源码守卫覆盖。

## 验证边界

未启动服务、未打开账本、无需监听；未执行浏览器截图或真实鼠标操作，本轮依据明确的不依赖浏览器验收契约。未运行 `sh scripts/ci-local.sh` 或宿主快检。已知两条既有失败交由后续审计/宿主结合完整测试判定，不隐藏、不跳过。
