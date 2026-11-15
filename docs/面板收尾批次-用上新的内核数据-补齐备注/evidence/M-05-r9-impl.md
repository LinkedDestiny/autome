# M-05 · 实现 #9

结论：待审。修复审计 #5 指出的浏览器夹具缺陷；无产品代码改动。按设计「总-Q1」与验收契约 ★ 条款，浏览器启动受沙箱拒绝的部分交审计/宿主复验，不声称浏览器通过。

## 修复与回归

- 将原 `startWindowTask` 的脚本生成原样抽到 `e2e/harness/window-task.ts`，浏览器用例调用同一生成器再调用 `newTask`。每步 `next-action` 加步号，避免同轮设计/评审产生相同文档、`git commit` 返回 1。
- C-2 的原判别断言及其余浏览器断言逐字保留，未改超时、重试、skip 或 Playwright 配置。审计检查本来就在项目测试体系内，无需复制成第二个同名测试。
- C-2 的 finally 中 stop 失败不再覆盖主断言；全局 teardown 仍负责进程清理。
- 新增 `test/window-task.test.ts` 并登记 unit 分组：在隔离临时 Git 仓库逐个执行实际生成的 23 个 shell 脚本，每步断言退出码为 0、累计提交数等于步号。22/23 步预放行，验证其提交路径，不冒称覆盖真实内核调度或持有等待。临时仓库关闭 hooks，finally 删除。

## 命令与结果

原始输出在 `.autome/output/`，以下均从 worktree 根目录执行。

1. 修前原样 `(cd apps/web && npx playwright test e2e/manual-items.spec.ts e2e/layout.spec.ts)`：启动处 `spawnSync ps EPERM`，0 例执行；日志 `m05-r9-red.txt`。这不是审计缺陷的红证据，不能冒称复现了 sessions=3。
2. 原样抽取脚本后、修复之前，`(cd apps/web && npx vitest run test/window-task.test.ts)`：1 例失败，明确在 step 3 返回 1；日志 `m05-r9-fixture-red.txt`。它直接复现审计定位的相邻文档无差异导致提前退出，保留在项目测试体系。
3. 修后 `(cd apps/web && npx vitest run test/task-detail-payload.test.ts test/task-detail-data.test.ts test/task-timeline.test.ts test/test-groups.test.ts test/window-task.test.ts)`：5 文件 **42/42**，0 跳过；日志 `m05-r9-unit.txt`。原契约四文件仍为 41 例，与上一轮及审计基线 **差 0**；新增夹具回归 1 例使本轮子集 **+1**，没有删测。
4. `cargo build -p automed -p autome-cli`：通过，`m05-r9-cargo.txt`；`npm run build --prefix apps/web`：通过，`m05-r9-build.txt`。
5. 修后再次原样运行第 1 项命令：退出 1，仍是启动前 `spawnSync ps EPERM`；日志 `m05-r9-browser-final.txt`。`(cd apps/web && npx playwright test e2e/manual-items.spec.ts e2e/layout.spec.ts --list)` 为 **19 例**（manual-items 7、layout 12），相对上一轮 **差 0**，日志 `m05-r9-list.txt`。上一轮审计执行的 18 通过/1 失败，本轮不能报告为已变成 19 通过。
6. `(cd apps/web && npx eslint e2e/manual-items.spec.ts e2e/harness/window-task.ts test/window-task.test.ts test-groups.ts)`：退出 0，`m05-r9-lint.txt`。`git diff --check` 通过。未执行全量门禁、快检或全仓静态门禁。

本轮唯一用例集合共 **61**（42 单测 + 19 浏览器），较上一轮对应 60 **+1**，来自新夹具回归；实际执行并通过 42，浏览器 19 未启动，不记为 skip。红/绿重跑不是新增用例，不重复累计。

## 标待审之前的四项自审清单

1. **情形表逐行**：C-Q1 服务端视图/字段裁剪由 payload 4 例通过；默认 20/固定 from/新增 5/更早全量由 data、payload 通过；标签按需读取与查询参数保留由 data 通过；历史实现会话依赖只读回归通过。C-Q2 60 会话体积夹具仍通过，未改变数据骨架。C-Q3 浏览器 C-2/C-2b/W7/一屏断言保留，19 例启动被拒，待审计补跑。新增夹具步骤 1–23 均逐步执行通过，覆盖审计点名的 2→3 重复文档。
2. **设计点名的失败分支**：本次 data 测试执行了内核不可达/null、503/504/404/not_found/其它错误、配置/历史失败降级、文档拒绝/缺失服务等分支，均通过。夹具脚本失败已先红后绿；stop 清理异常只做局部静态检查，未实跑真实内核已失败任务的 finally。未新增授权或 HTTP 接口，其新增失败分支不适用。浏览器与 teardown 的 ps 权限错误保留报告，没有修改环境检查。
3. **全部迁移**：data 执行默认→固定 from、from→追加、非法 from→默认、from→all、all/已有 from 下不重复钉住、文档/标签参数保留；timeline 22 例通过。夹具 1→23 的所有脚本提交迁移通过，特别是相同 round 的相邻步。真实第 22 会话持有→第 23 会话、新会话/展开更早后 DOM 与两层滚动保持，仍需浏览器复验。没有新增产品状态机。
4. **输入域边界**：data/payload 执行空会话及 1/19/20/21/60/65、无效/空 from、空文档与路径拒绝、查询编码与 all 优先；通过。新增夹具执行首步 1、旧失败边界 2→3、持有分支 22/23（预放行）；生成器仅供固定测试 slug 调用，未引入用户输入域。日期解析未变，闰日/跨年不适用本次夹具修复。

## 未完成的取证与交接

- 浏览器负向对照三项（不钉住窗口、按序号 key、withParams 丢键）无法越过相同 ps 预检，本轮未注入，不把上一轮纯函数变异当作浏览器证据。审计需按 M-05 契约运行它们。
- 原样审计浏览器检查无法取得红→绿证据；本轮提供实际共享脚本的确定性红→绿，不能替代真实 22/23 会话窗口、日志和滚动验收。
- 浏览器在 setup 的进程表预检即失败；未手动启动常驻服务、未访问真实用户账本。新增临时 Git 仓库已由 finally 回收。
- M-05 标待审，reopen 仍为 1，implementation-round 为 9/65。审计先补跑浏览器及负向对照；通过后下一实现轮推进 M-06。
