# M-07 · 审计 #10 独立复验

结论：**退回（实现缺陷，领域 shell，reopen 1→2）**。

## 验收命令
- shell-main 9 + preload 2 + shell-smoke 4 = 15 条，全过；native-shell 3/3；stage-web 成功。
- `npm --prefix apps/desktop run smoke`：本宿主 Electron 与监听可用，跑了两次，稳定 3/4：第 3 条「更新账本拒绝旧同包内核并显示横幅」失败（`等待横幅超时`）。日志 `.autome/output/smoke10a.log`。
- 第 1、2、4 条通过（r9 的进程标题问题已修）。

## 负向对照（真实注入，已还原，git status 干净）
- preload 多暴露 `extra`：preload 测试 1 失败 ✔。
- stop 不带 `--instance`：shell-main 2 条失败（退出绑定 instance、继任保留）✔。
- 其余三条（主进程内 import、不经启动器判活、不设 AUTOME_BUNDLED_CORE）本轮未注入；smoke 第 3 条本身已红，无法区分。

## 缺陷根因（smoke 第 3 条）
1. 夹具先跑 `autome status` 想"让账本记下真实构建标识"，但 `status` 不 claim 账本：实测 `select count(*) from kernel_mark` 为 0。随后旧构建标识的内核第一个 claim，账本被记成旧标识，没有"账本比内核新"，横幅永不出现。打包分支的 `UPDATE kernel_mark` 同样因无行而什么也不更新。
2. 即使补好账本（手工插入 9999999-… 行再启动薄壳），面板横幅真实文本为「这个内核（…）比推进过账本的内核（…）旧，已停止推进；请重新安装或设置 AUTOMED_BIN」，不含断言/设计写的子串「内核比账本旧」。产品横幅出现，行为本身可用；设计验收与实际文本不一致，需实现轮统一（改断言子串，如「比推进过账本的内核」，并同步设计条款）。
3. 附带：`/api/instance` 没有 `web_version`（设计 489 行要求带），`main.js:31` 因此每次打印「面板版本：undefined」。不阻断验收，但与设计不符，一并补上。

判别检查：`.autome/output/a10-probe.sh`（复现 1、2）。实现轮应用 `INSERT ... ON CONFLICT` 建行（或先让真实内核 serve 一次）并校正期望子串，把检查搬进 smoke。
