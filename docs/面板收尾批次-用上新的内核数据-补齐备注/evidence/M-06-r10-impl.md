# M-06 · 实现 #10

结论：D-1…D-4 已实现；M-06 保持开放。浏览器初始化与 D-5 进程诊断受沙箱 `ps EPERM` 阻断，未取得完整验收通过证据。上一轮审计通过 M-05，没有本里程碑退回检查需要迁入。

## 改动与验收

- D-1：离线前建草稿并等待页面显示、检查非空事件检查点；离线后从真实内核查该任务 task.created 序号，恢复后同时检查卡片和浏览器 data-live-seq。客户端仅在收到新 event 时写入，hello 与重复事件不推进；新增单测覆盖。
- D-2：项目页巡检断言四个列体有滚动标记、待开始列确实溢出、overflow-y 为 auto/scroll、实际滚动成功且页面 scrollTop 仍为零。
- D-3：cargo 汇总识别 Doc-tests 目标并重置失败段；新增前置单测目标与失败 doctest 同日志的回归。
- D-4：⑪ 检查 Build core/Web test 标题都存在且顺序正确，并检查 BUILD/WEB 实际输出及顺序。
- `cargo build -p automed -p autome-cli`、`npm run build --prefix apps/web` 均通过（m06-cargo.txt、m06-build.txt）。未运行项目全量或快检门禁。
- `node --test scripts/test/ci-summary.test.mjs scripts/test/ci-reuse.test.mjs`：26 条全部通过，0 失败/跳过，109.5 秒（m06-node.txt）。⑪ 恢复 prep 后单独复跑通过（m06-reuse-green.txt）。
- `(cd apps/web && npx vitest run test/w9-cleanup.test.ts test/live-client.test.ts test/test-groups.test.ts test/core-sidecar.test.ts)`：4 文件 35 条全部通过。契约子集为 cleanup 3 + live-client 9 = 12；守卫另外 23 条。
- `(cd apps/web && npx playwright test e2e/live.spec.ts e2e/layout.spec.ts)`：全局 setup 的 leftoverProcesses 调用 ps 被拒，14 条均未执行；不是 14 条通过或条件跳过。setup 在 prepare/spawn 前拒绝，未启动本轮浏览器、面板或内核服务。
- 全部原始日志在 `.autome/output/m06-*.txt`，不放入任务目录。

## 负向对照与红绿

- D-3 原实现跑新增 `--test-name-pattern='Doc-tests'`：红，实际归属 `previous (lib)`；加入识别分支后同命令绿（m06-summary-red/green.txt）。这是修复前后同一个项目回归用例。
- D-4 临时将⑪夹具的 `prep 'Build core'` 换成注释：旧断言绿（m06-reuse-old-mutant.txt），增强断言红在 `build >= 0`（m06-reuse-new-mutant.txt）。已恢复 prep；未改生产门禁调度。
- D-1 drain 不分发、D-2 去掉标记/overflow 的浏览器负向对照未执行：同一初始化权限阻断，不能把 setup 失败当负向对照命中。

## D-5 诊断结论与交接

`ps -eo pid,command` 被沙箱拒绝；项目采用的 `ps -axo pid=,ppid=,command=` 同样在浏览器初始化返回 EPERM。`lsof -p <本轮 shell pid>` 可用，但不能替代进程扫描和写入者归因。没有运行不带插桩的 20 次来冒充诊断；改前/改后次数均为 0，重试参数原样保留，未宣称根因已定位。

- H1：源码确认 CoreSupervisor.stop 等待 sidecar.stop，后者等待直接子进程 exit；只排除“正常停止路径根本不等直接子进程退出”的代码假设。后代写 state/ 的可能性未排除。
- H2：假 CLI/会话后代在清理期间写 sessions，未排除，需 ps + lsof 同时取证。
- H3：SQLite WAL/shm 延迟删除，未排除，没有失败残留条目证据。
- H4：w9Kernel 创建 core/admin/gateway，没有启动面板 HTTP 服务，可排除该夹具主动起面板日志写者；不能推广到整个 kernel 组。

在允许进程扫描的审计环境执行设计 D-Q5：定位所有带 maxRetries 的受影响清理点，临时设为 0；每次 rm 前记录 leftoverProcesses(root)、`lsof +D <root>`，失败记录残留条目；`(cd apps/web && npx vitest run --project kernel)` 连跑 20 次，日志逐次写 `.autome/output/d5/<run>.txt`。有写者证据才修根因，再无重试跑 20 次；证实不了保留重试并明确各假设是否排除。诊断插桩结束还原，不能减弱清理守卫。

## 标待审前的四项自审（已执行可运行项，未标待审）

1. **情形表逐行**：D-1 在线检查点→离线建卡→恢复观测已编码，浏览器未跑；D-2 四列标记/列体滚动/页面不滚动已编码，浏览器未跑；D-3 doctest 归属红绿已跑；D-4 缺 prep 的旧绿/新红已跑；D-5 权限预检失败，保留重试。没有省略未通过项。
2. **失败分支**：doctest 失败正确归属；缺 prep 必须失败；cleanup 持续 ENOTEMPTY 上抛并保留目录、启动失败仍等待清理，均由通过的 Vitest 子集覆盖。离线恢复未跑。授权拒绝不适用：本轮不新增授权操作。
3. **全部迁移**：live-client 单测覆盖 connecting→open→reconnecting→open、连接失败重试、hello/resume、reload 清检查点、在线/连接中/待重试时 stop，以及 core 状态通知；真实离线→恢复仍待浏览器验证。解析器 Running→Doc-tests→failures 已跑。无新增产品状态机。
4. **输入域边界**：live-client 既有零序号、重复/倒序、最大安全整数、空检查点和退避上限测试通过；新增 hello 不写 DOM 与新事件写 DOM 通过。汇总空/单/多目标由 node 验收覆盖。日期、闰年、前导零不适用：本次未新增日期或文本数值解析域。

## 用例数口径

本里程碑改前静态基线：node 25（summary 7 + reuse 18），Vitest 契约 11（cleanup 3 + live-client 8），浏览器 14（live 2 + layout 12）。改后分别 26、12、14：共 52，比同范围旧基线 50 增加 2（Doc-tests、liveSeq 各一条），无删减；另跑守卫 23 条。上一轮 M-05 为 Vitest 41 + 浏览器 19 = 60，范围不同，不把替换测试文件造成的总数差伪称新增/删除。

实际执行且通过 61 条（26 + 35），比上一轮实际执行的 60 条多 1；差来自本轮选择不同验收子集，并不代表只新增 1 条测试。浏览器登记的 14 条未执行，不计入通过数。红绿/变异的重复运行不重复累计用例总数。
