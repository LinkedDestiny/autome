# M-04 审计复验（审计 #4，实现轮 r5）

## 验收命令（独立运行，全部通过）
- `gate_history` 9 个通过；`prompt_rendering` 48（M-03 后基线 47 + 1）；end_to_end `gate_history` 2 个通过；vitest e2e-catalog 2 + test-groups 4 = 6 个通过（含真实 `--list`，本环境可运行，未受 ps EPERM 影响）。

## 负向对照（均注入后确认变红，已逐字还原，`git status` 干净）
1. 编号匹配去掉边界（`!boundary` 恒真）→ `gate_history_number_boundaries` 红。
2. 也读 `-impl.md`（后缀过滤改 `.md`）→ `gate_history_only_backlog_and_audit` 红。
3. 触发条件去掉 `node == "merge_gate"`（任何 GateFailed）→ `gate_history_following_quick_repair_has_no_history` 红。

## 结论
通过。无新造的保留检查（负向对照即判别检查）。ps EPERM 受阻项：本轮所跑验收均不依赖 ps，无需移入人工清单。
