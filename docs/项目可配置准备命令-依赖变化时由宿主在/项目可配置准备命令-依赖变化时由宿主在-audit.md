# 实现审计 · 审计 #2

M-02：通过。验收命令计数与预期一致（automed lib 614、end_to_end 88、autome-cli 20+11、过滤 `setup` 的 e2e 8）；fmt、clippy -D warnings、workspace 全量绿。
负向对照（缺陷注入后均还原）：启动准备后直接判 Ready → E1 失败；停止时不 `discard_node("setup")` → E4 失败；`poll_as` 文案写死「门禁」→ L1 失败；`running` 不认 `.started` → L4 失败；成功后记当前指纹 → E14 失败。「失败后不删 `.exit`」注入未被拦住，但 `gate::start` 先 `job.clear()`，该变体与正确实现等价，E2 已断言重跑后 `setup.started` 共 2 条。
一次 e2e 全量里 `bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it` 失败；基线 2553923 同样约 2/20、本分支 1/20，未配 `[setup]`，判为既有竞速，进 Backlog B-A2-01，不计缺陷。
无实现缺陷、无验证缺口。注入脚本 `.autome/output/a2-mut.sh`（可丢弃）。
