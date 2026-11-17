# M-02 r2 审计复验

## 负向对照（先跑，已用 git checkout 还原，工作区干净）
- 复盘分支改回 `ctx.store.task_metrics(&task.id)?`：`end_to_end retro` 7 过 2 败（`retro_gate_metrics_loop_prompt_and_final`、`manual_retro_gate_metrics_for_a_paused_task`），验收命令能拦住。
- 复盘前改用 `write_task_metrics` 落账本：同样 7 过 2 败（同两例），能拦住。

## 验收命令
- `cargo test -p automed --test prompt_rendering`：52 过（与契约预期一致）。
- `cargo test -p automed --test end_to_end retro`：9 过（与契约预期一致）。
- 相关子集：`--lib retro_metrics` 1 过（读失败退回存量、再退回 None，且不写 `task.metrics`）；`--lib gate_metrics` 9 过；`cargo fmt --all -- --check` 通过。
- 宿主快检在 f9e950d786 上已通过（简报所述）。

## 自造检查
上面两个缺陷注入即为判别检查：两种错误实现都被 E1/E2 的提示词与「不落账本」断言区分。结果：均失败，还原后均通过。

## 结论
通过。无产品缺陷，无验证缺口，无人工验收项。
