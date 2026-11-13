# M-02 实现轮 3

## 修复与范围

审计退回的根因是 `test/w9-http.test.ts` 的 `post` 无条件解析 JSON。`securityMiddleware` 对 `/api/` 返回 JSON，对页面返回 `text/plain`。辅助函数现按 Content-Type 解析，文本保留原文；JSON 解析异常通过 Promise reject 报告，避免异步回调未捕获异常。响应体类型包含 string，读取成功添加回包前明确检查类型。未改产品行为。

同类检查：检索项目 HTTP 测试中的 `JSON.parse(text)`，本次涉及的无条件解析位于此辅助函数。它服务本文件全部四个用例。审计判别检查已在项目测试体系中，原样保留「添加页 action 与 single fetch 坏 Host/Origin 拒绝且无注册」，未删断言、未改为跳过。

## 命令与结果

- 修复前原样运行：`cd apps/web && npx vitest run test/w9-http.test.ts`。4 例全部在启动服务时遇到 `listen EPERM: operation not permitted 127.0.0.1`。原始输出 `.autome/output/M-02-r3-red.txt`。这是环境失败，未复现到审计的 JSON SyntaxError，不能算目标缺陷的红证据。
- 修复后原样运行同一命令：4 例仍受同一 EPERM 阻塞，未取得绿证据。输出 `.autome/output/M-02-r3-after.txt`。
- M-02 验收命令：`cd apps/web && npx vitest run test/add-project.test.ts test/add-project-real.test.ts test/w9-http.test.ts test/w9-layout.test.ts test/palette.test.ts test/shell.test.ts test/labels.test.ts test/styles.test.ts test/bell-view.test.ts`。
- 验收结果：9 文件，59 用例，55 通过、4 失败、0 跳过；失败均为 HTTP 服务监听 EPERM。与上一轮审计基线 59 例相比差值 **0**，没有增加、删除或跳过用例；上一轮 58 过 1 败，本轮环境提前阻塞了整个 HTTP 文件，其余 8 文件全部通过。输出 `.autome/output/M-02-r3-acceptance.txt`。
- `git diff --check` 通过。未运行全量门禁、宿主快检或无关测试。

## 标待审之前的四项自审

1. **情形表逐行**：本轮执行的 add-project 11 例与 real 4 例覆盖：不存在目录提示、已有非仓库提示、Git 根接管提示、工作区成员及建议提示；默认 docs 选择；确认前检查与路径相同约束；规范化 loader 仅调 probe；两种跳转；确认拒绝原文；skip 失败留下项目及设置链接；原生按钮 SSR 隐藏与函数识别；空/非空侧栏及命令面板入口；离线禁用；补全防抖/乱序/网络失败/卸载取消。以上通过。layout、labels、styles 对布局规则与文案守卫通过。页面及 single fetch 的 Host/Origin 拒绝与无注册检查已执行，但在监听阶段阻塞，未取证。
2. **失败分支**：add-project 第 4 例逐项注入拒绝授权、读盘失败、请求超时、内核未连接，原文返回通过（替身错误透传测试，不冒充真实磁盘故障）；第 6、7 例覆盖确认拒绝及 add 成功后 skip 失败；第 11 例覆盖补全外部请求不可达；real 第 1 例空输入拒绝、第 4 例真实重复目录拒绝通过。HTTP 安全拒绝因 EPERM 未取证。
3. **全部迁移**：未检查/空输入→不可确认，检查相同路径→可确认，路径改变→不可确认；确认不跳过→设置页，跳过→项目页且真实 init commit；确认拒绝→错误，添加成功但 skip 失败→保留链接且不可重复添加；补全输入→150ms 请求→最新响应、旧响应被隔离、失败→错误、卸载→取消，均由本轮上述测试通过。无新增状态机。
4. **域边界**：real 第 1 例覆盖空串、相对路径、家目录外、`..`、NUL、超过 512 字符拒绝与 512 字符路径通过；add-project 第 2 例单成员、第 9 例空/非空项目列表、第 8 例原生能力缺失/非函数/函数，第 11 例 149ms/150ms 边界，均通过。前导零、闰日、跨年不适用：此里程碑无日期或数值输入域。

## 结论

修复已落地，完整 HTTP 红绿验证受沙箱监听权限阻塞。M-02 保留「开放」，reopen 不变为 1，implementation-round 为 3/30。下一轮在允许本机监听的环境原样运行 HTTP 回归及 M-02 验收，取得 59 例全过后才能标待审。人工 H-01/H-02 本轮未取证。未启动下一会话、未推进其他里程碑。
