# M-13 r23 集成复验（审计 #22）

结论：**通过**，里程碑表不动（M-13 保持已完成）。

范围：`40994c7..ddc1a3c`，rebase 后 `34ac401`、`5d0ca6c`（原 `a10992d`、`3e54e59`）。

1. 冲突标记：两个提交 `git grep -nE '^(<<<<<<<|>>>>>>>) '`（排除任务文档目录）均为空。
2. 冲突文件 `crates/autome-cli/src/main.rs`：base 侧的 `note` 动词（USAGE、allowed、`note_text`、测试）与 `task_state_line` 全部保留，web 侧的 `mod web`、`Some("web")` 分支、USAGE 行与 `usage_lists_web` 都在。`tests/cli.rs` 同样两侧并存。rebase 中消失的 `a10992d`/`3e54e59` 是同一补丁的改写，产品行为没有丢。
3. 测试没有被削弱：只新增（clock、login-path、web-service、cli 的 web 用例），既有文件的改动只是适配本地时间与中文文案。
4. 验收命令（M-13）与相关子集：vitest 10 文件 102 用例全过，跳过 0（含 timeline-failure-path、task-timeline、labels）。`cargo test -p autome-cli` 67 + 52 全过。
   - 首次跑 `build_id_cli_and_kernel_agree` 失败，原因是 `target/debug/automed` 还是旧提交的构建；`cargo build --workspace` 后通过。属环境陈旧，不是缺陷。
5. 负向对照：临时删掉 `Some("web")` 分支后，`web_forwards_arguments`、`web_forwards_core`、`web_exit_code`、`web_does_not_open_ledger` 失败；已还原，`git status` 干净。

无实现缺陷，无验证缺口。
