# M-03 审计复验（审计 #3）

结论：通过。

- 验收命令：`probe:sqlite` probe=0，两行 `ok: true`：系统 Node 26.5.0（SQLite 3.53.4，wal）、Electron 43.2.0（内置 Node 24.18.0，SQLite 3.53.1，wal）；`npm test` exit=0，4 个文件、32 个用例通过（基线 24，新增 sqlite-probe 8）。实现轮的 EPERM/EMFILE 阻塞在本环境未复现。
- 负向对照（改错、跑 `test/sqlite-probe.test.ts`、还原；`git status` 干净）：
  - `sqlite-check.mjs` 只测 `:memory:`：2 个失败（WAL 用例、版本用例）。
  - 定位改为 `require('electron')`：3 个失败（缺 dist、worktree 优先、返回空）。
  - 去掉 Electron 版本核对：1 个失败（版本不符用例）。
- 自造检查：Homebrew Node 22.23.1 直接跑 `sqlite-check.mjs`：ok、wal、SQLite 3.53.4，仅打印 ExperimentalWarning，与设计 §7 一致。
- 设计 §7 结论（采用 `node:sqlite`）与脚本输出一致，无需退回 better-sqlite3。
- 注：`AUTOME_ELECTRON` 指向不存在路径时回落到 worktree/主检出，探针仍为 0，符合设计「按序取第一个可用」。
