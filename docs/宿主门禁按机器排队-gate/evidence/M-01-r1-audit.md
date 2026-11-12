# M-01 r1 审计复验

结论：通过（附一处验证缺口，已当场补上）。

- 验收命令 `cargo test -p autome-domain && cargo test -p automed --lib`：domain 393 通过、automed --lib 655 通过。
  契约写 domain 394（基线 385+9）；实测在 `ed2460d` 干净检出上基线是 384，+9 = 393，新增数 9 / 7 与契约一致，差 1 来自契约基线笔误，非用例丢失。
- 新增用例复跑：domain `gate_queue_` 6、`gate_concurrency_` 3，automed `gate_concurrency_` 7，全部通过。
- 负向对照（均已还原，`git status` 干净）：
  1. `resolve` 去掉 `.min(global)` → `gate_concurrency_cannot_exceed_global` 红。
  2. `admission` 改为 `held + position - 1 < limit` → 4 个 `gate_queue_` 用例红（含「队首挡住后面所有人」）。
  3. `save_global` 写回 `config.gate.concurrency` 默认值 → `gate_concurrency_save_global_preserves_handwritten_value` 红。
  验收命令能拦住三类错误实现。
- 验证缺口：`cargo clippy --workspace --all-targets -- -D warnings` 报 `items_after_test_module`（`parse_gate_concurrency`
  放在 `config_io.rs` 的测试模块之后），会让 ci-local 失败；验收命令不含 clippy 所以没拦。产品行为没错，
  当场把函数移到测试模块之前；复验 fmt、clippy 全 workspace 干净，automed --lib 仍 655 通过。不计 reopen。
