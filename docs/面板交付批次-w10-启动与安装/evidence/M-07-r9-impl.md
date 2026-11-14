# M-07 · 实现轮 #9

结论：实现与纯测试完成，**保持开放，不标待审**。真实 Electron 冒烟受会话沙箱阻断，没有取得完整验收通过证据；不是协议失败，不增加 reopen。下一轮先在允许 Electron、监听及 ps 的环境复验同一验收命令，再决定报审。

## 实现与修复

- main.js 改为薄壳，src/shell/server.js 以 Electron 自带 Node 运行一次性 `start --json`，复用 A 项启动器；按自有/复用服务决定退出行为，自有服务通过 `stop --instance <原 instance> --wait 0` 停止，门禁未结束时通知保留后台服务。
- src/shell/window.js、ipc.js 限制导航及 IPC 来源；通知参数类型/长度和解析后的 origin 校验；preload 仅有 pickDirectory、notify。Web 通知优先原生能力，普通浏览器保持既有行为。
- 新增可重复执行的 stage-web、独立 smoke 入口。旧 renderer、旧测试和旧模块保留，M-08 再删除。暂存目录已 gitignore。
- 冒烟用临时 AUTOME_HOME/数据库及最小 PATH；读取输出后检查登记与 healthz，再通过 stdin 确认允许窗口退出，避免检查与停服竞态。未打包目标先用真实内核初始化账本，再给子进程设较旧 AUTOME_TEST_BUILD_ID；release 不支持此变量，打包目标仅在私有夹具数据库写更新标记。
- 上一轮审计 M-06 通过，无缺陷检查需要逐字搬入。开发时修正了重复暂存失败、错误使用“未来” debug build override（内核只允许往旧覆盖）的测试问题。
- 已复核 Electron 官方 [webContents 文档](https://www.electronjs.org/docs/latest/api/web-contents#event-will-navigate)：loadURL 不触发 will-navigate，所以通知目标在调用 loadURL 前独立检查同源。

## 命令与证据

原始输出全部在 `.autome/output/`，未运行全量门禁或宿主快检组合。

1. `cargo build -p automed -p autome-cli`：通过，当前源码构建内核；`m07-build-core.txt`。
2. `npm --prefix apps/web run build`：通过；`m07-build-web.txt`。
3. 逐字执行 M-07 验收：`(cd apps/desktop && node --test test/shell-main.test.js test/preload.test.js) && (cd apps/web && npx vitest run test/native-shell.test.ts) && npm --prefix apps/desktop run stage-web && npm --prefix apps/desktop run smoke`：前 11 + 3 条与暂存通过，smoke 未通过；`m07-acceptance.txt`。
   - smoke 1、3：Electron 在输出面板结果前 SIGABRT。
   - smoke 2：真实 Electron Node 启动服务时 `listen EPERM: operation not permitted 127.0.0.1`。
   - smoke 4 与 after 清理钩子：`spawnSync ps EPERM`。已尝试 stop --force，不能宣称通过进程无残留验收；临时目录保留供宿主检查。
4. `(cd apps/web && npx vitest run test/native-shell.test.ts test/bell-system.test.ts test/add-project.test.ts)`：22 条通过，0 跳过；`m07-web.txt`。
5. 三个临时负向变异各运行桌面 11 条：去掉 AUTOME_BUNDLED_CORE、stop 去掉 --instance、preload 增暴露方法，均 exit 1。输出 `m07-negative-{core-env,stop-instance,preload-extra}.txt`；全部还原后桌面 11 条再次全过，`m07-restored.txt`。
6. `git diff --check`：通过。

## 用例总数与基线差

- 里程碑契约是 **18 个显式用例**：shell-main 9、preload 2、native-shell 3、smoke 4。本轮 14 通过、4 环境阻塞失败，0 跳过。Node 另外将失败的 after 钩子记为一个文件级失败，因此 smoke 原始汇总为 tests 5 / fail 5，不把它隐藏成只有 4 次失败。
- 加直接相关回归 bell-system 8、add-project 11，去重后的显式用例共 **37**：33 通过、4 阻塞失败。将文件级钩子失败也计入运行器结果，共 **38**：33 通过、5 失败。
- 上一轮 M-06 基线为 26 条（10 testids + 4 w9-layout + 12 styles）。本轮显式总数净 **+11**：移出上一里程碑范围 26，增加 M-07 新用例 18，纳入直接受影响既有回归 19；不是原来的 26 条回归被删除。同范围代码基线为既有回归 19，本轮新增 18。
- 重复验收和负向变异执行不重复累加用例总数。

## 标待审之前的自审清单（已逐项执行；因冒烟受阻未报审）

1. **情形表逐行**
   - C1 自带 Node/一次性启动器、同包内核参数：shell-main 1、2 通过；最小 PATH 实机 smoke 1 受阻。
   - C3 两函数暴露与透传：preload 2 条通过；native-shell 的通知优先、按钮有无、project.pick 501 三行通过。
   - C3 目录选择成功/取消，IPC 同源/异源：shell-main 6 通过。
   - C3 通知合法/非法链接、点击聚焦导航：shell-main 7 通过；真实系统对话框、通知点击属 H-04/H-05，未以替身冒充人工验收。
   - C3 导航同源放行/异源拒绝，新窗口 deny 与 http(s) 系统打开：shell-main 8 通过。
   - C4 自有无门禁、自有有门禁、复用、继任实例四行：shell-main 3、4、5、9 通过，3/4/9 同时执行 main.js 的退出接线；真实复用 smoke 2 受阻。
   - C4 不同 web_version 仍连接：主进程仅记录版本日志，不拒绝连接。C6 旧内核横幅与残留检查 smoke 3/4 受阻。
2. **设计点名的失败分支**
   - IPC 拒绝异源授权、通知无效参数：6/7 通过。
   - 启动器读盘失败/外部服务不可达/空输出/无效实例：2 中的失败返回、spawn error、空 JSON、异源 URL 均拒绝，通过。
   - 启动器超时：2 使用 Node mock timers 推进 35 秒，拒绝超时，通过；停止失败未误报为门禁：4 通过。
   - 真实监听不可用、Electron 不可运行、ps 拒绝：明确失败不跳过，记录在上方。
3. **全部状态迁移**
   - 启动→新服务、启动→复用、启动→错误/超时：纯测试覆盖通过；实机启动/复用未取证。
   - 退出→自有停止、退出→门禁保留并通知、退出→复用保留、退出→继任实例保留不通知：3/4/5/9 通过。
   - IPC 目录成功→路径、取消→null、异源→拒绝；通知合法→show→点击聚焦/导航、非法→拒绝：6/7 通过。
   - 导航同源→允许、异源→阻止、开窗→deny：8 通过。服务崩溃与后台进程清理的实机证明仍受阻。
4. **输入域边界**
   - title/body 的空串、200/1000 长度及 201/1001；href 的 null、空串、500 与 501、数字、同源路径、绝对异源、`//evil.test`、反斜杠形式、javascript 协议：7 通过。
   - 来源空 URL、file URL、同主机不同端口：6 通过；没有原生函数时无目录按钮：native-shell 通过。
   - 日期/前导零/闰日不适用：此里程碑没有日期或数字文本输入域。

## 提交与后续

按当前分支提交本轮全部实现、测试与记录；不推送、不启动下个会话。提交使用 `-c core.hooksPath=/dev/null`，避免 post-commit 自行触发用户明确禁止本轮运行的全量门禁。M-07 保持开放，M-08 未推进。
