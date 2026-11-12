# 设计裁决记录

## 裁决 #1（设计轮 1，评审基线 `0b8070f`）

评审结论：2 项阻塞问题（D1-P01、D1-P02），均指向 M-01 的提交边界与取消完成判据。本轮逐条用源码复核，两条都成立，全部采纳。
没有已有裁决，所有主张复提计数为 0。裁决后设计已无剩余的六类问题，状态块 `status` 改为 `实现中`，`design-round` 0 → 1，里程碑表三行保持不变（仅 M-01 标题改成「单事务改建与取消」）。

### D1-P01 · 多次提交与事后删除不能满足改建的原子可见性

- 主张摘要：`create_task` 自行提交，新草稿在原任务取消前已对读者可见；`READ_METHODS` 不持驱动锁，CLI 是独立进程；R1 把崩溃窗口留给用户手工清理；第 3 步的 `stop_running_session` 在补偿之外；两条事件与 `start` 排队各自独立写入。
- 设计位置：方案 1 第 2～4 步、失败补偿、非目标、R1。
- 裁决：**采纳**。
- 证据：`store.rs:941-953` `create_task` 自开事务并提交；`dispatch.rs:232` 读方法绕过驱动锁；`dispatch.rs:1898` `stop_running_session` 经 `gate::discard_node`（`gate.rs:236`）可返回错误；`store.rs:531` `append_event` 单条写入。反例成立，不需要崩溃即可违反任务约束 4。
- 修改落点（设计文档）：方案 1 重写为「预检 → 停会话（唯一提交前副作用）→ `store.convert_task` 单个 SQLite 事务（新任务 + brief、原任务 `Cancelled`、归档标记、`task.created`/`task.converted`/`task.updated`）→ 提交后清理与 tick」；`start: true` 直接建成 `Queued`（与 `task.create` 非草稿一致），不再有单独的 `Start` 与 `task.started`；删除 `discard_task` 与补偿路径；R1、R2、待定问题 3/7、非目标同步改写；M-01 验收契约改为 `cargo test -p automed task_convert` 共 10 个用例（含跨连接 store 用例与三种提交阶段注入、停会话失败用例），负向对照随之更新；M-03 的全量数改为 1206（基线 1193 + 13）。
- 复提计数：0。

### D1-P02 · `Cancelled` 不能证明取消清理已经执行

- 主张摘要：`advance` 在 `scheduler.rs:1702` 先落库状态，`clear_retry`/`task.updated` 可能在 `perform` 之前失败；归档与删 worktree 在 `cancel_task` 内。「已是 Cancelled ⇒ perform 之后失败」的推断不成立，旧设计会在清理没做时返回成功。
- 设计位置：失败补偿第二分支。
- 裁决：**采纳**。
- 证据：已核对 `scheduler.rs:1702-1791`（`set_task_state` 先于 `clear_retry`、`task.updated`、`perform`）与 `cancel_task`（`3596-3635`：文件系统步骤全部 `let _`，唯一可失败的是末尾 `set_task_archived`）。`record_failed_action` 对 `Cancelled` 直接返回，不做任何补救。
- 修改落点：删除该推断。改建不再经 `advance`：取消转换由 `bug_loop::apply_for` 求出并校验 `Cancelled/RunCancel`，状态与归档标记并入提交事务；`cancel_task` 抽出纯文件系统的 `discard_checkout`（行为不变），提交后只调它，因此提交之后不存在可失败的数据库步骤，也不存在「状态已变、清理未做」需要判断的状态。验证覆盖：触发器拒绝原任务 `UPDATE tasks`（涵盖状态写入与归档标记失败，整体回滚）、拒绝 `task.converted` 事件、拒绝 brief 插入，以及停会话失败。`task.cancel` 自身的同类既有窗口超出本任务范围，进 Backlog B-01。
- 复提计数：0。

## 记录

- 本轮 Backlog 追加：B-01（`task.cancel` 既有窗口）、B-02（实现注意事项）。
- 争议项：无。
- 本轮没有运行代码；两条裁决依据源码阅读与评审给出的执行顺序。
- 提交：见本分支的最新提交；若提交被拒，由 Autome 兜底。
