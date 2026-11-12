# M-02 审计 #2 独立复验

- 验收命令：lib 614 通过；e2e 88（首次全量 1 个既有竞速失败，单跑与二次全量 workspace 均通过）；cli 20+11；e2e 过滤 `setup` 8。
- 竞速判定：失败用例不配 `[setup]`；基线 worktree 20 次失败 2 次，本分支 20 次失败 1 次。
- 负向对照（`.autome/output/a2-mut.sh`，每次注入后 `git checkout` 还原）：E1、E4、L1、L4、E14 均如期失败；注入「不删 .exit」存活，原因是 `gate::start` 先 `job.clear()`（等价变体）。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -D warnings`、`cargo test --workspace --all-targets` 全绿。
- 结论：通过。
