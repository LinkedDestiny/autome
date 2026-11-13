# `autome rerun <任务> fix` 绕过修复轮预算：超预算失败后仍能无限重跑修复

## 现象

Bug 任务因「修复轮次用完（上限 3）」进入失败后，`autome rerun <任务> fix` 会直接再起一个修复会话，不检查 `[bug] fix_rounds`，也不要求先 `autome extend`。连续 `rerun` 可以无限次绕过预算。

T-45（文档补齐）的设计轮读代码时发现（其 Backlog B-07，来源 D1-P01）：`crates/autome-domain/src/bug_loop.rs` 的 `apply_for` 里，`(TaskState::Failed | TaskState::Stopped, Trigger::RerunFrom { node })` 分支直接 `session(BugStep::Fix, …)`，没有经过 `start_fix`（预算计数与「修复轮次用完」判定都在 `start_fix` 里）。需求 Loop 的同类路径（`rerun <任务> impl`）怎么处理预算，定位轮一并核对，保持两条 Loop 一致。

## 复现步骤

1. 项目配置 `[bug] fix_rounds = 1`，建一个 Bug 任务，让修复门禁失败一次，任务因「修复轮次用完」失败；
2. `autome rerun <任务> fix`；
3. 观察：任务直接进入「运行中 · 修复」并起会话，没有被拒绝，也没有提示需要 `extend`。

自动化复现（end_to_end 夹具）：`fix_rounds = 1`，门禁脚本恒失败，等任务失败后调 `task.rerun_from { node: "fix" }`，断言结果。

## 期望行为

与 `extend` 的语义一致：预算用完的任务 `rerun fix` 应被拒绝，并在拒绝信息里指出用 `autome extend <任务> <n>` 追加修复轮；或者 `rerun` 消耗预算、超出即拒。两种取一种，设计定，但不能是「不计数」。`rerun` 其它节点（locate、repro_check）不受修复轮预算约束的行为保持不变。

## 实际行为

`rerun fix` 起会话，预算形同虚设。

## 环境

main `404b481`；`crates/autome-domain/src/bug_loop.rs`（`apply_for` 约 148 行的 `RerunFrom` 分支；`start_fix` 约 47 行）。

## 线索（可选）

- `[gate] same_failure_limit` 在 `rerun` / `extend` 之后重新计数（T-28），注意不要把两种计数混在一起。
- README Host gate 一节与 technical-design §5.6（T-45 正在补）按实际行为写了文档，修好后要同步改那几句。
