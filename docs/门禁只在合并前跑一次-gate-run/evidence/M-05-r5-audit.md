# M-05 · 审计 #5 独立复验

结论：通过。

- 验收命令整条跑通，退出码 0：domain `before_merge_prompt` 4；launcher 63；`protocol::` 66；prompt_rendering 17；e2e `prompt_before_merge` 2；两条 git 检查无输出（模板与 prompt 金样本分支未改）。数量与契约一致。
- 负向对照（均已还原，`git status` 干净）：
  1. `scheduler.rs` 给 `PromptSpec.gate_before_merge` 传 `None` → e2e 两个用例失败。
  2. `launcher.rs` 两处 `select_prompt(template, None)`（忽略模式）→ launcher 3 个 `before_merge_prompt_*` 失败。
  3. 需求 `GateFailed` 的 `before_merge` 文字改回「在同一命令下确认通过」→ `before_merge_prompt_feature_failure_limits_repair_scope` 失败。
  （首次尝试用 BSD sed 与错行号，改动未生效、用例仍绿，已改用 perl 并核对 `git diff --stat` 后重做。）
- `cargo fmt --all -- --check` 通过；全量门禁由宿主在合并前跑。
- 无实现缺陷、无验证缺口。
