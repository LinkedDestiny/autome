# M-04 · 审计 #4 独立复验

- 验收命令 `cargo test -p automed --lib timeout_visible && cargo test -p autome-cli timeout_visible`：automed lib 3、autome-cli 3（render 1、cli 集成 2），共 6，全绿，与预期一致。
- 负向对照（注入后均还原，`git status` 干净）：
  - `limit_minutes` 不经 `effective_limit`（旧会话给 null）→ ② 失败。
  - `session_json` 的 `timed_out` 字段丢失 → ① 与 ② 失败（task.get / session.log 共用 session_json，漏任一入口都会被拦）。
  - CLI 运行中行不读 `elapsed_minutes` → ⑤ 失败，⑥ 仍过（判别正确）。
- 引用宿主快检在 b902eb3115 通过；未跑全量。
- 手验项（桌面端会话列表显示、真实模型超时）已在 H-01/H-02。
- 结论：通过。
