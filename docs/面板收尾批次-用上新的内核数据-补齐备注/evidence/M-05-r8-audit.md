# M-05 · 审计 #5 复验（r8）

结论：退回（实现缺陷），详见审计文件。

| 检查 | 结果 |
|---|---|
| vitest 4 文件（payload / data / timeline / test-groups） | 41 通过，与预期 41 一致 |
| `cargo build -p automed -p autome-cli`、`npm run build --prefix apps/web` | 通过 |
| Playwright manual-items + layout | 19 个用例：18 通过，1 失败（C-2） |
| C-2 失败原因 | 夹具第 2、3 步文档相同 → 第 3 步 `git commit` 无改动 → 会话退出码 1 → 任务 failed，会话数停在 3；经 `task.get` 的 `state.reason = session_crashed` 与会话数 3 确认，重复 3 次一致 |
| 判别：夹具改成每步文档不同（临时副本，已删） | C-2、C-2b 通过 → 产品窗口钉住 / 滚动 / 日志保留行为正确 |
| 契约浏览器负向对照 | 未执行：夹具先挂，对照没有意义；留给修复后的实现轮 |
| 工作区 | 只改文档目录；临时副本已删除，`git status` 仅文档文件 |
