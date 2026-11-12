# M-04 r5 审计复验

## 负向对照（均已还原，git status 干净）
- 指纹不做数字归一化（gate_failure.rs 的数字分支禁用）→ 用例 1、3 失败（reason.kind 为 protocol）✔
- streak 不比较指纹 → 用例 2 失败（第 3 次就熔断）✔
- 重跑门禁（task.rs RerunFrom 分支）不给 DiscardGateRuns 改给 RunCoreStep → 用例 3 失败 ✔
- 过程注记：第一次误改了 Resume 与 extend 分支的 DiscardGateRuns，用例仍绿——这两处不在用例 3 的路径上（用例 3 走 rerun_from），不是缺口；改对 RerunFrom 分支后即红。

## 验收命令
- `cargo test -p automed --test end_to_end same_failure`：3 通过
- `end_to_end` 全量 83 通过（80+3）；`cargo test --workspace`：domain 374、automed 库 618、cli 19+11、prompt_rendering 16、real_cli 7，0 失败，1 忽略（t14_gate_logs）。
- 自读用例：用例 2 断言 prompt 含上次 tail、指纹逐次不同、same_failures 全为 1；用例 3 断言同 sha、无新实现会话、结果 4 条。

## 结论
通过。无实现缺陷、无验证缺口。
