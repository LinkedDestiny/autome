# M-05 · 实现轮 6/25

本轮只推进 M-05。上一轮 M-04 审计通过，没有要求搬入的缺陷复现检查。

## 实现

- 新增 SessionLog：原生 details、展开时订阅、收起或卸载时退订；控制器沿用同一 /ws 发送入口。日志窗口、暂停、翻页、重试和续传沿用 M-04 模型。React 提交后只调整日志容器 scrollTop，不主动改变焦点。
- 新增 tasks/:id 只读占位页，loader 只读 task.get，保持会话顺序；task/session 本任务事件触发重验。铃铛任务链接开放，原有项目未开放断言保留。
- 样式只用现有 token；页面独立滚动，日志区 60vh，关闭浏览器滚动锚定以避免内容更新带动视口。
- 新增视图 6 条、任务页 5 条测试。第一次 typecheck 发现测试 LoaderFunctionArgs 缺 url/pattern，按当前类型补齐；未改变产品契约。

## 命令与结果

原始输出都在 `.autome/output/m05-*.txt`，不转存进任务目录。

- `npm test --prefix apps/web -- test/session-log-view.test.ts test/task-page.test.ts test/bell-view.test.ts test/styles.test.ts`：29 用例，29 通过，失败/跳过 0；初验 m05-tests.txt，还原后复验 m05-tests-final.txt，退出码 0。
- `npm test --prefix apps/web -- test/session-log-model.test.ts test/session-log-controller.test.ts test/live-routes.test.ts`：22 用例，22 通过，失败/跳过 0，退出码 0（m05-related.txt）。
- `npm run lint --prefix apps/web`：退出码 0（m05-lint.txt）。
- `npm run typecheck --prefix apps/web`：首次报测试夹具类型错误；补齐后退出码 0（m05-types-final.txt）。
- `npm run build --prefix apps/web`：退出码 0（m05-build.txt）。
- `git diff --check`：通过。

用例总数：本轮正常验收及直接相关子集共 **51 个不同用例**。M-05 验收范围原有 bell-view 8 + styles 10 = 18，本轮新增 6 + 5 = 11，得到 29。上一轮 M-04 验收基线为 28，本轮主验收数比它多 1，但文件范围不同，不是全量增长 1；仓库测试净新增 11。本轮另跑的 22 个相关用例没有新增。负向注入重复运行不计入不同用例总数。没有运行全量门禁或宿主快检组合。

## 标待审之前的自审清单（逐项执行）

1. **情形表逐行**
   - 收起：SSR 检查 details、轮次/标签、运行/结束摘要，不发送消息（view 第 1 条）。
   - 展开：工具条、pre、空态；窗口前后提示及三个分页按钮的有/无（view 第 2、6 条）。
   - 跟随/暂停/跳到最新：模型追加滚动、暂停窗口保持、未读提示、恢复；适配器仅改本容器（view 第 3、4 条，model 相关用例）。
   - 大历史日志：窗口上限、首尾分页拼接完整、多字节计量（model 第 5、6、9、10 条）；视图显示分页入口（view 第 2 条）。
   - ended/rejected：中文原因和拒绝重试按钮（view 第 2 条）；四种结束原因、重试偏移（model 第 7 条）。
   - 任务页：标题/id、内核顺序、无阶段条/写按钮、唯一网关调用、路由及铃铛一致（task-page 第 1–3 条、bell-view）。
   - 中文及样式：中文文案表、禁用焦点抢占 API、颜色/圆角/焦点 token（view 第 5、6 条、styles 10 条）。
2. **失败分支**
   - gateway=null、503、504、not_found、404、其它读取失败逐个渲染检查（task-page 第 4 条）。
   - 日志拒绝及 retry、读盘失败等 log.end 原因呈现（view 第 2 条、model 第 7 条）；发送未连接等待 hello（controller 第 1 条）。
   - 空日志与空会话列表（view 第 2 条、task-page 第 3 条）。
   - 服务端授权、真实文件读取和网络超时注入不适用：本里程碑没有新增这些服务端分支，界面消费既有消息；本轮未重跑 M-02/M-03。
3. **全部迁移**
   - idle→subscribing（open）、subscribing→live（chunk）、active→ended/rejected、rejected→subscribing（retry）、active→idle（close）、ended→idle（close）由 model/controller 现有测试检查。
   - hello 在 subscribing/live 续传，在 idle/ended 不续传；外会话消息忽略；发送失败后 hello 重发、结束后 close 不退订，由 model/controller 检查。
   - pause/scrolled(false)、末页 scrolled(true)、非末页 scrolled(true)、jump、top、earlier、later、暂停期间追加由 model 逐项检查；top/bottom/无滚动三路由 view 第 3、4 条检查。
   - 本任务/其它任务 × task/session/project 事件六种组合由 task-page 第 5 条检查。
   - 真实 details 点击、React effect 卸载和浏览器滚动/焦点没有动态 DOM 取证；SSR 与控制器测试不冒充这部分证明，继续留给 H-01–H-05。
4. **输入域边界**
   - 4px 容差：距离底部 5px 为 false，4px 为 true；top=0、bottom=scrollHeight、无滚动保持原值（view 第 3、4 条）。
   - hiddenBefore/hiddenAfter=0 不出现分页按钮，正值出现；空串、单窗口历史、超过 256KiB 中文与多窗口完整拼接（view/model）。
   - 空会话列表及两会话顺序、任务 id 精确匹配与其它 id 不重验（task-page）。
   - 日期运算、前导零、闰日和跨年不适用：本组件不解析日期或数值 id；started_at 仅为内核会话字段。

## 负向对照

每次只注入一种错误，运行相应项目测试，再在 finally 原样恢复文件；五种注入命令均退出 1：

- paused-scroll：无 scroll 时也滚到底，view 适配器用例变红。
- focus：适配器主动 focus，view 不抢焦点与源码守卫变红。
- target：任务路由存在但 TARGETS=false，bell-view 和 task-page 路由一致性变红。
- old-bell：保留旧无任务链接断言，bell-view 变红。
- color：日志区硬编码十六进制颜色，styles 颜色守卫变红。

输出见 m05-negative.txt 及 m05-negative-<名称>.txt。还原后重新执行主验收。`git status --porcelain` 已检查：实现轮尚未提交的 8 个预期代码/测试文件存在，不能把它们清掉来满足设计中面向临时注入的“输出为空”；无注入残留，后续一并提交。

## 验收边界

真实浏览器人工项 H-01–H-05 未取证；本轮按设计使用 node SSR、假滚动元素和模型/控制器测试，没有把这些称为浏览器验收。全量门禁与宿主快检由 Autome 执行。未启动后续会话。
