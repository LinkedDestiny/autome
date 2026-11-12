# M-04 审计 #4 独立复验

- e2e 96 通过 / lib 617 通过 / `setup` e2e 16 通过；文档 `[setup]` 各 2；`.autome/config.toml` 与 2553923 一致。
- ci-local：除 `Test` 步外全部 ok；`Test` 红因是 gate.rs 既有测试竞速（Vanished vs 124），单跑 4/4 通过，基线同分支代码，Backlog B-06。
- 负向对照：①`needs_run` 恒真 → E11 失败；②关闭主检出互斥 → L3 失败；③`setup.skipped` 不去重 → E13 失败；④删掉 Cleanup 里的 skip_workspace 调用 → E13 仍过（工作区跳过另有调用点，属冗余而非缺口）。全部还原。
- 临时输出：.autome/output/m04a-*.txt
