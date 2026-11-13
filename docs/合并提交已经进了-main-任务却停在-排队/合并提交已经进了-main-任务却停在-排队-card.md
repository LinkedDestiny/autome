# 合并提交已经进了 main，任务却停在「排队（会从 验证 main 继续）」：合并后的验证与清理还在等会话槽位

## 现象

T-73 修好了「按合并要等会话槽位」，合并本身现在立即执行。但合并完成后的下一步仍然受槽位限制。2026-11-13 09:50 的现场（`parallel = 3`，三个设计 / 评审会话占满槽位）：

```
09:50  T-52 → 等你合并
09:50  T-52 → 运行中 · 合并中
09:50  T-52 → 排队（会从 验证 main 继续）
```

main 上已经有 `b4eaa71 merge(autome): T-52 …`，但任务不是「已完成」：worktree 与分支没清理，`[merge] after` 钩子没跑，`autome status` / 面板上它还占着一行「排队」。这一步本来只是「沿用合并前门禁的结果（内容相同）」，不跑任何命令，却要等别的任务的会话结束（十几到几十分钟）。

## 复现步骤

1. 项目配 `parallel = 1`、`[gate] command`（`reuse_for_main_check` 缺省 true）；任务 A 处于实现会话中；任务 B 停在 `AwaitMerge`，合并前门禁已在当前提交上通过。
2. 对 B 执行 `autome merge`，推进一个 tick。
3. 看 B 的状态。

## 期望行为

- B 在不等槽位的情况下走完合并尾段：`Merging` → `MainCheck`（沿用结果，不跑命令）→ `Cleanup` → 已完成；`[merge] after` 钩子按现有规则触发。
- `MainCheck` 需要真正跑一次门禁（内容不同、`reuse_for_main_check = false`、没有可沿用的结果）时：按 T-73 写明的口径处理（门禁是重负载，受 `[gate] concurrency` 排队约束）；是否占会话槽位由定位轮核对 T-73 的结论后保持一致，并在 Bug 文档里写明。无论哪种口径，状态显示都要说清楚在等什么（「等门禁」而不是泛泛的「排队」）。
- `Cleanup` 不等槽位。
- T-51 / T-73 的不变量不回退：会启动会话的用户动作满槽仍排队；`slots_in_use` 的现有测试通过。

## 实际行为

见现象。`Merging` 不再排队，但它完成后的 `MainCheck` 被准入逻辑当作占槽节点，改成了 `Requeued { resume: MainCheck }`。

## 环境

main `b4eaa71`；`crates/automed/src/scheduler.rs`（T-51 / T-73 的准入与 `slots_in_use`、合并尾段的推进）、`crates/autome-domain/src/task.rs`（`Node::is_core_step`、`(Merging, Ok)` → `MainCheck`）、`crates/automed/src/scheduler_gate_queue_tests.rs`。

## 线索（可选）

- T-73 的回归测试 `merge_does_not_wait_for_a_session_slot` 只断言到进入合并；本任务的回归测试应断言到 `Done`（沿用结果的路径）。
- 与 T-70（合并队列，设计待批准）改同一段代码，后合并的一方 rebase 时保留对方行为。
- 回归测试写完先跑 `cargo fmt --all -- --check`。复现命令用仓库根目录可直接执行的形式，例如 `cargo test --offline -p automed --lib merge_tail_reaches_done_without_a_session_slot -- --test-threads=1`。
