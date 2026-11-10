# M-01 实现 #2：Desktop 门禁修复

## 范围与结论

按本轮用户的最新指令修复宿主门禁，不推进新里程碑。M-01 原为开放，仍保持开放；其余里程碑状态不变。implementation-round 为 2/20。最新修复指令作为本次修改 apps/desktop/src/app-protocol.js 的特定授权，不扩大为 Desktop 功能开发。

根因：app-protocol 在模块顶层 require('electron')，仅检查导出的 Node 测试也触发 Electron 二进制定位/下载。本地缺少二进制导致两条既有用例失败。将 require 移至两个协议注册函数内；真实注册仍获取 Electron protocol，未模拟生产 API、未吞异常、未删测试或增加跳过。检查了 main.js、tools/screenshots.js、test/dom-harness.js 的全部调用点，两种注册入口保持原调用顺序。

无上一轮审计文件。Autome 指出的两条检查已经逐字存在于 apps/desktop/test/app-protocol.test.js，原样保留作为回归，不重复复制。

## 红绿证据与用例总数

- 修复前 `sh scripts/ci-local.sh`：exit=1，48 秒；Desktop 104 个用例，101 通过、2 失败、1 跳过，与用户所给基线总数差 0。两条 app-protocol 检查均复现 Electron failed to install correctly。同时 Rust Test 出现独立 PID 清理断言失败。原始输出：`.autome/output/r2-gate-red.txt`。
- 修复前 `node --test apps/desktop/test/app-protocol.test.js`：exit=1，2 个用例全部失败，原样复现用户列出的检查；输出 `.autome/output/r2-protocol-red.txt`。
- 修复后同一单文件命令：exit=0，2 个用例全部通过，基线差 0；输出 `.autome/output/r2-protocol-green.txt`。
- 修复后 `sh scripts/ci-local.sh`：exit=1，44 秒；Desktop 104 个用例，103 通过、0 失败、1 跳过；总数相对上一轮 104 差 0，变化仅为原有 2 个失败转通过。Format、Clippy、End to end、Adapter flags、Desktop、Contract 均通过，Test 未通过。输出 `.autome/output/r2-gate-green.txt`。
- Rust Test 两次均在 automed lib 的 534 个用例中 533 通过、1 失败，总数差 0；失败为 `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 的 `the wrapper removes its pid on the way out`（crates/automed/src/gate.rs:300）。源码 wrapper 先 mv 退出标记，再 rm PID（44–45 行），而测试观察 Done 后立即断言 PID 不存在，存在跨进程时序窗口。本轮不修改 crates，也不通过重试挑选绿色结果。此项意味着未取得完整门禁通过证据，最终以 Autome 在提交后重跑为准。
- Web 原有 12 用例未改动，本轮未重跑，不声明 Web 验收通过；上一轮回环监听阻塞仍未闭合。当前门禁尚未接入 Web（属于 M-04）。
- `git diff --check` 通过。未执行 package、真实 Electron UI 或人工验收。

## 标待审之前的自审清单

本轮不标待审，按修复范围逐项检查：

1. **情形表逐行**：模块导入/常量导出、两个注册函数导出均由原有两测试覆盖并通过；追加临时断言确认导入时 Electron 加载次数为 0，调用特权注册后为 1，调用协议注册后为 2；scheme 为 autome，特权 standard 保留；注册后的 handler 对首页返回 200、非 app host 返回 404。输出 `.autome/output/r2-registration.txt`。临时检查不计入项目用例总数，不能替代真实 Electron 验收。
2. **失败分支**：无 Electron 二进制的纯 Node 导入从红变绿；handler 非 app host 的 404 检查通过。授权、超时、外部服务、读盘失败不适用于本次加载时机修复，原协议处理逻辑未改。M-01 原有 HTTP 失败分支未在本轮重新验收。
3. **全部状态迁移**：导入→注册特权→注册协议的两次加载与 API 调用通过临时替身检查；三个实际调用方的顺序已静态核对。无新增状态机；真实 Electron 生命周期未实跑。
4. **输入域边界**：不适用；本次修改的两个注册函数均无参数，未改变 URL、端口或版本输入域。保留已有导出路径精确断言。

## 下一步

由 Autome 重跑提交上的宿主门禁；Rust PID 清理竞态仍阻塞完整门禁。M-01 后续仍需在允许回环监听的环境取得原有 12 用例及负向对照证据。本轮正常结束，不启动下一会话。
