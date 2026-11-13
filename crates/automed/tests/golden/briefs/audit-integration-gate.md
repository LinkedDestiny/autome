# 审计轮 简报 · 第 9 轮

这份简报是索引，不是设计文档的替代品。设计文档 `docs/example/example.md` 仍然是权威，本轮需要什么就去读什么——但先读这里，多数轮次不必整份拉进来。

## 本里程碑

还没有里程碑表，或者所有里程碑都已完成。

## 本轮要审的集成改动

本轮是**集成复验**：实现 #9 开始时里程碑表没有 `开放` / `待审`，却在 `docs/example/` 之外留下了提交。规则见 `docs/example/protocol/loop-protocol.md` 的「集成复验」一节。

触发原因：合并前门禁失败。命令：`make check`。输出末尾：
```text
error: lint failed
```

提交范围：`3f2a1c0..9e81b77`（默认分支基线 `55d0e12`），rebase 前后补丁不变的提交不列出。

列出的提交 2 个：
- `a1b2c3d` fix: resolve launcher conflict
  - crates/automed/src/launcher.rs

改动统计（`docs/example/` 之外）：
```text
crates/automed/src/launcher.rs | 2 +-
```
- `e4f5a6b` test: keep old template compatible
  - crates/automed/tests/end_to_end.rs

改动统计（`docs/example/` 之外）：
```text
crates/automed/tests/end_to_end.rs | 2 +-
```

rebase 前有、rebase 后找不到同一补丁的提交 1 个：
- `0c9d8e7` feat: launcher change
  - crates/automed/src/launcher.rs

改动统计（`docs/example/` 之外）：
```text
crates/automed/src/launcher.rs | 2 ++
```

改到的测试文件（逐个看 diff，留意测试前提有没有被改弱）：
- crates/automed/tests/end_to_end.rs

本轮证据必须写入 `docs/example/evidence/M-08-r9-audit.md`。结论二选一：通过（里程碑表不动）；或把受影响的里程碑退回 `开放`、`reopen` 加 1。两种都要写这个证据文件。

## 上一轮对手轮的结论

没有待审的里程碑。

## 预算与用户表态

本轮没有预算行。
