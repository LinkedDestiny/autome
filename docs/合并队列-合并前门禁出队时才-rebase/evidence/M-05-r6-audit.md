# M-05 r6 审计复验

- 验收命令：6 条 grep 全部命中，输出 `docs-ok`，退出码 0。
- 负向对照：对 README.md / technical-design.md 分别去掉含各模式的行（`grep -v`），6 个模式在剩余文本里均不再命中，说明 6 条 `grep -q` 各自依赖一处真实内容，缺任一处命令不会输出 `docs-ok`。
- 相对 merge-base 的文档差异：只删 3 处与本任务相关的旧表述（沿用条件、Rebase→AwaitMerge、Merging→Cleanup 等），无误删；`diff main` 里 quick / `[budget]` 的删除来自 main 已前进，不是本分支改动。
- 与实现对照：`task.merge` 的 `cancel` 参数、`merge.unlocked{reason:"cancel"}`、`config_warnings`、`(Rebase|MergeGate, Conflict)` 转换均在代码中存在，文档描述一致。
- 结论：通过。
