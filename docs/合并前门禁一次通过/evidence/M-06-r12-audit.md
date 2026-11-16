# M-06 审计 #8 独立复验（实现轮 r12）

- vitest 三文件（task-page-effect / live-routes / test-groups）：22 通过，与基线一致。
- ★ 构建（web build + cargo build automed/autome-cli）成功；`-g 'B-3|BL-01'` 五次独立调用全部 2 passed（10 条），退出码 0。日志 `.autome/output/m06-a8-pw1..5.log`。本会话浏览器可用（无 EPERM）。
- 负向对照：`node apps/web/scripts/test-related-mutation.mjs` 退出 0；BL-01 在注入 `&& false` 下 5 次均因 `not.toContainText` 依赖文案未更新而失败；工作区还原后 `git status` 干净。日志 `.autome/output/m06-a8-mut.log`。
- B-3 读取顺序：97–99 行现为先读依赖任务、后读前置（active ⇒ done），5 次无误红，上一轮第 5 次竞态未再现。
- 未单独重做「B-3 改回单次读取」变异（竞态概率性，且该缺陷已由读取顺序修复）；记为未做。
- 宿主快检：简报称在 3a910286f8 通过（退出码 0）；本轮引用，不替代上述证据。
- 新造检查：无；结论：通过。
