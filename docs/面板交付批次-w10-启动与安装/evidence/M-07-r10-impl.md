# M-07 · 实现轮 10/65

结论：两处冒烟验证逻辑已修复，纯测试通过；真实 Electron 冒烟被本轮沙箱阻断，M-07 保持开放，reopen 保持 1。不将未取得的 4/4 证据写成通过。

## 修复与回归

- 服务计数按 launcher.ts 实际设置的 `autome-web <home> serve` 标题精确匹配，排除其它 home、start 与 core；修复原 `autome-web.js serve` 永远匹配不到的问题。
- 旧账本用例显式要求「内核比账本旧」横幅；主进程在 smoke 模式用 MutationObserver 等待 DOM 条件，10 秒上限，成功和超时均断开观察。正常页面不等待不存在的横幅。此修改不改变正常启动路径，也不把固定睡眠当成就绪。
- 审计抓缺陷的检查本来就在项目 `smoke/shell-smoke.test.js`，保留复用用例、横幅原始断言及清理检查；不再复制第二套冒烟。审计 a9-probe.js 是打印进程的诊断脚本（含绝对 worktree 路径），其判据已纳入进程匹配回归。
- 修改前原样执行 `npm --prefix apps/desktop run smoke`，见 `.autome/output/r10-smoke-red.log`：4 个业务用例失败，after 清理钩子另产生 1 个文件级失败；Electron SIGABRT、监听 `listen EPERM`、`spawnSync ps EPERM`。本轮未重现上轮两条具体断言，不能把环境失败称为缺陷红证据。
- 将上轮原匹配表达式和原一次性 DOM 读取逐字代入新 helper，运行 `node --test apps/desktop/test/shell-smoke.test.js`：4 条中 1 通过、3 失败，见 `.autome/output/r10-regression-red.log`；恢复修复后该文件 4/4 通过，包含在最终桌面测试日志中。这是纯回归红绿，不代替真实 Electron 红绿。

## 验收命令与总数

- `node --test apps/desktop/test/shell-main.test.js apps/desktop/test/preload.test.js apps/desktop/test/shell-smoke.test.js`：15/15，0 跳过；`.autome/output/r10-final-desktop.log`。
- `(cd apps/web && npx vitest run test/native-shell.test.ts)`：3/3，0 跳过；`.autome/output/r10-web.log`。
- `npm --prefix apps/desktop run stage-web`：退出 0；`.autome/output/r10-stage.log`。
- 修复后原样 `npm --prefix apps/desktop run smoke`：仍受同样环境阻断；`.autome/output/r10-smoke-after.log`。4 条业务用例与清理钩子均未获得通过证据。
- 上轮本范围基线 18 条（11 桌面 + 3 Web + 4 smoke）；本轮 22 条（15 + 3 + 4），净增 4，均来自 `test/shell-smoke.test.js`，不是跳过或减少原验收。纯测试 18 条通过；smoke 4 条未通过，测试运行器额外报告的文件级清理失败不算新增业务用例。
- `git diff --check` 通过。未运行全量门禁或宿主快检。

## 标待审之前的四项自审（本轮未标待审）

1. **情形表逐行**：C1 Electron Node / 一次性 start 启动器由 shell-main 1、2 验证；C3 两函数 preload、目录选择、notify、IPC 同源、导航隔离由 preload 1–2、shell-main 6–8、native-shell 1–3 验证；C4 自有、复用、门禁、继任实例分别由 shell-main 3、5、4、9 验证，均通过。C2 打包及 C5 删除属于 M-08，不适用于本轮。C6 真实最小 PATH 启动、复用唯一进程、旧内核横幅、无残留四行仍阻断。
2. **失败分支**：shell-main 2 的读盘失败、不可达、空回包、无效地址、启动超时均通过；4 的停止失败通过；6 的异源 IPC 拒绝和目录取消通过；7 的非法链接和超长参数拒绝通过。新增回归的横幅永不出现→超时失败通过。真实 Electron 的失败/清理由原 smoke 报出，未屏蔽 ps 错误，不能声称进程残留已检查通过。
3. **全部迁移**：未启动→新建/复用、运行→自有正常停止/门禁保留/复用不停止/继任保留由原 9 条测试覆盖通过；通知→点击聚焦与导航、目录→选择/取消通过。新增横幅无 DOM→中间离线→目标出现、目标已存在、无需横幅、始终缺失→超时四种迁移均通过。真实服务生命周期待宿主 smoke。
4. **域边界**：原通知标题 0/200/201、正文 0/1000/1001、href null/空/500/501、非字符串、协议相对与反斜杠跨源通过；新增进程 home 含中文空格、相似前缀、不同动作、内核命令通过。日期/闰日/跨年不适用：本里程碑不接收日期输入。

## 负向对照

逐项临时注入并 finally 恢复，运行 shell-main + preload，均退出 1：不调用 spawn（替换为抛错的主进程路径）、去掉 stop 的 --instance、绕过 start 改成 serve、多暴露 preload extra。日志为 `.autome/output/r10-negative-{no-spawn,no-instance,bypass-launcher,extra-preload}.log`。恢复后 15 条桌面测试再次全过。绕过启动器对应的真实 smoke 负向和移除 AUTOME_BUNDLED_CORE 的真实横幅负向未跑成，原因同上；不能仅凭纯测试宣称这些真实负向通过。

## 下一步

在允许 Electron、监听和 ps 的宿主上执行设计中的验收命令（包含新增纯回归），取得 smoke 4/4，并补齐剩余真实负向对照后再标待审。未启动下一会话。
