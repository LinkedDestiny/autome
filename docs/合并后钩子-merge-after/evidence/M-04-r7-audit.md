# M-04 · r7 · 审计复验

结论：通过，M-04 标 `已完成`。

- 负向对照：README 去掉 `[merge]` / `AUTOME_MERGE_COMMIT` 后计数为 0；往 `.autome/config.toml` 追加一行后 `git diff --exit-code` 非零。均能使验收失败，已还原（工作区干净）。
- 验收命令：`[merge]` 计数 README 2、技术设计文档 3；`AUTOME_MERGE_COMMIT` README 1；打印 `AUTOME-DIR-UNCHANGED`；main 是 HEAD 的祖先。
- 直接读文档：README 示例块含 `[merge]` 两行，After merge 条目覆盖时机、环境变量、默认 10 分钟、失败只报警、跳过、gitignore、停止后重跑；技术设计文档 §3.3 / §8.1 / §9 均有对应段落。
- `cargo test --workspace --all-targets`：1 个失败 `gate_queue_upgrade_collects_two_old_jobs_without_killing_them`（B-08，干净 main 也复现的间歇失败）；单独连跑 3 次全部通过。其余 1206 通过、1 忽略。
- `sh scripts/ci-local.sh` 退出 0（680 秒）：Format、Clippy、Rust、Desktop、Web（385 全过）各步通过。实现 #7 的 Electron SIGABRT / Web EPERM 属环境问题，本轮未复现。
- 临时产物：`.autome/output/m04a-*.txt`。H-01、H-02 仍由用户合并前确认。
