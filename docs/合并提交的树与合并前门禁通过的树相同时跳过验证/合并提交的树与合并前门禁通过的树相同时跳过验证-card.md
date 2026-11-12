# 合并提交的树与合并前门禁通过的树相同时，跳过「验证 main」

## 背景

配了 `[gate]` 的任务合并要跑两次完整门禁：合并前门禁（MergeGate，在 rebase 后的 head 上）和合并后的验证 main（MainCheck，在合并提交上）。任务分支已经 rebase 到 main 最新提交之上时，`--no-ff` 合并提交的树与分支 head 的树逐字节相同，第二次门禁是在同一份内容上重跑一遍。

实测（2026-11-12）：T-48 合并提交 `502e656` 的树 `a60b8ee` 等于其第二父提交（分支 head）的树 `a60b8ee`；T-42 `6fb7ef1` 同样（`1451f33`）。当天 102 次门禁共 18.6 小时，中位数 10–13 分钟一次；每个任务合并都多付一次 12 分钟左右的重复门禁，还占一个门禁名额，拖慢其它任务（并发门禁下 Rust 测试从约 3 分钟涨到 10 分钟）。重复的那一次只可能因为环境抖动而失败（当天 T-33、T-39 的 main_gate_failed 警报都是这种误报）。

## 要做什么

1. 进入 MainCheck 时，比较合并提交的树（`<merge>^{tree}`）与本任务最近一次**通过**的 MergeGate 所在提交的树。相同 → 不启动作业，写 `gate.reused { node: "main_check", sha: <合并提交>, from_sha: <MergeGate 的提交>, tree }`（沿用 MergeGate 同 sha 复用已有的 `gate.reused` 事件形状，字段只增不改），按通过推进到 Cleanup。
2. 树不同（合并时 main 又前进、产生了真正的合并）→ 行为与今天完全相同，照常跑 MainCheck。
3. 没有通过的 MergeGate 记录（例如门禁是任务停留期间才配上的）→ 照常跑。
4. Bug 任务的 MergeGate 命令是 `( 复现命令 ) && ( 门禁命令 )`，包含了门禁命令，同样适用。
5. `autome run` 打印一行「T-x 验证 main 沿用合并前门禁的结果（内容相同）」；`task.get.gate` 等读者按现有 `gate.reused` 的处理显示。
6. 可关：`[gate] reuse_for_main_check = false` 时永远照常跑（缺省 true）。

## 不做什么

- 不改 MergeGate、Gate 的行为；不改 MainCheck 失败时「只报警」的语义。
- 不比较除树以外的东西（提交信息、作者不影响门禁结果）。
- 工作区项目今天就跳过 MainCheck，不变。

## 验收标准

1. e2e：任务 rebase 后过 MergeGate 再合并，没有任何 `gate.started{node: main_check}`，有一条 `gate.reused{node: main_check}`，任务 Done；门禁脚本的调用次数比今天少一次。
2. e2e：按下合并后 main 再前进一步（走自动 rebase 之外、让合并提交的树与 MergeGate 的树不同的构造），MainCheck 照常运行。
3. e2e：`reuse_for_main_check = false` 时行为与今天逐事件相同。
4. 现有 MainCheck 相关用例里断言「合并后跑了门禁」的，按新缺省逐条说明并改写（不是删除）；其余现有用例不改。
5. `cargo test --workspace`、`sh scripts/ci-local.sh` 全绿；README Host gate 一节补一句。

## 参考

- `crates/automed/src/scheduler.rs` 的 `run_gate`（MergeGate 同 sha 的 `gate.reused` 分支、MainCheck 的 `_verify-<sha>` 检出）。
- `crates/autome-domain/src/task.rs` 的 `Merging → MainCheck → Cleanup`。
- T-47（门禁排队）改同一个函数，设计时注意两者的先后：复用判断在准入之前，不占名额。
