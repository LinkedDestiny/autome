# 实现审计 · 审计 #9

里程碑 M-07：**退回（实现缺陷，领域 shell，reopen 0→1）**。

失败命令：`npm --prefix apps/desktop run smoke`（验收契约末段），4 条中 2 条失败：
- 「已有 autome web 服务只复用」：`0 !== 1`，进程匹配串 `autome-web.js serve` 与 `launcher.ts:134` 设置的进程标题 `autome-web <home> serve` 不符。
- 「更新账本拒绝旧同包内核并显示横幅」：`banner` 为空，期望含「内核比账本旧」；`main.js:37` 读取横幅无等待。

通过项：shell-main + preload 11/11，native-shell 3/3，stage-web，preload 负向对照有效。
证据：docs/面板交付批次-w10-启动与安装/evidence/M-07-r9-audit.md；复现 `.autome/output/smoke9.log`、`.autome/output/a9-probe.js`。
下一轮：修 smoke 进程匹配与横幅读取（或证明横幅路径），smoke 须 4/4，并做完剩余负向对照。
