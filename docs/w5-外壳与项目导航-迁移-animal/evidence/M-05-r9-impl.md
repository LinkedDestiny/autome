# M-05 r9 实现轮：Desktop 门禁超时调查

## 结论与范围

本轮是门禁修复轮，基线 HEAD a1689a4，宿主失败提交 8d0c0ffe8707；implementation-round 为 **9/25**。M-01..M-05 原本均为已完成，本轮保持里程碑表原样，不推进或关闭里程碑。**未完成宿主超时修复，未取得门禁通过证据。** 本轮仅增强原测试诊断。

用户本轮明确要求修复 Desktop 门禁，按该最新指令对原任务只读目录 apps/desktop 的两个测试文件作最小诊断改动；不改 Desktop 产品代码、脚本、内核或依赖，不放宽超时，不新增 skip。

## 失败证据与复现

- 宿主原日志：/Users/dannie/project/autome-v2/.autome/output/gates/T-35/gate-8d0c0ffe8707.log；只读核对，摘录在 .autome/output/r9-host-errors.txt。
- 原检查：electron dom harness: real-DOM assertions against the actual renderer；错误为 dom-harness.js did not finish within 60s，耗时 60153 ms。stderr 的 No handler registered 是该测试故意不启动 sidecar 的既有离线场景，不能单凭它断定根因。
- 修改前运行 npm test --prefix apps/desktop：退出 1，DOM 外层用例约 141 ms 退出，status=null。日志 .autome/output/r9-desktop-before.txt。
- 单独以同一 Electron 二进制运行同一 harness，记录 status=null、signal=SIGABRT，stderr 为空，见 .autome/output/r9-electron-exit.json。本机 Electron 崩溃报告的主线程栈含 abort、___RegisterApplication_block_invoke、_RegisterApplication；摘录见 .autome/output/r9-crash-extract.txt。这是启动阶段失败，不是宿主已运行页面后的 60 秒超时，不能称为原缺陷红灯复现。
- 修改后再次执行 npm test --prefix apps/desktop：退出 1，约 163 ms，错误现明确包含 signal=SIGABRT、error=none；没有 loading page 日志。见 .autome/output/r9-desktop-after.txt。**没有红转绿。**
- 审计 #6 为通过，没有审计退回的临时检查需要搬迁；宿主捕获失败的原检查已经在项目测试体系内，保留其名称、全部断言、执行顺序和 60000 ms 超时。

## 改动与未闭合假设

dom.test.js 的异常退出信息增加 signal 与 spawn error，区分启动崩溃、启动失败与超时。dom-harness.js 在 stderr 记录页面加载、夹具完成、每次 evaluate 开始及结束耗时、已完成的 check 名称和 renderer gone；stdout 仍为原 JSON，结果语义不变。它们是为下一次宿主运行保留的定向诊断，不是产品修复。

候选原因依次为：隐藏窗口动画帧不产生（最后阶段停在含 requestAnimationFrame 的 evaluate）；页面执行未返回（某次 evaluate 只有 started）；Electron 启动异常（没有页面阶段日志，且带 signal）。本地仅证实最后一种启动失败，不能据此判断宿主根因。不盲改隐藏窗口行为、替换动画帧或增加等待时间。

## 标待审之前的自审清单

1. **情形表逐行**：本轮不改 W5 情形映射，也不新增待审。原 DOM 正常 JSON、子检查失败、超时、异常退出四条路径的判据均保留；异常退出已实跑，准确报 SIGABRT；其余 DOM 路径因启动失败未执行，不能报通过。
2. **失败分支**：原 Desktop 60 秒超时仍判失败；本地启动失败已前后实跑，均退出 1。授权、读盘失败、外部服务不可达不适用，本轮不改变这些产品路径。超时分支仅有宿主原始失败证据，修改后未取证。
3. **全部迁移**：本轮不改产品状态机；测试进程启动→页面加载→夹具注入→evaluate→check→退出的诊断覆盖已静态核对。实跑止于启动，后续迁移未取证，不等同全链路通过。
4. **输入域边界**：不适用；本轮不新增或改变用户输入域，原测试参数、夹具和断言值不变。

## 验证与用例基线

- node --check apps/desktop/test/dom-harness.js 与 node --check apps/desktop/test/dom.test.js 均退出 0；git diff --check 通过。
- Desktop 修改前后均为 **105 条：104 通过、1 失败、0 skipped**。相对实现 #8 的 105 差 **0**，相对审计 #6 完整 165 差 **-60**，原因是 Electron 启动失败，内部 60 条子检查未展开，不是删除用例。新增用例数 0。
- 已启动原命令 sh scripts/ci-local.sh，日志 .autome/output/r9-ci-before.txt；最终退出 **1**，用时 **517 秒**。命令启动后、Desktop 阶段执行前加入诊断；不把该运行表述为修改前全量。

## 后续责任

由 Autome 在本轮提交 SHA 上重跑原门禁，以该结果为准。若仍超时，根据最后一条 evaluate started、已完成 check 和 renderer gone 定位，再修复并取得同命令绿灯。当前会话未能启动 Electron，未尝试绕过权限；不是协议失败。

## 全量结果与最终检查

- Format、Clippy、Rust Test、Contract、Web lint、Web typecheck、Web build 全部通过；Desktop 与 Web test 失败。完整日志见 .autome/output/r9-ci-before.txt，计数摘要见 r9-counts.txt、r9-web-results.txt。
- Rust **1194** 条：1193 通过、0 失败、1 原有 ignored；相对实现 #8 的 1194 差 **0**。
- Web **364** 条 / 43 文件：333 通过、12 失败、19 skipped；相对实现 #8 及本次宿主基线均差 **0**。gateway-http 11 条及 shell-live SSR 1 条失败；notifier-live 3 条、server 9 条、eventbus-live 7 条前置未执行，日志为监听 EPERM / watcher EMFILE。未新增、删除或主动跳过任何用例。
- Desktop **105** 条：104 通过、1 失败、0 skipped；仍为启动 SIGABRT，内部 60 条未展开。当前不能取得用户要求的同命令绿灯；不把诊断补充当作已修复。
- 状态字段唯一、implementation-round 9/25、里程碑表与 HEAD 逐字一致、repos 原样、retro 五段且不超过 200 字的脚本检查通过；git diff --check 通过。
