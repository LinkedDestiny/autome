# M-01 r1 审计复验

结论：通过。

## 负向对照（逐项注入、跑 `cargo test -p automed task_convert`、`git checkout crates` 还原）

| 注入 | 结果 |
|---|---|
| A 删掉 `set_task_state(from, Cancelled)` | 红：store 用例、用例 1 |
| B 归档标记移到 commit 之后 | 红：store 用例 |
| C 删掉 `discard_checkout` | 红：用例 1 |
| D 不复制 attachments | 红：用例 1 |
| E 去掉节点/归档检查 | 红：用例 3 |
| F 停会话移到提交之后 | 红：用例 8 |
| G brief 做 trim | 红：用例 1 |

全部还原，`git status` 干净。（未单独注入「拆成两次提交」「写进 READ_METHODS」，前者由 store 用例与 R2 注入覆盖，后者由用例 1 的断言覆盖。）

## 验收
- `cargo test -p automed task_convert`：lib 9 + end_to_end 1 = 10 个通过，与契约一致。
- `cargo test --workspace`：1203 passed、0 failed、1 ignored（基线 1193 + 10）。原始输出 `.autome/output/audit1-workspace.txt`。
- 代码阅读：预检顺序、单事务 `convert_task`（含归档标记与三条事件）、提交后清理，均与设计一致。
- 宿主门禁（引用）：`ci-local.sh` 退出码 0。
