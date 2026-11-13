# 实现 #11：M-06 安装验收条件跳过导致 Web test 门禁失败

## 范围与修复

本轮为用户指定的门禁修复轮，不推进新里程碑。M-01 至 M-07 的已审状态原样保留；上一轮审计 #8 通过，没有退回缺陷或待移植的临时检查。implementation-round 更新为 11/35，next-action 指向宿主重跑快检与合并前门禁。

只读定位宿主原始日志 `/Users/dannie/project/autome-v2/.autome/output/gates/T-84/merge_gate-2e62bf4f574e.log`：Web 共 722 例，721 通过、1 失败；唯一失败为 `test/core-sidecar.test.ts > 测试源码不按条件跳过真实内核用例`，断言指出 `install-alignment.test.ts` 的两处 `test.skipIf(process.platform !== 'darwin')`。摘录保存在 `.autome/output/r11-host-failure.txt`。

根因：M-06 的平台条件跳过与项目已有的禁止跳过检查冲突。检查扫描所有测试源码，即使当前 macOS 上这两例实际运行通过，仍会正确拒绝条件跳过声明。同类扫描 `.autome/output/r11-skip-scan.txt` 确认仅这两处命中。修复为普通 `test`，在创建安装夹具前明确断言平台为 darwin；平台不符直接失败，不静默略过。原有源码检查逐字保留，已在项目测试体系内，无须复制第二份。安装脚本、生产实现、校验和、真实 CLI/core 身份比对及资源清理断言均不变。设计 §13 与 M-06 的平台口径同步改为显式前置条件失败。

## 命令与结果

1. 原样复现（红）：`npm run test --prefix apps/web -- test/core-sidecar.test.ts -t '测试源码不按条件跳过真实内核用例'`，1 个选中用例失败；另 17 个为名称过滤未执行，输出 `.autome/output/r11-red.txt`。
2. 修复后同命令（绿）：1 个选中用例通过；另 17 个仍为名称过滤未执行，输出 `.autome/output/r11-green.txt`。
3. 完整相关子集：`npm run test --prefix apps/web -- test/core-sidecar.test.ts test/install-alignment.test.ts test/core-binary.test.ts`，3 文件、**29 用例全部通过，0 失败、0 跳过**，输出 `.autome/output/r11-subset.txt`。所有真实内核使用测试夹具的临时 AUTOME_HOME / 数据库；安装同时使用临时 PREFIX，没有接触真实账本或安装位置。
4. `git diff --check` 通过。未运行全量门禁或宿主快检；最终结论由 Autome 提交后重跑决定。人工验收 H-01 至 H-03 本轮未取证。

**总数与基线差**：本轮唯一用例数为 29（18 sidecar + 2 安装 + 9 查找）；单独红绿重跑是其中 1 例的重复执行，不另计。宿主失败日志中同三文件也是 29 例，差 **0**，从 28 通过/1 失败变成 29 通过。M-06 原验收基线 2 例，本轮安装仍为 2，差 0；增加执行 27 例来自现有 sidecar 与查找回归。与上一实现轮 #10 的 18 例相比数字为 +11，但它执行 Rust 守卫/集成测试，本轮执行 Web 子集，不能当作新增覆盖。未重跑其余 693 个 Web 用例，不宣称 722 例门禁已全绿。

## 四项自审清单

1. **情形表逐行**：M-06 §13 的四步均由两条安装用例实跑：包内 autome/README 与 SHA256 正确；空 PREFIX 安装并校验成功；PATH 查到安装的 CLI 且参数为 core；真实握手 env.get 的 build_id 与 CLI --version 一致。相关查找表九例全部通过：显式覆盖、包优先、本地 CLI 优先、cargo/profile 兜底、空覆盖、包相邻路径、autome 加 core、PATH 首个命中、忽略旧 local-bin automed。平台前提为实际 Darwin；条件跳过扫描绿。
2. **失败分支**：本次缺陷分支由原源码检查修复前红、修复后绿直接验证，没有关闭或豁免检查。相关 sidecar 完整套件实跑不存在的内核、进程退出、请求超时/迟到回复等失败路径，全部通过。设计 M-06 未点名新的授权或远端服务失败分支，使用本地 tar 安装，不适用。非 macOS 的新增明确失败提示仅作代码核对，本机无法实际切换 OS，不声称跨平台实跑。
3. **全部迁移**：安装夹具实跑未安装→包生成→安装→查找→真实 core 启动/握手→停止/临时目录清理；sidecar 套件覆盖正常请求回复、退出后拒绝、主动停止不报崩溃、超时与迟到回复。测试注册从条件执行改为无条件执行；没有新增产品状态或数据库迁移，M-02 的 v7→v8 迁移不属本轮改动范围。
4. **域边界**：安装测试使用含空格的临时路径、尚不存在的 PREFIX、单个发布包、排除 cargo 的 PATH（确保 --binary 不重新构建）；查找测试覆盖空 AUTOMED_BIN、文件不存在、默认/release profile、多个 PATH 候选。均通过。本轮无日期、数值或其它新增产品输入域，其边界不适用。

## 提交

本轮仅提交测试、设计当前口径与证据/retro；不推送、不启动下一会话。宿主快检与合并前门禁待 Autome 重新运行。
