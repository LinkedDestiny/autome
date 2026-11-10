# M-02 第 2 轮审计独立复验

结论：通过。

## 验收命令（自己跑）
- `protocol eval protocol/` 退出 0，版本 protocol/v2，8 项通过 0 不通过（1 项历史警告）；`prompt_rendering` 16 passed（bug_loop 5）；`launcher::tests::bug_loop` 3 passed；`bug_loop_protocol` 7 passed；`protocol::` 61 passed；e2e 版本冻结用例 1 passed。
- `cargo test --workspace` 全绿（domain 345、automed lib 595、end_to_end 59 等，0 failed）。输出 `.autome/output/a2-accept.txt`、`a2-ws.txt`。

## 负向对照（注入后用 `git checkout` 还原，工作区干净）
| 注入 | 结果 |
|---|---|
| `prompts/fix.md` 删 `{budget_line}` | `protocol eval` 报 ✗ 且不通过；`…requires_budget_line…` 失败 |
| 契约示例写成 `复现命令 = …` | `…contract_example_parses_with_the_kernel_parser` 失败 |
| 定位去掉卡片前置段 | `bug_loop_locate_prompt_with_a_card_matches_its_golden` 失败 |
| 既有 `impl.md` 加一行 | `every_role_prompt_renders_exactly_what_the_constants_did` 与预算占位用例失败 |
输出 `.autome/output/a2-neg.txt`。四条全部被拦住，验收有区分力。

## 判定
无产品缺陷，无验证缺口，无人工验收项。
