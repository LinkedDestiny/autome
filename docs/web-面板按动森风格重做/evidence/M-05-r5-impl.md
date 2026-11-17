# M-05 · 实现 #5

## 范围与实现

仅推进 M-05。新增 TaskSummary 服务端模型，九步需求/六步 Bug、六项统计与开放时间起点；loader 使用完整会话计算，仍只传现有会话窗口。阶段组返回行 key，不复制会话。时间线补用户等待、门禁用时、行类别、退回判定及倒序分组/筛选。页面重排留 M-06。

状态位置复用现有 active/paused/queued/failed/stopped/cancelled 语义。退回字段已核对真实回包为 `reopen_count`；门禁 steps 字段已核对 `crates/automed/src/gate.rs` 的 `read_report`：ran/reused/not_run/unverified。等待区间复用 waitingTransition，与总览口径一致。

上一轮审计 M-04 通过，无退回检查需要逐字移植。现有断言未删减或放宽。

## 命令与结果

- 基线：本里程碑验收命令（新增两文件尚不存在），6 文件、58 用例通过；`.autome/output/m05-baseline.txt`。
- 验收：`(cd apps/web && npx vitest run test/task-summary.test.ts test/task-summary-real.test.ts test/task-timeline.test.ts test/task-stage.test.ts test/task-detail-data.test.ts test/task-detail-payload.test.ts test/task-detail-ssr.test.ts test/test-groups.test.ts)`，8 文件、89 用例全通过，0 跳过；`.autome/output/m05-final.txt`。
- 同范围较起点 +31：task-summary 新增 20、task-summary-real 新增 4、timeline +4、stage +2、detail-data +1。上一轮 M-04 是另一套范围（82 单测和 7 浏览器），不能当作本轮同范围基线。
- 原有 payload/SSR 的 150 KB 守卫通过；真实 w7-flow 跑到 await_merge，再合并至 done，门禁分类与终结 TaskMetrics 相等。
- 首轮红：空时间线夹具的日期比较、运行会话测试夹具角色错误；修正后绿。真实内核首轮业务断言通过，但复用 teardown 的全机 `ps` 被沙箱 EPERM 拒绝；新夹具改为管理自己创建的 core 子进程，await core.stop 后删除临时根目录，与现有 board-done-real 方式一致。此测试不宣称全机进程扫描通过。
- 快检初次发现 JSON fixture 的 NodeNext 导入与旧快照类型问题，已修复；独立 `npm run typecheck` 通过（`.autome/output/m05-typecheck.txt`）。指定快检最终结果见下。

## 标待审之前的四项自审

1. **情形表逐行**：
   - 需求九步、Bug 六步：真实回包步骤与会话用时断言通过；暂停/失败/停止/排队/取消定位通过。
   - 批准等待：驳回后再批准两段相加通过；真实 task.updated 时间差通过，不取会话间隙。
   - 六数：cost 总额、runtime 分组、轮次与里程碑退回、终结 metrics、指定节点门禁计数通过；零费用与无计费区分通过。
   - 用户操作：十种既有 trigger 与 task.decided 共十一种保留；reply 退出 not_a_bug 带时长，其余非退出操作不带时长。
   - 门禁：result 执行/沿用步数、同 node/log 时长通过；reused/skipped 保留行但不计数，repro_check/main_check 不计数。
   - 退回：审计 pending→open 会话、contradicted、失败门禁、reject 通过；两筛选与阶段/组内倒序通过。
   - 服务端投影：60 会话汇总完整而窗口只有 20，组内只存 key；窗口、SSR 和体积守卫通过。
2. **失败分支**：事件截断使 gates=null；事件服务失败也投影为缺失。现有 detail-data 的内核离线/404/超时、文档读盘 EACCES、非法文档拒绝测试全部通过。外部服务调用不适用：新增汇总是纯函数，未新增外部服务。新授权写入不适用：本里程碑不增加写操作。
3. **全部迁移**：五种等待（批准/合并/补充信息/not_a_bug/failed）逐种进入、同状态保持、退出；等待切换到另一等待；paused 排除；开放等待随 now 增长；完成后固定。需求/bug 完成、暂停/排队/失败/停止/取消位置测试通过。审计待审→开放用具体 snapshot 回归通过。
4. **输入域边界**：空事件/零会话、单运行会话、零费用、全无计费、0/1/19/20/21/60 会话窗口、非法/空/from 前导零、不匹配文档路径均通过。日历输入不适用：本模型不接收用户日期字符串，不做日期分组；以账本 ISO 时间毫秒差计算。最大窗口由现有 60 会话体积守卫覆盖，未新增任意输入数量上限。

## 结论

M-05 验收通过，提交独立审计；未运行全量 `scripts/ci-local.sh`，未推进 M-06。

补充修复：指定快检的 clock 守卫发现汇总模块使用 `new Date`；已改为直接保留毫秒起点，没有放宽守卫。新增直接相关 clock 子集 10 用例；本轮总验证范围为 89 + 10 = 99，相同范围起点为 58 + 10 = 68，增量仍为 +31。

最终相关验收：上述 M-05 命令再追加 `test/clock.test.ts`，9 文件、99 用例通过，0 跳过（`.autome/output/m05-accept-complete.txt`）。新增断言确认实现步骤不混入快检用时；最终源码已通过此范围。没有增加业务测试数量，仍较同范围起点 +31。

快检复跑说明：前几次检查重叠，引起共享 build 目录 ENOTEMPTY/构建竞态，这些失败不作为产品缺陷或通过证据；已等待全部结束，再以 `.autome/output/m05-quick-serial.txt` 的单次串行结果为准。快检自动选择浏览器步骤时，沙箱的 `ps EPERM` 会阻止 e2e 初始化/清理；没有放宽或修改项目进程守卫。

指定快检最终串行命令：`sh scripts/ci-quick.sh --since 599182c7d04474e2a354f405491bbc6ed8b76835`。执行 51 步，失败 1 步，仅 Web e2e 初始化/清理被 `spawnSync ps EPERM` 拒绝；fmt、Clippy、lint、typecheck、Web/core 构建及已准入测试通过。120 秒准入预算后的项目测试由脚本标为未跑，不算通过；原始输出 `.autome/output/m05-quick-serial.txt`。M-05 契约的独立验收仍是 9 文件 99 用例全过；浏览器环境阻塞留宿主复验，不将其记为协议失败。`git diff --check` 通过。
