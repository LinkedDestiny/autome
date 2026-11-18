# 实现审计

## M-07 · 退回（实现缺陷）· 审计 #7 · 领域：任务详情时间线几何

- 验收命令 86 项通过；三条负向对照（删 min-block-size、允许折叠换行、仅会话输出 details.timeline-text）均使验收失败，已还原。
- **缺陷**：本沙箱浏览器可跑。`cd apps/web && npx playwright test e2e/layout.spec.ts -g "需求详情展开日志"` 失败于 `layout.spec.ts:88`：`expect(row.height).toBeLessThanOrEqual(44)`，Expected <= 44，Received 51（连续两次一致）。违反 M-07（D-1，时间线折叠行 ≤44px、前 10 条完整可见）及操作者备注「验收断言不要放宽」。
- 截图 `.autome/output/m07-collapsed-detail.png`：时间线区只完整露出约 8 条；前两条「系统」行高约 51px，会话行约 40px，说明无会话的系统行比设计的 40px 紧凑行高出约 11px（根因未定位，建议实现轮用 `timeline-geometry` 附件量各行盒模型）。10 条几何断言（:89-93）在 :88 之前已不可达。
- 判别检查：即上面的 playwright 命令（不改源码、可原样搬进回归）；输出在 `.autome/output/playwright/`。
- 实现轮 r10–r13 称「浏览器受环境阻塞」，本轮实测并非如此（ps/监听均可用），下一轮先跑该命令复现再修。
