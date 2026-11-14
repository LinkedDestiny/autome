# M-07 · 实现 #11

结论：修复上一轮 shell 缺陷；真实冒烟受环境阻断，保持开放（reopen 仍为 2，domain-review）。未标待审，未运行全量门禁或宿主快检。

## 修复与红绿回归

- 审计检查 `.autome/output/a10-probe.sh` 逐字复制到项目 `apps/desktop/smoke/a10-probe.sh`。原样执行前后均输出 0（日志 `r11-probe-red.log` / `r11-probe-green.log`），证明 status 不创建 kernel_mark 行；该脚本本身没有失败断言，不能把退出 0 当成产品通过。
- 在项目 node:test 体系新增 `shell-ledger.test.js`，使用真实 CLI status 与 SQLite 检查，并调用 smoke 共用夹具。先使用原 UPDATE：失败（actual undefined，expected 更新的 build_id），见 `.autome/output/r11-ledger-red.log`；改 INSERT ... ON CONFLICT 后通过，见 `r11-ledger-green.log`。同时验证空表插入、已有行覆盖、单行不重复。
- 两种 smoke 目标共用 seedNewerLedger，修复未打包和打包分支；横幅匹配真实文本「比推进过账本的内核」。原审计的 Electron 横幅复现受本环境限制，不能声称已经取得端到端绿证据。
- `/api/instance` 从 Web package.json 返回 web_version，声明版本 2.0.0 并同步锁文件；smoke 第 1 条新增真实 HTTP 版本断言，待宿主执行到该断言。
- smoke 清除继承的 AUTOME_BUNDLED_CORE，防止测试环境替生产代码补变量。负向判据修正为同包来源断言：较新的账本对 fallback 内核也会显示旧内核横幅，不能以横幅缺失判定是否选择了同包内核。

## 命令与结果、用例基线

日志均在 `.autome/output/`。

- `node --test apps/desktop/test/shell-main.test.js apps/desktop/test/preload.test.js apps/desktop/test/shell-smoke.test.js apps/desktop/test/shell-ledger.test.js`：16/16，通过；最终日志 `r11-final-pure.log`。
- `(cd apps/web && npx vitest run test/native-shell.test.ts)`：3/3，通过；`r11-native.log`。
- `npm --prefix apps/desktop run stage-web`：成功；`r11-final-stage.log`。
- `npm --prefix apps/desktop run smoke`：修复前后均未能完成，Electron SIGABRT、listen EPERM 127.0.0.1、spawnSync ps EPERM；`r11-smoke-red.log`、`r11-final-smoke.log`。4 个声明用例均失败，after 清理钩子另产生 1 个文件级失败；无法证明无残留，未放宽清理或添加跳过。
- 当前声明用例总数 **23 = 16 + 3 + 4**，上一轮基线 **22 = 15 + 3 + 4**，差 **+1** 来自新增 shell-ledger 回归。19 通过、4 环境失败、0 跳过；按 runner 失败记录计数另有清理钩子失败，故其输出 tests=5，不把它误算成新增业务用例。
- 三条剩余负向注入：把 start 改成主进程 import 服务、把 start 改为 serve 绕过启动器判活、删除 AUTOME_BUNDLED_CORE；各自运行 shell-main 均 exit 1，日志 `r11-negative-main-import.log`、`r11-negative-bypass-launcher.log`、`r11-negative-no-bundle.log`。逐次恢复原文件，恢复后 9/9（`r11-restored.log`），最终 16/16 再确认。真实 smoke 负向行为仍未实跑通过。

## 四项自审清单

1. **情形表逐行**：C1 子进程可执行文件 / 参数 / Node 环境由 shell-main 第 2 条通过；C2 同包变量由该条通过、真实路径由 smoke 待验；C3 两函数与参数透传 preload 2 条通过，异源拒绝 / 目录选择及取消 / 通知 / 导航由 shell-main 第 6–8 条通过，Web 原生通知 / 按钮 / project.pick 501 三条通过；C4 复用、自己启动、版本响应中前两项纯测试通过，版本 HTTP 与真实复用仍受阻；C5 删除与打包属 M-08，本轮不适用；C6 真实冒烟 4 条环境受阻，不视作通过。
2. **失败分支**：shell-main 第 2 条实跑读盘失败、外部不可达、空回包、非法 URL、35 秒超时；第 4 条实跑停止无响应；第 6 条实跑异源拒绝（含空 origin）与选择取消；shell-smoke 实跑横幅等待超时。全部纯测试通过。真实端口、Electron、ps 的失败原样保留。
3. **全部迁移**：未运行→start→自有 / 复用；自有→退出停止 / 门禁保留 / 停止报错 / 继任保留；复用→退出不停止；通知→点击聚焦及导航；同源导航放行 / 异源阻止 / 新窗口拒绝与 http(s) 外链交系统，均对应 shell-main 9 条通过。横幅立即可见 / 延迟出现 / 超时由 shell-smoke 通过；真实旧内核→DOM 横幅仍待验。
4. **域边界**：通知 title 200/201、body 1000/1001、href 500/501、空串、null、非字符串、协议相对与反斜线异源 URL，均由 shell-main 第 7 条通过。账本空表 / 单行 / 已有值覆盖由新增回归通过。日期、闰日等不适用，本里程碑无日期输入域。

## 后续与提交

需在允许 Electron 启动、监听本地端口与 ps 的环境跑设计验收契约，取得 smoke 4/4（包括版本接口与进程清理）后再标待审。保持本轮 11/65；未启动下一会话。
