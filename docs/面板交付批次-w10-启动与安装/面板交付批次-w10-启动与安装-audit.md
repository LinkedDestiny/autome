# 实现审计 · 审计 #10

里程碑 M-07：**退回（实现缺陷，领域 shell，reopen 1→2）**。

失败命令：`npm --prefix apps/desktop run smoke`，4 条中第 3 条「更新账本拒绝旧同包内核并显示横幅」失败（`等待横幅超时`；`.autome/output/smoke10a.log`）。其余 15 条纯测试 + native-shell 3 条全过，第 1、2、4 条 smoke 通过。
根因：①夹具的 `autome status` 不会写 kernel_mark，旧内核先 claim，账本不比内核新；②真实横幅是「这个内核…比推进过账本的内核…旧」，不含期望子串「内核比账本旧」；③`/api/instance` 缺 `web_version`（设计要求）。
判别检查：`.autome/output/a10-probe.sh`；证据 docs/面板交付批次-w10-启动与安装/evidence/M-07-r10-audit.md。
下一轮：修夹具与期望文本、补 web_version，smoke 须 4/4，并补完剩余三条负向对照。
