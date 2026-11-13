# M-02 · 实现 #10：合并门禁 Web test 修复

## 范围与根因

修复轮，implementation-round 为 10/30，不推进新里程碑。上轮审计没有退回缺陷；本轮三个判别检查都已在项目测试体系中，原样先跑，不另抄一份。添加页产品源码改动后，按设计风险条款把 M-02 从已完成重新报为待审；reopen 保留原值 1，其余里程碑不变。

宿主原始日志：`/Users/dannie/project/autome-v2/.autome/output/gates/T-82/merge_gate-d54b49d10b28.log`，2413–2484 行。该提交 Web test 总计 800 例，797 通过、3 失败、0 跳过：

- `gateway-task-card.test.ts > project.probe 经 HTTP 由真实内核展开家目录，越界路径被拒`：旧断言期待 `/var/...`，实际为 `/private/var/...`。设计 §2 要求 realpath，产品正确；期望改为 `realpathSync(join(home, 'repo'))`，保留 exists 与越界拒绝断言。
- `live-routes.test.ts > 路由 loader 导出守卫报告缺少 liveShouldRevalidate 的文件`：添加页漏导出重验策略。接入 `liveShouldRevalidate(byKind('project.'))`，项目事件与 reload 重验，任务事件不触发，导航及表单遵循默认行为。追溯设计 §1、R-05 的实时外壳集成。
- `settings-onboarding.test.ts > 6 项目页设置链接`：测试 hydration 数据漏掉 M-05 新增的 alerts，SSR 访问 undefined.length。真实 loader 始终返回数组，故补齐测试数据的 `alerts: []`，保留设置链接断言，不在产品中掩盖错误数据。

同类检查：现有路由导出由 live-routes 全目录 AST 守卫覆盖；搜索看板 SSR 使用点，board、alerts 的测试数据已带 alerts；路径断言搜索定位到上述唯一 `path: join(home, ...)`，现有 paths 与 project-confirm-real 已覆盖 canonical path。未修改安全守卫、未降低断言或设置跳过条件。

## 命令、红绿证据与数量

命令均从 `apps/web` 执行，完整输出写入 worktree `.autome/output/`。

1. 修复前原样复现：`npx vitest run test/gateway-task-card.test.ts test/live-routes.test.ts test/settings-onboarding.test.ts`。
   日志 `r10-red.txt`。18 例，12 通过、6 失败、0 跳过；live-routes 与 settings-onboarding 各复现宿主同一失败，HTTP 文件 4 例全在 listen EPERM 处受阻，没有到路径断言。该子集在宿主基线中也是 18 例，差 0。
2. 修复后直接相关子集：`npx vitest run test/live-routes.test.ts test/settings-onboarding.test.ts test/add-project.test.ts test/paths.test.ts test/project-confirm.test.ts test/project-confirm-real.test.ts test/board.test.ts test/alerts.test.ts`。
   日志 `r10-green.txt`。8 文件 **62 例全部通过**，0 失败、0 跳过。同范围修改前 61 例，差 **+1**，来自新增的添加页重验行为用例（项目 added/removed、无关事件、空批、reload、查询导航、POST）。与上一实现轮扩展子集 199 例相比少 137，是选取范围改变，不表示全量用例减少；与宿主全量 800 例不可直接当作全量比较。本轮只新增 1 例，未删除用例，全量预期 801，未实跑。
3. 修复后 HTTP 单例：`npx vitest run test/gateway-task-card.test.ts -t 'project.probe 经 HTTP'`。
   日志 `r10-http.txt`。发现 4 例，选中 1 例在 listen EPERM 处失败，3 例因名称过滤跳过。路径修正尚无本轮 HTTP 绿灯，不把真实内核非 HTTP 子集通过冒充 HTTP 验收。
4. `git diff --check` 通过。未运行 ci-local、Web 全量或宿主快检；快检与合并门禁由 Autome 依据最终提交重新执行。

## 标待审之前的自审清单

1. **情形表逐行**：设计 §3 probe 不存在目录、非仓库、仓库根、有成员四种事实由 add-project 第 1 例通过；默认工作区/文档仓库、确认路径一致性、两种成功跳转、错误与部分成功显示、原生选择器条件、入口、禁用状态、补全乱序分别由该文件已有用例通过。M-02 无另设情形表。新增事件筛选全部通过。
2. **失败分支**：add-project 覆盖补全网络失败和取消、内核拒绝与 skip_error；project-confirm 覆盖拒绝、超时及内核不可达；paths 覆盖非法输入、目录读失败和符号链接逃逸；均通过。HTTP 越界拒绝原断言保留但受 EPERM 阻塞，未取证。不涉及终端执行，终端失败分支不适用。
3. **全部迁移**：未检查→检查→允许确认、修改输入→禁止确认、确认→项目页/设置页、部分成功→保留项目与提示由 add-project 通过；项目事件→重验、任务事件/空批→不重验、reload→重验、查询导航/POST→默认重验由新增用例通过；真实注册→重复拒绝→移除→再注册由 project-confirm-real 通过。无新增业务状态机。
4. **输入域边界**：paths 检查空串、缺 HOME、511/512/513 长度、NUL、..、反斜杠、同前缀兄弟目录、HOME 符号链接、隐藏前缀及 49/50/51/60 项，通过。Onboarding 保存空串、最大长度与超长、非法产物路径由 settings-onboarding 通过。重验空事件批次通过；日期边界不适用，本轮未改日期输入。

## 后续与限制

M-02 待独立审计；宿主需在允许本地监听的环境复跑 HTTP 单例。保留所有人工验收项，本轮没有声称真实浏览器交互验收。全量与快检结论以宿主实际执行为准。
