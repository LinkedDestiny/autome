# M-04 独立复验（审计 #4）

- 负向对照：把 `render.rs` 的 `Some(resume)` 分支改成恒返回「排队」，`cargo test -p autome-cli` → `requeued_states_show_the_original_node` 失败（9 过 1 败）。已还原（备份在 `.autome/output/render.rs.orig`，`git status` 干净）。
- 验收命令：`cargo test -p autome-cli`（10 + 4 通过）、`cargo fmt --all -- --check` 无错误。
- 额外：`cargo test --workspace` 914 通过 0 失败；`cargo clippy --all-targets -- -D warnings` 无告警。
- 判据核对：13 个可恢复节点都显示「排队（会从 X 继续）」，事件行同样；`{"state":"queued"}` 与 `resume:null` 仍为「排队」。CLI 中 `queued` 的唯一生产匹配点就是 `state_line`，无遗漏。
- 结论：通过。
