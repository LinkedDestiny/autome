# M-07 · r11 · 审计 #11 独立复验

结论：通过。

## 验收命令（本轮自己跑）
- desktop 四文件纯测试：16 条全过（shell-main 9、preload 2、shell-smoke 4、shell-ledger 1）。
- web native-shell：3/3。
- stage-web 退出 0；`npm --prefix apps/desktop run smoke`：4/4 通过（真实 Electron 在本会话可监听，r10/r11 的环境阻断未复现；日志 .autome/output/a11-smoke.log）。
- 另核 `web_version` 已进 `/api/instance`（server/main.ts、instance.ts），smoke 第 3 条的真实横幅「比推进过账本的内核」命中。

## 负向对照（注入后已全部还原，git status 干净）
1. 去掉 ELECTRON_RUN_AS_NODE → shell-main 第 2 条失败。
2. 去掉 AUTOME_BUNDLED_CORE → shell-main 第 2 条失败。
3. stop 去掉 `--instance` → shell-main 第 3、9 条失败。
4. preload 多暴露一个方法 → preload 第 1 条失败。
5. 「不经启动器判活」一条未单独注入：由 shell-main 第 1 条（只 spawn 一次启动器）与 smoke 第 2 条（真实服务进程只有一个）覆盖，二者本轮均绿。
