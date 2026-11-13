# M-03 审计复验（实现轮 r4）· 通过

## 验收命令（自己跑）
`cargo test -p autome-domain merge_conflict_ && cargo test -p automed --lib merge_conflict_ && cargo test -p automed --test end_to_end merge_conflict_` → 退出码 0；domain 2、lib 2、e2e 2，共 6，与契约一致。`cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings` 退出码均 0。

## 负向对照（均已还原，产品文件 `git status` 干净）
- A：需求侧 `(Rebase | MergeGate, Conflict)` 改成只认 `Rebase` → domain `merge_conflict_a_dequeue_conflict_goes_back_to_implement` 与 e2e 需求用例失败。
- B：Bug 侧 `(MergeGate, CoreStepDone Conflict)` 分支的节点改成 `Merging`（使其不匹配）→ domain `merge_conflict_a_bug_dequeue_conflict_goes_back_to_fix` 与 e2e Bug 用例失败。A+B 合起来即契约所述 2 domain + 2 e2e 全红。
- C：出队冲突路径在 `rebase_all` 前先写 `gate.started` → lib `merge_conflict_a_dequeue_conflict_yields_the_queue_and_counts_the_streak` 失败（tests 第 624 行断言）。
- 首次用 sed 注入在 macOS 上语法失败，未生效，随后改 perl/python 重做；该次“全绿”不是对照结果。

## 独立核对
e2e 需求用例断言：冲突后 B 回 `implement`、无 `gate.started`、`merge.unlocked` 1 条、注入文件 `README.md`、授权仍有效；第三个任务的 `gate.started` 早于 B 修好后的 `gate.started`；B 最终 done、`merge.direct` 1 条、main 内容为冲突方。Bug 用例同样断言回 `fix`、门禁次数不变、授权保留、修复轮提示含“修复轮”。契约要求的次序条件已被断言。

## 结论
通过。无实现缺陷、无验证缺口。临时检查在 `.autome/output/`（不入库）。提交由 Autome 兜底（若提交失败）。
