# M-03 独立复验（审计 #3，对应实现轮 r4）

- 验收命令：6 个文件、37 个用例全过（含真实内核 task-timeline-real 3、task-log-live 3）；本沙箱可监听 localhost，实现轮的 EPERM 不复现。输出在 `.autome/output/a3-run1.txt`。
- 负向对照（临时注入后立即 `git checkout` 还原，工作区无残留）：
  - session-log `defaultOpen ?? true` → session-log-view 已有用例失败；
  - 时间线 `entries.reverse()` → task-timeline 4 例失败（含第 1、稳定键）；
  - 门禁键改回 `(node,kind,sha)` → 门禁重跑、稳定键 2 例失败；
  - 「符合预期」改按 `kind==='repro'` → Bug 复现用例等 2 例失败；
  - 摘要改用最新快照而非逐会话快照 → 快照摘要用例失败。
- 其它：`tsc --noEmit` 通过，三个时间线/日志源文件 eslint 无输出；源码核对 `task-timeline-model.ts` 与设计「执行时间线」一节一致（门禁身份取首事件 seq、repro 仅关联 repro_check、setup 排队不进门禁条目）。
- 进程：本轮结束后无本轮遗留的 automed/假 CLI 进程（ps 中的存活进程均来自其它 worktree 的旧测试）。
- 结论：通过。
