# M-04 第 4 轮 审计复验（审计 #4）

结论：通过。

## 验收命令
`cargo test -p autome-cli --test cli kernel_guard`：8 个用例通过，0 失败、0 忽略（与预期 8 一致）。

## 负向对照（均已还原，`git status` 干净）
| 缺陷注入 | 结果 |
|---|---|
| `drive` 每次跳过都打那句话（去掉「消息变化才打」判断） | 失败 3 个：run_says_it_once_and_stays_up、run_json、run_resumes |
| `older_kernel` 当普通 skipped（打「另一个驱动」） | 失败 4 个，含第 1 号 |
| 常驻 run 被拒后直接退出 | 失败 3 个，含第 1 号 |
| `serve_stdio` 往 stdout 先打一行 | 第 6 号 `core_subcommand_serves_stdio` 失败 |

## 自造检查
临时 AUTOME_HOME，先真实标识 `run --once` 写账本，再以 `AUTOME_TEST_BUILD_ID` 扮旧内核：
- doctor 文本含「内核：<旧>（账本记录：<新>）」与那句话；
- `run --once` 退出码 1，stderr 为 `autome：<那句话>`；
- `AUTOME_ALLOW_OLDER_KERNEL=true`（非字面量 1）无效，仍被拒；
- `--version` 带覆盖后的旧标识。

## 说明
整个 cli.rs 跑一次时 `build_id_cli_and_kernel_agree` 失败：`target/debug/automed` 是旧提交构建的（`cargo test -p autome-cli` 不重建它）。按 M-01 验收命令先 `cargo build -p automed` 后 2 个 build_id 用例通过。属环境陈旧，非产品缺陷。
