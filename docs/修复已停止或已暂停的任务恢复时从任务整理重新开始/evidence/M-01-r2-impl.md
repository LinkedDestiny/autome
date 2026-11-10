# M-01 · 实现 #2 · 门禁修复轮

日期：2026-11-10。implementation-round：2/15。M-01 保持待审，未推进 M-02/M-03。无审计文件，上一轮尚未审计；不存在需要搬入的审计临时检查。输入提交为 `1ac5c0b`（上一个实现提交 `efa159a`）。

## 复现、诊断与处理

1. 原样运行 `sh scripts/ci-local.sh`，完整输出 `.autome/output/impl-2-ci-red.txt`。Desktop 两个 `app-protocol.test.js` 用例复现用户给出的 `Electron failed to install correctly`；Format、Clippy、Test、End to end、Adapter flags、Contract 本次均通过。用户摘要里的 Rust Test 失败本次未复现，摘要没有提供该失败详情，不推断它已被修复。
2. 缩小复现：`node --test apps/desktop/test/app-protocol.test.js`，2 个用例均失败，输出 `.autome/output/impl-2-desktop-red.txt`。这两个逐字保留的现有用例就是当前回归检查，无需复制同名检查或删改断言。
3. 检查假设：二进制未安装、版本/平台不匹配、加载路径异常。现场包版本为 43.2.0，平台 darwin arm64；包内缺少 dist 和 path.txt。app-protocol 在顶层 require electron 时失败，尚未运行测试断言。现有缓存只有 44.0.0 / 44.7.0，不能替代锁定版本。没有设置伪造 path.txt、替换版本或跳过测试。
4. 尝试原包安装器：`electron_config_cache="$PWD/.autome/output/electron-cache" node apps/desktop/node_modules/electron/install.js`，退出 1，`TypeError: fetch failed`；记录 `.autome/output/impl-2-electron-install.txt`。使用工作区缓存避免写入其它目录。
5. 网络探测：`curl -I -L --max-time 15 https://github.com/electron/electron/releases/download/v43.2.0/electron-v43.2.0-darwin-arm64.zip`，提示 `Could not resolve host: github.com`；记录 `.autome/output/impl-2-download-probe.txt`。当前沙箱不允许提升权限，无法取得匹配二进制。

本轮未修改产品代码或依赖清单：任务 §4 明确排除 apps/scripts 的改动，当前证据指向本地依赖缺失，不是恢复状态机错误。安装未成功，因此不声称完成修复或取得绿灯。待环境可下载时，在本 worktree 安装锁定 Electron，再原样重跑门禁。原始日志只放 `.autome/output/`，不改其余 Autome 文件。

## 验收与用例基线

- `cargo test -p autome-domain requeued`：9/9 通过，与实现 #1 的 9 个相差 0。输出 `.autome/output/impl-2-domain.txt`。
- `cargo test -p automed --lib requeued`：6/6 通过，与实现 #1 的 6 个相差 0。输出 `.autome/output/impl-2-lib.txt`。
- 首次完整门禁 Rust 全量：895 个（CLI 8+4、domain 288、automed lib 540、e2e 38、prompt 10、real_cli 7），全通过。相较上轮枚举基线 895 差 0；相较上轮实际执行的两个单元套件 828 多执行 67 个，来自 CLI 12、e2e 38、prompt 10、real_cli 7，不是新增测试。门禁额外重复执行 e2e 38 与 adapter 1，不重复计为新用例。
- Desktop 总数 104，101 通过、2 失败、1 跳过；相较用户提供的上一轮门禁 104 差 0。Rust 与 Desktop 合计独立用例 999，本轮没有新增、删除或修改测试。
- 安装尝试后的同命令复验：见 `.autome/output/impl-2-ci-after.txt`；最终结果在下节记录。

## 四项自审清单

本轮没有新标待审，但仍复验 M-01 的现有清单。以下检查均由本轮 9+6 专项用例及全量门禁执行，不借用旧日志作通过证据。

1. **情形表逐行**：§1 queued_tasks/计数/槽位、queue_position、start_retro 拒绝均通过；§2 Paused 恢复、Stopped 非门禁恢复、Stopped 三门禁清理、已关角色拒绝、Queued 从 Intake 出队、Requeued 会话节点重入、排队时关角色失败、内核节点出队、五种控制事件拒绝、Cancel 迁移逐行通过。§3 M-01 的 R1 不重复启动、R3 按原会话取 inject 均通过；R2 不适用，仍属于 M-03。
2. **失败分支**：六角色关闭拒绝及出队 Failed、作业目录为普通文件时 I/O 失败记录 Failed、无 PID 等待 3 秒超时、延迟 PID 真实进程清理、空/未知/数值节点 JSON 拒绝均通过。授权拒绝不适用，本里程碑没有新增授权接口；外部服务不可达在本轮依赖安装上已实测，阻塞如上，未伪报绿灯。
3. **全部状态迁移**：13 个合法节点的 Paused/Stopped→Requeued、Requeued→Active（7 会话/6 内核）、Queued→Active Intake、六角色关闭恢复拒绝/出队 Failed、Requeued 五种控制拒绝及 Cancel→Cancelled 全通过；槽满保持排队、腾槽回原节点、Stop 拒绝不杀会话、清理动作失败→Failed 均通过。
4. **输入域边界**：13 节点枚举、旧 queued 缺 resume/null、合法 audit、空串/unknown/数字、旧 paused/stopped 往返、混合/空队列与单槽、gate 文件前缀保留 merge_gate、缺目录/缺 PID/延迟 PID 均通过。日期及数值最大值不适用，本轮未增加此类输入域。Electron 平台和版本已核对，缓存版本不匹配，未强行复用。

## 结论

M-01 保持待审；本轮环境修复尚未完成，不是协议失败。H-01～H-03 人工验收未运行。门禁最终以 Autome 在本轮提交上重跑为准。

最终复验：`sh scripts/ci-local.sh` 退出 1，用时 79 秒，Rust Test/Format/Clippy/End to end/Adapter flags/Contract 全部通过，Desktop 仍为 104 个、101 通过、2 失败、1 跳过，两个失败仍为相同 Electron 安装错误。Rust 用例总数仍为 895，与首次复现差 0；没有取得红→绿证据。`git diff --check` 通过。
