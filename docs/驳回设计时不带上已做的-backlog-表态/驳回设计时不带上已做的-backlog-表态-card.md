# 驳回设计时不带上已做的 Backlog 表态，随后批准又触发一轮多余的设计

## 现象

T-61（2026-11-12）：设计卡点上先 `autome decide T-61 backlog B-02 include`（B-01、B-03 ignore），再 `autome reject T-61 "<意见>"`。

- 驳回启动的设计轮（plan 会话 `ses-4eea0f61…`）只注入了驳回意见，没有带表态：`decisions` 表里三条的 `carried_by` 在这一轮结束后仍为空、`consumed_at` 为空。
- 第二轮设计定稿后 `autome approve T-61`：因为还有一条未消费的 include，任务没有进实现，而是又起了一轮设计（plan 会话 `ses-455b065e…`，`carried_by` 这时才写上），之后还要再过评审、裁决，才第三次停在「等你批准设计」。
- 第二轮设计其实已经按驳回意见里的文字把 B-02 写进了方案，第三轮没有新内容，白花约 25 分钟和三个会话。

## 复现步骤

1. 需求任务停在等你批准设计，Backlog 有一条待决；
2. `decide <任务> backlog B-01 include`；
3. `reject <任务> "意见"` → 设计、评审、裁决 → 再次停在等你批准设计；
4. `approve <任务>` → 任务回到设计轮，而不是进入实现。

自动化复现：end_to_end 夹具里按上面四步，断言第 3 步的设计会话 prompt 文件含该条表态正文（`Inject::Decisions` 的内容），且第 4 步之后任务进入实现。

## 期望行为

驳回启动的设计轮同时带上所有已表态、未消费的条目（驳回意见与表态在同一个 prompt 里），该轮正常结束即消费；之后 `approve` 直接进实现。没有待送达表态时，驳回行为与今天相同。

## 实际行为

驳回轮只带驳回意见；表态留到下一次 `approve` 才送达，多出一整轮设计 / 评审 / 裁决。

## 环境

main（T-62 合并之后）；`crates/autome-domain/src/task.rs`（`Reject` 的转换只给 `Inject::Rejected`，`approve` 在 `decisions.included > 0` 时 `start(Some(Inject::Decisions))`）；`crates/automed/src/scheduler.rs` 的 `carry_decisions`。

## 线索（可选）

`docs/plans/2026-10-11-decisions-take-effect-next-round.md` 的 D1（裁决轮 / 实现轮处理表态）还没做，这是同一类「表态送达时机」问题里最便宜的一处。
