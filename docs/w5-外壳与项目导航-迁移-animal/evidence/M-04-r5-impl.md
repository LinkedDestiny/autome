# M-04 · 实现 #5

## 范围与实现

仅推进 M-04（R-01、R-02），不推进 M-05。起点 a6c2096，工作区干净。根 loader 调用 loadShell，通过 gateway.read 获取一次 project.list，并行读取每个 project.get；按 countByColumn 计算项目计数，首页用同一 columnOf 归列。保留终态任务供后续跳转候选使用，剔除 request/metrics 等非白名单字段。新增项目卡片、首页两列、简版任务卡与编码链接，沿用 animal-island token、内部滚动、焦点环。状态行集中在 labels.ts。

已有测试的必要接线调整：server 两条首页断言由占位文案改为“总览”；load-context 的假网关区分 project.list/project.get，原一次读断言改为精确两次读及调用参数，仍覆盖开发与生产 SSR、空列表和错误。没有新增依赖，没有修改内核、desktop、事件总线、网关白名单或 M-05。

上一审计 #4 通过 M-03，没有退回的缺陷检查，故无须逐字迁移审计回归。开发中修正了 JSX 替换残片、假网关 undefined 与 undefined 误匹配，以及测试误把 events.since 发给页面白名单的问题；事件取证现用 CoreSupervisor，与服务端事件总线相同，生产 loader 仍只使用 gateway.read。

## 标待审之前的自审清单

1. **情形表逐行**：shell-data #2 与 task-columns #1 检查 draft、queued、queued.resume → todo；active implement/gate → running；await_design_approval/await_merge/await_info/not_a_bug → needs_you；paused/stopped/failed → needs_you；done → done；cancelled/未知 → 不入列。项目计数和卡片集合一致，数字 ID 倒序（T-12 在 T-9 前）。overview #1–4 检查有数据、计数 0、单项目失败“—”及原因、无项目、空列、离线双列、全部等人状态文案、类型/ID/标题/项目/阶段，以及任务链接。通过。
2. **设计点名的每条失败分支**：shell-data #3 对拒绝读取、读盘失败、504 超时、503 离线逐一注入 project.get，失败项目 counts=null/error 原样，其他项目正常；#4 对列表超时、null 网关返回离线空数据；load-context #5 对列表 503/504/409 原样传递。空输入由 #4 和 overview #3 覆盖。无外部远程服务，本功能仅访问本地内核，不适用外部服务授权交互。监听限制另列，未伪装通过。
3. **全部状态迁移**：本层无独立状态机，不修改内核迁移规则。所有读模型载体由 task-columns #6 覆盖（active.node、paused.resume、queued.resume、stopped.at、failed.at）；shell-data #3 实跑项目读取失败→恢复；既有 live-status 检查 offline→connected 与首个 hello 已 connected 两条恢复路径。shell-live #2 使用真实 CLI 令 active→stopped，真实账本事件交给根 shouldRevalidate，经路由重验后两个计数各变化 1。load-context #9 检查 task/project/bug/merge 触发、config/session 不触发。通过；浏览器真实 WebSocket 交互仍归 H-04/H-05。
4. **输入域边界**：空项目、空任务、单卡、计数 0、fix_round=0、未知状态、前导零 T-02 与 T-10 数字排序、ID 空串与 a/b ?中 编码均实跑；task-columns 既有空串/未知值/单 stopped 回归通过。日期、闰日、跨年不适用，本里程碑不解析日期；ID 数值由内核生成，未新增最大值契约。

## 负向对照（均已还原）

- 计数改为直接取 counts.awaiting_user：`npm test --prefix apps/web -- test/shell-data.test.ts test/shell-live.test.ts -t '^[12] '`，退出 1，shell-data #2、shell-live #1/#2 三条变红。日志 `.autome/output/r5-negative-counts.txt`。
- 根匹配器删除 task.：`npm test --prefix apps/web -- test/load-context.test.ts test/shell-live.test.ts -t '根重验|^2 '`，退出 1，load-context #9 与 shell-live #2 变红。日志 `.autome/output/r5-negative-events.txt`。
- taskHref 去掉编码：`npm test --prefix apps/web -- test/overview.test.ts`，退出 1，overview #2/#5 变红。日志 `.autome/output/r5-negative-encoding.txt`。
- 以上筛选命令里的 skipped 是明确未选中的用例；不混入全量验收统计。还原后单元与真实内核复验结果见下。

## 命令与结果

