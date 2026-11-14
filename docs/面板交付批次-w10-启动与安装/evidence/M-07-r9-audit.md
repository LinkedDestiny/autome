# M-07 r9 审计复验

结论：**实现缺陷**（领域 shell）。验收命令末段 `npm --prefix apps/desktop run smoke` 4 条里 2 条失败。

## 负向对照
- preload 多暴露 `extra` 方法：preload 第 1 条失败（pass 1 / fail 1），还原后 2/2 通过。判别有效。
- 其余对照（spawn 改 import、去 `--instance`、不经启动器）需改产品代码，本轮只对 preload 做了，其余未跑。

## 验收命令（自己跑）
- shell-main + preload：11/11 通过。
- native-shell（vitest）：3/3 通过。
- stage-web：成功。
- smoke：4 条，通过 2、失败 2（本机 Electron 可起、可监听、`ps` 可用，不是环境阻塞）。输出：`.autome/output/smoke9.log`。

## 失败
1. 「已有 autome web 服务只复用」`smoke/shell-smoke.test.js:85`：`scan(f).filter(row => row.command.includes('autome-web.js serve')).length` 得 0，期望 1。
   根因：`apps/web/server/launcher.ts:134` 把进程标题设成 `autome-web <home> serve`，`ps` 里看不到 `autome-web.js serve`。实测 `ps` 行为 `57765 1 autome-web /var/.../a9-Wk4s8h serve`。匹配串与真实进程标题不符，该条永远红（设计 smoke 第 2 条要求「服务进程只有一个」）。
2. 「更新账本拒绝旧同包内核并显示横幅」`:98`：`value.banner` 为空串，期望含「内核比账本旧」。
   `main.js:37` 在 `loadURL` 刚返回就一次性读 `[data-testid=live-banner]`，没有等待；横幅由客户端取数后渲染时会读空。我用 curl 取页面 HTML 也无横幅，说明是客户端渲染；未能用 Electron 探针确认延迟后是否出现（探针挂起，已终止并清理）。需实现轮确认：是读取过早，还是旧内核横幅在此路径下根本不出现。

## 判别检查
`.autome/output/smoke9.log`（复现，红）；`.autome/output/a9-probe.js`（打印 `ps` 中服务进程标题）。实现轮修复后原样重跑 smoke 应转绿。
