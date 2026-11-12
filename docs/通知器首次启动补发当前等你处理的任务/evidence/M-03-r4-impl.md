# M-03 · 实现 #4（门禁修复）

本轮仅修复宿主门禁失败，不推进新里程碑。原有 M-01～M-03 均已由审计关闭，状态保持不变；implementation-round 更新为 4/15。全量门禁以 Autome 合并前重跑为准。

## 定位与修复

- 宿主日志：`/Users/dannie/project/autome-v2/.autome/output/gates/T-49/merge_gate-1679ccc4da5c.log`，失败详情摘录保存在 `.autome/output/r4-host-failures.txt`。
- R1「真实账本一个 await_merge 任务，空面板库首次启动恰 1 条」在 `notifier-backfill-real.test.ts:59` 的严格键集合对比失败：真实 waiting 项多出 `gate_queue_position`。R1 尚未调用 pull，R2「重启面板并追加一条不改状态的 task.updated，通知数不变」因此以空列表为基线，第一次补发后比较失败，是同一根因的连带失败。
- 源码 `crates/automed/src/dispatch.rs` 的 dashboard_get 确认该字段总是序列化；等待用户处理、没有排队的任务值为 null。仅在共享 `dashboard-double.ts` 补上 `gate_queue_position: null`。检查其全部调用方 notifier/notifier-backfill-real/notifier-real，没有其他同类共享替身遗漏。生产代码和内核未修改，严格键集合断言原样保留。
- 设计当前接口形状同步为七键。上一轮审计通过、没有退回检查需要搬入；本次宿主抓到问题的检查已经在项目测试体系中，直接原样运行红绿，不复制或放宽断言。

## 命令与结果

命令均从 worktree 根目录执行；完整输出只存 `.autome/output/`。

- 红：`cd apps/web && npx vitest run test/notifier-backfill-real.test.ts` → 4 个用例，2 失败、2 通过，与宿主相同的两个断言失败，耗时 70.28 秒；日志 `r4-red.log`。用例总数相对上一轮同文件基线 4，差为 0。
- 绿：`cd apps/web && npx vitest run test/notifier-backfill-real.test.ts test/notifier.test.ts test/store.test.ts` → 39 个全部通过（真实补发 4、通知器 17、存储 18），0 失败、0 跳过，耗时 80.27 秒；较上轮同子集 39 个差 0，没有增删用例；日志 `r4-green.log`。
- `npm run lint --prefix apps/web` → 退出码 0，零警告；日志 `r4-lint.log`。
- `npm run typecheck --prefix apps/web` → 退出码 0；日志 `r4-typecheck.log`。
- 本轮新增/删除用例均为 0。宿主报告全量 399 个，较审计 #3 的 315 个多 84 个；这是两次执行所处集成版本的基线差异，本轮没有新增测试，不将该差额算作本轮覆盖。本轮仅运行相关子集，不声称全量 399 个通过，不运行 ci-local 或全量 npm test。

## 四项自审

1. **情形表逐行**：以下相关子集检查全部通过。R1 检查严格真实形状、仅补当前任务、来源/序号/标题/正文/项目/五键推送；R2 检查重启后同状态不重发；R3 检查停服期间 reject 后再进入新增一次；R4 检查 v1 历史播种及同状态不重发。D3 六行由首次启动与 N3 覆盖：稳定等待 1 条、离开再进入 2 条、非等待后进入 1 条、状态先写事件后到 1 条、快照前离开 0 条、等待状态变更 2 条。七类事件通知的旧真实账本文件本轮未重跑：它仅引用替身的类型，本轮无运行时修改。
2. **失败分支**：notifier 既有拒绝（带 code）、未连接/超时、N3 三次不对齐、N4 三个读取位置各 error/close、写盘失败回滚与恢复重试均随子集执行；store 事务回滚也执行。真实文件系统读盘失败不适用：本轮仅补返回对象字段，不引入读文件逻辑；未宣称真实文件系统故障注入。
3. **全部状态迁移**：无游标→补发→重启同状态（R1/R2/N1）；在线/离线 W→X→W 与 W1→W2→W1（N2/R3）；v1 有游标→播种→正常读取（S1/N5/R4）；无 notifier 游标/仅 other 游标→不播种（S1）；旧账本→清状态→新补发（S4/N6）；失败/关闭→无提交→重试成功（N4）。同一 publish 通道的消息断言执行，浏览器/系统横幅本轮不取证。
4. **输入域边界**：空账本与游标 0（首次启动）、单任务（R1）、多任务（N4/R4）、MAX_SAFE_INTEGER 最新序号查询（真实 RPC）、缺游标/已有游标/other 游标（S1）、同状态更大序号（R2）均由子集覆盖。新增字段的无排队值 null 与真实接口键集合核对；正整数排队位置不适用，夹具只生成等待用户的任务，不模拟门禁排队。日期、前导零、闰日、跨年不适用，本修复不解析这些输入。

## 交付边界

本轮只维护证据、当前设计与 retro 一行；不改归档审计和旧证据。全量门禁、浏览器和人工验收仍由宿主/用户原流程执行。

- `git diff --check` 通过；提交结果见当前分支历史。
