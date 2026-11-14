# 实现审计 · 审计 #10

## Convergence Note

1. 未闭合领域：shell。当前失败：`npm --prefix apps/desktop run smoke` 第 3 条（横幅超时，`.autome/output/smoke10a.log`）。
2. 根因：smoke 夹具假设 `status` 会写账本（实测不会，kernel_mark 0 行）；断言子串与真实横幅不一致；`/api/instance` 缺 `web_version`。同类范围：smoke 的打包分支 `UPDATE kernel_mark` 同样落空；r9 只验了进程标题，r10 实现轮因沙箱从未真跑过第 3 条，故漏检。
3. 下一轮：夹具建账本行（INSERT ... ON CONFLICT 写入更新的构建标识）、期望子串改为真实文本并同步设计、补 web_version、做完三条剩余负向对照。完成条件：smoke 4/4。
4. 应红：当前 smoke（3/4）；完成应绿：验收契约整条命令；不得退化：shell-main 9、preload 2、shell-smoke 4、native-shell 3。

里程碑 M-07：**退回（实现缺陷，领域 shell，reopen 1→2）**。

失败命令：`npm --prefix apps/desktop run smoke`，4 条中第 3 条「更新账本拒绝旧同包内核并显示横幅」失败（`等待横幅超时`；`.autome/output/smoke10a.log`）。其余 15 条纯测试 + native-shell 3 条全过，第 1、2、4 条 smoke 通过。
根因：①夹具的 `autome status` 不会写 kernel_mark，旧内核先 claim，账本不比内核新；②真实横幅是「这个内核…比推进过账本的内核…旧」，不含期望子串「内核比账本旧」；③`/api/instance` 缺 `web_version`（设计要求）。
判别检查：`.autome/output/a10-probe.sh`；证据 docs/面板交付批次-w10-启动与安装/evidence/M-07-r10-audit.md。
下一轮：修夹具与期望文本、补 web_version，smoke 须 4/4，并补完剩余三条负向对照。
