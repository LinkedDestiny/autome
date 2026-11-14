# M-10 · r19 · 审计 #18 独立复验

验收：`cd apps/web && /usr/bin/time -p npm run e2e`（ps 可用环境）→ 预检通过，Playwright 20 passed、0 跳过，real 149.80 s（≤180，globalTimeout 170 s 未触发）。D-1 31.2s、D-2 13.9s、D-3 19.5s、D-4 6.1s。日志 `.autome/output/a18-e2e.log`。

负向对照（临时注入，均已 `git checkout` 还原，工作区干净）：
1. D-1 合并后加整页 `reload()` → 失败，`expect(loads).toBe(1)` Expected 1 / Received 2。有效。
2. D-3 harness live 服务改 1000ms tick → 失败，Expected 3000 / Received 1000。有效。
3. 不重放 events.since：依次注入 (a) drain 分发丢弃，(b) resume 事件置空，(c) 加上 live.tsx 恢复后 revalidateAll 关闭，(d) 再加 live-client 重连 onReload 关闭——D-4 四次都仍通过。**此条对照无法区分**；见 Backlog BL-10。产品行为（离线期间建的卡恢复后出现）正确，不是实现缺陷，属验证缺口；修复需改 spec，审计轮不得改，交实现轮。

结论：通过，M-10 已完成，不计 reopen。
