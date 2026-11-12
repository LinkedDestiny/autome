# M-05 · 审计 #5 独立复验

验收命令（自己跑）：
- `cargo test -p autome-cli -- render::tests::gate_` → 11 通过（现有 9 + 新增 2）。
- `node --test apps/desktop/test/labels-gate-kind.test.js` → 3 通过。
- 文档 grep 组合命令 → 退出码 0。
- 补充：`cargo test -p autome-cli` 全 crate 36 + 16 通过；`cargo fmt --all -- --check`、`cargo clippy -p autome-cli --all-targets -- -D warnings` 通过。

负向对照（均已还原，`git status` 干净）：
- render.rs 的 `kind == "quick"` 判断改成 `false`：`gate_quick_status_line_names_the_kind`、`gate_quick_run_lines_name_the_quick_run` 失败（9 过 2 败）。
- labels.js `gate.kind === 'quick'` 改成 `false`：桌面「需求快检」「FixGate 快检」失败（1 过 2 败）。
- README 保留原句：第三条验收退出码 1。

审阅：`main.rs` 的 `task_detail` 改走 `render::gate_status_line`，非快检时格式与原 println 一致（"  门禁      节点 在 sha 上结论（命令）  日志 …"）；`task.js` 经 `gateTagLabel`，无 kind 回落 `nodeLabel`。README / 技术设计 §8.1 内容与设计 §11 一致，两处旧句已改。

耗时实测：实现轮证据已含四段与整条热缓存耗时（装 web 依赖后取得），非「未取证」；我未复测数值。

人工项：H-01、H-02 已在清单中，由用户合并前确认。
结论：通过。无缺陷、无验证缺口。
