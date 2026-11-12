# M-01 r1 审计复验

- 验收命令三段：32 / 43 / 38 个用例全部通过，与契约一致。
- 全工作区 `--list` 1216 个（= 1209 + 7），运行 1215 通过 + 1 ignored（基线即有）、0 失败。
- 负向对照（逐个注入、跑对应用例、`git checkout` 还原，工作区干净）：
  - `unwrap_or(true)`→`unwrap_or(false)`：`defaults_true_and_project_overrides` 红。
  - 解析兜底接受字符串 `"false"`：`rejects_non_booleans` 红。
  - `tree_id` 改取 `{commit}`：`tree_id_of_a_no_ff_merge…`、`tree_id_of_an_unknown_commit…` 红。
  - `second_parent` 改取 `^1`：`second_parent_of_a_no_ff_merge…` 红。
  四个变异全部被拦住。
- 运行时行为：diff 只含配置字段、解析、两个只读 git 函数与测试；无人读开关，行为不变。
- 脚本：`.autome/output/m01-mut.sh`。
