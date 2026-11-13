# M-02 r3 审计复验

- 负向对照 1（确认后一律跳项目页，去掉 skip 判断）：add-project 第 5 条与 add-project-real 第 1 条失败，已还原。
- 负向对照 2（内核拒绝换自拟文案）：add-project 第 6 条与 add-project-real 第 4 条失败，已还原。
- 验收命令：9 个文件、59 个用例全过（与预期一致，skipped 0）。w9-http 4 例（含上轮失败的坏 Host/Origin 用例）在本环境可监听，全过。
- 结论：通过。`git status` 在还原后无产品文件改动。
