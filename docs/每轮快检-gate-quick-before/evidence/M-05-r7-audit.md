# 审计 #7 · 独立复验（实现 #7 的 CLI 测试夹具修复）

本轮没有 `待审` 里程碑（M-01–M-05 均已完成）。复验范围：实现 #7 改的 `crates/autome-cli/tests/cli.rs`，对应 M-05 验收契约的 autome-cli 子集。

- `cargo test -p autome-cli --test cli -- run_json_tick_error`：1 通过；连续再跑 8 次，8/8 通过（竞态已消除，不是碰巧）。
- `cargo test -p autome-cli -- render::tests::gate_`：11 通过，与 M-05 契约预期一致。
- 差异核对（`git diff HEAD~2`）：只改测试夹具（`--draft` 建任务、断言无运行会话、加强错误前缀与 TOML 断言、无 `idle`），产品代码零改动，SIGINT 与无 idle 尾行检查保留。
- 负向对照：本轮不改动被跟踪文件，沿用 M-05 审计 #5 已做的对照；新增断言（`starts_with("{id}:")` 与 `TOML parse error`）比原来的 `contains(id)` 更严，不会放过会话回收错误抢先输出。
- 完整门禁由宿主合并前重跑，本轮不跑。
