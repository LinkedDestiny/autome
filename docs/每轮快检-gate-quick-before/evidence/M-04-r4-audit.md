# M-04 · 审计 #4 复验

结论：通过。

## 验收命令（自己跑）
- `cargo test -p automed --lib -- gate_quick_prompt gate_quick_brief before_merge_prompt brief::tests::before_merge`：25 通过（与契约预期一致）。
- `cargo test -p automed --test prompt_rendering`：31 通过。
- `cargo run -p automed -- protocol eval protocol/`：退出码 0（8 项通过、0 不通过、1 警告：继承条文无 CHANGELOG 说明，与本任务无关）。
- `cargo fmt --all -- --check` 退出码 0；`cargo clippy --all-targets -- -D warnings` 退出码 0。
- 旧金样：`git diff --name-status 042fd06 HEAD` 中 golden 目录只有 4 个 A，没有 M，即旧金样逐字节不变。

## 负向对照（三条都做了，均已用 git checkout 还原，工作区干净）
1. `gate_line_for_mode` 不看 `kind`（`if false && …`）：`brief::tests::gate_quick_brief_actual_result_and_timeout_in_both_modes` 与金样 `gate_quick_audit_brief_matches_golden` 失败。
2. `fix_gate_note` 固定返回 `BUG_REVIEW_GATE_NOTE`：`gate_quick_prompt_legacy_templates_correct_once`、`…commands_are_data_and_empty_results_are_compatible`、`…review_follows_result_across_mode_changes` 与金样 `gate_quick_bug_prompts_match_goldens` 失败。
3. 在占位符检查之前用 `.replace` 塞入 `{gate_line}` / `{fix_gate_note}`：`before_merge_prompt_role_…`、`before_merge_prompt_bug_…`、`gate_quick_prompt_commands_are_data_…` 与 `bug_inputs_and_gate_modes_preserve_user_data_independently` 失败。

验收命令能拦住三类错误实现。

## 新造的判别检查
上述三处注入即为本轮判别检查，结果都是红；没有发现实现缺陷或验证缺口。
