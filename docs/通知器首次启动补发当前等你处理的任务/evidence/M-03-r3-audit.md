# M-03 · 审计 #3 独立复验

- 本沙箱可监听，实现轮的 EPERM 不复现。`cargo build -p automed -p autome-cli` 后定向运行 3 文件 9 通过（backfill-real 4、notifier-real 2、notifier-live 3）。
- `npm test`：34 文件 315 通过 0 失败（= 基线 301 + 14，其余文件相对共同祖先未改）；`lint`、`typecheck`、`build` rc=0。
- 末尾三行：store 14 18、notifier 11 17、notifier-real 2 2，与预期一致。`crates/` 相对共同祖先无改动。
- 负向对照：在 notifier.ts 首次启动 backfill 处传空 notifications（等价于不补发）→ 验收 1、3 失败（2 failed | 2 passed），git checkout 还原，工作区干净。
- 人工验收清单 4 条保留给用户合并前确认。
- 结论：通过。输出日志在 .autome/output/m03-*.log。
