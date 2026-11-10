# M-01 r5 审计复验

- 验收命令全串跑通过：域 requeued 9、automed lib requeued 6、域全量 288、lib 全量 540，clippy、fmt 无输出错误（输出在 `.autome/output/a1-accept.txt`）。
- `cargo test --workspace` 全绿，0 失败（`.autome/output/a1-ws.txt`）。
- 负向对照逐条做过并已还原（`git status` 干净）：
  - A Resume 产出 `Queued`：域 3 个用例失败（Paused→Requeued、Stopped→Requeued、门禁 discard）。
  - B 出队一律 `StartIntake`：「13 节点出队映射」「Paused 保留节点」失败。
  - C 去掉 `skip_serializing_if`：JSON 形状用例失败。
  - D `queued_tasks` 不收 `Requeued`：账本 1 + 调度 2 个用例失败。
  - E `discard_node` 等待 pid 时限改 0：门禁用例失败。
- 自查 `Queued` 残留匹配点：只剩 `autome-cli` 的 `idle`（设计 B-03 已知、不动）与 apply 内对 `Queued` 本身的处理。
- 结论：通过。无人工项。
