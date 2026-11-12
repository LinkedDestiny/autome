# M-02 r2 审计复验

结论：通过。

## 负向对照（逐项注入、跑 `cargo test -p autome-cli task_convert`、还原）

| 注入 | 结果 |
|---|---|
| A `--start` 移出 task 选项白名单 | 红：cli 用例 2 |
| B `USAGE` 删掉 `task convert` 行 | 红：cli 用例 1 |
| C `render.rs` 提示改回 `task new --kind feature --brief` | 红：render 用例 |
| D `--json` 只打印 `.task` | 红：cli 用例 1、2 |

全部还原，`git status` 干净。

## 验收
- `cargo test -p autome-cli task_convert`：render 1 + cli 2 = 3 个通过；cli.rs 集成测试共 15 个。
- `cargo test --workspace`：26+15+384+656+102+16+7 = 1206 passed，0 failed，1 ignored，与 M-03 契约的数字一致。原始输出 `.autome/output/m02-audit-cargo.txt`。
- README 第 207 行已含 "turn a not-a-bug task into a feature"。
- 未跑宿主门禁（Desktop/Web 环境阻塞，实现轮已记；与 M-02 范围无关，M-03 全量回归会覆盖 web）。
