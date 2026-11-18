# M-07 · 实现 #10 · 宿主快检修复，保持待审

## 范围与根因

起点 `77b0fb02938e7ab3a86b2996f5f33f6b530f5993`。本轮不推进新里程碑。宿主失败检查已在项目测试体系 `apps/web/e2e/layout.spec.ts`「1512×944 需求详情展开日志」中：第 85 行原断言 `expect(box.y + box.height).toBeLessThanOrEqual(bounds!.y + bounds!.height);`。宿主实际为 942.53125 > 920，发生在展开日志之前，违反总-Q1 首屏十条完整可见契约。本轮逐字保留该检查，无需另复制同一用例。

40px 行高之外仍叠加旧布局的行上下 2px 外边距、分组标题上下 8px 外边距；前轮只取得静态行高证明，沙箱未执行几何断言，漏掉了这些累计高度。本轮统一将时间线行纵向外边距归零、分组标题纵向外边距减为 4px。保留横向 12px、字体、40px 最小行高、边框、分组排序、筛选、内容与展开行为。检查同类位置仅这两处，未修改其它页面密度或 M-08 内容。

## 红绿与命令证据

1. 修复前原样运行 `(cd apps/web && npx playwright test e2e/layout.spec.ts -g '1512×944 需求详情展开日志')`，日志 `.autome/output/m07-r10-red.txt`：globalSetup 被 `spawnSync ps EPERM` 拒绝，0 个实际用例执行。不能将环境错误称为本地红复现；产品红证据来自用户提供的宿主输出。
2. 先新增间距守卫，再运行 `(cd apps/web && npx vitest run test/task-timeline.test.ts)`：36 个，35 通过、1 失败（新增间距守卫），日志 `.autome/output/m07-r10-density-red.txt`。
3. CSS 修复后运行 M-07 原验收命令 `(cd apps/web && npx vitest run test/task-timeline.test.ts test/timeline-failure-path.test.ts test/task-detail-ssr.test.ts test/task-page.test.ts test/testids.test.ts test/styles.test.ts)`：6 文件，85/85 通过，0 跳过；包含新增守卫的绿色结果，SSR 150 KB 上限也通过。日志 `.autome/output/m07-r10-accept.txt`。相对上轮同命令 84 个 **+1**，仅新增首屏密度间距守卫，没有删除或放松原断言。
4. `(cd apps/web && npx vitest related --run --project unit app/styles/app.css)`：4 文件，33/33 通过，0 跳过；日志 `.autome/output/m07-r10-related.txt`。本轮没有增删该集合用例，差 0；上轮 related 244 的范围同时包含两个 TSX/TS 模块，不能与本轮只改 CSS 的集合混为基线。
5. 修复后再次原样运行第 1 条浏览器命令，仍在 globalSetup 遇到 `ps EPERM`，0 个实际执行，前后差 0。日志 `.autome/output/m07-r10-browser-after.txt`。原布局断言的本地绿结果未取得；不能把静态守卫当作浏览器验收，交宿主复验。

没有运行 `sh scripts/ci-local.sh`，没有改动浏览器夹具、端口限制或失败标准。

## 标待审之前的自审清单（本轮维持待审）

1. **情形表逐行**：D-Q1 会话、门禁、用户、系统行沿用同一行外边距规则，分组标题同一规则；task-timeline 验证四类别、完整文字可达、费用结果、长摘要和分页入口。D-Q2 文字与日志独立展开、运行中日志默认打开的结构用例均通过。总-Q1 首屏十条、行高 ≤44px 的原浏览器断言保留，执行受阻，未声明通过。
2. **失败分支**：原验收内 timeline-failure-path 6 个用例通过，覆盖门禁失败/超时、准备失败、空或缺失路径、中文路径与原始字段；task-timeline 的读失败/截断/崩溃等既有用例通过。本轮只改 margin，授权拒绝与外部服务不可达不适用，没有新增请求或权限分支。
3. **全部迁移**：既有模型测试中运行→结束/停止/超时/崩溃、门禁排队→运行→结果、筛选与退回均通过；文字/日志的关闭与打开结构守卫通过。没有新增状态或事件，原浏览器折叠→展开→折叠与日志展开流程未改动，但本地受环境阻塞未实跑。
4. **输入域边界**：空列表、单条、长文字、无失败详情、earlier=0/1 等既有验收通过。前导零、闰日、跨年、最大值不适用：本轮无输入解析改动。1512×944 的十条可见边界由原 e2e 继续守卫，窄屏视觉未实测，不作通过声明。

## 状态与交接

implementation-round 为 10/65；M-07 保持待审，M-08 及之后未推进，repos 未增删。后续由宿主重跑快检并独立审计，本轮不启动下一会话。原始长输出均在 `.autome/output/`；设计文档只更新状态和最新证据指针。

## 指定快检最终结果

`sh scripts/ci-quick.sh --since 77b0fb02938e7ab3a86b2996f5f33f6b530f5993` 退出 1：21 步，19 成功、2 失败，58 秒，相关阶段 36 秒，未超 120 秒准入预算。日志 `.autome/output/m07-r10-quick.txt`。Format、Clippy、lint、typecheck、Web build 均通过。

Vitest 九次文件运行共 89 个用例：88 通过、1 失败，无跳过；相对上轮快检 281 个表面差 -192，由 --since 起点变化和本轮仅 CSS/测试改动导致相关测试选择范围缩小，并非删除用例；同范围唯一新增为 task-timeline 的 1 个守卫。该集合与 M-07 验收有重叠，不相加当成去重总数。

失败项：shell-live 第 3 条监听开发 SSR，`listen EPERM: operation not permitted 127.0.0.1`（同时打印 watcher EMFILE）；Web e2e 在准备/清理阶段 `spawnSync ps EPERM`，35 条浏览器用例未执行，较宿主基线实际执行 35 条少 35 条，原因是环境阻塞。未改测试跳过、监听逻辑或清理检查来伪造通过。原几何回归仍须由宿主执行确认，快检最终结论以提交后宿主重跑为准。

`git diff --check` 通过；状态检查确认 10/65、M-07 待审及唯一最新证据指针。
