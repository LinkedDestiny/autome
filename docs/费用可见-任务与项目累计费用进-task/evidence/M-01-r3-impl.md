# M-01 实现轮 #3：Desktop 门禁复现

## 范围与结论

按本轮修复指令，不推进新里程碑；M-01 保持待审，implementation-round 为 `3/25`。没有审计文件或上一轮审计判别检查，因此不存在需要逐字迁入测试体系的检查。

本轮未修复 Desktop 门禁，未取得同命令全绿证据。没有修改生产代码、测试断言、跳过条件、超时或门禁脚本；不以 Node 模式替代真实 Electron DOM 验收。环境阻塞不是协议失败。最终结果以 Autome 在提交后的提交上重跑为准。

## 复现与判别

1. 原样启动 `sh scripts/ci-local.sh > .autome/output/r3-ci.txt 2>&1`，启动时工作区干净。
2. `npm test --prefix apps/desktop > .autome/output/r3-desktop.txt 2>&1`：105 个用例，104 通过、1 失败、0 跳过。失败为既有 `electron dom harness: real-DOM assertions against the actual renderer`，消息为 `dom-harness.js exited null, signal: SIGABRT, spawn error: none`，stderr 为空。相对上一轮 105 个，差 **0**。DOM 内部子检查没有展开，不能称完整 DOM 已验收。
3. 三个候选原因：系统 GUI 启动环境、Electron 安装完整性、项目 harness 初始化。最小探针不加载项目，只执行 Electron `--version`，同样 `status=null, signal=SIGABRT`；因此本会话失败不依赖费用代码或 harness。相同二进制加 `ELECTRON_RUN_AS_NODE=1` 执行版本打印成功（43.2.0），但 stderr 有 `task_name_for_pid: (os/kern) failure (5)`。日志 `.autome/output/r3-electron-probes.txt`。这只是区分启动路径的诊断，不是通过证据。
4. `codesign --verify --deep apps/desktop/node_modules/electron/dist/Electron.app` 报 `code has no resources but signature indicates they must be present`，日志 `.autome/output/r3-codesign.txt`。没有证明该签名状态导致本次 SIGABRT，故不盲目重签或替换依赖。旧 Electron 崩溃报告含 `_RegisterApplication`，时间不是本轮，不作为本次根因证据。
5. 不导入项目的 `node:net.createServer().listen(0, '127.0.0.1')` 报 `EPERM listen EPERM: operation not permitted 127.0.0.1`，日志 `.autome/output/r3-listen.txt`。当前会话确有回环监听限制，不能把随后的 Web 环境失败归因于产品代码；也不能把它当作宿主 Desktop 失败的同根因证明。

宿主只提供了 Desktop 阶段失败摘要，没有具体断言。当前 SIGABRT 是否与宿主同根因尚未核实。下一轮需要宿主保留 Desktop 完整 stderr、退出信号与失败测试名；当前权限模式不允许提权，不尝试绕过沙箱。

## 标待审之前的自审清单

本轮保持原待审状态，未新增里程碑；四项仍逐项核对：

1. **情形表逐行**：已结束 Some(1.25)/Some(2.5) 合计 3.75；已结束 None 计入会话数和缺失数；运行中不计数、不计费、不生成新角色键。聚合 4 个单测本轮通过。
2. **失败分支**：空输入与缺失费用已通过。M-01 纯聚合无新增授权、读盘、外部服务、超时失败分支，不适用。Electron 信号中止实际触发，既有测试仍判失败，没有吞错或跳过。
3. **全部迁移**：既有聚合测试实际覆盖 Running 排除，以及 Exited(0)、Exited(1)、Killed、Vanished 计入；6 个 Role、3 个 BugStep、Intake、Onboarding 键均覆盖通过。本轮无新增状态机或恢复逻辑，不适用额外迁移。
4. **输入域边界**：空集合、单个 None、0、0.001、1,000,000 均在聚合单测中通过。日期、前导零、字符串解析不适用，本轮不涉及这些输入域。

## 验收统计

`cargo test -p autome-domain --lib cost_summary_`：4/4 通过，较上一轮同子集 4 个差 **0**，无增删。日志 `.autome/output/r3-cost-summary.txt`。

Web 宿主基线为 301 个全通过；Desktop 上一轮本地基线 105 个。不得用探针数量替代测试数量，也不得将未完成的全量视为 301 个通过。

门禁中的 `cost_rollup_task_get_sums_two_sessions_and_counts_missing_cost` 已通过；连同聚合单测，M-01 验收共 **5** 个通过，相对上一轮 5 个差 **0**，没有增删用例。

全量门禁 Rust 套件为 25、14、388、1（忽略）、648、0、104、16、7，总计 **1203** 个：1202 通过、1 个既有忽略；相对上一轮 1203，差 **0**，没有增删测试。全量门禁内 Desktop 也重现相同 SIGABRT（105 个，104 通过、1 失败），与独立运行一致。

## 完整门禁与交接

`sh scripts/ci-local.sh` 本轮完整结束，退出 **1**，耗时 1061 秒，未主动中断。Format、Clippy、Rust 全量、Contract、Web lint、Web typecheck、Web build 通过；Desktop、Web test 失败。原始输出 `.autome/output/r3-ci.txt`。

Web 全量为 **301** 个用例：271 通过、11 失败、19 跳过；33 个文件中 29 通过、4 失败。相对宿主上一轮 301 个，总数差 **0**，无增删测试。11 个 gateway-http 用例因 `listen EPERM` 失败；eventbus-live、notifier-live、server 的初始化失败使 19 个用例跳过，server 还记录 EMFILE。这些跳过并非本轮添加跳过条件。上一轮本地全量未结束，无可比较的本地完整计数。

本轮没有能证明有效的代码修复，因此没有红转绿记录，不将诊断伪称修复。需要在可启动 Electron GUI、允许回环监听的宿主上复跑本轮提交并保留 Desktop 具体错误，才能继续判别宿主失败原因。M-01 仍待审；H-01–H-03 未执行；没有推进 M-02。

`git diff --check` 与本轮状态/里程碑/证据指针/retro 格式检查通过。只提交设计状态、证据和 retro，不修改协议及 `.autome/` 配置。
