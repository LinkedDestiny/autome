# M-01 · 实现轮 1/55

结论：里程碑验收通过，报待审；快检浏览器阶段受沙箱限制，不能声称全量通过。没有上一轮审计及待搬迁的审计回归检查。

## 改动

- `.seg` 统一链接、按钮、radio 的尺寸、颜色、白底阴影和黄色焦点环；横向溢出可滚动，不再纵向拉伸为通栏。
- `.seg span` 收窄到 `.seg label > span`，保留任务页签计数胶囊外观。
- 看板筛选添加 `seg`，时间线用 `SegmentedTabs`；删除看板、时间线、任务详情页签自己的选中外观，只保留页面布局。
- 新增 `ui-base` 4 条并登记 unit；`ui-kit` 新增链接分段的空/单元素与语义检查。原有样式守卫未放松。
- 现有 e2e「1512×944 Loop 配置」补选中底色、阴影、4px 间距和全局/项目往返检查；用例数量不变。技能项目选择按设计留给 M-06，文档胶囊留给 M-08。

## 命令与结果

1. 基线：`(cd apps/web && npx vitest run test/ui-kit.test.ts test/styles.test.ts test/task-timeline.test.ts test/board.test.ts test/testids.test.ts test/test-groups.test.ts)`：6 文件，102 用例通过；与设计轮同范围基线差 0。输出 `.autome/output/m01-baseline.txt`。
2. 最终验收：`(cd apps/web && npx vitest run test/ui-base.test.ts test/ui-kit.test.ts test/styles.test.ts test/task-timeline.test.ts test/board.test.ts test/testids.test.ts test/test-groups.test.ts)`：7 文件，**107 用例全部通过，较基线 +5**，来自 ui-base +4、ui-kit +1；各文件分别 4、21、15、29、19、15、4；0 失败、0 跳过。输出 `.autome/output/m01-tests.txt`。
3. 三个设计负向对照，逐一临时改动，每次运行 `(cd apps/web && npx vitest run test/ui-base.test.ts)`，随后恢复：删除链接选中选择器为 1 失败/3 通过；恢复 `.seg span` 为 2 失败/2 通过；恢复裸 `nav.timeline-filters` 为 2 失败/2 通过。每次总数 4，与本轮 ui-base 基线差 0。输出 `.autome/output/m01-negative-{selected,pill,timeline}.txt`。恢复后第 2 项最终验收全绿。
4. 关联子集：`(cd apps/web && npx vitest related --run app/components/task-timeline.tsx 'app/routes/projects.$id.tsx' app/styles/app.css)`：30 文件，共 237 用例，229 通过、2 失败、6 因套件初始化失败未执行。不是主动增加跳过；此前无此范围实跑基线，不与 107 混算差值。输出 `.autome/output/m01-related.txt`。
5. 最终静态检查：`(cd apps/web && npm run lint && npm run typecheck)` 通过，`git diff --check` 通过。输出 `.autome/output/m01-static-final.txt`。
6. 指定快检：`sh scripts/ci-quick.sh --since 396669f43c6d20f18f226c0ba5267b2dd0f25058`，退出 1，168 秒，95 步执行、0 沿用、1 失败（Web e2e）；实跑 unit 合计 800 用例通过，78 条命令由脚本按 120 秒预算留给合并前门禁。此前无同计划快检基线，范围随选取和预算变化，不把 800 与里程碑 102 作差。完整输出与未跑命令在 `.autome/output/m01-quick.txt`。未运行 `sh scripts/ci-local.sh`。
7. 旧标记检索：`rg -n 'timeline-filters|board-filters|task-detail-tabs' apps/web/test apps/web/e2e apps/web/scripts`，修改前 1 行、修改后 3 行：原 task-page 的 `seg task-detail-tabs` 断言及新增 ui-base 的统一样式/组件守卫，没有遗留旧外观断言。输出 `.autome/output/m01-old-{before,after}.txt`。

## 标待审之前的四项自审

1. **情形表逐行**（A-Q1）：
   - Loop 范围：全局和项目 SSR 均为 nav.seg、有当前标记；项目页签分别渲染配置/项目信息当前态；通过 ui-base。
   - 任务详情页签：docs/changes/decisions/notes 各渲染一次，恰一个当前项且文字对应；计数 Pill 保持原类；通过 ui-base。
   - 看板任务类型：SSR 的按钮组带 seg、aria-pressed；原 board 的全部筛选组合仍通过。
   - 时间线：全部/门禁/退回分别 SSR，当前标记准确；原 task-timeline 的 URL 参数保留与返回入口检查通过；源码确实使用 SegmentedTabs。
   - 技能项目选择：不适用，本轮设计明确留给 M-06。
   - 既有 radio：未改组件；ui-kit 的两态、双向回调、空/单元素、禁用检查通过，共享 CSS 的 checked/focus 规则通过。
   - 文档选择：不适用，是文档胶囊，按 M-08 处理。
2. **失败分支**：A-Q1 没有新增授权、读盘、超时或外部服务失败分支，不适用；空选项和禁用项通过 ui-kit，原 board 的 503/409/无网关用例仍通过。新增 SSR 测试最初误写标签名、遗漏 onboarding 夹具字段导致失败，按生产标记与现有夹具形状修正后通过，未改生产契约。
3. **全部迁移**：本轮不新增状态机。radio 全局→项目、项目→全局回调由 ui-kit 执行；看板 all/feature/bug 的 3×3 组合由 board 执行；时间线三个筛选的选中项及返回链接由 ui-base/task-timeline 检查；四个详情页签与两个 Loop 页签均逐态 SSR。真实浏览器点击往返已写进 e2e，受环境阻塞尚未取得通过证据，不能把 SSR 当成真实交互验收。
4. **输入域边界**：分段组件空子项、单链接、空 radio 选项、单个禁用项、整体禁用均执行通过。CSS 容器的 fit-content/max-width/横向滚动与不影响嵌套胶囊均有断言。数值、日期、前导零等不适用，分段控件没有这些输入域。

## 环境阻塞与宿主复验

- `task-timeline-real.test.ts`「正常出口无存活进程」及清理：`spawnSync ps EPERM`。
- `shell-live.test.ts`「3 listening development SSR contains the real project and needs-you count」：`listen EPERM 127.0.0.1`。
- `task-live-real.test.ts` 的「CLI 暂停后一个 tick 内收到事件、重验并返回新状态」「八个前缀使用真实内核事件，本任务重验、其它任务不重验」「收尾无存活进程」，以及 `task-log-live.test.ts` 的「页面历史会话折叠，运行中会话默认展开」「时间线历史与运行会话均能接入 W8 日志订阅且收到增量」「收尾无存活进程」：套件初始化监听端口被拒，清理 ps 也被拒，因此 6 条未执行。
- 快检 Web e2e 在 setup/cleanup 被 ps EPERM 拒绝，未取得本轮 Loop 选中外观浏览器证据，也无法通过该扫描证明无遗留进程。未连接真实面板或账本。
- 宿主重跑：`(cd apps/web && npx vitest related --run app/components/task-timeline.tsx 'app/routes/projects.$id.tsx' app/styles/app.css)`；浏览器重点复验：`(cd apps/web && npx playwright test e2e/layout.spec.ts --grep '1512×944 Loop 配置')`；再执行宿主规定的同款快检。真实账本目视仍属 H-01/H-02。
