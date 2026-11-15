# 审计 · M-06（审计 #7）

结论：**通过**，M-06 标为已完成。

验收命令全部重跑通过（node 26、vitest 12、Playwright 14，含 live D-4 与 layout 项目页）。四项负向对照均命中：Doc-tests 分支删除、⑪ 夹具缺 prep、drain 对恢复期事件不分发（卡片仍出现、`data-live-seq` 停在 218 < 219 而失败）、列体 `overflow-y: visible`；均已还原。

D-5：把 11 个文件的 `maxRetries` 临时设 0，连跑 `vitest run --project kernel`，第 9 次出现 ENOTEMPTY（`…/w9-*/home/.claude`），前 8 次通过。结论：不带重试会复发，写入者在假 HOME 的 `.claude` 下，未进一步定位到进程；按设计「证实不了则保留重试」，重试保留，产品未改。详见 `evidence/M-06-r10-audit.md`。

Backlog：无新增。
