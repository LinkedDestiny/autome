# W6 项目页与任务卡：四列看板、需求/Bug 筛选、网页上用模板创建任务卡（含附件）

## 背景

计划 `docs/plans/2026-11-10-web-dashboard-via-loop.md` §6「W6 项目页与任务卡」。W5（T-35）已经合并：外壳、左侧项目列表、首页总览、⌘K、中文标签表 `app/lib/labels.ts`、`app/load-context.ts`（`getLoadContext` 把 `gateway` 与面板库交给 loader/action）、`app/components/{task-columns.ts,overview-card.tsx,shell-data.ts,project-list.tsx}` 都在了。K1（T-13）给了 `task.create` 的 `kind` / `brief` / `draft` / `attachments` 与 `task.start`；T-37（网关放行 `task.create` 的 `kind`/`brief`/`draft` 与 `task.start`）正在收尾，W6 的「创建并开始 / 存为待开始」依赖它。

## 要做什么

1. 路由 `apps/web/app/routes/projects.$id*`：项目页按 §3.2 的四列显示任务卡——**待开始 / 进行中 / 等你处理 / 已完成**（列的归属规则复用 W5 的 `task-columns.ts`，不要再写一份）；顶部可按需求 / Bug 筛选。
2. 任务卡组件 `apps/web/app/components/task-card*`：类型标签、id、标题、阶段进度条（§4.3，需求与 Bug 各一张表，来自 `labels.ts`）、一句当前动态（如「实现第 3 轮 · M-02 · 运行 12 分钟」「等 Claude Code 冷却到 14:20」「等你合并」）、门禁结果、等你处理角标。
3. 新建任务卡：选类型 → 模板预填的 Markdown 编辑器（§3.1 的两套模板）→ 附件（有大小上限，服务端 `apps/web/server/uploads*` 校验后落到任务附件目录，经 `task.create.attachments` 交给内核）→ 「创建并开始」或「存为待开始」（`draft`）。
4. 待开始的卡上有「开始」按钮（`task.start`）。
5. 任务被另一个进程（命令行、驱动）推进时，卡片不刷新页面就换列（W3 的 `useLiveEvents`）。
6. 「等你处理」角标的未读数用 W4 面板库的 `store.unreadByTask(<级别为「需要你」的通知 kind>)`，库对象经 `getLoadContext` 取（W4 的 Backlog B-02）。

## 不做什么

- 任务详情页（W7）、会话日志（W8）、项目设置与 Onboarding（W9）。
- 不新增 JSON-RPC 方法；不改 `crates/`；不改 `apps/desktop/`。
- 不做拖动排序（T-53 之后另提）。

## 验收标准

1. 列定义与 §3.2 一致，有测试与 `task-columns.ts` 交叉核对；筛选只影响显示。
2. 卡片内容覆盖上面第 2 条的每一项，Bug 任务显示修复轮次而不是里程碑。
3. 新建任务卡：两种类型的模板预填正确；附件超过上限被服务端拒绝并给中文提示；「创建并开始」后卡片出现在进行中列，「存为待开始」出现在待开始列。
4. 待开始的卡点「开始」后不刷新页面进入排队 / 进行中。
5. 用命令行推进一个任务（`autome approve` 等），浏览器里的卡片在一个 tick 周期内换列（集成测试）。
6. 未读角标与 W4 库一致（测试）。
7. `npm --prefix apps/web test`、lint、typecheck、build 全绿；`sh scripts/ci-local.sh` 全绿；1512×944 下项目页 `scrollHeight ≤ clientHeight`（列内部滚动除外）。

## 参考

- 计划 §3.1（模板）、§3.2（列）、§4.3（进度表示）、§5.2（WebSocket）、§5.5（本机安全：上传同样要过 Host/Origin 检查与大小限制）。
- W5 产物：`apps/web/app/components/`、`apps/web/app/load-context.ts`、`apps/web/app/lib/labels.ts`、`apps/web/test/task-columns.test.ts`。
- 内核：`crates/automed/src/dispatch.rs` 的 `task.create`（`kind`/`brief`/`draft`/`attachments`/`doc_refs`）、`task.start`、`project.get`、`dashboard.get`；`apps/web/server/gates/write-gate.ts` 的字段上限（T-37 放行后的版本）。
- 设计 token 与硬规则：`apps/web/app/styles/tokens.css`、`docs/development/` 下的设计规范。
