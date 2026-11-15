# M-05 r9 审计复验（审计 #6）

结论：通过。

- 验收命令逐段重跑：vitest 4 文件 41 用例全过；cargo build、web build 通过；Playwright manual-items + layout 共 19 个全过（layout 12、manual-items 7，含 C-2 55 秒与 C-2b），浏览器在本轮可用。
- 负向对照（临时注入，均已 `git checkout` 还原，status 干净），各只跑 C-2/C-2b：
  - 窗口起点不钉住（`pinned = -1`）：C-2 失败（toHaveCount），C-2b 过。
  - 条目 key 按窗口内序号：C-2 失败（toHaveAttribute，日志被折叠）。
  - `withParams` 丢其它键：C-2 与 C-2b 都失败。
  - 未注入的对照：同一命令（sed 误未生效）下 C-2/C-2b 通过，说明失败来自注入。
- loader 返回整份数据的对照由 task-detail-payload 的 ≤150 KB 断言承担，本轮未重做，也无产品行为疑点。
- 上一轮缺陷（夹具两步文档相同导致第 3 步 commit 失败）已由 `e2e/harness/window-task.ts` 及其单测修复，C-2 现稳定凑满 22 个会话。
- 人工验收项：无需移动。
