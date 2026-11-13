# M-04 · 实现轮 #5

## 范围与实现

- 新增写 RPC `task.move`，校验位置参数、任务状态和参照范围后，使用现有 `reposition` 与 `reorder_queue` 完成本项目重排；未定位 Draft 按参照、移动目标顺序补入。
- 返回 task、queue_position、order，记录完整 task.moved payload，不调用 tick。
- task.start 仅在 queued_seq 为空时分配队尾位置，保留已移动或作为参照的 Draft 位置。
- 上一轮 M-03 审计通过，没有审计退回缺陷或需要逐字迁入的检查。

## 命令与结果

- `cargo test -p automed queue_move_`：7 个通过（lib 6、e2e 1），失败 0、忽略 0。日志 `.autome/output/m04-tests-final.txt`。
- `cargo test -p automed task_start_queues_a_draft`：1 个通过。日志 `.autome/output/m04-start.txt`。
- `cargo test -p automed --test end_to_end task_card_a_started_draft_waits_behind_the_parallel_limit`：1 个通过。日志 `.autome/output/m04-start-e2e.txt`。
- 本轮验收总数 9，较本轮相同过滤范围的原有基线 2 增加 7（新增 lib 6、e2e 1）；较上一轮 M-03 验收总数 13 减少 4，因为本轮执行 queue_move_ 7 + 原有 start 回归 2，上一轮执行 after_blocked_ 7 + after_create_ 6，范围不同，并非删除用例或跳过测试。测试函数之外的二进制报告 0 个匹配，不计入用例数。
- `git diff --check` 通过；两个修改文件经 rustfmt 格式化。设计的四条负向变异未在本轮另行注入；对应判别断言已覆盖，留待独立审计。


初次编译发现测试失败原因枚举名称写错，改用 SessionCrashed；同时修正 scheduling_wait 诊断闭包。初次单元测试 6 个，5 通过、1 失败：失败是测试把构造夹具的 task.created 也算入禁止新增事件；改为断言没有 task.moved 并保留 queued_seq 不变断言。

## 标待审之前的自审清单

1. **情形表逐行**：三种位置与事件 payload → positions_and_event；两个未定位 Draft 的 before/after → unpositioned_draft_reference；跨项目序号隔离 → other_project_unchanged；已移动 Draft 启动保位 → draft_keeps_position_on_start；未移动 Draft 启动入尾 → unmoved_draft_starts_at_tail_and_singleton；三任务第三个移首后先启动 → 同名 e2e。均属于 queue_move_ 子集，最终 7 个全部通过；以下清单所列检查均随该子集执行通过。
2. **失败分支**：位置零个/两个、top false、错误 JSON 类型、自己、跨项目、不存在、空串、运行/暂停/停止/失败/Done/Cancelled 目标或参照 → rejects_without_changes。拒绝无移动事件且队列序号不变。授权、读盘失败、外部服务不可达、超时注入不适用：M-04 设计未点名这些分支，没有新增外部调用，数据库错误沿用错误传播。
3. **全部迁移**：Queued 与 Requeued 移动后仍排队，Draft 移动后仍 Draft；Draft start 保位/无位置入尾；e2e 槽释放后移动目标 Queued→Active，其余仍 Queued。Active、Paused、Stopped、Failed、Done、Cancelled 不允许 move。move 是写通道，断言不在 READ_METHODS。无其它新增状态迁移。
4. **域边界**：单元素 top、队首/队尾/中间、两个未定位 Draft、空 id/前导零不存在 id、null/数字/string 类型、top false 均覆盖；日期、闰日、跨年不适用，无日期输入。队列序号最大值不属于 RPC 输入，沿用 M-01 已验收的 store 溢出回滚检查。

## 验证边界

未运行全量门禁及宿主快检；只格式化本轮两个 Rust 文件。未触碰真实账本，测试使用 Sandbox/World 临时目录。CLI、Web 属于后续里程碑。
