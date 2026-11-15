# M-09 · r17 · 审计复验

验收命令（独立重跑）：residue 单测 18 通过；end_to_end `residue_` 3 通过（9.68s）；task-timeline 24 通过。数与契约一致。
clippy（automed）与 fmt 检查通过。`residue::sweep` 在 scheduler 的合并清理与取消清理两处各有调用。

负向对照（逐个注入、跑、`git checkout` 还原，工作区干净）：
1. 去掉控制终端规则 → residue `residue_tty_never_signalled`、`residue_tty_acquired_before_kill` 失败；e2e `residue_merge_preserves_other_checkouts_and_controlling_terminal` 失败。
2. 路径改字符串前缀比较 → `residue_components_not_prefixes` 失败。
3. 复核不比较进程起始时间 → `residue_pid_reused_no_kill_or_survivor` 失败。
4. 时间线不看 `kind: tty` → task-timeline「无法判断与有控制终端分别显示」失败。

自造检查：以上四个变异即判别检查，均能被验收命令拦下；未发现产品缺陷。
未跑：宿主全量门禁；Web 监听端口类用例不在本契约内。提交由本轮自行完成（被拒则 Autome 兜底）。
