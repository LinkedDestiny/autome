# 裁决记录

## 第 1 轮

- 轮次：设计轮 d = 1（`design-round` 0 → 1）。
- 评审依据：`门禁连续因同一原因失败时熔断-停下等人-review.md`（设计评审 #1，基线 `eb6775b`）。
- 评审结论：2 条属于协议六类的问题（R-01 违反任务目标、R-02 内部逻辑矛盾），1 条非阻塞建议（S-01）。
- 本轮结论：两条均采纳并已修改设计；修改后没有剩余的六类问题，`status` 改为 `实现中`，里程碑表完整（5 个）。
- 提交：见 git 记录；若提交被拒则由 Autome 兜底。

### D1-P01（来自 R-01）重启落在「落账之后、推进状态之前」会重复计数，甚至漏掉已落账的熔断

- 主张摘要：`run_gate` 先写 `gate.result`，后经 `finish_gate` → `advance` 写任务状态，两次独立的写；其间重启，作业文件仍在，下一个 tick 会重读同一份结果。设计 §2 把「上一条 `tripped: true`」当作「用户已介入」，于是已落账的第 N 次熔断被读成计数 1，任务又退回实现（违反验收 2）；第 2 次的 `count=2` 落账后重启则把同一作业计成第 3 次（提前熔断）。
- 设计位置：§2「用户介入后从零开始」、§4「run_gate」、风险段「写事件与推进状态之间崩溃」。
- 裁决：**采纳**。
- 证据：
  - `crates/automed/src/scheduler.rs`：`run_gate` 在 `Done` 且 head 未变之后先 `append_event("gate.result")`（约 2847 行），再 `finish_gate`（2911 行）→ `advance` → `set_task_state`（约 1525 行）；`store.rs` 的 `append_event` 是独立 INSERT，两者之间没有事务。
  - 作业文件只在 `job.clear()` / `discard_node` 时删除；`poll` 在 `.exit` 标记存在时恒返回 `Done`。所以重启后的 tick 必然再次读到同一份结果。
  - 评审的反例成立：设计原 §2 的表确实会把重放读成 1。原风险段「最坏是提前一轮熔断」的说法错误，还会漏掉熔断。
- 修改落点：
  - §4 新增「重放」判据：`prev` 是带 `fingerprint` 的失败 `gate.result`，且其后该任务没有 `gate.started` 或 `session.started` 事件 → 同一次运行结果的重复消费，**不写新事件**，沿用 `prev` 的 `same_failures` / `tripped` / `fingerprint_summary`。选这两类事件作边界，是因为人工 `rerun` 门禁节点会丢弃作业并重新启动（写 `gate.started`），实现 / 修复轮启动会话写 `session.started`——因此同一 head 上的人工重跑、会话没有新提交而读回旧作业结果，都不会被误判成重放（评审要求保留人工重跑从零计数，且指出仅按 `node + sha` 去重不够；这里的判据不依赖 sha）。
  - §4 新增只读查询 `any_event_after`；`last_event_where` 改为 `last_event_in`（返回 seq 与 kind，见 D1-P02）。
  - §2 在「用户介入后从零开始」处注明该规则只对新的一次运行成立。
  - 风险段改写；M-03 新增「重放 · 已熔断」「重放 · 未熔断」两个 `scheduler.rs` 用例（手工写入与 `run_gate` 同形状的事件来模拟崩溃窗口，并断言 `prev` 之后有 `session.started` 时不当重放），负向对照补两条。
- 已知残余：作业启动与 `gate.started` 落账之间（两次写之间）崩溃的极窄窗口，账本无法识别，最坏是那一次运行沿用上一次的决定。已写进设计，不为它加作业侧标记。
- 复提计数：0

### D1-P02（来自 R-02）`merge_gate` 沿用通过只写 `gate.reused`，按设计读历史会跨过这次通过继续累计

- 主张摘要：§4 只查同节点 `gate.result`；`merge_gate` 同 sha 沿用上次通过结果的分支只写 `gate.reused` 就 `finish_gate(Passed)`，不写 `gate.result`。`失败 F、失败 F、沿用通过、失败 F` 会读到旧的 `count=2` 而误熔断，违反任务「中间通过则清零」与设计自己的「该节点通过即清零」。
- 设计位置：§2 上一条结果表与按节点清零规则；§4「读上一条」。
- 裁决：**采纳**。
- 证据：`scheduler.rs` 2789 行起的沿用分支（`ctx.store.append_event("gate.reused", …)` 后直接 `finish_gate(…, Passed)`）确实不经过 `gate.result` 的写入；特性与 Bug 的 `merge_gate` 共用该分支。评审的 Requeued{Rebase} 反例与现有状态机转换一致。
- 修改落点：
  - §2：`prev` 定义为同节点最近一条「门禁结论」事件（`gate.result` 或 `gate.reused`），表中「通过」一行并入 `gate.reused`；补上判据 `失败 F、失败 F、沿用通过、失败 F` → 1。
  - §4：`last_event_in(kinds=["gate.result","gate.reused"], field="node", value=…)`；`gate.reused` 映射为 `ok: true`。沿用条件、`gate.reused` 事件含义、通过后的节点一律不改（约束 1）。
  - 评审顺带问的「配置移除时直接放行」分支（`selected_gate` 为空，不写事件）：裁决为**不是**计数边界——没有跑门禁，既无新结果也无通过，按「不算结果的运行」处理；已写进 §2。
  - M-03 新增「沿用通过清零」用例，负向对照补一条。
- 复提计数：0

### S-01（非阻塞建议）M-04 用例 2「所有 `gate.result` 的 `same_failures` 都是 1」

- 裁决：**采纳**（文案澄清，不是阻塞项，不计复提）。
- 修改落点：M-04 用例 2 改为「所有参与计数的失败 `gate.result`（带 `fingerprint` 字段的那些，通过的那条不带）的 `same_failures` 都是 1」。

### 数字联动

- M-03 新增 automed 库测试 9 → 12（scheduler 4 → 7）；M-03 预期总数 615 → 618。
- M-05 全量预期 1127 → 1130（1084 + 16 + 13 + 12 + 3 + 2）。

### 复提与争议项

本轮没有复提；争议项小节仍为「（无）」。
