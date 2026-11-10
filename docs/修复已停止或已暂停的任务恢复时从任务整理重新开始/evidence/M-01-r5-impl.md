# M-01 · 实现 #5 · Desktop 门禁修复

implementation-round：5/15。输入 HEAD：1200f22；Autome 报告失败提交：065ffdb2d7。本轮仅修复门禁，M-01 保持待审，M-02/M-03 不推进。无上一轮审计文件。

## 复现、修复与范围

- 原样运行 `sh scripts/ci-local.sh`：失败，用时 99 秒，只有 Desktop 失败；日志 `.autome/output/impl-5-red.txt`。两个原有 app-protocol 测试均报 `Electron failed to install correctly`。
- 原样运行 `node --test apps/desktop/test/app-protocol.test.js`：2 个用例全部失败；日志 `.autome/output/impl-5-protocol-red.txt`。失败检查已经逐字存在于项目测试体系中，本轮保留原文件、名称和断言，不复制、不跳过、不弱化。
- 原因：普通 Node 导入 app-protocol 的常量和函数时，顶层 `require('electron')` 立即读取尚未安装的二进制路径。两个用例根本没有调用 Electron API，却被运行时安装状态阻断。
- 修复：只把 Electron 导入移动到 `registerSchemeAsPrivileged` 和 `registerAppProtocol` 内。实际注册仍加载同一模块、调用同一 API；没有捕获安装错误或伪造二进制路径。检查 src/test 同类导入：src 只有此处，DOM harness 确实在 Electron 下运行，dom.test.js 已明确处理缺失二进制，不修改。
- 范围依据：任务原 §4 限制 crates；本轮最新用户指令明确要求修复指定 Desktop 失败。据此仅对 apps/desktop/src/app-protocol.js 作必要例外，不扩展里程碑或修改脚本、依赖版本、测试断言。
- 修后原样重跑两个测试：2 个通过、0 失败、0 跳过，日志 `.autome/output/impl-5-protocol-green.txt`。
- 补充 Node 检查用 Module._load 提供 protocol 替身，实际调用两个注册函数，断言 scheme 为 autome、注册 handler 为 autome、异域响应 404、首页响应 200，全部通过，日志 `.autome/output/impl-5-registration-check.txt`。这只是调用检查，不作为真实 Electron 验收。

## 标待审之前的四项自审清单

本轮保持原待审状态，仍执行四项。

1. **情形表逐行**：设计 §2 第 1 行暂停恢复、第 2 行非门禁停止恢复、第 3 行三门禁停止清理/暂停不清理、第 4 行已关角色拒绝、第 5 行新建出队 Intake、第 6 行角色重入、第 7 行排队关闭角色失败、第 8 行内核步骤、第 9 行五类控制拒绝、第 10 行取消，均由本轮 `cargo test -p autome-domain requeued` 的 9 个用例覆盖通过。§1 排队处理由 `cargo test -p automed --lib requeued` 的 6 个用例及全量覆盖。日志 `.autome/output/impl-5-domain.txt`、`.autome/output/impl-5-lib.txt`。
2. **失败分支**：上述专项通过角色关闭拒绝/Failed、门禁目录 I/O 失败、无 PID 超时、延迟 PID 清理及非法 JSON 检查。Electron 缺失时两个导出检查红→绿；真实 DOM 缺失仍按原测试跳过。新增授权/外部服务调用不适用，本轮没有这些接口。
3. **全部迁移**：专项遍历 13 节点恢复/出队、三门禁两种停法、角色开关、五类控制拒绝、取消；lib 专项验证槽满→腾槽、Stop 拒绝且不杀原会话。均通过。Desktop 本次只有加载时机变化，无新增状态迁移；R2/M-03 不适用，本轮未推进。
4. **输入域边界**：专项覆盖 queued 缺省/null、新状态往返、空串/未知字符串/数字拒绝、旧 paused/stopped、空/混合队列、单槽、门禁前缀隔离，均通过。Desktop 补充检查首页与非 app 域；日期、前导零、最大数值不适用，本次未新增这些输入域。

## 验收结果

`sh scripts/ci-local.sh` 修后退出 0，用时 76 秒，Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract 全部通过。日志 `.autome/output/impl-5-green.txt`。未跑可选 package。

- Rust 独立用例 895 个全部通过（CLI 12、domain 288、automed lib 540、e2e 38、prompt 10、real_cli 7），较上一轮基线 895 差 0。
- Desktop 104 个：103 通过、0 失败、1 跳过，较上一轮 104 差 0；原先失败的两个导出用例变为通过。真实 DOM 跳过数仍为 1。
- 独立用例总数 999，较上一轮 999 差 0；门禁额外重复 e2e 38 与 Adapter flags 1，共 1038 次执行，较上一轮差 0。没有新增、删除或修改测试。
- 两个 app-protocol 单独复验及 15 个 requeued 专项是额外重复运行，不计入独立总数。补充 protocol 替身检查也不计入正式测试总数。

## 限制

Electron 43.2.0 二进制仍缺失，真实 DOM 用例保持既有显式跳过；H-01～H-03 未实测。此次修复不声称安装或 GUI 已修复。最终门禁以 Autome 在提交后重跑的结果为准。
