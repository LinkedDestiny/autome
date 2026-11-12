# 合并队列设计裁决记录

## 第 1 轮（design-round 0 → 1）

### 评审结论

评审轮（`...-review.md`）提出 3 项阻塞问题（R1-P01 – R1-P03）与 2 项非阻塞建议（N-01、N-02）。本轮逐条对照源码（`scheduler.rs` `run_gate`、`gate_queue.rs`）核实，3 项全部成立，均采纳并已改入设计。三项属于协议六类里的「违反任务目标」（任务 §3.2/§3.6/§8.1）与「内部逻辑矛盾」，修正后没有剩余的六类问题，状态块改为 `实现中`、`current-milestone: M-01`，等用户批准。本轮的修改未再经评审轮复核，由用户批准环节把关。

### 逐条裁决

#### D1-P01（对应 R1-P01）出队 rebase 后重跑准备命令时尚未持锁，后来的任务可以先合并

- 设计位置：D1 第 3、5、6 步；D3 起点。
- 裁决：**采纳**。
- 证据：`scheduler.rs` `run_gate` 中 `ensure_with_hold` 在 `gate_queue::admit` 之前，`Ready::Pending` 直接 `return Ok(())`（3402–3421 行）；`gate_queue.rs` 的 `QUEUE_EVENTS` 含 `setup.started`，`waiters` 只认最后一条为 `gate.queued` 的任务。因此旧设计「rebase 后回到第 2 步异步重跑准备、第 6 步才拿锁」存在评审所述的间隙，已授权的 C 可以先拿锁合并。
- 修改落点：D1 重写——拿锁提前到第 1 步（沿用、准备、准入、出队 rebase 全部在锁内）；rebase 后「从第 1 步再走一遍」；G1 的保证改述为「第 5、6 步同一次调用」；D3 起点与释放表（出队冲突改为释放）、「不死锁」①③⑤；R2；M-01 契约新增 `merge_queue_a_pending_setup_after_the_dequeue_rebase_keeps_the_section` 并让既有 setup 用例断言重跑期间持锁；负向对照加「拿锁推迟到 rebase 之后则失败」。
- 复提计数：0

#### D1-P02（对应 R1-P02）沿用门禁结果直接拿锁，能越过更早的合并队列条目

- 设计位置：D1 第 2 步（旧）；D4。
- 裁决：**采纳**。
- 证据：旧第 2 步沿用先于 `admit`（`scheduler.rs` 3376–3391 行直接 `finish_gate`），而 seq 排序只在 `admit` / `waiters` 里起作用；D4 只保证等锁者保存 seq，不约束绕过准入的路径。评审反例（创建顺序与入队顺序相反、持锁者失败后 C 沿用）成立。
- 修改落点：新增纯函数 `merge_queue::turn`，D1 第 1 步对已授权任务按 seq 取资格（有持锁者，或有 seq 更早的已授权排队者，都要等）；沿用路径在第 1 步之后，不再绕过顺序；D4 补充用 seq 而非 tick 内顺序取资格，`waiters` 与第 1 步共用同一判定；M-01 契约加 domain 用例 `merge_queue_turn_follows_the_holder_then_the_queue_order` 与 lib 用例 `merge_queue_a_reusable_gate_does_not_jump_an_earlier_queued_task`（用评审给的反序安排），预期总数 16 → 19。
- 复提计数：0

#### D1-P03（对应 R1-P03）撤销接口在 Merging 状态返回成功，却不阻止随后的自动合并

- 设计位置：D2 `task.merge {cancel}`；D4 Merging 分支。
- 裁决：**采纳**，取评审建议的第一种方案（按状态限定边界），不引入 Merging 退回。
- 证据：旧 D2 对 `cancel` 不限定状态，而 `Merging` 在下一次 tick 无条件 `merge_all`，且派发层写完后立即 `tick`，反例成立。已开始合并后退回会牵涉工作区部分仓库已合并，成本与风险都大于拒绝。
- 修改落点：D2 新增 `merge_queue::cancel_press(state)` 表——`Merging | MainCheck | Cleanup` 拒绝「合并已经开始，无法撤销」，`Done | Cancelled` 拒绝，其余在有效授权为真时写撤销；持锁时撤销同次写 `merge.unlocked { reason: "cancel" }`（D3 释放表补一行）；M-02 契约新增 lib 用例 `merge_auth_cancel_is_rejected_once_the_merge_has_started`，`merge_auth_merge_press_by_state` 同时覆盖 `cancel_press`，预期 16 → 17，负向对照补一条。
- 复提计数：0

### 非阻塞建议的处置

- N-01（M-01 负向对照不应依赖 `merge.auto { by: behind }`）：**采纳**，已改 M-01 负向对照，以门禁调用数、祖先关系与 `Merging → Behind` 回退为证据，并写明 `auto = true` 变体不会写该事件。
- N-02（T-51 已合并）：**采纳**，D4 的 T-53 段改为「T-53 / T-51」，写明 T-51（`bf399db`）改的是会话启动并行名额与 gate_queue 测试，不改 `run_gate` 与 `waiters` 排序，实现轮开头再核对。

### Backlog 追加

- B-05：持锁者在宿主队列等名额或等准备作业时同项目其它已授权任务都在等锁（本轮修正的取舍），实测等待过长时再改为准入后拿锁。

### 争议项

无。

### 定稿判断

三项阻塞均已在设计里闭合，五个里程碑的验收命令、预期、负向对照仍各自齐全，没有剩余的六类问题。`status` 改为 `实现中`，`design-round` 为 `1/15`，`current-milestone: M-01`。提交由 Autome 兜底（若本轮提交被拒）。
