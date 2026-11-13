# M-05 实现 #7：合并门禁修复

## 范围与结论

本轮是修复轮，不推进新里程碑。实现轮数为 7/25；五个里程碑均已由审计关闭，保留原状态，不由实现轮重新标记已完成。修复新建页文案引用（M-04）与 M-05 监听测试计时。未改内核、事件总线、ticker 或其他仓库。

宿主 `87d89fb01a43` 的原始门禁日志：`/Users/dannie/project/autome-v2/.autome/output/gates/T-58/merge_gate-87d89fb01a43.log`。Web 共 55 文件、461 用例，459 通过、2 失败、0 跳过。失败为：

- `test/labels.test.ts > 7 views do not duplicate status, column or stage Chinese literals`：`projects.$id.tasks.new.tsx: 待开始` 硬编码。保留原检查不改，按钮引用 `COLUMN_LABELS.todo`，最终可见文案仍为“存为待开始”。
- `test/board-live-listen.test.ts > 1 external stop reaches websocket within 4500 ms`：`task.updated timeout 4500 ms`。`dispatch.rs:2089-2090` 先终止会话再提交 Stop；`launcher.rs:1273` 固定等待 3000 ms；Web ticker 又可能等待约 3000 ms。原命令启动时开始的 4500 ms 预算混入内核停止等待，随 ticker 相位偶发失败。没有证据证明事件总线漏发，不修改范围外实现。

修正计时口径：命令前订阅目标任务 stopped 事件，命令成功回复后最多等待 4500 ms；回复前收到事件也保留。命令本身仍受现有 30 秒 execFile 超时约束。测试辅助函数清理超时与监听器，缺失事件仍失败，命令失败直接传播。设计 §6、M-05 契约同步说明该口径，不再声称从启动 stop 算起满足 4500 ms。此指标是命令完成到 Web 事件的延迟，不是停止操作端到端耗时。

上一轮审计结论为通过，没有新退回检查需要迁入。本次宿主抓错的两个用例本就属于项目测试体系，未删除或跳过；另补无端口依赖的计时回归。

## 命令、红绿与用例总数

以下命令均在 `apps/web` 执行；原始输出只在 `.autome/output/`。

1. 原样复现：`npx vitest run test/labels.test.ts test/board-live-listen.test.ts` → 2 文件、总数 10，7 通过、3 失败、0 跳过。文案原断言复现为红；两个监听用例在建立监听时均被 `listen EPERM: operation not permitted 127.0.0.1` 阻断，未到达宿主超时断言，不能说本地复现了同一超时。日志 `r7-red.txt`。
2. 计时红：将原测试“命令启动就计时、与事件并行等待”的算法抽入测试辅助函数，再跑 `npx vitest run test/board-live-timing.test.ts` → 总数 4，1 通过、3 失败（另报 2 个延迟处理拒绝错误）。虚拟时间中 stop 用时 3000 ms，事件在 5900 ms 到达，旧算法 4500 ms 超时；回复前事件与超时边界检查同样发现旧口径问题。日志 `r7-timing-red.txt`。这是受限环境下的机制回归，不替代真实监听复现。
3. 修复后：`npx vitest run test/board-live-timing.test.ts test/labels.test.ts test/task-new.test.ts` → 3 文件、总数 21，21 通过、0 失败、0 跳过。日志 `r7-green.txt`。
4. 最终验收与直接相关子集：`npx vitest run test/board-live.test.ts test/board-live-listen.test.ts test/live-routes.test.ts test/board-live-timing.test.ts test/labels.test.ts test/task-new.test.ts` → 6 文件、总数 37，35 通过、2 失败、0 跳过；仅监听两例 EPERM。总耗时 74.36 秒，真实内核非监听 setup/tests/cleanup 18079 ms。日志 `r7-related.txt`。
5. `git diff --check` 通过。

同口径基线差：M-05 上轮验收 16 例，本轮契约 20 例，+4 全来自 `board-live-timing.test.ts`；最终相关子集 37 = M-05 20 + labels 8 + task-new 9。上轮实现相关子集 36 与本轮 37 相差 +1，其文件集合不同：移出 task-card 12、board 8，加入 labels 8、task-new 9、timing 4，即 -20 +21。没有删除或跳过用例。相对宿主全量 Web 基线 461，代码增加 4 例，预计下次全量为 465，本轮未实跑全量，不将预计值当通过证据。Desktop 本轮未跑、未改，不报未经验证的用例总数。

## 自审清单（修复轮仍逐项执行）

1. **情形表逐行**：新建页 SSR 仍有两个提交按钮、写闸门关闭置灰、模板与附件提示不变（task-new 9 例通过）；labels 原源码守卫通过。M-05 draft/queued 按钮、空槽/满槽启动、六类事件 matcher、非事件导航、通知订阅/取消订阅均由 board-live 7 与 live-routes 7 通过覆盖。真实 WebSocket 与停止后新 SSR 两例 EPERM，未取证。
2. **失败分支**：计时用例验证事件永不到达时在命令回复后 4500 ms 拒绝，命令失败立即传播且监听/计时器清理；board-live 验证未知/空 intent、400/409/503/504 与无网关；task-new 验证占位标题、超字节数、超附件数、失效编号、重名与 400/409/503 原文。读盘失败不适用：本轮没有修改读取路径。真实监听不可用已明确失败，未用 skip 隐藏。
3. **全部迁移**：真实内核非监听 draft→active、draft→queued、active→stopped 与 router 重验通过；通知有数字→无数字、无数字→有数字、订阅→取消订阅通过。新增测试等待器的命令执行→成功→事件、事件先到→命令成功、成功→事件超时、执行→命令拒绝四条路径通过。未新增产品状态迁移。
4. **域边界**：计时覆盖回复前事件（延迟记 0）、3000 ms 停止耗时加 2900 ms 传播、回复后 4499 ms 尚未超时/4500 ms 必须超时；命令拒绝后计时器数为 0。新建页原输入验证边界由 task-new 通过覆盖。日历、最大 ID 等不适用：本轮不改这些输入域。

## 交接

未跑 `sh scripts/ci-local.sh`、全量 Web/Desktop 或宿主快检。宿主需在本轮提交上重新运行快检与合并门禁，重点复验真实监听两例及其时序；本地 35/37 不代表全量门禁通过。人工验收 H-01..H-07 不新增取证。
