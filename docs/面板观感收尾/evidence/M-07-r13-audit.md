# M-07 r13 审计复验

1. 验收命令（apps/web，vitest 六文件）：86 通过 / 0 失败，≥82 达标。
2. 负向对照：
   - 删 `.timeline-row` 的 `min-block-size` → task-timeline 1 失败；
   - 去掉 `.timeline-text > summary` 的 `nowrap` → 1 失败；
   - 无会话条目改为不输出 `details.timeline-text` → 5 失败（含第 5 条 R1-P01）。
   均已还原，`git status` 干净。
3. 自造判别检查：直接跑 e2e `layout.spec.ts -g 需求详情展开日志`，失败：行高 Received 51，Expected <= 44（两次）。截图 `.autome/output/m07-collapsed-detail.png`，首屏仅约 8 条完整。
4. 操作者备注 #1：要求不放宽断言并给出预算；断言现实测不通过，视为未达成。
结论：实现缺陷，退回开放，reopen 1。
