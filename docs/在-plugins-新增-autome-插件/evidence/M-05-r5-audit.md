# M-05 审计 #5 独立复验

- 验收命令 `sh plugins/autome/tests/run.sh evals`：E1–E5 全过（5/5）；`run.sh all` 59/59，与契约合计一致。
- 负向对照（逐项注入，复验后 `git checkout -- evals` 还原，工作区干净）：
  - 去掉 kind-ambiguous grader 的「阳性对照」标题 → E2 失败
  - README 去掉 `--max-cost-usd 2.00` → E5 失败
  - 清空 bug-missing-repro grader → E1、E2 失败
  - case.yaml 加 `scaffold_script` → E4 失败
  - ask-before-inventing grader 规则段去掉「验收标准」 → E3 失败
  - `runs: 0` → E1 失败
  - README 去掉 `不要加 --allow-tools Bash` → E5 失败
- 自造检查：三个场景 prompt 与 grader 对照 SKILL.md 第 1 步（类型不明先问、验收标准/复现步骤/期望行为不得推断、一次问完）语义一致；`claude plugin validate plugins/autome --strict` 通过。
- 未做：真实 `claude plugin eval`（收费）→ 已是 H-05 人工项，非本里程碑自动验收。
- 结论：通过。无验证缺口。
