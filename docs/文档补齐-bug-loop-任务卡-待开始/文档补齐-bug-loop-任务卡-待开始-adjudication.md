# 裁决记录

本文件只增不改。

## 裁决轮 #1（design-round 0/15 → 1/15）

**评审结论。** 设计评审 #1 提出 1 条属于六类的问题（D1-P01），另有 2 条非阻塞建议（S-01、S-02）。

**本轮结论。** D1-P01 采纳并已改设计；S-01、S-02 进 Backlog。改后没有剩余的六类问题，设计定稿：状态块 `status` 改为 `实现中`，里程碑表三行完整。

### D1-P01 修复轮预算被写成所有启动路径的硬上限

| 项 | 内容 |
|---|---|
| 稳定 ID | D1-P01 |
| 主张摘要 | T-27 / R-13 把「已开轮数达到上限即 `Failed(FixBudget)`」写成所有修复启动路径的行为，但手动 `rerun … fix` 不经预算检查。 |
| 设计位置 | D4 T-27；D5 R-13（影响 M-01 §5.6 预算段与 M-03 README 配置条目）。 |
| 裁决 | **采纳**（第 4 类：违反任务目标——陈述与代码行为不符）。 |
| 证据 | `crates/autome-domain/src/bug_loop.rs`：`start_fix` 检查 `progress.fix_round >= limit`；`apply_for` 的 `Failed/Stopped + RerunFrom` 分支对 `Node::Fix` 直接 `session(BugStep::Fix, None, ctx)`，不经 `start_fix`；`ExtendBudget` 分支同样直接 `session(...)`.with_budget。`start_fix` 的调用点只有复现命中（`ReproCheck`）、`fix_gate` 失败、复核打回、rebase 冲突、合并门禁失败等自动转换。`crates/automed/src/scheduler.rs` 的 `Action::StartBugSession` 分支对 `BugStep::Fix` 总是追加 `bug.fix_round_started`，所以手动重跑计入已开轮数。测试 `bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only` 以 `fix_round: 20` 与 `FixBudget { limit: 1 }` 断言重跑成功、进入 `Active(Fix)`。本轮已复读上述源码确认。 |
| 修改落点 | 设计文档 D4：T-27 改为「每开一个 `fix` 会话（含手动重跑）记一条 `bug.fix_round_started`；正常自动转换在开下一修复轮前检查，达到上限即 `Failed(FixBudget)`」；新增 T-36（手动 `rerun … fix` 不经预算检查，仅计入已开轮数；锚点为上述测试名）。D5：R-13 补「自动返工在开下一轮前检查……手动 `autome rerun <task> fix` 不经该检查，只计入已开轮数」。D3 的预算引用改为 T-27、T-28、T-36。D7：锚点行 72 → 73，检查条数 102/130/154 → 103/131/155，基线失败数 76/102、76/130、83/154 → 77/103、77/131、84/155（已在基线上重跑确认）。Backlog 新增 B-07 记录该疑似代码缺陷（本任务不修）。 |
| 同类检查 | 复查了 T-23..T-33、R-09/R-10/R-13 与 `apply_for` 全部 `session(BugStep::Fix…)` / `start_fix` 调用：自动返工路径（T-23..T-26）经 `start_fix`；T-28 追加预算与 T-31/R-09 手动重跑直接开会话，已由 T-28、T-36 分别如实表述。无其它同类陈述。 |
| 复提计数 | 0（首次提出） |

### 非阻塞建议

| ID | 主张摘要 | 裁决 | 落点 |
|---|---|---|---|
| S-01 | README R-04 只提第一个 `# ` 标题，T-42 已有「无 `#` 标题时取第一非空行」的回退，可在 README 补一句。 | 不触发复审；纳入 Backlog | B-08 |
| S-02 | M-03 验收命令中 `echo`/`tail` 位于 `&&` 链之后，整条命令退出码可能来自 `tail`。 | 不触发复审（预期已要求核对 `M-03 PASS 155/155` 与 `ci-local exit 0`）；纳入 Backlog | B-09 |

### 争议项

本轮无复提，无冻结的争议项。

### 提交

见 git 日志；若提交被拒，由 Autome 会话结束后兜底提交。