- `cargo build -p automed -p autome-cli`：退出 0（1.24 秒），日志 `.autome/output/r5-cargo.txt`。
- `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`：最终均退出 0，日志 `r5-lint-final.txt`、`r5-typecheck-final.txt`。
- M-04 原样验收：`npm test --prefix apps/web -- test/shell-data.test.ts test/overview.test.ts test/shell-live.test.ts test/server.test.ts test/live-routes.test.ts`，退出 1，30 个用例，20 passed、1 failed、9 skipped；79.32 秒。相比这组文件原有 server 9 + live-routes 7 = 16 个，增加 **14** 个（shell-data 6、overview 5、shell-live 3）。日志 `.autome/output/r5-target-final.txt`。
- 上述唯一失败是 shell-live #3 `listening development SSR contains the real project and needs-you count`，报 `listen EPERM 127.0.0.1`。server 的 beforeAll 启动 dev 时报 watcher EMFILE/监听 EPERM，导致以下 9 条未执行：dev /healthz；dev 首页 SSR 与模块脚本；prod /healthz；prod 首页与 JS 资源；prod 未知路径 404；只监听 IPv4 回环；端口占用中文提示；HMR 外站 Origin 403；首页错误 Host 403。交审计轮原样复验，不删除、不跳过、不降低断言。
- 负向对照还原后：`npm test --prefix apps/web -- test/shell-data.test.ts test/overview.test.ts test/live-routes.test.ts test/load-context.test.ts test/labels.test.ts test/task-columns.test.ts test/shell.test.ts`：退出 0，**44/44**，skipped 0，16.23 秒；该选择集原有 33 个，本轮增加 11（6 + 5）。日志 `.autome/output/r5-green-unit.txt`。
- 负向对照还原后：`npm test --prefix apps/web -- test/shell-live.test.ts -t '^[12] '`：退出 0，2 passed、1 因命令筛选未选中；总数 3，新增 3。真实内核 setup/cleanup 总计 8,369 ms，单次 loadShell 10 ms。这是明确排除监听用例后的补充证据，不替代原样验收；日志 `.autome/output/r5-green-live.txt`。
- `sh scripts/ci-local.sh` 的 Rust Test 步骤：退出 0，共 **1,095** 个（与上一实现轮基线相同，差 **0**），259 秒。独立 End to end 步骤退出 101，81 个中 80 passed、1 failed，267.47 秒；失败为 `bug_regression_guard_checks_each_workspace_member`，`crates/automed/tests/end_to_end.rs:5125` 等待结束时状态仍为 `Active { node: MergeGate }`。本轮未修改内核。随后 `cargo test --test end_to_end bug_regression_guard_checks_each_workspace_member -- --exact --nocapture` 退出 0，1 passed、80 filtered out，77.56 秒（`.autome/output/r5-rust-recheck.txt`）。此用例在 Rust 全量与单独重跑通过、独立端到端全量失败，原因未核实，不能称为修复。
- 门禁 Desktop：`npm test --prefix apps/desktop` 退出 1，**104** 个（相对上一轮差 **0**），103 passed、1 failed、0 skipped。`electron dom harness: real-DOM assertions against the actual renderer` 的子进程退出码为 null，断言期望 0，stderr 为空；单独在 apps/desktop 执行 `node --test test/dom.test.js` 仍退出 1、1 failed（`.autome/output/r5-desktop-recheck.txt`）。与上一实现轮 103 passed + 1 skipped 的分布不同，本轮确实尝试执行 harness 但失败，原因未核实；未改 Desktop、未把失败改为 skip。
- 门禁 Web 全量：`npm test --prefix apps/web` 退出 1，42 文件、**357** 个用例，326 passed、12 failed、19 skipped，160.98 秒。相对上一审计运行基线 **343 增加 14**（shell-data 6 + overview 5 + shell-live 3），没有删除用例；设计中的 343 是静态枚举数，运行时多 14 个既有参数化展开，已澄清 M-04 预期。失败为 gateway-http 全部 11 条和 shell-live #3 的监听 EPERM；eventbus-live 7、notifier-live 3、server 9 因监听/EMFILE 初始化失败未执行。相对上一实现轮的分布增加 13 passed + 1 failed，未执行数不变；不可报告 skipped=0。
- 门禁下 `shell-live.test.ts` 三条共 13,684 ms，包含 setup/cleanup 的实测为 **13,776 ms**；单次 `loadShell` **17 ms**。测试及 beforeAll 上限 **120,000 ms**，超过整文件实测的 2 倍（27,552 ms）；fixture 命令上限 60,000 ms 也超过该值。监听用例成功路径的完整耗时仍需宿主复验，不能拿 EPERM 快速失败推断成功耗时。
- 门禁 `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`npm run build --prefix apps/web` 均退出 0。Format、Clippy、Rust Test、Adapter flags、Contract 亦通过；门禁脚本整体退出 1，总耗时 **828 秒**，失败步骤是 End to end、Desktop、Web test。完整日志 `.autome/output/r5-ci.txt`。不把单独重跑成功当成全门禁成功。
- `git diff --check` 退出 0。三个负向变更均已还原，当前 diff 只有 M-04 实现、测试与本轮记录。浏览器截图/1512×944/真实键盘及 WebSocket 视觉验收未取证：本环境无法监听，沿用 H-01–H-07 人工项，不声称完成视觉验收。

## 结论与后续

M-04 标为**待审**，不是已完成。非监听实现与负向对照通过，审计轮需在可监听环境复跑原样验收和全量门禁，并确认上述 Rust 偶发失败及 Desktop harness 失败。状态写 implementation-round: 5/25，保留其余里程碑状态；本轮不启动下一会话、不推送。
