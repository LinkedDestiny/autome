# 审计 · 第 15 轮 · M-09

结论：**通过**，M-09 标 `已完成`（reopen 保持 2，收敛模式随里程碑关闭恢复 normal）。

- 验收命令：vitest 33 条全过；Playwright 16 通过、0 跳过。
- 负向对照 1/3/4 各自让对应用例失败；对照 2 原写法（去掉看板列体标记）不会失败——验证缺口，产品无误。已把契约改为去掉时间线 `data-scroll-region`（manual-items 第 2 条失败，实测），不计 reopen。
- 全部注入已还原；详见 `evidence/M-09-r16-audit.md`。

## Backlog 提示
- `oneScreen` 不检查深层滚动区标记；如需覆盖看板列体，可加一条读取 `.board-column-body[data-scroll-region]` 的断言（改进建议，不阻塞）。
