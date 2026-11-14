# M-03 · 实现 #5

## 修复与回归

只推进 M-03，reopen 保持 1、领域 package，implementation-round 为 5/65。
安装提示改为显式变量边界 `${panel_dest}；`，避免 macOS /bin/sh 3.2 把全角分号首字节并入变量名。两份安装/打包脚本同类搜索：其余命中是注释与转义美元符号的 README 文案，无其它可执行缺陷。

审计脚本 .autome/output/a4-install-msg-check.sh 逐字复制到 apps/web/test/fixtures/install-msg-check.sh；cmp 返回 0。install-alignment 新增 1 条调用原脚本，仍检查 stderr 为空、stdout 含成功消息和 PATH 提示。用私有无空格 TMPDIR 适配原脚本未加引号的临时路径；外层 afterEach 清理。Git、Claude、Codex 探测用临时假命令，避免 doctor 的环境告警污染安装诊断，不调用真实 CLI 登录。移除空 AUTOME_TEST_BUILD_ID，所有账本与安装路径仍在临时目录。

- 红：修复前运行 `cd apps/web && npx vitest run test/install-alignment.test.ts -t '安装成功消息'`，1 失败、5 条因名字筛选未执行；原始输出 .autome/output/r5-red-final.txt 明确含 line 188 / unbound variable。不是条件跳过。
- 绿：恢复修复后执行完整 M-03 四文件验收，新增回归通过，原始输出 .autome/output/r5-acceptance-final.txt。未修改审计脚本来放宽判据。
- 调试阶段发现原脚本对带空格 TMPDIR 不兼容、doctor 对缺 CLI 报 stderr；调整测试夹具后重新取得上述最终红绿证据。中间日志不作为最终验收结论。

## 验收命令与结果

`cd apps/web && npx vitest run test/install-alignment.test.ts test/core-binary.test.ts test/ci-config.test.ts test/docs-web.test.ts`

总数 26 = install-alignment 6 + core-binary 10 + ci-config 8 + docs-web 2；上轮基线 25，差 +1 来自安装消息回归。24 通过、2 失败、0 跳过。两条失败均为沙箱拒绝 ps：

- 空 PREFIX 安装、打开生产面板、同包内核、重复安装与停止：在预检 leftoverProcesses 即报 spawnSync ps EPERM，尚未启动服务；本轮没有 HTTP 200 或重复安装链路通过证据。
- 安装验收清理全部临时服务与内核，无残留：进程扫描报 spawnSync ps EPERM，不能声称残留验证通过。

`sh -n scripts/install-cli.sh scripts/package-cli.sh apps/web/test/fixtures/install-msg-check.sh` 返回 0，无输出（因 vitest 非 0，独立执行契约的 shell 检查）。`git diff --check` 返回 0。未运行全量门禁或宿主快检。

## 标待审之前的自审清单

1. **情形表逐行**：A2 发布包文件、生产依赖、校验和、README：安装第 1 条通过；自定义 PREFIX 布局和启动生产服务：第 2 条被 ps 预检阻塞；A2 离线 --binary、缺 --web 与空参数拒绝：第 4 条通过；A3 bundle/环境/PATH/cargo 优先级及旧 automed 排除：10 条通过；A4 旧 Node/无 Node/受支持 Node：安装第 3 条通过；A10 CI 构建顺序：8 条通过；A11 README 与计划入口：2 条通过；新增安装成功和 PATH 提示：回归通过。真实内核调用点重新枚举，直接或 options 间接传 binaryPath；older-kernel-core 使用 mock，安装启动链路通过同包变量显式指定内核。
2. **失败分支**：缺 Node、Node 21.7.3 / 22.21.9 拒绝面板、缺 --web、空 --web 均通过。拒绝授权、外部服务不可达、超时不适用本次本地包与消息修复。读盘失败没有本里程碑设计点名的新分支，本轮不改这些行为。ps 环境失败原样保留，交宿主复验。
3. **全部迁移**：空安装→CLI-only（缺/旧 Node）通过；空安装→CLI+面板（22.22.0 / 23.0.0）通过；已有面板→替换→启动→停止属于第 2 条，受 ps 阻塞未取证。回归证明成功安装后继续输出 PATH 提示并执行 doctor。没有新增状态机。
4. **输入域边界**：空 Node、21.7.3、22.21.9、22.22.0、23.0.0；空 --web；非默认且含空格 PREFIX：对应安装测试通过。变量紧邻中文全角分号：新增回归红绿通过。日期/最大数值/单元素不适用安装消息。原样审计脚本使用不含空格的临时根目录，带空格安装路径由原有 Node 版本矩阵覆盖。

按任务 §6.5 保留断言、不条件跳过，提交待审并明确两项环境阻塞，不能把本轮称为验收全绿；宿主/审计需复验完整契约。未修改其它里程碑，不启动下一会话。
